//! EC.M1l-b outside gate (spec: cea-cdrt-knowledge/spec/items/EC.M1l-b.md): moirai-semantics at
//! b1e254c reads the descriptors the candidate `describe` writes for the five enum-keyed fixtures and
//! resolves `Root.entries`' key to the enum the descriptor names. `moirai_gate.sh` copies this file
//! into a scratch copy of the Moirai lane as `moirai-semantics/tests/`; it is never committed there.
//! ECM1LB_DESCRIPTORS names a folder of `<stem>.json`; ECM1LB_IDENTITY the registered identity.tsv.

use std::{fs, path::Path};

use moirai_semantics::{KeyKind, MergeRule, Shape, from_descriptor};
use serde_json::Value;

#[test]
fn moirai_semantics_reads_every_enum_keyed_descriptor() {
    let dir = std::env::var("ECM1LB_DESCRIPTORS").expect("ECM1LB_DESCRIPTORS names the folder");
    let tsv = std::env::var("ECM1LB_IDENTITY").expect("ECM1LB_IDENTITY names identity.tsv");
    let table = fs::read_to_string(tsv).unwrap();
    let mut failures = Vec::new();
    for line in table.lines() {
        let [id, name, digest, _] = line.split('\t').collect::<Vec<_>>()[..] else {
            panic!("a malformed row {line}")
        };
        let stem = Path::new(id).file_stem().unwrap().to_string_lossy();
        let Ok(bytes) = fs::read(Path::new(&dir).join(format!("{stem}.json"))) else {
            failures.push(format!("{id}: no descriptor"));
            continue;
        };
        let d: Value = serde_json::from_slice(&bytes).unwrap();
        let s = match from_descriptor(&d) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{id}: {e:?}"));
                continue;
            }
        };
        let root = s.classes.iter().find(|c| &*c.name == "Root").unwrap();
        let rule = &root
            .declared
            .iter()
            .find(|f| &*f.name == "entries")
            .unwrap()
            .merge;
        let key = match rule {
            MergeRule::Attribute {
                shape: Shape::Keyed { key },
                ..
            }
            | MergeRule::Containment {
                shape: Shape::Keyed { key },
                ..
            } => key,
            other => {
                failures.push(format!("{id}: `Root.entries` reads as {other:?}"));
                continue;
            }
        };
        match key {
            KeyKind::Enum { class }
                if s.enums
                    .get(class.0 as usize)
                    .is_some_and(|e| &*e.name == name) => {}
            other => failures.push(format!("{id}: key {other:?}, not the enum `{name}`")),
        }
        if s.digest != digest {
            failures.push(format!("{id}: digest {}, registered {digest}", s.digest));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
