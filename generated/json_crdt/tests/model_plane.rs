//! The model plane at the node binary: mp7, mp8, mp13 and mp30 to mp33 of
//! the validation plan, driven the way the e2e process backend starts a node
//! — the `network_node` example as a child process, spoken to over HTTP —
//! and mp32 in process, over the conformance module the binary installs.
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
use sha2::{Digest, Sha256};

/// The conformance module exactly as the node binary installs it, so mp32
/// checks the text that runs and not a copy.
#[allow(dead_code)]
#[path = "../examples/conformance/mod.rs"]
mod conformance;
#[path = "support/conformance_workload.rs"]
mod workload;

use conformance::{Schema, check_structure};
use workload::Generator;

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
    // were about the header and not about the route. A write the descriptor
    // declares, since the structural check refuses a root key `Root` does not.
    let (_, reply) = node.post(
        &path,
        &ops::object_update("eClass", ops::string_insert('R', 0)),
    );
    assert_eq!(reply["success"], json!(true), "{reply}");
    assert_eq!(node.model_document(&model_id)["eClass"], json!("R"));
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

// ---------------------------------------------------------------------------
// Conformance: the structural check at the intake (mp30 to mp33)
// ---------------------------------------------------------------------------

/// The digest rule, as the node and `arachne_codegen::metamodel_digest`
/// compute it, for a descriptor this test writes itself.
fn digest_of(descriptor: &Value) -> String {
    format!("{:x}", Sha256::digest(descriptor.to_string()))
}

fn metamodel_id_of(descriptor: &Value) -> Value {
    json!({ "nsURI": descriptor["nsURI"], "digest": digest_of(descriptor) })
}

/// `bt` with an enum attribute, since neither shipped descriptor has one:
/// `TreeNode` gains `status` of kind `enum` over the `Status` the descriptor
/// already lists. Another digest, the same `nsURI`.
fn bt_with_status() -> Value {
    let mut descriptor = read_json(&example(BT));
    descriptor["classes"]["TreeNode"]["attributes"]
        .as_array_mut()
        .expect("TreeNode has attributes")
        .push(json!({
            "name": "status", "kind": "enum", "enum": "Status",
            "many": false, "required": false, "isId": false
        }));
    descriptor
}

/// A `METAMODEL_DIR` holding the descriptors given, each written pretty
/// under its file name, so the node keys them by the digest over the parsed
/// value and not over the bytes.
fn descriptor_dir(name: &str, descriptors: &[(&str, &Value)]) -> PathBuf {
    let run = RUN_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("json-crdt-mp-{}-{run}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch descriptor directory");
    for (file, descriptor) in descriptors {
        std::fs::write(
            dir.join(file),
            serde_json::to_string_pretty(descriptor).expect("a descriptor serializes"),
        )
        .expect("a descriptor file");
    }
    dir
}

/// An operation at `path` under the root, the keys wrapped as `Object.Update`.
fn at(path: &[&str], leaf: Value) -> Value {
    let (first, rest) = path.split_first().expect("a path");
    let nested = rest
        .iter()
        .rev()
        .fold(leaf, |op, key| json!({ "Object": { "Update": [key, op] } }));
    ops::object_update(first, nested)
}

/// A character of the class tag at `path`, the way the editor writes one.
fn class_char(path: &[&str], ch: char, pos: usize) -> Value {
    let mut full: Vec<&str> = path.to_vec();
    full.push("eClass");
    at(&full, ops::string_insert(ch, pos))
}

impl NodeProcess {
    /// Posts one operation to a model and returns the node's answer.
    fn post_op(&self, model_id: &str, op: &Value) -> Value {
        let (status, reply) = self.post(&format!("/api/model/{model_id}/op"), op);
        assert_eq!(status, 200, "{reply}");
        reply
    }

    /// Posts an operation the descriptor rules out and returns the refusal.
    fn refused(&self, model_id: &str, what: &str, op: &Value) -> String {
        let reply = self.post_op(model_id, op);
        assert_eq!(
            reply["success"],
            json!(false),
            "{what} was applied: {op} -> {reply}"
        );
        let message = reply["message"].as_str().expect("a message").to_string();
        eprintln!("refused {what}: {message}");
        message
    }

    fn accepted(&self, model_id: &str, what: &str, op: &Value) {
        let reply = self.post_op(model_id, op);
        assert_eq!(
            reply["success"],
            json!(true),
            "{what} was refused: {op} -> {reply}"
        );
    }

    /// Creates a model under a descriptor this test wrote itself.
    fn create_model_under(&self, descriptor: &Value) -> String {
        let (status, reply) = self.post(
            "/api/models",
            &json!({ "metamodel_id": metamodel_id_of(descriptor) }),
        );
        assert_eq!(status, 201, "{reply}");
        reply["model_id"].as_str().expect("a model_id").to_string()
    }
}

/// **MP30** — the structural check refuses what the descriptor alone rules
/// out: seven refusals, each answered `success: false` with a sentence naming
/// the class or the feature at fault, then a well-formed write as the
/// positive control.
///
/// The wire writes a string one character at a time, so a class name or an
/// enum literal is judged character by character: at its first character
/// that fits no class or literal allowed at the slot. `Class` under
/// `BehaviorTree.child` is therefore refused at the `a`, after `C` and `l`,
/// which fit `CloseDoor`, went through; the document then holds that
/// two-character prefix, which the editor's diagnostics report, and this test
/// asserts the document exactly rather than pretending otherwise.
#[test]
fn mp30_the_structural_check_refuses_what_the_descriptor_alone_rules_out() {
    let bt = read_json(&example(BT));
    let uml = read_json(&example(UML));
    let with_status = bt_with_status();
    let dir = descriptor_dir(
        "mp30",
        &[
            (BT, &bt),
            (UML, &uml),
            ("bt-status.metamodel.json", &with_status),
        ],
    );
    let node = NodeProcess::start("mp30", &[("METAMODEL_DIR", dir.to_str().expect("utf-8"))]);
    let model = node.create_model_under(&bt);
    let fixture = node.create_model_under(&with_status);

    // 1. An object of `eClass` `Clas` at the root: the root is a `Root`.
    let message = node.refused(
        &model,
        "eClass `Clas` at the root",
        &class_char(&[], 'C', 0),
    );
    assert!(
        message.contains("the document root") && message.contains("Root"),
        "{message}"
    );

    // 2. The abstract `TreeNode` under `BehaviorTree.child`: no concrete
    //    class allowed there begins with `T`.
    let message = node.refused(
        &model,
        "the abstract TreeNode under BehaviorTree.child",
        &class_char(&["main", "child"], 'T', 0),
    );
    assert!(
        message.contains("`BehaviorTree.child`") && message.contains("Sequence"),
        "{message}"
    );

    // 3. A `colour` feature on the `TreeNode` under `BehaviorTree.child`.
    let message = node.refused(
        &model,
        "a colour feature on a TreeNode",
        &at(&["main", "child", "colour"], ops::string_insert('r', 0)),
    );
    assert!(
        message.contains("`colour`") && message.contains("`BehaviorTree.child`"),
        "{message}"
    );

    // 4. `ID`, a string, written with a `Number.Inc`.
    let message = node.refused(
        &model,
        "BehaviorTree.ID written as a number",
        &at(&["main", "ID"], ops::number_inc(1.0)),
    );
    assert!(
        message.contains("`BehaviorTree.ID`") && message.contains("`Number`"),
        "{message}"
    );

    // 5. `Class` under `BehaviorTree.child`, typed as the editor types it:
    //    refused at the `a`, the first character no `TreeNode` has there.
    node.accepted(
        &model,
        "the C of Class",
        &class_char(&["main", "child"], 'C', 0),
    );
    node.accepted(
        &model,
        "the l of Class",
        &class_char(&["main", "child"], 'l', 1),
    );
    let message = node.refused(
        &model,
        "the a of Class under BehaviorTree.child",
        &class_char(&["main", "child"], 'a', 2),
    );
    assert!(
        message.contains("`BehaviorTree.child`")
            && message.contains("`a` at position 2")
            && message.contains("CloseDoor"),
        "{message}"
    );

    // 6. A second object under `BehaviorTree.child`, whose `many` is false:
    //    a list where one object belongs.
    let message = node.refused(
        &model,
        "a list under BehaviorTree.child",
        &at(
            &["main", "child"],
            json!({ "Array": { "Insert": { "pos": 1,
                "op": { "Object": { "Update": ["eClass", ops::string_insert('S', 0)] } } } } }),
        ),
    );
    assert!(
        message.contains("`BehaviorTree.child` holds one `TreeNode`"),
        "{message}"
    );

    // 7. On the fixture model, `status` opened with `P`: no literal of
    //    `Status` begins so.
    let message = node.refused(
        &fixture,
        "PAUSED as a Status",
        &at(&["main", "child", "status"], ops::string_insert('P', 0)),
    );
    assert!(
        message.contains("`TreeNode.status`")
            && message.contains("`Status`")
            && message.contains("`P`"),
        "{message}"
    );
    assert_eq!(
        node.model_document(&fixture)["main"],
        Value::Null,
        "the fixture model holds something after seven refusals"
    );

    // The positive control: a `BehaviorTree` under `Root.behaviortrees`, and
    // the document holds it, the header, the `Cl` prefix from 5 and nothing else.
    node.accepted(
        &model,
        "a BehaviorTree under Root.behaviortrees",
        &at(
            &["behaviortrees"],
            json!({ "Array": { "Insert": { "pos": 0,
                "op": { "Object": { "Update": ["eClass", ops::string_insert('B', 0)] } } } } }),
        ),
    );
    let document = node.model_document(&model);
    assert_eq!(
        document,
        json!({
            "__model": { "modelId": model, "metamodelId": metamodel_id(BT) },
            "main": { "child": { "eClass": "Cl" } },
            "behaviortrees": [{ "eClass": "B" }],
        }),
        "the document holds something a refused operation put there"
    );
    std::fs::remove_dir_all(dir).ok();
}

/// **MP31** — a refused operation leaves the log and the counters untouched:
/// both op counters and the state are byte for byte what they were, and the
/// positive control then moves both counters by one.
#[test]
fn mp31_a_refused_operation_leaves_the_log_and_the_counters_untouched() {
    let node = NodeProcess::start("mp31", &[("METAMODEL_DIR", EXAMPLES)]);
    let model = node.create_model(BT);
    for (pos, ch) in "Sequence".chars().enumerate() {
        node.accepted(
            &model,
            "a Sequence under BehaviorTree.child",
            &class_char(&["main", "child"], ch, pos),
        );
    }
    let counters = |node: &NodeProcess| {
        (
            node.model_metric(&model, "ops_applied"),
            node.model_metric(&model, "delivered_ops"),
        )
    };
    let (before, state_before) = (
        counters(&node),
        node.get(&format!("/api/model/{model}/state")),
    );

    let message = node.refused(
        &model,
        "an eClass under BehaviorTree.child no allowed class begins with",
        &class_char(&["main", "child"], 'X', 0),
    );
    assert!(message.contains("`BehaviorTree.child`"), "{message}");
    assert_eq!(counters(&node), before, "a counter moved on a refusal");
    assert_eq!(
        node.get(&format!("/api/model/{model}/state")),
        state_before,
        "the state moved on a refusal"
    );

    node.accepted(
        &model,
        "BehaviorTree.ID",
        &at(&["main", "ID"], ops::string_insert('m', 0)),
    );
    assert_eq!(counters(&node), (before.0 + 1, before.1 + 1));
    assert_eq!(node.model_document(&model)["main"]["ID"], json!("m"));
}

/// **MP32** — two schemas from one descriptor agree on every operation: the
/// schema parsed from `bt.metamodel.json` as shipped, from the same
/// descriptor with its top-level keys reordered, and from the bytes the node
/// serves for a model bound to it, give one verdict — accept, or refuse with
/// one sentence — on each of 10,000 seeded operations, one in five malformed,
/// and a second pass in reverse order gives the same verdicts.
#[test]
fn mp32_two_schemas_from_one_descriptor_agree_on_every_operation() {
    let seed = std::env::var("MP32_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x5eed_0000_0000_0032u64);
    let shipped = std::fs::read_to_string(example(BT)).expect("bt.metamodel.json");
    let parsed: Value = serde_json::from_str(&shipped).expect("JSON");
    let reordered = {
        let field = |key: &str| format!("{}: {:#}", json!(key), parsed[key]);
        format!(
            "{{\n{}\n}}\n",
            [
                "rootClasses",
                "package",
                "enums",
                "classes",
                "nsURI",
                "formatVersion"
            ]
            .map(field)
            .join(",\n")
        )
    };
    assert_ne!(reordered, shipped);
    assert_eq!(
        serde_json::from_str::<Value>(&reordered).expect("JSON"),
        parsed
    );
    let node = NodeProcess::start("mp32", &[("METAMODEL_DIR", EXAMPLES)]);
    let model = node.create_model(BT);
    let (status, served) = node
        .try_request("GET", &format!("/api/model/{model}/metamodel"), None)
        .expect("the served descriptor");
    assert_eq!(status, 200, "{served}");
    drop(node);

    let schemas = [
        ("shipped", Schema::parse(&shipped).expect("a schema")),
        ("reordered", Schema::parse(&reordered).expect("a schema")),
        ("served", Schema::parse(&served).expect("a schema")),
    ];
    let mut generator = Generator::new(&schemas[0].1, seed, false);
    let ops: Vec<(Value, Option<&'static str>)> = (0..10_000)
        .map(|i| {
            if i % 5 == 4 {
                let (op, way) = generator.malformed();
                (op, Some(way))
            } else {
                (generator.conforming(), None)
            }
        })
        .collect();

    let verdicts: Vec<Result<(), String>> = ops
        .iter()
        .map(|(op, way)| {
            let verdict = check_structure(&schemas[0].1, op);
            match way {
                None => assert!(
                    verdict.is_ok(),
                    "seed {seed}: a well-formed operation was refused: {op} -> {verdict:?}"
                ),
                Some(way) => assert!(
                    verdict.is_err(),
                    "seed {seed}: an operation malformed as {way} was accepted: {op}"
                ),
            }
            verdict
        })
        .collect();
    for (name, schema) in &schemas[1..] {
        for ((op, _), expected) in ops.iter().zip(&verdicts) {
            assert_eq!(
                &check_structure(schema, op),
                expected,
                "seed {seed}: the {name} schema disagrees with the shipped one on {op}"
            );
        }
    }
    for ((op, _), expected) in ops.iter().rev().zip(verdicts.iter().rev()) {
        assert_eq!(
            &check_structure(&schemas[0].1, op),
            expected,
            "seed {seed}: a verdict depends on what came before: {op}"
        );
    }
    let refused = verdicts.iter().filter(|verdict| verdict.is_err()).count();
    eprintln!(
        "MP32 seed {seed}: {refused} of {} operations refused",
        ops.len()
    );
}

/// **MP33**, the node half — the default log and a log with no binding are
/// never checked: the `Clas` operation of mp30 is refused on the model route
/// and accepted through `/api/op` on the rig's pinned default log.
#[test]
fn mp33_the_default_log_and_a_log_with_no_binding_are_never_checked() {
    let node = NodeProcess::start(
        "mp33",
        &[
            ("METAMODEL_DIR", EXAMPLES),
            ("LOG_ID", "c0113c7ed10c0113c7ed10c0113c7ed1"),
        ],
    );
    let model = node.create_model(BT);
    let op = class_char(&[], 'C', 0);
    let message = node.refused(&model, "eClass `Clas` at the root of a bound model", &op);
    assert!(message.contains("Root"), "{message}");
    let (status, reply) = node.post("/api/op", &op);
    assert_eq!(status, 200, "{reply}");
    assert_eq!(
        reply["success"],
        json!(true),
        "the default log was checked: {reply}"
    );
    let (status, state) = node.get("/api/state");
    assert_eq!(status, 200, "{state}");
    assert_eq!(decode(&state["json"])["eClass"], json!("C"));
}
