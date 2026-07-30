//! Generates deployment artifacts: `network_node` example and `Dockerfile`.
//!
//! These are written alongside the CRDT project so that every generated
//! crate is immediately runnable as a networked replica and Docker image.

use std::path::Path;

use heck::ToUpperCamelCase;

/// Where the generated project and the Moirai workspace sit inside the Docker
/// build context.
///
/// The generated manifest reaches Moirai through a relative `path` dependency,
/// so the image can only build if the context reproduces the same arrangement.
/// Both fields are paths relative to the context root.
#[derive(Debug, Clone)]
pub struct BuildContextLayout {
    /// Generated project, e.g. `"arachne/generated/json_crdt"`.
    pub project_dir: String,
    /// Moirai workspace, e.g. `"moirai"`.
    pub moirai_dir: String,
}

/// Parameters derived from the project/package names that the two renderers
/// need.
pub struct DeploymentCtx {
    /// Crate name with dashes (e.g. `"json-crdt"`).
    pub project_name: String,
    /// Crate name with underscores for `use` imports (e.g. `"json_crdt"`).
    pub crate_import: String,
    /// Ecore package name as-is (e.g. `"json"`).
    pub package_name: String,
    /// The generated Log type (e.g. `"JsonLog"`).
    pub log_type: String,
    /// Layout the `Dockerfile` expects of its build context, or `None` when
    /// the manifest carries absolute `path` dependencies — those can never
    /// resolve inside an image, so no concrete layout can be described.
    pub build_context: Option<BuildContextLayout>,
}

impl DeploymentCtx {
    pub fn new(
        project_name: &str,
        package_name: &str,
        build_context: Option<BuildContextLayout>,
    ) -> Self {
        let crate_import = project_name.replace('-', "_");
        let log_type = format!("{}Log", package_name.to_upper_camel_case());
        Self {
            project_name: project_name.to_string(),
            crate_import,
            package_name: package_name.to_string(),
            log_type,
            build_context,
        }
    }
}

/// Write `examples/network_node.rs` and `Dockerfile` into the project root.
pub fn write_deployment_artifacts(root: &Path, ctx: &DeploymentCtx) -> std::io::Result<()> {
    let examples_dir = root.join("examples");
    std::fs::create_dir_all(&examples_dir)?;

    std::fs::write(
        examples_dir.join("network_node.rs"),
        render_network_node(ctx),
    )?;
    std::fs::write(root.join("Dockerfile"), render_dockerfile(ctx))?;

    Ok(())
}

/// Generates a `network_node` example that runs a single replica as a
/// TCP-backed node with an optional HTTP adapter.
///
/// The HTTP API is implemented as a separate adapter in `moirai_network::http_api`
/// and can be combined with other adapters (WebSocket, CLI, etc.).
fn render_network_node(ctx: &DeploymentCtx) -> String {
    let DeploymentCtx {
        crate_import,
        log_type,
        package_name,
        ..
    } = ctx;

    format!(
        r#"//! Network node for the generated {package_name} CRDT.
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
//! docker build -t {crate_import} .
//! docker run -e REPLICA_ID=a -e LISTEN_PORT=9001 -e HTTP_PORT=8081 \
//!     -p 9001:9001 -p 8081:8081 {crate_import}
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

use {crate_import}::package::{log_type};
use moirai_network::HashMap;
use moirai_network::dashboard::DashboardConfig;
use moirai_network::discovery::DiscoveryConfig;
use moirai_network::generic::Node;

/// How other replicas reach this one's replication listener.
///
/// Not the same as what the process binds: in a container the bind is
/// `0.0.0.0:9001` while the reachable address is the container's name on the
/// user-defined network. Docker sets `HOSTNAME` to the container id, and
/// Compose registers that as a DNS alias, so it is the right default; override
/// with `ADVERTISE_ADDR` when a stable service name is wanted instead.
fn advertise_addr(listen_port: u16) -> String {{
    env::var("ADVERTISE_ADDR").unwrap_or_else(|_| {{
        let host = env::var("HOSTNAME")
            .ok()
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        format!("{{host}}:{{listen_port}}")
    }})
}}

fn main() {{
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
    for spec in peers_str.split(',').filter(|s| !s.is_empty()) {{
        let parts: Vec<&str> = spec.split(':').collect();
        if parts.len() >= 3 {{
            let peer_id = parts[0].to_string();
            let addr = format!("{{}}:{{}}", parts[1], parts[2]);
            all_members.push(peer_id.clone());
            peer_addresses.insert(peer_id, addr);
        }}
    }}

    let member_refs: Vec<&str> = all_members.iter().map(|s| s.as_str()).collect();

    let mut node = Node::<{log_type}>::new(
        replica_id.clone(),
        &member_refs,
        listen_port,
        peer_addresses,
    );

    node.enable_state_query();
    node.enable_state_transfer();

    if let Some(port) = http_port {{
        node.start_http(port);
    }}

    // Discovery is opt-in. Unset `BOOTNODE_URL` and everything below behaves
    // exactly as it did before phase 1, which is what keeps the existing e2e
    // suite an honest guard rail.
    if let Ok(bootnode_url) = env::var("BOOTNODE_URL") {{
        if !bootnode_url.is_empty() {{
            node.enable_discovery(DiscoveryConfig {{
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
            }});
        }}
    }}

    // Monitoring is opt-in for the same reason and on the same terms: no
    // `DASHBOARD_URL`, no thread, no outbound request, no delivery trace.
    if let Some(config) = DashboardConfig::from_env(&replica_id) {{
        node.enable_dashboard(config);
    }}

    // Give peers time to start, then connect
    thread::sleep(Duration::from_secs(2));
    node.connect();

    eprintln!(
        "[{{}}] Running. POST ops to http://localhost:{{}}/api/op",
        replica_id,
        http_port.unwrap_or(0)
    );

    node.run();
}}
"#
    )
}

/// Rust toolchain the generated `Dockerfile` pins.
///
/// A concrete stable version, not a floating tag: the generated image must
/// rebuild identically in CI. The generated crate builds on stable since the
/// `let_chains` feature it used stabilised in Rust 1.88.
const RUST_IMAGE: &str = "rust:1.97-slim";

/// Generates a multi-stage Dockerfile that builds the `network_node` example
/// into a minimal runtime image.
fn render_dockerfile(ctx: &DeploymentCtx) -> String {
    let project_name = &ctx.project_name;

    // Without a known layout the manifest holds absolute `path` dependencies,
    // which cannot resolve inside an image; say so instead of emitting a
    // recipe that is guaranteed to fail.
    let (layout_doc, project_dir) = match &ctx.build_context {
        Some(layout) => (
            format!(
                "# The context must reproduce the layout the Cargo.toml path dependencies
# encode, i.e. it must contain both of:
#
#   <context>/{:<width$}  <- this project
#   <context>/{:<width$}  <- the moirai workspace",
                layout.project_dir,
                layout.moirai_dir,
                width = layout.project_dir.len().max(layout.moirai_dir.len()),
            ),
            layout.project_dir.clone(),
        ),
        None => (
            "# WARNING: this project was generated with absolute Moirai path dependencies,
# which cannot resolve inside an image. Regenerate with
# `--moirai-path-style relative` before building."
                .to_string(),
            ".".to_string(),
        ),
    };

    format!(
        r#"# Dockerfile for {project_name} network node
#
{layout_doc}
#
# Quick start:
#
#   docker build -f <path-to-this>/Dockerfile -t {project_name} <context>
#
#   docker run -e REPLICA_ID=a -e LISTEN_PORT=9001 -e HTTP_PORT=8081 \
#       -p 9001:9001 -p 8081:8081 {project_name}

FROM {RUST_IMAGE} AS builder

WORKDIR /app

# Copy the entire build context (moirai workspace + generated project)
COPY . .

# Build the network_node example in release mode
WORKDIR /app/{project_dir}
RUN cargo build --release --example network_node

# --- Runtime stage ---
FROM debian:bookworm-slim

# `curl` is required by HEALTHCHECK below
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/{project_dir}/target/release/examples/network_node /app/network_node

ENV REPLICA_ID=default
ENV LISTEN_PORT=9001
ENV HTTP_PORT=8081
ENV PEERS=

EXPOSE 9001 8081

# Lets orchestrators and test harnesses wait on readiness instead of sleeping
HEALTHCHECK --interval=1s --timeout=2s --start-period=2s --retries=30 \
    CMD curl -fsS "http://localhost:$HTTP_PORT/api/health" || exit 1

CMD ["/app/network_node"]
"#
    )
}
