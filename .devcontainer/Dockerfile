# syntax=docker/dockerfile:1.7

FROM rust:1.93-bookworm

RUN apt-get update \
    && apt-get install --yes --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY . /opt/arachne-build

WORKDIR /opt/arachne-build
RUN cargo build --locked --release -p arachne-cli \
    && install target/release/arachne /usr/local/bin/arachne \
    && rm -rf target

WORKDIR /workspaces/arachne

CMD ["sleep", "infinity"]
