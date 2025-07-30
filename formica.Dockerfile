# Dockerfile for running a node

FROM rust:1.81-alpine AS builder

RUN mkdir -p /app
WORKDIR /app

# Install node binary
RUN apk add curl bash
RUN curl -sSL https://raw.githubusercontent.com/maidsafe/antup/main/install.sh | bash
RUN cp /usr/local/bin/antup /app/
RUN /app/antup node -n -p /app

FROM alpine AS runtime
# Make an /app dir, which everything will eventually live in
WORKDIR /app

# Copy antup binary to the /app directory
COPY --from=builder /app/antup /app/

# Copy the node binary to the /app directory
COPY --from=builder /app/antnode /app/

# Set any required env variables
# Set default port numbers for node and its metrics service
ENV NODE_PORT=12000
ENV METRICS_PORT=14000

# This can be used to set the rewards address. This is the address
# that will receive the rewards for the node: --rewards-address <REWARDS_ADDRESS>
ENV REWARDS_ADDR_ARG=''

ENV UPNP_ARG=''
ENV IP_ARG=''

# Define whether to enable node logs.
ENV NODE_LOGS_ARG='--log-output-dest /app/node_data/logs'

#EXPOSE $NODE_PORT/udp
#EXPOSE $METRICS_PORT/tcp

# Run the node
CMD ["sh", "-c", \
      "if [ -e '/app/node_data/secret-key-recycle' ]; then rm -f /app/node_data/secret-key*; fi \
      && /app/antnode \
      ${UPNP_ARG} \
      ${IP_ARG} \
      --port ${NODE_PORT} \
      --metrics-server-port ${METRICS_PORT} \
      --root-dir /app/node_data \
      ${NODE_LOGS_ARG} \
      --bootstrap-cache-dir /app/node_data \
      ${REWARDS_ADDR_ARG} \
      evm-arbitrum-one" \
    ]