//! EC.M1j outside gate (spec: cea-cdrt-knowledge/spec/items/EC.M1j.md): moirai-semantics, unchanged,
//! reads the descriptors the candidate `describe` writes for a package tree. `moirai_gate.sh` copies
//! this file into a scratch copy of the Moirai lane as `moirai-semantics/tests/`; it is never
//! committed there. ECM1J_DESCRIPTORS names a folder of `<name>.json` descriptors, each beside
//! `<name>.id`, the identity line describe printed.

use std::{collections::BTreeMap, fs, path::Path};

use moirai_semantics::{MergeRule, MetamodelSemantics, from_descriptor};
use serde_json::{Value, json};

fn read(dir: &Path) -> BTreeMap<String, (Value, Value)> {
    let mut all = BTreeMap::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            let d = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            let id = serde_json::from_slice(&fs::read(path.with_extension("id")).unwrap()).unwrap();
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            all.insert(name, (d, id));
        }
    }
    all
}

fn class_name(s: &MetamodelSemantics, slot: u16) -> &str {
    &s.classes[slot as usize].name
}

#[test]
fn moirai_semantics_reads_every_subpackage_descriptor() {
    let dir = std::env::var("ECM1J_DESCRIPTORS").expect("ECM1J_DESCRIPTORS names the folder");
    let all = read(Path::new(&dir));
    let mut failures = Vec::new();
    for (name, (d, id)) in &all {
        let s = match from_descriptor(d) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{name}: {e:?}"));
                continue;
            }
        };
        let roots: Vec<&str> = s.roots.iter().map(|r| class_name(&s, r.0)).collect();
        let ok = json!({"digest": s.digest, "nsURI": &*s.ns_uri}) == *id
            && json!(roots) == d["rootClasses"]
            && s.classes.iter().all(|c| {
                let supers: Vec<&str> = c.supers.iter().map(|x| class_name(&s, x.0)).collect();
                json!(supers) == d["classes"][&*c.name]["superTypes"]
            });
        if !ok {
            failures.push(format!(
                "{name}: identity, roots or supertypes differ from the JSON"
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");

    // The tree fixture: a subpackage root, supertypes both ways, targets into two depths.
    let s = from_descriptor(&all["tree"].0).unwrap();
    let class = |n: &str| s.classes.iter().find(|c| &*c.name == n).unwrap();
    let roots: Vec<&str> = s.roots.iter().map(|r| class_name(&s, r.0)).collect();
    assert_eq!(roots, ["Shop", "catalog::Inventory"]);
    assert_eq!(
        class_name(&s, class("Special").supers[0].0),
        "catalog::Item"
    );
    assert_eq!(class_name(&s, class("catalog::Item").supers[0].0), "Named");
    let target = |c: &str, f: &str| match &class(c)
        .declared
        .iter()
        .find(|x| &*x.name == f)
        .unwrap()
        .merge
    {
        MergeRule::Containment { target, .. } | MergeRule::Reference { target, .. } => {
            class_name(&s, target.0).to_owned()
        }
        other => panic!("{c}.{f}: {other:?}"),
    };
    assert_eq!(target("Shop", "items"), "catalog::Item");
    assert_eq!(target("Shop", "tags"), "catalog::detail::Tag");
    assert_eq!(target("catalog::Inventory", "stock"), "Stock");
    eprintln!("moirai gate: {} descriptors read", all.len());
}
