FROM rust:1.84-bullseye as builder
WORKDIR /usr/src/marked-space
COPY . .
RUN cargo install --path .

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y openssl ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/marked-space /usr/local/bin/marked-space
ENTRYPOINT ["marked-space"]
