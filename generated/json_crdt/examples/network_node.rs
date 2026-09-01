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
//! # Monitoring
//!
//! Set `DASHBOARD_URL` to have the replica post what it delivers, and what the
//! CRDT did with it, to a `moirai-dashboard`. Outbound only, so it works from
//! behind NAT; unset means no thread and no request.
//!
//! - `DASHBOARD_URL`         — unset means no reporting at all
//! - `DASHBOARD_INTERVAL_MS` — gap between state snapshots, default `1000`
//!
//! # Metamodel discovery
//!
//! The node can serve the crate's metamodel descriptor on
//! `GET /api/metamodel`, so a metamodel-agnostic client can shape itself to
//! whatever node it connects to. `METAMODEL_PATH` names the descriptor file;
//! unset, the node tries `metamodel.json` in the working directory (the
//! generator writes one next to the crate manifest). Without a readable
//! descriptor the endpoint answers 404, exactly like any unknown path.
//!
//! - `METAMODEL_PATH` — descriptor file, default `metamodel.json`
//!
//! # Log identity
//!
//! Every replica of a session must host the same log, and `LOG_ID` names it:
//! 32 lowercase hex characters, the same value on every replica. Unset, the
//! replica mints a fresh id and prints it — right for the replica that
//! creates a session, wrong for one joining it, whose peers would refuse its
//! events as belonging to another log.
//!
//! - `LOG_ID` — the log this replica hosts; unset mints a fresh one
//!
//! # HTTP API
//!
//! - `POST /api/op`        — submit a JSON-serialised operation
//! - `GET  /api/state`     — query the current CRDT state
//! - `GET  /api/metamodel` — metamodel descriptor, when configured
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
use moirai_network::dashboard::DashboardConfig;
use moirai_network::discovery::DiscoveryConfig;
use moirai_network::generic::Node;
use moirai_protocol::log_id::LogId;

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

    // The log this replica hosts. Set, `LOG_ID` means join that log; unset,
    // a fresh id is minted and printed, which is right only for the replica
    // that creates the session — peers hosting a different log refuse each
    // other's events.
    let log_id = match env::var("LOG_ID") {
        Ok(raw) => LogId::parse(&raw).unwrap_or_else(|err| {
            eprintln!("[{replica_id}] invalid LOG_ID `{raw}`: {err}");
            std::process::exit(1);
        }),
        Err(_) => {
            let id = LogId::generate();
            eprintln!("[{replica_id}] log id: {id}");
            id
        }
    };

    let mut node = Node::<JsonLog>::new_with_log_id(
        replica_id.clone(),
        &member_refs,
        listen_port,
        peer_addresses,
        log_id,
    );

    node.enable_state_query();
    node.enable_state_transfer();

    // Metamodel discovery is opt-in on the same terms as everything below:
    // no readable descriptor, no `/api/metamodel` — the endpoint answers 404
    // exactly as it always has. Must run before `start_http`, which
    // snapshots the descriptor.
    let metamodel_path = env::var("METAMODEL_PATH").ok();
    let metamodel_explicit = metamodel_path.is_some();
    let metamodel_path = metamodel_path.unwrap_or_else(|| "metamodel.json".to_string());
    match std::fs::read_to_string(&metamodel_path) {
        Ok(descriptor) => {
            eprintln!("[{replica_id}] serving metamodel descriptor from `{metamodel_path}`");
            node.serve_metamodel(descriptor);
        }
        Err(err) if metamodel_explicit => {
            eprintln!(
                "[{replica_id}] cannot read METAMODEL_PATH `{metamodel_path}`: {err}; \
                 /api/metamodel stays 404"
            );
        }
        Err(_) => {}
    }

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

    // Monitoring is opt-in for the same reason and on the same terms: no
    // `DASHBOARD_URL`, no thread, no outbound request, no delivery trace.
    if let Some(config) = DashboardConfig::from_env(&replica_id) {
        node.enable_dashboard(config);
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
