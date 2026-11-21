//! EC.06 (`cea-cdrt-knowledge/spec/items/EC.06.md`): `describe` writes a `datatypes` section, one
//! entry per user EDataType, and takes a datatype's `eDataType` warning line out of the block when
//! nothing else of it is left out. Goldens in `fixtures/ec06/expected` are 9908485's run extended by
//! `tests/ec06/model.py` from the XML; `fixtures/ec06/base` holds 9908485's stdout and stderr. Census
//! N is `tests/ec06/census.py`; its pairs are read by `ec05_describe_carries`'s census table test.

use std::{fs, path::PathBuf, process::Command};

use serde_json::{Value, json};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ec06")
}

fn describe_in(dir: PathBuf, file: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_arachne"))
        .current_dir(dir)
        .env_remove("RUST_LOG")
        .arg("describe")
        .arg(file)
        .output()
        .expect("run arachne describe")
}

fn identity(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    text.lines()
        .rfind(|l| !l.is_empty())
        .unwrap_or("")
        .to_owned()
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
        (c, b) => Err(format!(
            "candidate {:?}, 9908485 {:?}",
            c.map(|_| "table").map_err(|e| e.to_string()),
            b.map(|_| "table").map_err(|e| e.to_string())
        )),
    }
}

fn dt(class_name: Option<&str>) -> Value {
    json!({"instanceClassName": class_name, "serializable": true})
}

#[test]
fn describe_writes_each_user_datatype_and_keeps_the_rest() {
    let exits = fs::read_to_string(dir().join("expected/exit.tsv")).unwrap();
    let rows: Vec<(&str, i32)> = exits
        .lines()
        .map(|l| {
            let (name, code) = l.split_once('\t').unwrap();
            (name, code.parse().unwrap())
        })
        .collect();
    assert_eq!(rows.len(), 5, "exit.tsv registers 5 fixtures");
    let mut failures = Vec::new();
    let mut out = std::collections::BTreeMap::new();
    for (name, exit) in &rows {
        let stem = name.replace('/', "_");
        let run = describe_in(dir(), &format!("{name}.ecore"));
        if run.status.code() != Some(*exit) {
            failures.push(format!(
                "{name}: exit {:?}, expected {exit}",
                run.status.code()
            ));
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
        if *exit != 0 {
            continue;
        }
        let cand: Value = serde_json::from_slice(&run.stdout).unwrap_or(Value::Null);
        let base: Value =
            serde_json::from_slice(&fs::read(dir().join(format!("base/{stem}.out"))).unwrap())
                .unwrap();
        if let Err(e) = same_table(&cand, &base) {
            failures.push(format!("{name}: moirai-semantics: {e}"));
        }
        let base_id = identity(&fs::read(dir().join(format!("base/{stem}.err"))).unwrap());
        let moved = identity(&run.stderr) != base_id;
        if moved != (*name != "none") {
            failures.push(format!("{name}: digest moved {moved}, from {base_id}"));
        }
        out.insert(
            name.to_string(),
            (cand, String::from_utf8_lossy(&run.stderr).into_owned()),
        );
    }

    // Note 44 W3: each datatype under its key, with what the file writes; nothing else moves.
    let checks: [(&str, Value, Value); 6] = [
        (
            "base datatypes",
            out["base"].0["datatypes"].clone(),
            json!({
                "Date": dt(Some("java.util.Date")), "Documented": dt(Some("java.lang.String")),
                "Generic": dt(None), "Opaque": dt(None), "Unused": dt(Some("int")),
                "sub::Code": dt(Some("java.lang.String"))
            }),
        ),
        (
            "base classes and roots",
            json!([
                out["base"].0["classes"]
                    .as_object()
                    .map(|c| c.keys().cloned().collect::<Vec<_>>()),
                out["base"].0["rootClasses"],
                out["base"].0.get("packages")
            ]),
            json!([["Root"], ["Root"], null]),
        ),
        (
            "cross/root datatypes",
            out["cross/root"].0["datatypes"].clone(),
            json!({
                "Stamp": dt(Some("java.util.Date")), "parts::Code": dt(Some("java.lang.String")),
                "parts::Money": dt(Some("java.math.BigDecimal")), "parts::Rate": dt(Some("double"))
            }),
        ),
        (
            "cross/dep datatypes",
            out["cross/dep"].0["datatypes"]
                .as_object()
                .map(|d| d.keys().cloned().collect::<Vec<_>>())
                .into(),
            json!(["Code", "Money", "Rate", "Spare"]),
        ),
        (
            "none",
            out["none"].0.get("datatypes").cloned().into(),
            Value::Null,
        ),
        (
            "base eDataType lines",
            json!(
                out["base"]
                    .1
                    .lines()
                    .filter(|l| l.contains("eDataType"))
                    .collect::<Vec<_>>()
            ),
            json!([
                "warning: dropped eDataType at Documented",
                "warning: dropped eDataType at Generic"
            ]),
        ),
    ];
    for (name, got, want) in checks {
        if got != want {
            failures.push(format!("{name}: read {got}, written {want}"));
        }
    }
    let dep_digest = serde_json::from_str::<Value>(&identity(out["cross/dep"].1.as_bytes()))
        .map(|v| v["digest"].clone())
        .unwrap_or(Value::Null);
    if out["cross/root"].0["dependsOn"][0]["digest"] != dep_digest {
        failures.push(format!(
            "cross/root: dependsOn {}, the dependency's digest {dep_digest}",
            out["cross/root"].0["dependsOn"]
        ));
    }

    // The claim of note 44 W3, on the file it names.
    let rsc = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../arachne-parser/rsc");
    let run = describe_in(rsc, "ExampleEcore.ecore");
    let d: Value = serde_json::from_slice(&run.stdout).unwrap_or(Value::Null);
    let got = json!([
        d["datatypes"],
        d["classes"].get("Address"),
        d["rootClasses"]
            .as_array()
            .map(|r| r.contains(&json!("Address")))
    ]);
    if got != json!([{"Address": dt(None)}, null, false]) {
        failures.push(format!(
            "ExampleEcore: [datatypes, classes.Address, Address a root] read {got}"
        ));
    }

    assert!(
        failures.is_empty(),
        "{} failures over {} fixtures:\n{}",
        failures.len(),
        rows.len() + 1,
        failures.join("\n")
    );
}
