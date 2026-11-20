//! EC.M1m: when root selection's two passes find no root, `describe` takes as roots every
//! containment component of concrete classes that no class outside it contains (spec:
//! cea-cdrt-knowledge/spec/items/EC.M1m.md).
//!
//! A component is a strongly connected component of the containment graph among concrete classes
//! (s contains t when a class s inherits declares a containment typed by a class t inherits, s != t).
//! A package with no concrete class still refuses: its first stderr line is the root-class line
//! and one later line says it has no concrete EClass.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::{Value, json};

const CORPUS: &str =
    "/volatile/home/vp242103/datasets/modelset/unpacked/modelset/raw-data/repo-ecore-all/data";

fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn fixture(name: &str) -> PathBuf {
    tests_dir().join("fixtures/ecm1m").join(name)
}

fn describe(input: &Path, out: Option<&Path>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_arachne"));
    cmd.arg("describe").arg(input);
    if let Some(out) = out {
        cmd.arg("-o").arg(out);
    }
    cmd.output().expect("run arachne describe")
}

/// Stderr lines with ANSI colour codes removed.
fn err_lines(out: &Output) -> Vec<String> {
    let text = String::from_utf8_lossy(&out.stderr);
    let mut plain = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c == 'm' {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    plain.lines().map(str::to_owned).collect()
}

fn described(input: &Path) -> Value {
    let out = describe(input, None);
    assert!(
        out.status.success(),
        "describe should read {}: {:?}",
        input.display(),
        err_lines(&out)
    );
    serde_json::from_slice(&out.stdout).expect("a JSON descriptor")
}

fn keys(d: &Value) -> Vec<String> {
    d["classes"].as_object().unwrap().keys().cloned().collect()
}

/// The refusal of a package with no concrete class, or why `input` is not one.
fn no_concrete_refusal(input: &Path, package: &str) -> Result<(), String> {
    let file = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ecm1m-{}-{package}.json", std::process::id()));
    let _ = fs::remove_file(&file);
    let with_o = describe(input, Some(&file));
    let out = describe(input, None);
    let lines = err_lines(&out);
    let first =
        format!("[ERROR arachne] Error: Failed to find a root class in `{package}` package");
    let causes = lines
        .iter()
        .skip(1)
        .filter(|l| l.contains("no concrete") && l.contains("EClass"))
        .count();
    if out.status.success() || with_o.status.success() || !out.stdout.is_empty() || file.exists() {
        return Err(format!("not refused: {lines:?}"));
    }
    if lines.first() != Some(&first) || causes != 1 {
        return Err(format!(
            "first line not `{first}`, or not one later line with `no concrete` and `EClass`: {lines:?}"
        ));
    }
    if lines.iter().any(|l| l.contains("declares no EClass")) {
        return Err(format!(
            "the no-EClass cause on a package with EClasses: {lines:?}"
        ));
    }
    Ok(())
}

#[test]
fn components_no_outside_class_contains_are_the_roots() {
    // {A, B} and {X, Y} contain each other; {C, D} is contained from B, so it is no root.
    let d = described(&fixture("cycles.ecore"));
    assert_eq!(d["rootClasses"], json!(["A", "B", "X", "Y"]));
    assert_eq!(keys(&d), ["A", "B", "C", "D", "X", "Y"]);
}

#[test]
fn a_root_of_the_first_two_passes_keeps_the_third_from_running() {
    // Pass 1 finds R; the {A, B} cycle beside it is not a root (EC.M1n's question, not this one).
    assert_eq!(
        described(&fixture("beside.ecore"))["rootClasses"],
        json!(["R"])
    );
    // Pass 2 finds the abstract Node; the {Group, Leaf} component it contains is not a root.
    assert_eq!(
        described(&fixture("composite.ecore"))["rootClasses"],
        json!(["Node"])
    );
}

#[test]
fn a_package_with_no_concrete_class_refuses_with_that_cause() {
    no_concrete_refusal(&fixture("abstract_only.ecore"), "shapes").unwrap();
}

fn input(id: &str) -> PathBuf {
    let path = match id.split_once('/') {
        Some(("modelset", rel)) => Path::new(CORPUS).join(rel),
        Some(("fixture", rel)) => tests_dir().join("fixtures/ec01").join(rel),
        _ => panic!("an unknown id {id}"),
    };
    assert!(path.is_file(), "{id} must be at {}", path.display());
    path
}

#[test]
fn the_registered_corpus_inputs_take_their_registered_roots() {
    let registered = fs::read_to_string(tests_dir().join("ecm1m/roots.tsv")).unwrap();
    let roots: Vec<(&str, Value)> = registered
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (id, r) = l.split_once('\t').unwrap();
            (id, serde_json::from_str(r).unwrap())
        })
        .collect();
    let none = fs::read_to_string(tests_dir().join("ecm1m/no-concrete.txt")).unwrap();
    let none: Vec<&str> = none.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!((roots.len(), none.len()), (37, 1));
    let mut failures = Vec::new();
    for (id, want) in &roots {
        let out = describe(&input(id), None);
        let got = out
            .status
            .success()
            .then(|| serde_json::from_slice::<Value>(&out.stdout).ok())
            .flatten()
            .map(|d| d["rootClasses"].clone());
        if got.as_ref() != Some(want) {
            failures.push(format!("{id}: roots {got:?}, registered {want}"));
        }
    }
    for id in &none {
        if let Err(e) = no_concrete_refusal(&input(id), "orgreliablesourcecuttlefishcoremodel") {
            failures.push(format!("{id}: {e}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of 38 registered inputs not as registered:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
