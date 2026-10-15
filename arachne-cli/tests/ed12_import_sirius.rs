//! ED.12 (`cea-cdrt-knowledge/spec/items/ED.12.md`): `arachne import-sirius <project.json>
//! --descriptor <descriptor.json>` reads a Sirius Web / SysON project export and prints, on
//! stdout, the body `POST /api/model/{id}/ops` takes (`{"ops": [...]}`), the node operations
//! that build the model. Every reference whose target is in the project names the target's
//! `elementId` instead of the Sirius id SysON wrote; a reference whose target is outside the
//! project (a standard-library element) is written as the source wrote it and reported on
//! stderr, never dropped. Containment, attribute values and order are kept.
//!
//! The fixture `fixtures/ed12/survey.project.json` is a hand-written excerpt in SysON's JSON
//! shape with our own names and ids (no byte of any SysON model). `survey.expected.json` is the
//! same model written by hand as the node's canonical read-out, keeping only the keys whose
//! value is written (see [`normalise`]).
//!
//! - L1, `ed12_l1_*`: the command's exit code, the shape of stdout, no Sirius id left in it, and
//!   the report on stderr.
//! - L2, `ed12_l2_*`: a `model_node` started here takes the descriptor and a model under it, the
//!   printed batch is posted as printed, applied whole, and the read-back equals the expected
//!   state; every local reference resolves by the editor's rule (value equals an element's
//!   `elementId`, of a class conforming to the reference's target), and the library reference
//!   resolves nothing.
//! - `ed12_oracle_*`: checks of the instrument itself, green before any implementation.
//!
//! AR.06 (`cea-cdrt-knowledge/spec/items/AR.06.md`) re-pins the descriptor to AR.04's annotated
//! SysML (`demo/sysml.json`, digest `2df00538`), under which names, ids and the annotation's
//! `source` are multi-value registers, and adds `ed12_l1_register_*`: every text leaf of the
//! excerpt is one whole register write, and no character is inserted one by one.
//!
//! ```bash
//! ED12_NODE_BIN=/volatile/home/vp242103/.cache/cea-crdt-scratch/bin/model_node-04f5085 \
//!   nice -n 10 cargo test -j 2 -p arachne-cli --test ed12_import_sirius -- --test-threads=1 --nocapture
//! ```
//!
//! No descriptor, or (L2 only) no node binary: `E2E-SKIP ed12: <why>` and return. A skip is
//! not a pass. A claim failure panics with `ED12 claim:`; a harness failure with `set-up:`.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// `arachne digest demo/sysml.json`: SysON's SysML v2 descriptor as AR.04 annotated it, names,
/// ids and literals as multi-value registers (it was `ebd58069` before AR.06, now kept as
/// `demo/sysml-ebd58069.json`).
///
/// SI.10 (`cea-cdrt-knowledge/spec/items/SI.10.md`, legacy register L1): `demo/sysml.json` is
/// regenerated with SI.05's identity rule (`a75d8af4...`), under which a reference is a `Link` of
/// its target's creation identity, which a batch printed offline cannot know, and a reference to
/// a standard-library element outside the project names no creation at all. `import-sirius`
/// under the identity rule is not part of SI.10; this test holds what it claims on the descriptor
/// it was written for, kept by SI.10 as `demo/sysml-2df00538.json`.
const DESCRIPTOR_DIGEST: &str = "2df00538ec6fc36639611b6e57d0926e8422bce4aa7a17d4083bdbcd1f761457";
/// The one reference of the fixture whose target is outside the project.
const LIBRARY_VALUE: &str = "sysml:AttributeDefinition sysmllibrary:///de9b8ac4-d3a5-4f9a-840d-fe7afd0b7f92#42c66539-5bc6-43c6-9806-00600d1d5a66";
/// The `elementId` of the FeatureTyping that holds it, under `type`.
const LIBRARY_HOLDER: &str = "2c09fb6d";
const LIBRARY_FEATURE: &str = "type";
/// References of the fixture whose target is in the project, counted by hand (spec section 4).
const LOCAL_REFERENCES: usize = 13;
/// The batch under `2df00538`, counted by hand from the fixture (AR.06 spec): one `New` for the
/// root, 32 containment `Insert`s, 59 register `Write`s (32 `elementId`s, 12 `declaredName`s,
/// the annotation's `source`, 14 references) and 7 `Enable`s (6 `isComposite`, 1 `isAbstract`).
const REGISTER_BATCH_OPS: usize = 99;
const LOG_ID: &str = "ed12ed12ed12ed12ed12ed12ed12ed12";
const HEALTH_TIMEOUT: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ed12")
        .join(name)
}

fn project_path() -> PathBuf {
    fixture("survey.project.json")
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("set-up: reading {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("set-up: {} is not JSON: {e}", path.display()))
}

/// The SysML descriptor and its text, checked against the registered digest; `Err` when absent.
fn descriptor() -> Result<(PathBuf, String, Value), String> {
    let path = std::env::var_os("ED12_DESCRIPTOR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../demo/sysml-2df00538.json")
        });
    let text = fs::read_to_string(&path).map_err(|e| {
        format!(
            "no descriptor at {} ({e}); set ED12_DESCRIPTOR",
            path.display()
        )
    })?;
    let value: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("set-up: {} is not JSON: {e}", path.display()));
    let digest = arachne_codegen::metamodel_digest(&value);
    assert_eq!(
        digest,
        DESCRIPTOR_DIGEST,
        "set-up: {} is not the registered SysML descriptor",
        path.display()
    );
    Ok((path, text, value))
}

fn run_import(descriptor: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arachne"))
        .arg("import-sirius")
        .arg(project_path())
        .arg("--descriptor")
        .arg(descriptor)
        .env("RUST_LOG", "error")
        .output()
        .unwrap_or_else(|e| panic!("set-up: running arachne: {e}"))
}

/// Every Sirius `id` the project fixture carries, one per object that has one.
fn sirius_ids(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let (Some(Value::String(id)), Some(_)) = (map.get("id"), map.get("eClass")) {
                out.push(id.clone());
            }
            map.values().for_each(|v| sirius_ids(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| sirius_ids(v, out)),
        _ => {}
    }
}

/// Every `elementId` the project fixture carries.
fn element_ids(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(id)) = map.get("elementId") {
                out.push(id.clone());
            }
            map.values().for_each(|v| element_ids(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| element_ids(v, out)),
        _ => {}
    }
}

/// The read-out with every key whose value is what an unwritten feature reads as removed
/// (`""`, a zero, `false`, `null`, `[]`, `{}`; `moirai-interp/src/eval.rs` and `leaf.rs`),
/// `eClass` always kept. The fixture writes none of these values, so on it this keeps exactly
/// the written keys.
fn normalise(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, v) in map {
                let unwritten = match v {
                    Value::Null => true,
                    Value::Bool(b) => !b,
                    Value::String(s) => s.is_empty(),
                    Value::Number(n) => n.as_f64() == Some(0.0),
                    Value::Array(a) => a.is_empty(),
                    Value::Object(o) => o.is_empty(),
                };
                if key == "eClass" || !unwritten {
                    out.insert(key.clone(), normalise(v));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(normalise).collect()),
        other => other.clone(),
    }
}

/// Where two documents differ, by path, the first twenty.
fn differences(want: &Value, got: &Value, at: &str) -> Vec<String> {
    let mut out = Vec::new();
    match (want, got) {
        (Value::Object(w), Value::Object(g)) => {
            for key in w.keys().chain(g.keys().filter(|k| !w.contains_key(*k))) {
                match (w.get(key), g.get(key)) {
                    (Some(a), Some(b)) => out.extend(differences(a, b, &format!("{at}.{key}"))),
                    (Some(a), None) => out.push(format!("{at}.{key}: {a} expected, absent")),
                    (None, Some(b)) => out.push(format!("{at}.{key}: absent expected, {b}")),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(w), Value::Array(g)) if w.len() == g.len() => {
            for (i, (a, b)) in w.iter().zip(g).enumerate() {
                out.extend(differences(a, b, &format!("{at}[{i}]")));
            }
        }
        (Value::Array(w), Value::Array(g)) => out.push(format!(
            "{at}: {} items expected, {} read",
            w.len(),
            g.len()
        )),
        _ if want != got => out.push(format!("{at}: {want} expected, {got}")),
        _ => {}
    }
    out.truncate(20);
    out
}

// ---------------------------------------------------------------------------
// The editor's resolution rule, from the descriptor alone
// ---------------------------------------------------------------------------

/// The descriptor's class table: supertypes, and the reference features (non-derived,
/// `merge.kind == "reference"`) each class sees with their target.
struct Table {
    supers: BTreeMap<String, Vec<String>>,
    references: BTreeMap<String, Vec<(String, String)>>,
    id_attribute: BTreeMap<String, Vec<String>>,
}

impl Table {
    fn new(descriptor: &Value) -> Table {
        let classes = descriptor["classes"]
            .as_object()
            .expect("set-up: the descriptor has no `classes` object");
        let mut supers = BTreeMap::new();
        let mut references = BTreeMap::new();
        let mut id_attribute = BTreeMap::new();
        for (name, class) in classes {
            let own_supers: Vec<String> = class["superTypes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect();
            supers.insert(name.clone(), own_supers);
            let refs: Vec<(String, String)> = class["references"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|f| f["merge"]["kind"] == json!("reference"))
                .map(|f| {
                    (
                        f["name"].as_str().unwrap_or("").to_string(),
                        f["merge"]["target"].as_str().unwrap_or("").to_string(),
                    )
                })
                .collect();
            references.insert(name.clone(), refs);
            let ids: Vec<String> = class["attributes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|f| f["isId"] == json!(true))
                .filter_map(|f| f["name"].as_str().map(str::to_string))
                .collect();
            id_attribute.insert(name.clone(), ids);
        }
        Table {
            supers,
            references,
            id_attribute,
        }
    }

    /// `class` and every class above it.
    fn lineage(&self, class: &str) -> Vec<String> {
        let mut out = vec![class.to_string()];
        let mut at = 0;
        while at < out.len() {
            for s in self.supers.get(&out[at]).into_iter().flatten() {
                if !out.contains(s) {
                    out.push(s.clone());
                }
            }
            at += 1;
        }
        out
    }

    fn conforms(&self, class: &str, target: &str) -> bool {
        self.lineage(class).iter().any(|c| c == target)
    }

    /// The reference features `class` sees, each name once (the nearest declaration).
    fn references_of(&self, class: &str) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for (name, target) in self
            .lineage(class)
            .iter()
            .flat_map(|c| self.references.get(c).cloned().unwrap_or_default())
        {
            if !out.iter().any(|(seen, _)| *seen == name) {
                out.push((name, target));
            }
        }
        out
    }

    fn id_value(&self, element: &serde_json::Map<String, Value>) -> Option<String> {
        let class = element.get("eClass")?.as_str()?;
        self.lineage(class)
            .iter()
            .flat_map(|c| self.id_attribute.get(c).cloned().unwrap_or_default())
            .find_map(|name| {
                element
                    .get(&name)
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .filter(|v| !v.is_empty())
    }
}

/// Every element of a read-out, in document order.
fn elements<'a>(value: &'a Value, out: &mut Vec<&'a serde_json::Map<String, Value>>) {
    match value {
        Value::Object(map) => {
            if map.contains_key("eClass") {
                out.push(map);
            }
            map.values().for_each(|v| elements(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| elements(v, out)),
        _ => {}
    }
}

/// Resolves every reference value of `doc` as the editor does (`rules.ts`, `byIdValue`): the
/// first element whose id attribute equals the value, of a class conforming to the reference's
/// target. Answers the count resolved and the `(holder id, feature, value)` of every other.
fn resolve(table: &Table, doc: &Value) -> (usize, Vec<(String, String, String)>) {
    let mut all = Vec::new();
    elements(doc, &mut all);
    let ids: Vec<(String, String)> = all
        .iter()
        .filter_map(|e| Some((table.id_value(e)?, e.get("eClass")?.as_str()?.to_string())))
        .collect();
    let mut resolved = 0;
    let mut unresolved = Vec::new();
    for element in &all {
        let class = element["eClass"].as_str().unwrap_or("");
        let holder = table.id_value(element).unwrap_or_default();
        for (feature, target) in table.references_of(class) {
            let values: Vec<String> = match element.get(&feature) {
                Some(Value::String(s)) if !s.is_empty() => vec![s.clone()],
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
                Some(Value::Null) | None => Vec::new(),
                Some(other) => vec![other.to_string()],
            };
            for value in values {
                let hit = ids
                    .iter()
                    .any(|(id, c)| *id == value && table.conforms(c, &target));
                if hit {
                    resolved += 1;
                } else {
                    unresolved.push((holder.clone(), feature.clone(), value));
                }
            }
        }
    }
    (resolved, unresolved)
}

// ---------------------------------------------------------------------------
// The instrument, checked on its own fixtures
// ---------------------------------------------------------------------------

#[test]
fn ed12_oracle_expected_state_is_the_fixture_with_its_references_resolved() {
    let (_, _, descriptor) = match descriptor() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("\nE2E-SKIP ed12: {why}");
            return;
        }
    };
    let table = Table::new(&descriptor);
    let project = read_json(&project_path());
    let expected = read_json(&fixture("survey.expected.json"));
    assert_eq!(
        normalise(&expected),
        expected,
        "set-up: the expected state holds a key whose value reads as unwritten"
    );
    let (resolved, unresolved) = resolve(&table, &expected);
    println!(
        "\nED12 oracle expected state: {resolved} references resolve, unresolved {unresolved:?}"
    );
    assert_eq!(
        resolved, LOCAL_REFERENCES,
        "set-up: the expected state's local references"
    );
    assert_eq!(
        unresolved,
        vec![(
            LIBRARY_HOLDER.to_string(),
            LIBRARY_FEATURE.to_string(),
            LIBRARY_VALUE.to_string()
        )],
        "set-up: the expected state's unresolved references"
    );
    let mut ids = Vec::new();
    sirius_ids(&project, &mut ids);
    let mut project_elements = Vec::new();
    element_ids(&project, &mut project_elements);
    let mut expected_elements = Vec::new();
    element_ids(&expected, &mut expected_elements);
    assert_eq!(
        project_elements, expected_elements,
        "set-up: the two fixtures do not hold the same elementIds in the same order"
    );
    let mut all = Vec::new();
    elements(&expected, &mut all);
    // 32 objects with a Sirius id, and the one EAnnotation SysON writes without.
    assert_eq!((ids.len(), all.len()), (32, 33), "set-up: object counts");
}

// ---------------------------------------------------------------------------
// L1: the command
// ---------------------------------------------------------------------------

#[test]
fn ed12_l1_import_prints_one_batch_and_reports_the_library_reference() {
    let (path, _, _) = match descriptor() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("\nE2E-SKIP ed12: {why}");
            return;
        }
    };
    let output = run_import(&path);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    println!(
        "\nED12 L1 exit {:?}, stdout {} bytes, stderr:\n{}",
        output.status.code(),
        stdout.len(),
        stderr.trim_end()
    );
    assert!(
        output.status.success(),
        "ED12 claim: `arachne import-sirius` did not succeed on the excerpt: {:?}\n{stderr}",
        output.status.code()
    );

    let batch: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("ED12 claim: stdout is not one JSON value: {e}"));
    let keys: Vec<&String> = batch
        .as_object()
        .map(|m| m.keys().collect())
        .unwrap_or_default();
    assert_eq!(
        keys,
        vec!["ops"],
        "ED12 claim: stdout is not the body `POST /api/model/{{id}}/ops` takes"
    );
    let count = batch["ops"].as_array().map_or(0, Vec::len);
    println!("ED12 L1 ops printed {count}");
    assert!(count > 0, "ED12 claim: the batch holds no operation");

    let mut ids = Vec::new();
    sirius_ids(&read_json(&project_path()), &mut ids);
    let left: Vec<&String> = ids
        .iter()
        .filter(|id| stdout.contains(id.as_str()))
        .collect();
    println!("ED12 L1 Sirius ids left in the operations {}", left.len());
    assert!(
        left.is_empty(),
        "ED12 claim: the operations still carry Sirius ids: {left:?}"
    );

    let unresolved: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with("unresolved: "))
        .collect();
    let want = format!("unresolved: {LIBRARY_HOLDER} {LIBRARY_FEATURE} {LIBRARY_VALUE}");
    assert_eq!(
        unresolved,
        vec![want.as_str()],
        "ED12 claim: the library reference is not reported exactly once, as registered"
    );
    let summary = format!("references: {LOCAL_REFERENCES} resolved, 1 unresolved");
    assert!(
        stderr.lines().any(|line| line == summary),
        "ED12 claim: stderr has no line `{summary}`"
    );
}

/// Every string a leaf operation carries whole, as `(op, value)`: `Write`, `Add` or `Insert` of a
/// `Str`; and how many `InsertChar`s the batch holds.
fn whole_strings(value: &Value, out: &mut Vec<(String, String)>, chars: &mut usize) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(leaf)) = map.get("Leaf") {
                for (op, arg) in leaf {
                    if op == "InsertChar" {
                        *chars += 1;
                    }
                    let text = arg["Str"].as_str().or_else(|| arg["value"]["Str"].as_str());
                    if let Some(text) = text {
                        out.push((op.clone(), text.to_string()));
                    }
                }
            }
            map.values().for_each(|v| whole_strings(v, out, chars));
        }
        Value::Array(items) => items.iter().for_each(|v| whole_strings(v, out, chars)),
        _ => {}
    }
}

/// Every string the expected state holds but `eClass`: the excerpt's names, ids, the annotation's
/// `source` and the reference values, each once per place it is written.
fn expected_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                match v {
                    Value::String(s) if key != "eClass" => out.push(s.clone()),
                    _ => expected_strings(v, out),
                }
            }
        }
        Value::Array(items) => {
            for v in items {
                match v {
                    Value::String(s) => out.push(s.clone()),
                    _ => expected_strings(v, out),
                }
            }
        }
        _ => {}
    }
}

#[test]
fn ed12_l1_register_text_leaves_are_whole_register_writes() {
    let (path, _, _) = match descriptor() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("\nE2E-SKIP ed12: {why}");
            return;
        }
    };
    let output = run_import(&path);
    assert!(
        output.status.success(),
        "ED12 claim: `arachne import-sirius` did not succeed on the excerpt: {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let batch: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("ED12 claim: stdout is not one JSON value: {e}"));
    let count = batch["ops"].as_array().map_or(0, Vec::len);
    let mut whole = Vec::new();
    let mut chars = 0;
    whole_strings(&batch, &mut whole, &mut chars);
    let mut written: Vec<String> = whole
        .iter()
        .filter(|(op, _)| op == "Write")
        .map(|(_, text)| text.clone())
        .collect();
    let others: Vec<&(String, String)> = whole.iter().filter(|(op, _)| op != "Write").collect();
    let mut want = Vec::new();
    expected_strings(&read_json(&fixture("survey.expected.json")), &mut want);
    written.sort();
    want.sort();
    println!(
        "\nED12 L1 register: ops {count}, InsertChar {chars}, whole Write {}, other whole {}, \
         expected strings {}",
        written.len(),
        others.len(),
        want.len()
    );
    let mut failures = Vec::new();
    if chars != 0 {
        failures.push(format!("{chars} characters inserted one by one"));
    }
    if !others.is_empty() {
        failures.push(format!("strings carried by other ops: {others:?}"));
    }
    if written != want {
        failures.push(format!(
            "the whole Writes are not the expected state's strings, once each: \
             written {written:?}, expected {want:?}"
        ));
    }
    if count != REGISTER_BATCH_OPS {
        failures.push(format!("{count} ops, {REGISTER_BATCH_OPS} counted by hand"));
    }
    assert!(
        failures.is_empty(),
        "ED12 claim: the text leaves are not one register write each:\n  {}",
        failures.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// L2: the printed batch on a live node
// ---------------------------------------------------------------------------

#[test]
fn ed12_l2_the_printed_batch_builds_the_excerpt_on_a_live_node() {
    let (path, text, descriptor) = match descriptor() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("\nE2E-SKIP ed12: {why}");
            return;
        }
    };
    let binary = match node_binary() {
        Ok(b) => b,
        Err(why) => {
            eprintln!("\nE2E-SKIP ed12: {why}");
            return;
        }
    };
    let scratch =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("ed12-l2-{}", std::process::id()));
    fs::create_dir_all(&scratch).expect("set-up: scratch directory");
    let node = Node::spawn(&binary, &scratch).unwrap_or_else(|why| panic!("set-up: {why}"));

    let (status, answer) = node
        .request("POST", "/api/metamodels", Some(&text))
        .unwrap_or_else(|why| panic!("set-up: {why}"));
    println!("\nED12 L2 POST /api/metamodels {status} {answer}");
    assert!(
        (status == 201 || status == 200)
            && answer["metamodel"]["digest"] == json!(DESCRIPTOR_DIGEST),
        "set-up: the node did not take the descriptor: {status} {answer}"
    );
    let (status, answer) = node
        .request(
            "POST",
            "/api/models",
            Some(&json!({ "metamodel_id": DESCRIPTOR_DIGEST }).to_string()),
        )
        .unwrap_or_else(|why| panic!("set-up: {why}"));
    let model = answer["model_id"]
        .as_str()
        .unwrap_or_else(|| panic!("set-up: no model created: {status} {answer}"))
        .to_string();
    println!("ED12 L2 model {model}");

    let output = run_import(&path);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        output.status.success(),
        "ED12 claim: `arachne import-sirius` did not succeed on the excerpt: {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let count = serde_json::from_str::<Value>(&stdout)
        .ok()
        .and_then(|b| b["ops"].as_array().map(Vec::len))
        .unwrap_or(0);

    let started = Instant::now();
    let (status, answer) = node
        .request("POST", &format!("/api/model/{model}/ops"), Some(&stdout))
        .unwrap_or_else(|why| panic!("set-up: {why}"));
    println!(
        "ED12 L2 POST ops: {count} posted, answered {status} {answer} in {:.1} s",
        started.elapsed().as_secs_f64()
    );
    assert!(
        status == 200 && answer["applied"] == json!(count),
        "ED12 claim: the node did not apply the printed batch whole: {status} {answer}"
    );

    let (status, state) = node
        .request("GET", &format!("/api/model/{model}/state"), None)
        .unwrap_or_else(|why| panic!("set-up: {why}"));
    assert_eq!(
        status, 200,
        "set-up: reading the state answered {status}: {state}"
    );
    let expected = read_json(&fixture("survey.expected.json"));
    let read = normalise(&state);
    let diffs = differences(&expected, &read, "$");
    println!(
        "ED12 L2 read-back equal to the expected state {}",
        diffs.is_empty()
    );
    let table = Table::new(&descriptor);
    let (resolved, unresolved) = resolve(&table, &state);
    println!("ED12 L2 references resolved {resolved}, unresolved {unresolved:?}");
    drop(node);
    let _ = fs::remove_dir_all(&scratch);

    assert!(
        diffs.is_empty(),
        "ED12 claim: the read-back differs from the expected state:\n  {}",
        diffs.join("\n  ")
    );
    assert_eq!(
        (resolved, unresolved),
        (
            LOCAL_REFERENCES,
            vec![(
                LIBRARY_HOLDER.to_string(),
                LIBRARY_FEATURE.to_string(),
                LIBRARY_VALUE.to_string()
            )]
        ),
        "ED12 claim: not every local reference resolves, or the library one does"
    );
}

fn node_binary() -> Result<PathBuf, String> {
    for var in ["ED12_NODE_BIN", "MOIRAI_E2E_INTERP_NODE_BIN"] {
        if let Some(raw) = std::env::var_os(var) {
            let path = PathBuf::from(&raw);
            return if path.is_file() {
                Ok(path)
            } else {
                Err(format!(
                    "{var} points at {}, which is not a file",
                    path.display()
                ))
            };
        }
    }
    Err("no model_node: set ED12_NODE_BIN".to_string())
}

/// One `model_node` on ports it binds itself, holding no descriptor and no peer.
struct Node {
    child: Child,
    http: u16,
    log: PathBuf,
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Node {
    fn spawn(binary: &Path, scratch: &Path) -> Result<Node, String> {
        let log = scratch.join("node.log");
        let ports = scratch.join("node.ports.json");
        let out = fs::File::create(&log).map_err(|e| e.to_string())?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        let child = Command::new(binary)
            .current_dir(scratch)
            .env("REPLICA_ID", "ed12")
            .env("LOG_ID", LOG_ID)
            .env("LISTEN_PORT", "0")
            .env("HTTP_PORT", "0")
            .env("PORTS_FILE", &ports)
            .env_remove("PEERS")
            .env_remove("PEERS_FILE")
            .env_remove("BOOTNODE_URL")
            .env_remove("DASHBOARD_URL")
            .env_remove("METAMODEL_PATH")
            .env_remove("METAMODEL_DIR")
            .env_remove("ADVERTISE_ADDR")
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .map_err(|e| format!("spawning {}: {e}", binary.display()))?;
        let mut node = Node {
            child,
            http: 0,
            log,
        };
        let deadline = Instant::now() + HEALTH_TIMEOUT;
        loop {
            if let Ok(text) = fs::read_to_string(&ports)
                && let Ok(value) = serde_json::from_str::<Value>(&text)
                && let Some(port) = value["http_port"].as_u64()
            {
                node.http = port as u16;
                break;
            }
            if let Ok(Some(status)) = node.child.try_wait() {
                return Err(format!("the node exited with {status}:\n{}", node.tail()));
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "the node never reported its ports:\n{}",
                    node.tail()
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        loop {
            if let Ok((200, body)) = node.request("GET", "/api/health", None)
                && body["replica_id"] == json!("ed12")
            {
                return Ok(node);
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "the node never answered /api/health:\n{}",
                    node.tail()
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn tail(&self) -> String {
        let text = fs::read_to_string(&self.log).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        lines[lines.len().saturating_sub(25)..].join("\n")
    }

    /// One HTTP/1.1 request over a fresh connection; the status and the body as JSON (a string
    /// when it is not JSON).
    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<(u16, Value), String> {
        let mut stream = TcpStream::connect(("127.0.0.1", self.http))
            .map_err(|e| format!("{method} {path}: {e}"))?;
        stream
            .set_read_timeout(Some(REQUEST_TIMEOUT))
            .map_err(|e| e.to_string())?;
        let body = body.unwrap_or("");
        let head = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream
            .write_all(head.as_bytes())
            .and_then(|()| stream.write_all(body.as_bytes()))
            .map_err(|e| format!("{method} {path}: {e}"))?;
        let mut raw = Vec::new();
        let mut buf = [0u8; 65536];
        let (head_end, length, chunked) = loop {
            let n = stream
                .read(&mut buf)
                .map_err(|e| format!("{method} {path}: {e}"))?;
            if n == 0 {
                return Err(format!(
                    "{method} {path}: connection closed before the headers"
                ));
            }
            raw.extend_from_slice(&buf[..n]);
            if let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok());
                let chunked = head.contains("transfer-encoding: chunked");
                break (end + 4, length, chunked);
            }
        };
        loop {
            let have = raw.len() - head_end;
            let done = match length {
                Some(len) => have >= len,
                None if chunked => raw[head_end..].ends_with(b"0\r\n\r\n"),
                None => false,
            };
            if done {
                break;
            }
            let n = stream
                .read(&mut buf)
                .map_err(|e| format!("{method} {path}: {e}"))?;
            if n == 0 {
                break;
            }
            raw.extend_from_slice(&buf[..n]);
        }
        let status_line = String::from_utf8_lossy(&raw[..head_end]).to_string();
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or_else(|| format!("{method} {path}: no status in {status_line}"))?;
        let mut payload = raw[head_end..].to_vec();
        if let Some(len) = length {
            payload.truncate(len);
        } else if chunked {
            payload = dechunk(&payload);
        }
        let text = String::from_utf8_lossy(&payload).to_string();
        Ok((
            status,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        ))
    }
}

fn dechunk(mut data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(end) = data.windows(2).position(|w| w == b"\r\n") {
        let size = usize::from_str_radix(
            String::from_utf8_lossy(&data[..end])
                .split(';')
                .next()
                .unwrap_or("")
                .trim(),
            16,
        )
        .unwrap_or(0);
        if size == 0 {
            break;
        }
        let start = end + 2;
        out.extend_from_slice(&data[start..(start + size).min(data.len())]);
        data = &data[(start + size + 2).min(data.len())..];
    }
    out
}
