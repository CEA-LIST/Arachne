//! EC.05 (`cea-cdrt-knowledge/spec/items/EC.05.md`): `describe` carries the facts EC.04 only warned
//! about. Goldens in `fixtures/ec05/expected` are 40f4043's run extended by `tests/ec05/model.py`
//! from the XML; `fixtures/ec05/base` holds 40f4043's stdout. The census table test reads the pairs
//! census N writes (`EC05_PAIRS=<out-dir>/pairs`, run with `--ignored`).

use std::{fs, path::PathBuf, process::Command};

use serde_json::{Value, json};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ec05")
}

fn describe(name: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_arachne"))
        .current_dir(dir())
        .env_remove("RUST_LOG")
        .arg("describe")
        .arg(format!("{name}.ecore"))
        .output()
        .expect("run arachne describe")
}

fn digest(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let last = text.lines().rfind(|l| !l.is_empty()).unwrap_or("");
    serde_json::from_str::<Value>(last)
        .map_or_else(|_| last.to_owned(), |v| v["digest"].to_string())
}

/// `from_descriptor` of `cand` and of `base` agree, the digest aside (it moves by design).
fn same_table(cand: &Value, base: &Value) -> Result<(), String> {
    match (
        moirai_semantics::from_descriptor(cand),
        moirai_semantics::from_descriptor(base),
    ) {
        (Ok(mut c), Ok(b)) => {
            c.digest = b.digest.clone();
            if c == b {
                Ok(())
            } else {
                Err("tables differ".into())
            }
        }
        (Err(c), Err(b)) if c.to_string() == b.to_string() => Ok(()),
        (c, b) => Err(format!(
            "candidate {:?}, 40f4043 {:?}",
            c.map(|_| "table").map_err(|e| e.to_string()),
            b.map(|_| "table").map_err(|e| e.to_string())
        )),
    }
}

fn feature<'a>(d: &'a Value, class: &str, name: &str) -> &'a Value {
    ["attributes", "containments", "references"]
        .iter()
        .flat_map(|p| d["classes"][class][*p].as_array().into_iter().flatten())
        .find(|f| f["name"] == name)
        .unwrap_or(&Value::Null)
}

#[test]
fn describe_carries_each_fact_and_moves_the_digest() {
    let exits = fs::read_to_string(dir().join("expected/exit.tsv")).unwrap();
    let names: Vec<&str> = exits
        .lines()
        .map(|l| l.split('\t').next().unwrap())
        .collect();
    assert_eq!(names.len(), 16, "exit.tsv registers 16 fixtures");
    let mut failures = Vec::new();
    let mut out = std::collections::BTreeMap::new();
    for name in &names {
        let stem = name.replace('/', "_");
        let run = describe(name);
        if run.status.code() != Some(0) {
            failures.push(format!("{name}: exit {:?}", run.status.code()));
            continue;
        }
        if run.stdout != fs::read(dir().join(format!("expected/{stem}.out"))).unwrap() {
            failures.push(format!("{name}: stdout differs from the golden"));
        }
        let want = fs::read(dir().join(format!("expected/{stem}.err"))).unwrap();
        if run.stderr != want {
            failures.push(format!(
                "{name}: stderr\n{}expected\n{}",
                String::from_utf8_lossy(&run.stderr),
                String::from_utf8_lossy(&want)
            ));
        }
        let cand: Value = serde_json::from_slice(&run.stdout).unwrap_or(Value::Null);
        let base: Value =
            serde_json::from_slice(&fs::read(dir().join(format!("base/{stem}.out"))).unwrap())
                .unwrap();
        if let Err(e) = same_table(&cand, &base) {
            failures.push(format!("{name}: moirai-semantics: {e}"));
        }
        out.insert(name.to_string(), (cand, digest(&run.stderr)));
    }

    // Note 44 W2: one fact apart, a different digest; each fact read where it was written.
    let base = &out["base"];
    let checks: [(&str, Value, Value); 12] = [
        (
            "default",
            feature(&out["default"].0, "Timed", "timeout")["default"].clone(),
            json!("30"),
        ),
        (
            "changeable",
            feature(&out["changeable"].0, "Named", "name")["changeable"].clone(),
            json!(false),
        ),
        (
            "unsettable",
            feature(&out["unsettable"].0, "Timed", "timeout")["unsettable"].clone(),
            json!(true),
        ),
        (
            "e_keys",
            feature(&out["e_keys"].0, "Node", "children")["eKeys"].clone(),
            json!(["#//Named/name"]),
        ),
        (
            "upper_five",
            feature(&out["upper_five"].0, "Node", "tags")["upperBound"].clone(),
            json!(5),
        ),
        (
            "lower_two",
            feature(&out["lower_two"].0, "Node", "tags")["lowerBound"].clone(),
            json!(2),
        ),
        (
            "literal_value",
            out["literal_value"].0["enumLiterals"]["Status"][1].clone(),
            json!({"name": "SUCCESS", "value": 7, "literal": null}),
        ),
        (
            "literal_literal",
            out["literal_literal"].0["enumLiterals"]["Status"][2].clone(),
            json!({"name": "FAILURE", "value": 2, "literal": "failed"}),
        ),
        (
            "super_order",
            out["super_order"].0["classes"]["Node"]["superTypeOrder"].clone(),
            json!(["Timed", "Named"]),
        ),
        (
            "interface",
            out["interface"].0["classes"]["Named"]["interface"].clone(),
            json!(true),
        ),
        (
            "ns_prefix",
            out["ns_prefix"].0["nsPrefix"].clone(),
            json!("behavior"),
        ),
        (
            "literal",
            out["literal"].0["enumLiterals"]["Kind"].clone(),
            json!([{"name": "A", "value": 1, "literal": "a"}]),
        ),
    ];
    for (name, got, want) in checks {
        if got != want {
            failures.push(format!("{name}: read {got}, written {want}"));
        }
        if name != "literal" && out[name].1 == base.1 {
            failures.push(format!("{name}: same digest as base, {}", base.1));
        }
    }
    let base_super = &base.0["classes"]["Node"];
    if base_super["superTypes"] != json!(["Named", "Timed"])
        || out["super_order"].0["classes"]["Node"]["superTypes"] != json!(["Named", "Timed"])
    {
        failures.push("superTypes is no longer the sorted list".into());
    }
    if out["default_value_attr"].1 != base.1 {
        failures.push("default_value_attr: a `defaultValue` attribute moved the digest".into());
    }
    assert!(
        failures.is_empty(),
        "{} failures over {} fixtures:\n{}",
        failures.len(),
        names.len(),
        failures.join("\n")
    );
}

/// Every census pair census N wrote: 40f4043's descriptor and the candidate's read to one table.
#[test]
#[ignore = "reads census N's pairs: EC05_PAIRS=<out-dir>/pairs"]
fn census_descriptors_read_to_the_same_table() {
    let pairs =
        PathBuf::from(std::env::var("EC05_PAIRS").expect("EC05_PAIRS names census N's pairs"));
    let mut names: Vec<_> = fs::read_dir(&pairs)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".base.json"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no pair in {}", pairs.display());
    let read =
        |n: &str| -> Value { serde_json::from_slice(&fs::read(pairs.join(n)).unwrap()).unwrap() };
    let (mut tables, mut refused, mut failures) = (0, 0, Vec::new());
    for name in &names {
        let base = read(name);
        let cand = read(&name.replace(".base.", ".cand."));
        match same_table(&cand, &base) {
            Ok(()) if moirai_semantics::from_descriptor(&base).is_ok() => tables += 1,
            Ok(()) => refused += 1,
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    println!(
        "EC05 M: {} pairs, {tables} equal tables, {refused} refused alike, {} differ",
        names.len(),
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "{} pairs differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
