# MongoDB Atlas Billing Exporter

Prometheus exporter for MongoDB Atlas billing info.

### Usage

```
USAGE:
    mongo-atlas-billing-exporter [OPTIONS] --org <org> --private_key <private_key> --public_key <public_key>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -o, --org <org>                    Set org id [env: ATLAS_BILLING_EXPORTER_ORG_ID=]
    -p, --port <port>                  Set port to listen on [env: ATLAS_BILLING_EXPORTER_LISTEN_PORT=]  [default: 8080]
    -s, --private_key <private_key>    Set MongoDB Atlas Private Key [env: ATLAS_BILLING_EXPORTER_PRIVATE_KEY=]
    -k, --public_key <public_key>      Set MongoDB Atlas Public Key [env: ATLAS_BILLING_EXPORTER_PUBLIC_KEY=]
    -t, --timeout <timeout>            Set default global timeout [env: ATLAS_BILLING_EXPORTER_TIMEOUT=]  [default: 60]
```

### Exporter Metrics
```
# HELP Atlas billing rate per sku
# TYPE atlas_billing_item_cents_rate gauge
atlas_billing_item_cents_rate

# HELP Atlas billing total cost per sku
# TYPE atlas_billing_item_cents_total gauge
atlas_billing_item_cents_total
```
