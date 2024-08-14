# Dockerfile for running app on UmbrelOS

FROM rust:slim

WORKDIR /app
RUN cargo install trunk
RUN rustup target add wasm32-unknown-unknown
COPY . .

# FIXME: nodejs not installed
RUN npm i -D daisyui@latest

RUN trunk build

CMD ["trunk", "serve", "--address", "0.0.0.0", "--port", "3000"]
EXPOSE 3000
