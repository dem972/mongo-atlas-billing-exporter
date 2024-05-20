from rust:bookworm as builder

RUN apt-get update && apt-get install -y --no-install-recommends \
		libssl-dev=3.0.11-1~deb12u2 \
		pkg-config=1.8.1-1 && \
	apt-get autoremove -y && \
	apt-get clean  && \ 
    rm -rf /var/lib/apt/lists/* && \
	mkdir /app  && \
	mkdir /app/bin 

COPY src /app/src/
COPY Cargo.toml /app

RUN cargo install --path /app --root /app  && \
	strip app/bin/mongo-atlas-billing-exporter


FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
		openssl=3.0.11-1~deb12u2 && \
	apt-get autoremove -y && \
    apt-get clean && \ 
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/bin/ ./

ENTRYPOINT ["/app/mongo-atlas-billing-exporter"]
EXPOSE 8080
