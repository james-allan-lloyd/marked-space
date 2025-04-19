FROM --platform=$BUILDPLATFORM rust:1.84-bullseye AS builder
WORKDIR /usr/src/marked-space

# Install any required system dependencies for cross-compilation
RUN apt-get update && apt-get install -y \
  gcc-aarch64-linux-gnu \
  gcc-arm-linux-gnueabihf libc6-armhf-cross libc6-dev-armhf-cross \
  pkg-config

# Copy the Rust files
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs  # Dummy file to force dependency resolution
RUN cargo fetch

COPY .cargo ./.cargo
COPY rustup-target-add.sh ./
COPY src ./src

RUN ls

# Compile the Rust application based on the architecture
# Architecture is determined by the --platform flag passed to docker buildx
ARG TARGETARCH
RUN bash ./rustup-target-add.sh

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y openssl ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/marked-space /usr/local/bin/marked-space
ENTRYPOINT ["marked-space"]
