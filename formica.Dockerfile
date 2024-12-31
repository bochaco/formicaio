# Dockerfile for running a node

FROM debian:bookworm-slim AS builder

RUN mkdir -p /app
WORKDIR /app

# Install node binary
RUN apt-get update -y && apt-get install -y curl
RUN curl -sSL https://raw.githubusercontent.com/maidsafe/antup/main/install.sh | bash
RUN /usr/local/bin/antup node -p /app

FROM debian:bookworm-slim AS runtime
# Make an /app dir, which everything will eventually live in
WORKDIR /app

RUN apt-get update -y \
  # Temporary fix to use nginx since the node metrics server is exposed only at ip 127.0.0.1
  && apt-get install -y nginx \
  && apt-get autoremove -y \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*

# Copy antup binary to the /app directory
COPY --from=builder /usr/local/bin/antup /app/

# Copy the node binary to the /app directory
COPY --from=builder /app/antnode /app/
RUN /app/antnode --version

# Set any required env variables
# Set default port numbers for node and its metrics service
ENV NODE_PORT=12000
ENV METRICS_PORT=14000

# This can be used to set the rewards address. This is the address
# that will receive the rewards for the node: --rewards-address <REWARDS_ADDRESS>
ENV REWARDS_ADDR_ARG=''

EXPOSE $NODE_PORT
EXPOSE $METRICS_PORT

# Run the node
CMD ["sh", "-c", \
      "echo \"server { listen ${METRICS_PORT}; server_name localhost; location /metrics { proxy_pass http://127.0.0.1:9090/metrics; include /etc/nginx/proxy_params; } }\" > /etc/nginx/sites-available/default \
      && nginx \
      && if [ -e '/app/node_data/secret-key-recycle' ]; then rm -f /app/node_data/secret-key*; fi \
      && /app/antnode --home-network \
      --port ${NODE_PORT} \
      --metrics-server-port 9090 \
      --root-dir /app/node_data \
      --log-output-dest /app/node_data/logs \
      ${REWARDS_ADDR_ARG} \
      evm-arbitrum-sepolia" \
    ]