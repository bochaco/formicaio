# Dockerfile for running a node

FROM rust:1-alpine AS builder

RUN mkdir -p /app
WORKDIR /app

# Install node binary
RUN apk add curl bash
RUN ARCH=$(uname -m); \
    if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then \
      URL="https://github.com/WithAutonomi/ant-node/releases/download/v0.10.0/ant-node-cli-linux-arm64.tar.gz"; \
    else \
      URL="https://github.com/WithAutonomi/ant-node/releases/download/v0.10.0/ant-node-cli-linux-x64.tar.gz"; \
    fi; \
    curl -L "$URL" | tar xz -C ./
RUN /app/ant-node --version

FROM alpine AS runtime
# Make an /app dir, which everything will eventually live in
WORKDIR /app

# Copy the node binary to the /app directory
COPY --from=builder /app/ant-node /app/
# Copy the bootstrap peers list to the /app directory
COPY --from=builder /app/bootstrap_peers.toml /app/

# Set any required env variables
# Set default port numbers for node and its metrics service
ENV NODE_PORT=12000
ENV METRICS_PORT=14000

# This can be used to set the rewards address. This is the address
# that will receive the rewards for the node: --rewards-address <REWARDS_ADDRESS>
ENV REWARDS_ADDR_ARG=''

ENV IPV4_ONLY_ARG=''

# Define whether to enable node logs.
ENV NODE_LOGS_ARG='--log-dir /app/node_data/logs'

# Run the node
CMD ["sh", "-c", "while true; \
  do \
  CURRENT_VERSION=$(/app/ant-node --version); \
  if [ -e '/app/node_data/secret-key-recycle' ]; then rm -f /app/node_data/secret-key*; fi \
  && /app/ant-node \
  --stop-on-upgrade \
  ${IPV4_ONLY_ARG} \
  --port ${NODE_PORT} \
  --metrics-port ${METRICS_PORT} \
  --root-dir /app/node_data \
  --enable-logging \
  ${NODE_LOGS_ARG} \
  --bootstrap-cache-dir /app/node_data \
  ${REWARDS_ADDR_ARG} \
  --evm-network arbitrum-one; \
  EXIT_CODE=$?; \
  NEW_VERSION=$(/app/ant-node --version); \
  if [ \"${NEW_VERSION}\" != \"${CURRENT_VERSION}\" ]; then \
    echo \"Version changed from ${CURRENT_VERSION} to ${NEW_VERSION}, restarting...\"; \
  else \
    echo \"Version is the same ${NEW_VERSION}, not restarting.\"; \
    exit ${EXIT_CODE}; \
  fi; \
  done" \
]