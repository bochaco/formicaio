# Dockerfile for running app on UmbrelOS

# Get started with a build env with Rust nightly
# FROM rustlang/rust:nightly-bullseye as builder
# If you’re using stable, use this instead
FROM rust:1.80-bullseye AS builder

# Install cargo-binstall, which makes it easier to install other
# cargo extensions like cargo-leptos
# Install cargo-binstall for Linux amd64/arm64
RUN export TARGET="$(uname -m)" && wget -O cargo-binstall-linux-musl.tgz https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-$TARGET-unknown-linux-musl.tgz
RUN tar -xvf cargo-binstall-linux-musl.tgz
RUN cp cargo-binstall /usr/local/cargo/bin

# Install cargo-leptos
RUN cargo binstall cargo-leptos -y

# Add the WASM target
RUN rustup target add wasm32-unknown-unknown
RUN rustup component add rustfmt

# Make an /app dir, which everything will eventually live in
RUN mkdir -p /app
WORKDIR /app
COPY . .

# Build the app
RUN cargo leptos build --release -vv

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update -y \
  && apt-get install -y --no-install-recommends openssl ca-certificates \
  && apt-get autoremove -y \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*

# Copy the server binary to the /app directory
COPY --from=builder /app/target/release/formicaio /app/

# Copy Sqlite migrations files
COPY --from=builder /app/migrations /app/migrations

# /target/site contains our JS/WASM/CSS, etc.
COPY --from=builder /app/target/site /app/site

# Copy Cargo.toml if it’s needed at runtime
COPY --from=builder /app/Cargo.toml /app/

# Set any required env variables and
ENV RUST_LOG="info"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8080"
ENV LEPTOS_SITE_ROOT="site"

EXPOSE 8080

# Run the server
CMD ["/app/formicaio"]
