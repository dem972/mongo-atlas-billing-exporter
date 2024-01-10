from rust:bookworm as builder

RUN mkdir /app 
RUN mkdir /app/bin 

COPY src /app/src/
COPY Cargo.toml /app

RUN apt-get update && apt-get install -y libssl-dev pkg-config
RUN cargo install --path /app --root /app
RUN strip app/bin/mongo-atlas-billing-exporter

FROM debian:bookworm-slim
RUN apt-get update && apt install -y openssl
WORKDIR /app
COPY --from=builder /app/bin/ ./

ENTRYPOINT ["/app/mongo-atlas-billing-exporter"]
EXPOSE 8080
