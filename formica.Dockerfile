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

# Specify whether the node is operating from a home network and situated
# behind a NAT without port forwarding capabilities.
# Setting this flag, activates hole-punching in antnode to facilitate direct
# connections from other nodes.
# If this not enabled and the node is behind a NAT, the node is terminated.
ENV HOME_NETWORK_ARG='--home-network'

#EXPOSE $NODE_PORT/udp
#EXPOSE $METRICS_PORT/tcp

# Run the node
CMD ["sh", "-c", \
      "if [ -e '/app/node_data/secret-key-recycle' ]; then rm -f /app/node_data/secret-key*; fi \
      && /app/antnode \
      ${HOME_NETWORK_ARG} \
      --port ${NODE_PORT} \
      --metrics-server-port ${METRICS_PORT} \
      --root-dir /app/node_data \
      --log-output-dest /app/node_data/logs \
      --bootstrap-cache-dir /app/node_data \
      ${REWARDS_ADDR_ARG} \
      evm-arbitrum-sepolia" \
    ]