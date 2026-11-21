//! EC.M1l-b: `describe` writes an enum-keyed map's key enum by its descriptor name,
//! `{"kind": "enum", "class": "<name>"}`, instead of refusing (spec:
//! cea-cdrt-knowledge/spec/items/EC.M1l-b.md).
//!
//! The five registered fixtures (`fixtures/ecm1l_b/expected/identity.tsv`) are the inputs 060d88f
//! refuses for an enum-keyed map: an attribute value, a zero-literal key enum, a containment value,
//! a subpackage key enum with an optional enum value, and a key enum from another file. Each must
//! be described exactly as its golden says, the golden built by `ecm1l_b/twins.py` from two twins
//! 060d88f describes, and print the registered identity line.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::{Value, json};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The `merge` of `Root.entries`, for the failure message.
fn entries_merge(d: &Value) -> Value {
    d["classes"]["Root"]["containments"]
        .as_array()
        .and_then(|cs| cs.iter().find(|c| c["name"] == "entries"))
        .map(|c| c["merge"].clone())
        .unwrap_or(Value::Null)
}

#[test]
fn the_enum_keyed_maps_060d88f_refuses_are_described_with_the_key_enum_by_name() {
    let dir = fixtures().join("ecm1l_b/expected");
    let table = fs::read_to_string(dir.join("identity.tsv")).unwrap();
    let rows: Vec<Vec<&str>> = table.lines().map(|l| l.split('\t').collect()).collect();
    assert_eq!(rows.len(), 5, "identity.tsv registers five inputs");
    let mut failures = Vec::new();
    for row in &rows {
        let [id, name, digest, ns_uri] = row[..] else {
            panic!("a malformed identity.tsv row {row:?}")
        };
        let stem = Path::new(id).file_stem().unwrap().to_string_lossy();
        let golden: Value =
            serde_json::from_slice(&fs::read(dir.join(format!("{stem}.json"))).unwrap()).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_arachne"))
            .arg("describe")
            .arg(fixtures().join(id))
            .output()
            .expect("run arachne describe");
        let stderr = String::from_utf8_lossy(&out.stderr);
        if !out.status.success() {
            failures.push(format!("{id}: not described: {}", stderr.trim()));
            continue;
        }
        let Ok(got) = serde_json::from_slice::<Value>(&out.stdout) else {
            failures.push(format!("{id}: stdout is not JSON"));
            continue;
        };
        if got != golden {
            failures.push(format!(
                "{id}: descriptor differs from its golden; `Root.entries` merge {} (expected {}, key enum `{name}`)",
                entries_merge(&got),
                entries_merge(&golden)
            ));
        }
        let identity = stderr
            .lines()
            .last()
            .and_then(|l| serde_json::from_str::<Value>(l.trim()).ok());
        if identity != Some(json!({"digest": digest, "nsURI": ns_uri})) {
            failures.push(format!(
                "{id}: identity line {identity:?}, expected digest {digest}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of 5 registered inputs: {failures:#?}",
        failures.len()
    );
}
