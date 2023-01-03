use crate::https::HttpsClient;
use clap::ArgMatches;
use std::error::Error;
use hyper::{Body, Request, Response};
//use serde_json::{Value};
use url::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::create_https_client;
use crate::error::Error as RestError;

type BoxResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    amount_billed_cents: u64,
    amount_paid_cents: u64,
    created: String,
    credits_cents: u64,
    end_date: String,
    id: String,
    line_items: Vec<LineItem>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LineItem {
    cluster_name: Option<String>,
    created: String,
    end_date: String,
    quantity: f64,
    sku: String,
    start_date: String,
    total_price_cents: u64,
    unit: String,
    unit_price_dollars: f64
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Compressed {
    cluster_name: Option<String>,
    quantity: f64,
    sku: String,
    total_price_cents: u64,
    unit: String,
    unit_price_dollars: f64,
    end_date: String
}

#[derive(Clone, Debug)]
pub struct State {
    pub client: HttpsClient,
    pub url: Url,
    pub org: String
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
        let url = opts.value_of("url").unwrap().parse().expect("Could not parse url");
        let org = opts.value_of("org").unwrap().parse().expect("Could not get org id");

        Ok(State {
            client,
            url,
            org
        })
    }

    pub async fn get_pending(&self) -> Result<Data, RestError> {
        let path = format!("orgs/{}/invoices/pending", self.org);
        let body = self.get(&path).await?;
        let bytes = hyper::body::to_bytes(body.into_body()).await?;
        let value: Data = serde_json::from_slice(&bytes)?;
        Ok(value)
    }

    pub async fn get(&self, path: &str) -> Result<Response<Body>, RestError> {
        let uri = format!("{}/{}", &self.url, path);
        log::debug!("getting url {}", &uri);
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

        match response.status().as_u16() {
            404 => return Err(RestError::NotFound),
            403 => return Err(RestError::Forbidden),
            401 => return Err(RestError::Unauthorized),
            200 => {
                Ok(response)
            }
            _ => {
                log::error!(
                    "Got bad status code getting config: {}",
                    response.status().as_u16()
                );
                return Err(RestError::UnknownCode)
            }
        }
    }

    pub async fn get_metrics(&self) -> Result<(), RestError> {
        let data = self.get_pending().await?;
        log::debug!("data: {:?}", data);

        let mut map: HashMap<String, Compressed> = HashMap::new();

        for item in data.line_items {
            let name = match &item.cluster_name {
                Some(e) => format!("{}_{}", e, item.sku),
                None => item.sku.to_string()
            };

            log::debug!("Working on {}", name);

            match map.get_mut(&name) {
                Some(k) => {
                    log::debug!("Found existing {} in map", &name);
                    k.total_price_cents = k.total_price_cents + item.total_price_cents;
                    k.quantity = k.quantity + item.quantity;
                    k.unit_price_dollars = item.unit_price_dollars;
                    k.end_date = item.end_date;
                },
                None => {
                    log::debug!("Did not find existing {} in map", &name);
                    let value = Compressed {
                        cluster_name: item.cluster_name.clone(),
                        quantity: item.quantity.clone(),
                        sku: item.sku.clone(),
                        total_price_cents: item.total_price_cents.clone(),
                        unit: item.unit.clone(),
                        unit_price_dollars: item.unit_price_dollars.clone(),
                        end_date: item.end_date.clone()
                    };
                    map.insert(name, value);
                }
            }
        }

        log::debug!("{:?}", map);

        for (key, value) in map {
            let labels = [
                ("cluster_name", value.cluster_name.unwrap_or("".to_string())),
                ("sku", value.sku.clone()),
            ];
            metrics::gauge!("atlas_billing_item_cents_total", value.total_price_cents.clone() as f64, &labels);

            match chrono::DateTime::parse_from_rfc3339(&value.end_date) {
                Ok(end_date) => {
                    let difference = chrono::Utc::now() - end_date.with_timezone(&chrono::Utc);
                    if &difference < &chrono::Duration::hours(30) {
                        log::debug!("Including {}. Difference is {}", key, difference);
                        metrics::gauge!("atlas_billing_item_cents_rate", value.unit_price_dollars.clone() as f64, &labels);
                    } else {
                        log::debug!("Skipping {}, as it is more than one day old. Difference is {}", key, difference);
                    }
                },
                Err(e) => {
                    log::error!("Error converting end_date to Utc, skipping {}: {}", key, e);
                }
            }
        }
            

        Ok(())
    }
}
