use crate::https::HttpsClient;
use chrono::Datelike;
use chrono::Utc;
use clap::ArgMatches;
use hyper::header::{HeaderValue, AUTHORIZATION};
use hyper::{Body, Request, Response};
use std::error::Error;
//use serde_json::{Value};
use digest_auth::AuthContext;
//use url::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::create_https_client;
use crate::error::Error as RestError;

type BoxResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

static URL: &str = "https://cloud.mongodb.com/api/atlas/v1.0";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    amount_billed_cents: u64,
    amount_paid_cents: u64,
    created: String,
    credits_cents: u64,
    end_date: String,
    id: String,
    line_items: Vec<LineItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LineItem {
    cluster_name: Option<String>,
    created: String,
    end_date: String,
    quantity: f64,
    group_name: Option<String>,
    sku: String,
    start_date: String,
    tags: Option<Option<HashMap<String, Vec<String>>>>,
    total_price_cents: u64,
    unit: String,
    unit_price_dollars: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Compressed {
    cluster_name: Option<String>,
    quantity: f64,
    group_name: Option<String>,
    sku: String,
    total_price_cents: u64,
    unit: String,
    unit_price_dollars: f64,
    tags: Option<Option<HashMap<String, Vec<String>>>>,
    end_date: String,
    start_date: String,
}

#[derive(Clone, Debug)]
pub struct State {
    pub client: HttpsClient,
    pub public_key: String,
    pub private_key: String,
    pub org: String,
}

impl State {
    pub async fn new(opts: ArgMatches<'_>) -> BoxResult<Self> {
        // Set timeout
        let timeout: u64 = opts
            .value_of("timeout")
            .unwrap()
            .parse()
            .unwrap_or_else(|_| {
                eprintln!("Supplied timeout not in range, defaulting to 60");
                60
            });

        let client = create_https_client(timeout)?;
        let public_key = opts
            .value_of("public_key")
            .unwrap()
            .parse()
            .expect("Could not parse public_key");
        let private_key = opts
            .value_of("private_key")
            .unwrap()
            .parse()
            .expect("Could not parse private_key");
        let org = opts
            .value_of("org")
            .unwrap()
            .parse()
            .expect("Could not get org id");

        Ok(State {
            client,
            public_key,
            private_key,
            org,
        })
    }

    pub async fn get_pending(&self) -> Result<Data, RestError> {
        let path = format!("orgs/{}/invoices/pending", self.org);
        let body = self.get(&path).await?;
        let bytes = hyper::body::to_bytes(body.into_body()).await?;
        let value: Data = serde_json::from_slice(&bytes)?;
        Ok(value)
    }

    pub async fn get_last_invoice_id(&self) -> Result<String, RestError> {
        let path = format!("orgs/{}/invoices?itemsPerPage=2", self.org);
        let body = self.get(&path).await?;
        let bytes = hyper::body::to_bytes(body.into_body()).await?;
        let value: Value = serde_json::from_slice(&bytes)?;

        // Extract results array from json
        let results = &value["results"].as_array().ok_or(RestError::NotFound)?;

        // Extract the id field from the last item in results array
        let id = &results
            .last()
            .ok_or(RestError::NotFound)?
            .get("id")
            .ok_or(RestError::NotFound)?;

        Ok(id.as_str().expect("Cannot unwrap id as string!").to_owned())
    }

    pub async fn get_last_invoice(&self) -> Result<Data, RestError> {
        let id = self.get_last_invoice_id().await?;

        let path = format!("orgs/{}/invoices/{}", self.org, id);
        let body = self.get(&path).await?;
        let bytes = hyper::body::to_bytes(body.into_body()).await?;
        let value: Data = serde_json::from_slice(&bytes)?;
        Ok(value)
    }

    pub async fn get(&self, path: &str) -> Result<Response<Body>, RestError> {
        let uri = format!("{URL}/{path}");
        log::debug!("getting initial response {}", &uri);
        let req = Request::builder()
            .method("GET")
            .uri(&uri)
            .body(Body::empty())
            .expect("request builder");

        // Send initial request
        let response = match self.client.request(req).await {
            Ok(s) => s,
            Err(e) => {
                log::error!("{{\"error\":\"{}\"", e);
                return Err(RestError::Hyper(e));
            }
        };

        // Get digest headers, we are expecting a 401 status code
        let mut www_auth_header = match response.status().as_u16() {
            401 => match response.headers().get("WWW-Authenticate") {
                Some(www_authenticate) => {
                    digest_auth::parse(www_authenticate.to_str().unwrap_or("error"))?
                }
                None => {
                    log::error!("Inital request did not yield www-authenticate header");
                    return Err(RestError::MissingHeader);
                }
            },
            _ => return Err(RestError::UnexpectedCode),
        };

        // Generate Digest Header Context
        let context = AuthContext::new(self.public_key.clone(), self.private_key.clone(), path);

        // Use context and compute with www_auth_header returned from API
        let answer = www_auth_header.respond(&context)?;
        let header_digest_auth = HeaderValue::from_str(&answer.to_string())?;

        log::debug!("Using digest header for authenticated request{}", &uri);
        let mut req2 = Request::builder()
            .method("GET")
            .uri(&uri)
            .body(Body::empty())
            .expect("request builder");

        // Add auth header to second request
        req2.headers_mut().insert(AUTHORIZATION, header_digest_auth);

        // Send initial request
        let response2 = match self.client.request(req2).await {
            Ok(s) => s,
            Err(e) => {
                log::error!("{{\"error\":\"{}\"", e);
                return Err(RestError::Hyper(e));
            }
        };

        match response2.status().as_u16() {
            404 => Err(RestError::NotFound),
            403 => Err(RestError::Forbidden),
            401 => Err(RestError::Unauthorized),
            200 => Ok(response2),
            _ => {
                log::error!(
                    "Got bad status code getting config: {}",
                    response.status().as_u16()
                );
                Err(RestError::UnknownCode)
            }
        }
    }

    pub async fn get_metrics(&self) -> Result<(), RestError> {
        let day = Utc::now().date_naive().day();

        log::debug!("We are on the {} day of the month", day);

        let data = match day {
            1 => self.get_last_invoice().await?,
            _ => self.get_pending().await?,
        };

        log::debug!("data: {:?}", data);

        let mut map_total: HashMap<String, Compressed> = HashMap::new();
        let mut map_rate: HashMap<String, Compressed> = HashMap::new();

        // Get most recent metric date across all metrics
        let current_date = match data.line_items.iter().max_by_key(|y| y.end_date.clone()) {
            Some(i) => i.end_date.clone(),
            None => return Ok(()),
        };

        for item in data.line_items {
            let name = match &item.cluster_name {
                Some(e) => format!("{}_{}", e, item.sku),
                None => item.sku.to_string(),
            };

            log::debug!("Working on {} from {}", name, item.end_date);

            // Add metric to the total HashMap
            match map_total.get_mut(&name) {
                Some(k) => {
                    log::debug!("Found existing {} in map_total, adding up total", &name);

                    // Atlas prices sku's per region, so we need to get the sum
                    k.total_price_cents += item.total_price_cents;
                    k.quantity += item.quantity;
                }
                None => {
                    log::debug!("Did not find existing {} in map_total", &name);
                    let value = Compressed {
                        cluster_name: item.cluster_name.clone(),
                        quantity: item.quantity,
                        sku: item.sku.clone(),
                        group_name: item.group_name.clone(),
                        total_price_cents: item.total_price_cents,
                        unit: item.unit.clone(),
                        unit_price_dollars: item.unit_price_dollars,
                        tags: item.tags.clone(),
                        start_date: item.start_date.clone(),
                        end_date: item.end_date.clone(),
                    };
                    map_total.insert(name.clone(), value);
                }
            }

            // Only include metric if the end_date is today
            if item.end_date == current_date {
                // Add most recent metrics to hashmap
                match map_rate.get_mut(&name) {
                    Some(k) => {
                        log::debug!("Found existing {} in map_rate", &name);
                        // This metric has the same start date, indicating a SKU present in multiple regions
                        // Therefore, get the sum of all
                        // Atlas prices sku's per region, so we need to get the sum
                        k.unit_price_dollars += item.unit_price_dollars;
                        log::debug!("{} is already set in map_rate, and has the same end_date. Adding up total price to get {}", &name, k.unit_price_dollars);
                    }
                    None => {
                        log::debug!("Did not find existing {} in map_rate", &name);
                        let value = Compressed {
                            cluster_name: item.cluster_name.clone(),
                            quantity: item.quantity,
                            sku: item.sku.clone(),
                            group_name: item.group_name.clone(),
                            total_price_cents: item.total_price_cents,
                            unit: item.unit.clone(),
                            unit_price_dollars: item.unit_price_dollars,
                            tags: item.tags.clone(),
                            start_date: item.start_date.clone(),
                            end_date: item.end_date.clone(),
                        };
                        map_rate.insert(name, value);
                    }
                }
            }
        }

        log::debug!("Total: {:?}", map_total);
        log::debug!("Rates: {:?}", map_rate);

        for (_key, value) in map_total {
            let project = match value.tags {
                Some(inner_option) => match inner_option {
                    Some(map) => map.get("project").and_then(|v| v.get(0)).map(|val| val.clone()),
                    None => None,
                },
                None => None,
            };            
            let labels = [
                ("cluster_name", value.cluster_name.unwrap_or("".to_string())),
                ("group_name", value.group_name.unwrap_or("".to_string())),
                ("sku", value.sku.clone()),
                ("project", project.unwrap_or("".to_string())),
            ];
            metrics::gauge!(
                "atlas_billing_item_cents_total",
                value.total_price_cents as f64,
                &labels
            );
        }

        for (_key, value) in map_rate {
            let project = match value.tags {
                Some(inner_option) => match inner_option {
                    Some(map) => map.get("project").and_then(|v| v.get(0)).map(|val| val.clone()),
                    None => None,
                },
                None => None,
            };           
            let labels = [
                ("cluster_name", value.cluster_name.unwrap_or("".to_string())),
                ("group_name", value.group_name.unwrap_or("".to_string())),
                ("sku", value.sku.clone()),
                ("project", project.unwrap_or("".to_string())),
            ];

            if value.unit == "GB hours" || value.unit == "server hours" {
                // Get overall rate in cents per hour
                metrics::gauge!(
                    "atlas_billing_item_cents_rate",
                    value.unit_price_dollars,
                    &labels
                );
            } else {
                // Convert cents per day to cents per hour
                // Get overall rate in cents per hour
                let rate = value.total_price_cents as f64 / value.quantity / 100.0 / 24.0;
                metrics::gauge!("atlas_billing_item_cents_rate", rate, &labels);
            }
        }

        Ok(())
    }
}
