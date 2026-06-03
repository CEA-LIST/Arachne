# Dockerfile for json-crdt network node
#
# Build context must be a directory that contains this project AND the moirai
# workspace (paths in Cargo.toml must resolve).
#
# Quick start (from the workspace root that contains both moirai and this project):
#
#   docker build -f <path-to-this>/Dockerfile -t json-crdt .
#
#   docker run -e REPLICA_ID=a -e LISTEN_PORT=9001 -e HTTP_PORT=8081 \
#       -p 9001:9001 -p 8081:8081 json-crdt

FROM rust:slim AS builder

RUN rustup default nightly

WORKDIR /app

# Copy the entire build context (workspace + generated project)
COPY . .

# Build the network_node example in release mode
RUN cargo build --release --example network_node -p json-crdt

# --- Runtime stage ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/examples/network_node /app/network_node

ENV REPLICA_ID=default
ENV LISTEN_PORT=9001
ENV HTTP_PORT=8081
ENV PEERS=

EXPOSE 9001 8081

CMD ["/app/network_node"]
