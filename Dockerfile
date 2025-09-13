# Dockerfile for running Formicaio app

# We first just install tailwindcss from a nodejs slim image
FROM node:24-alpine AS tailwindcss-builder

WORKDIR /app
COPY package.json package-lock.json ./

# Install tailwindcss modules
RUN npm install tailwindcss

# Now let's use a build env with Rust for the app
FROM rust:1-alpine AS builder

RUN apk update && \
    apk add --no-cache bash curl npm libc-dev binaryen

# Install cargo-binstall, which makes it easier to install other
# cargo extensions like cargo-leptos
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

# Install cargo-leptos
RUN cargo binstall cargo-leptos@0.2.43 -y

# Add the WASM target
RUN rustup target add wasm32-unknown-unknown

WORKDIR /work

# Copy tailwindcss modules, and nodejs binary, to the /app directory
# since they are required for building the app
COPY --from=tailwindcss-builder /app/node_modules /work/node_modules
COPY --from=tailwindcss-builder /usr/local/bin/node /usr/local/bin/node

# Now we can copy the source files to build them
COPY . .

# Define build args argument
ARG BUILD_ARGS
ENV BUILD_ARGS=${BUILD_ARGS}

# Build the app
RUN cargo leptos build --release $BUILD_ARGS -vv

# Finally use an Alpine image to build the final runtime image
# which contains only the built app and required resource files.
FROM alpine AS runtime
WORKDIR /app

RUN apk update \
  && apk cache purge \
  && rm -rf /var/lib/apt/lists/*

# Copy the server binary to the /app directory
COPY --from=builder /work/target/release/formicaio /app/

# Copy Sqlite migrations files
COPY --from=builder /work/migrations /app/migrations

# /target/site contains our JS/WASM/CSS, etc.
COPY --from=builder /work/target/site /app/site

# Copy Cargo.toml if it’s needed at runtime
COPY --from=builder /work/Cargo.toml /app/

# Set any required env variables and
ENV RUST_LOG="info"
ENV LEPTOS_SITE_ADDR="0.0.0.0:52100"
ENV LEPTOS_SITE_ROOT="site"

EXPOSE 52100

# Run the server
CMD ["sh", "-c", "/app/formicaio start --addr ${LEPTOS_SITE_ADDR}"]
