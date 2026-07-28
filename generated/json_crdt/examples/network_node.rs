//! Network node for the generated json CRDT.
//!
//! Runs a single replica with TCP peer-to-peer sync and an HTTP API.
//!
//! # Usage
//!
//! ```bash
//! # Single node
//! REPLICA_ID=a LISTEN_PORT=9001 HTTP_PORT=8081 \
//!     cargo run --example network_node
//!
//! # Two-node cluster
//! REPLICA_ID=a LISTEN_PORT=9001 HTTP_PORT=8081 PEERS=b:127.0.0.1:9002 \
//!     cargo run --example network_node &
//! REPLICA_ID=b LISTEN_PORT=9002 HTTP_PORT=8082 PEERS=a:127.0.0.1:9001 \
//!     cargo run --example network_node &
//! ```
//!
//! # Docker
//!
//! ```bash
//! docker build -t json_crdt .
//! docker run -e REPLICA_ID=a -e LISTEN_PORT=9001 -e HTTP_PORT=8081 \
//!     -p 9001:9001 -p 8081:8081 json_crdt
//! ```
//!
//! # Peer discovery
//!
//! Set `BOOTNODE_URL` to have the replica register with a bootnode session
//! directory every `RECONCILE_SECS` and dial whatever the roster returns. When
//! it is unset the replica behaves exactly as it always has: `PEERS` only,
//! dialled once. `PEERS` keeps working when both are set, as a static override.
//!
//! ```bash
//! # Three replicas that were never told about each other
//! BOOTNODE_URL=http://bootnode:7000 SESSION_ID=demo \
//!     REPLICA_ID=a LISTEN_PORT=9001 HTTP_PORT=8081 ADVERTISE_ADDR=node-a:9001 \
//!     cargo run --example network_node
//! ```
//!
//! - `BOOTNODE_URL`   — unset means no discovery at all
//! - `SESSION_ID`     — session to join, default `default`
//! - `ADVERTISE_ADDR` — `host:port` peers dial, default `$HOSTNAME:$LISTEN_PORT`
//! - `RECONCILE_SECS` — re-register interval, default `5`
//!
//! # HTTP API
//!
//! - `POST /api/op`        — submit a JSON-serialised operation
//! - `GET  /api/state`     — query the current CRDT state
//! - `GET  /api/metrics`   — causal-stability and log-size counters
//! - `GET  /api/health`    — health check
//! - `GET  /api/peers`     — list connected peers
//! - `POST /api/leave`     — deregister from the bootnode session
//! - `POST /api/pause/<id>`  — simulate disconnection from a peer
//! - `POST /api/resume/<id>` — resume and auto-sync with a peer
//! - `POST /api/pause-all`   — pause all peers
//! - `POST /api/resume-all`  — resume all peers

use std::env;
use std::thread;
use std::time::Duration;

use json_crdt::package::JsonLog;
use moirai_network::HashMap;
use moirai_network::discovery::DiscoveryConfig;
use moirai_network::generic::TcpNode;

/// How other replicas reach this one's replication listener.
///
/// Not the same as what the process binds: in a container the bind is
/// `0.0.0.0:9001` while the reachable address is the container's name on the
/// user-defined network. Docker sets `HOSTNAME` to the container id, and
/// Compose registers that as a DNS alias, so it is the right default; override
/// with `ADVERTISE_ADDR` when a stable service name is wanted instead.
fn advertise_addr(listen_port: u16) -> String {
    env::var("ADVERTISE_ADDR").unwrap_or_else(|_| {
        let host = env::var("HOSTNAME")
            .ok()
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        format!("{host}:{listen_port}")
    })
}

fn main() {
    let replica_id = env::var("REPLICA_ID").unwrap_or_else(|_| "replica-a".to_string());
    let listen_port: u16 = env::var("LISTEN_PORT")
        .unwrap_or_else(|_| "9001".to_string())
        .parse()
        .expect("Invalid LISTEN_PORT");
    let http_port: Option<u16> = env::var("HTTP_PORT").ok().and_then(|p| p.parse().ok());
    let peers_str = env::var("PEERS").unwrap_or_default();

    // Parse PEERS=id:host:port,...
    let mut peer_addresses: HashMap<String, String> = HashMap::default();
    let mut all_members: Vec<String> = vec![replica_id.clone()];
    for spec in peers_str.split(',').filter(|s| !s.is_empty()) {
        let parts: Vec<&str> = spec.split(':').collect();
        if parts.len() >= 3 {
            let peer_id = parts[0].to_string();
            let addr = format!("{}:{}", parts[1], parts[2]);
            all_members.push(peer_id.clone());
            peer_addresses.insert(peer_id, addr);
        }
    }

    let member_refs: Vec<&str> = all_members.iter().map(|s| s.as_str()).collect();

    let mut node = TcpNode::<JsonLog>::new(
        replica_id.clone(),
        &member_refs,
        listen_port,
        peer_addresses,
    );

    node.enable_state_query();
    node.enable_state_transfer();

    if let Some(port) = http_port {
        node.start_http(port);
    }

    // Discovery is opt-in. Unset `BOOTNODE_URL` and everything below behaves
    // exactly as it did before phase 1, which is what keeps the existing e2e
    // suite an honest guard rail.
    if let Ok(bootnode_url) = env::var("BOOTNODE_URL") {
        if !bootnode_url.is_empty() {
            node.enable_discovery(DiscoveryConfig {
                bootnode_url,
                session: env::var("SESSION_ID").unwrap_or_else(|_| "default".to_string()),
                replica_id: replica_id.clone(),
                advertise_addr: advertise_addr(listen_port),
                interval: Duration::from_secs(
                    env::var("RECONCILE_SECS")
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(5),
                ),
            });
        }
    }

    // Give peers time to start, then connect
    thread::sleep(Duration::from_secs(2));
    node.connect();

    eprintln!(
        "[{}] Running. POST ops to http://localhost:{}/api/op",
        replica_id,
        http_port.unwrap_or(0)
    );

    node.run();
}
