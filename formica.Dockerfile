# Dockerfile for running a node

FROM rust:1.80-bullseye AS builder

# Make an /app dir, which everything will eventually live in
RUN mkdir -p /app
WORKDIR /app

# Install node binary
RUN curl -sSL https://raw.githubusercontent.com/maidsafe/safeup/main/install.sh | bash
RUN /usr/local/bin/safeup node -p /app

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update -y \
  && apt-get install -y --no-install-recommends openssl ca-certificates \
  && apt-get autoremove -y \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*

# Copy the node binary to the /app directory
COPY --from=builder /app/safenode /app/
RUN /app/safenode --version

# Set any required env variables
#ENV RUST_LOG="info"
# Set default port numbers for node and its RPC API
ENV NODE_PORT=12000
ENV RPC_PORT=13000

EXPOSE $NODE_PORT
EXPOSE $RPC_PORT

# Run the node
CMD ["sh", "-c", \
      "/app/safenode --home-network \
      --port ${NODE_PORT} \
      --rpc 0.0.0.0:${RPC_PORT} \
      --log-output-dest stdout" \
      #--log-format json" \
    ]