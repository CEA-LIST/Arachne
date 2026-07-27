# Dockerfile for json-crdt network node
#
# The context must reproduce the layout the Cargo.toml path dependencies
# encode, i.e. it must contain both of:
#
#   <context>/arachne/generated/json_crdt  <- this project
#   <context>/moirai                       <- the moirai workspace
#
# Quick start:
#
#   docker build -f <path-to-this>/Dockerfile -t json-crdt <context>
#
#   docker run -e REPLICA_ID=a -e LISTEN_PORT=9001 -e HTTP_PORT=8081 \
#       -p 9001:9001 -p 8081:8081 json-crdt

FROM rust:1.97-slim AS builder

WORKDIR /app

# Copy the entire build context (moirai workspace + generated project)
COPY . .

# Build the network_node example in release mode
WORKDIR /app/arachne/generated/json_crdt
RUN cargo build --release --example network_node

# --- Runtime stage ---
FROM debian:bookworm-slim

# `curl` is required by HEALTHCHECK below
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/arachne/generated/json_crdt/target/release/examples/network_node /app/network_node

ENV REPLICA_ID=default
ENV LISTEN_PORT=9001
ENV HTTP_PORT=8081
ENV PEERS=

EXPOSE 9001 8081

# Lets orchestrators and test harnesses wait on readiness instead of sleeping
HEALTHCHECK --interval=1s --timeout=2s --start-period=2s --retries=30 \
    CMD curl -fsS "http://localhost:$HTTP_PORT/api/health" || exit 1

CMD ["/app/network_node"]
