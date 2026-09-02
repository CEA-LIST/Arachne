//! The model plane at the node binary: mp7, mp8 and mp13 of the validation
//! plan, driven the way the e2e process backend starts a node — the
//! `network_node` example as a child process, spoken to over HTTP.
//!
//! The binary is `MOIRAI_E2E_NODE_BIN` when set, else the example `cargo
//! test` builds beside these tests; the descriptors are the checked-in
//! `examples/` of the repository, the same files the rig image ships.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use moirai_network::workload::ops;
use moirai_protocol::log_id::LogId;
use serde_json::{Value, json};

/// The checked-in descriptors, as the rig image ships them under `/metamodels`.
const EXAMPLES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");

const BT: &str = "bt.metamodel.json";
const UML: &str = "uml.metamodel.json";

static RUN_SEQ: AtomicU32 = AtomicU32::new(0);

fn node_binary() -> PathBuf {
    if let Ok(path) = std::env::var("MOIRAI_E2E_NODE_BIN") {
        return PathBuf::from(path);
    }
    let target = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("target"));
    target.join("debug").join("examples").join("network_node")
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("loopback bind")
        .local_addr()
        .expect("local addr")
        .port()
}

/// One `network_node` process, killed when dropped.
struct NodeProcess {
    child: Child,
    port: u16,
    log: PathBuf,
}

impl NodeProcess {
    /// Starts a node alone in its session with `env` on top of the basics,
    /// and waits for its HTTP API to answer. The working directory is a
    /// temporary one, as in the rig image, so the `metamodel.json` the
    /// generator writes beside this crate's manifest is not picked up as the
    /// default `METAMODEL_PATH`.
    fn start(name: &str, env: &[(&str, &str)]) -> Self {
        let binary = node_binary();
        assert!(
            binary.is_file(),
            "no node binary at {}: build the `network_node` example or set MOIRAI_E2E_NODE_BIN",
            binary.display()
        );
        let run = RUN_SEQ.fetch_add(1, Ordering::Relaxed);
        let log = std::env::temp_dir().join(format!(
            "json-crdt-mp-{}-{run}-{name}.log",
            std::process::id()
        ));
        let out = std::fs::File::create(&log).expect("log file");
        let err = out.try_clone().expect("log handle");
        let port = free_port();
        let child = Command::new(&binary)
            .env("REPLICA_ID", name)
            .env("LISTEN_PORT", free_port().to_string())
            .env("HTTP_PORT", port.to_string())
            .env("PEERS", "")
            .envs(env.iter().copied())
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .unwrap_or_else(|e| panic!("spawn {}: {e}", binary.display()));
        let node = Self { child, port, log };
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Ok((200, _)) = node.try_request("GET", "/api/health", None) {
                return node;
            }
            assert!(
                Instant::now() < deadline,
                "{name} never answered /api/health; its log:\n{}",
                node.log_tail()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn log_tail(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// One raw HTTP/1.1 exchange, `(status, body)`.
    fn try_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> std::io::Result<(u16, String)> {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port))?;
        let payload = body.unwrap_or("");
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            payload.len()
        )?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        let status = response
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .ok_or_else(|| std::io::Error::other(format!("no status line in {response}")))?;
        let body = response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
            .unwrap_or_default();
        Ok((status, body))
    }

    fn get(&self, path: &str) -> (u16, Value) {
        let (status, body) = self
            .try_request("GET", path, None)
            .unwrap_or_else(|e| panic!("GET {path}: {e}"));
        (status, parse(&body, path))
    }

    fn post(&self, path: &str, body: &Value) -> (u16, Value) {
        let (status, reply) = self
            .try_request("POST", path, Some(&body.to_string()))
            .unwrap_or_else(|e| panic!("POST {path}: {e}"));
        (status, parse(&reply, path))
    }

    /// The ids `GET /api/models` lists.
    fn hosted_models(&self) -> Vec<String> {
        let (status, reply) = self.get("/api/models");
        assert_eq!(status, 200, "{reply}");
        reply["models"]
            .as_array()
            .expect("a models array")
            .iter()
            .filter_map(|model| model["model_id"].as_str().map(str::to_string))
            .collect()
    }

    /// The decoded document of one model.
    fn model_document(&self, model_id: &str) -> Value {
        let (status, state) = self.get(&format!("/api/model/{model_id}/state"));
        assert_eq!(status, 200, "{state}");
        decode(&state["json"])
    }

    fn model_metric(&self, model_id: &str, field: &str) -> u64 {
        let (status, metrics) = self.get(&format!("/api/model/{model_id}/metrics"));
        assert_eq!(status, 200, "{metrics}");
        metrics[field]
            .as_u64()
            .unwrap_or_else(|| panic!("no `{field}` in {metrics}"))
    }

    /// Creates a model under `descriptor` and returns the id the node minted.
    fn create_model(&self, descriptor: &str) -> String {
        let (status, reply) = self.post(
            "/api/models",
            &json!({ "metamodel_id": metamodel_id(descriptor) }),
        );
        assert_eq!(status, 201, "{reply}");
        reply["model_id"]
            .as_str()
            .unwrap_or_else(|| panic!("no model_id in {reply}"))
            .to_string()
    }
}

impl Drop for NodeProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn parse(text: &str, path: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{path} answered non-JSON ({e}): {text}"))
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} should be readable: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} should be JSON: {e}", path.display()))
}

fn example(file: &str) -> PathBuf {
    Path::new(EXAMPLES).join(file)
}

/// The identity the fixture records for a checked-in descriptor, which is
/// what the codegen crate computed for it (its test holds the two together).
fn metamodel_id(file: &str) -> Value {
    let fixture = read_json(&example("fixtures/metamodel-digests.json"));
    let entry = &fixture[file];
    assert!(entry.is_object(), "no fixture entry for {file}");
    entry.clone()
}

/// The node's `{"Value": ...}` state as plain JSON, the way the editor decodes
/// it: strings are char arrays, `"Unset"` is null.
fn decode(node: &Value) -> Value {
    if node == "Unset" {
        return Value::Null;
    }
    let value = &node["Value"];
    if let Some(object) = value["Object"].as_object() {
        return Value::Object(
            object
                .iter()
                .map(|(key, child)| (key.clone(), decode(child)))
                .collect(),
        );
    }
    if let Some(array) = value["Array"].as_array() {
        return Value::Array(array.iter().map(decode).collect());
    }
    if let Some(chars) = value["String"].as_array() {
        return Value::String(chars.iter().filter_map(Value::as_str).collect());
    }
    if let Some(number) = value.get("Number") {
        return number.clone();
    }
    if let Some(boolean) = value.get("Boolean") {
        return boolean.clone();
    }
    panic!("unrecognised state node: {node}");
}

/// An operation that writes one character into `__model.metamodelId.digest`.
fn rewrite_header_digest(ch: char) -> Value {
    ops::object_update(
        "__model",
        json!({ "Object": { "Update": ["metamodelId",
            { "Object": { "Update": ["digest", ops::string_insert(ch, 0)] } }] } }),
    )
}

/// **MP7** — registration creates the log and writes the header once.
#[test]
fn mp7_registration_creates_the_log_and_writes_the_header_once() {
    let node = NodeProcess::start("mp7", &[("METAMODEL_DIR", EXAMPLES)]);
    let bt = metamodel_id(BT);

    let (status, reply) = node.post("/api/models", &json!({ "metamodel_id": bt }));
    assert_eq!(status, 201, "{reply}");
    let model_id = reply["model_id"].as_str().expect("a model_id").to_string();
    LogId::parse(&model_id).unwrap_or_else(|e| panic!("`{model_id}` is not a log id: {e}"));

    let header = node.model_document(&model_id)["__model"].clone();
    assert_eq!(
        header,
        json!({ "modelId": model_id, "metamodelId": bt }),
        "the header does not name the model and the bt pair"
    );
    let hosted = node.hosted_models();

    let (status, reply) = node.post(
        "/api/models",
        &json!({ "model_id": model_id, "metamodel_id": bt }),
    );
    assert_eq!(status, 409, "the same id was registered twice: {reply}");
    assert_eq!(node.model_document(&model_id)["__model"], header);

    let unknown = json!({ "nsURI": "http://example.org/nowhere", "digest": "0".repeat(64) });
    let (status, reply) = node.post("/api/models", &json!({ "metamodel_id": unknown }));
    assert_eq!(
        status, 422,
        "a model was hosted under a digest the node does not hold: {reply}"
    );
    assert_eq!(
        node.hosted_models(),
        hosted,
        "the refused registration left a model behind"
    );
}

/// **MP8** — a local operation on the header is rejected after creation.
#[test]
fn mp8_a_local_op_on_the_header_is_rejected_after_creation() {
    let node = NodeProcess::start("mp8", &[("METAMODEL_DIR", EXAMPLES)]);
    let model_id = node.create_model(BT);
    let header = node.model_document(&model_id)["__model"].clone();
    assert_eq!(header["metamodelId"], metamodel_id(BT));
    let path = format!("/api/model/{model_id}/op");
    let before = (
        node.model_metric(&model_id, "ops_applied"),
        node.model_metric(&model_id, "delivered_ops"),
    );

    for op in [
        rewrite_header_digest('x'),
        ops::object_remove("__model"),
        json!({ "JsonKind": { "Object": "Clear" } }),
    ] {
        let (status, reply) = node.post(&path, &op);
        assert_eq!(status, 200, "{reply}");
        assert_eq!(
            reply["success"],
            json!(false),
            "the header accepted a local write: {op} -> {reply}"
        );
    }

    assert_eq!(
        node.model_document(&model_id)["__model"],
        header,
        "the header moved"
    );
    assert_eq!(
        (
            node.model_metric(&model_id, "ops_applied"),
            node.model_metric(&model_id, "delivered_ops"),
        ),
        before,
        "the operation log grew"
    );
    // Positive control: the model is still writable, so the refusals above
    // were about the header and not about the route.
    let (_, reply) = node.post(
        &path,
        &ops::object_update("Sequence", ops::string_insert('a', 0)),
    );
    assert_eq!(reply["success"], json!(true), "{reply}");
    assert_eq!(node.model_document(&model_id)["Sequence"], json!("a"));
}

/// **MP13** — `METAMODEL_DIR` is keyed by digest, and `METAMODEL_PATH` still
/// works as the single-descriptor case.
#[test]
fn mp13_the_metamodel_dir_is_keyed_by_digest_and_metamodel_path_still_works() {
    let bt = metamodel_id(BT);
    let uml = metamodel_id(UML);
    let listing_of = |node: &NodeProcess| -> Vec<Value> {
        let (status, reply) = node.get("/api/metamodels");
        assert_eq!(status, 200, "{reply}");
        let mut listed: Vec<Value> = reply["metamodels"]
            .as_array()
            .expect("a metamodels array")
            .iter()
            .map(|entry| json!({ "nsURI": entry["nsURI"], "digest": entry["digest"] }))
            .collect();
        listed.sort_by_key(|entry| entry["digest"].to_string());
        listed
    };

    let both = NodeProcess::start("mp13-dir", &[("METAMODEL_DIR", EXAMPLES)]);
    let mut expected = vec![bt.clone(), uml.clone()];
    expected.sort_by_key(|entry| entry["digest"].to_string());
    assert_eq!(
        listing_of(&both),
        expected,
        "the listing is not keyed by digest with the right nsURI beside each"
    );

    for (descriptor, file) in [(&bt, BT), (&uml, UML)] {
        let (status, reply) = both.post("/api/models", &json!({ "metamodel_id": descriptor }));
        assert_eq!(status, 201, "{reply}");
        let model_id = reply["model_id"].as_str().expect("a model_id");
        let (status, served) = both.get(&format!("/api/model/{model_id}/metamodel"));
        assert_eq!(status, 200, "{served}");
        assert_eq!(
            served,
            read_json(&example(file)),
            "a model registered under {file} was served another descriptor"
        );
    }

    let alone = NodeProcess::start(
        "mp13-path",
        &[("METAMODEL_PATH", example(BT).to_str().expect("utf-8 path"))],
    );
    assert_eq!(
        listing_of(&alone),
        vec![bt],
        "the single descriptor is not listed under its own digest"
    );
}
