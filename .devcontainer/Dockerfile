FROM rust:1.93-bookworm AS builder

WORKDIR /src
COPY . .
RUN cargo build --locked --release -p arachne-cli

FROM rust:1.93-bookworm

COPY --from=builder /src/target/release/arachne /usr/local/bin/arachne
