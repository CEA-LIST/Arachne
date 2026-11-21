//! EC.M1n: when root selection's first pass finds roots, `describe` also takes as roots every
//! containment component of concrete classes that no class outside it contains, so the classes of
//! a component beside those roots are listed (spec: cea-cdrt-knowledge/spec/items/EC.M1n.md).
//!
//! A component is a strongly connected component of the containment graph among concrete classes
//! (s contains t when a class s inherits declares a containment typed by a class t inherits, s != t).
//! When the roots are the second pass's abstract classes and such a component's classes would not
//! be listed, describe refuses: its first stderr line is the root-class line and one later line
//! names the component's classes.

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
    tests_dir().join("fixtures").join(name)
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

/// `(rootClasses, class keys)` of a described input, or why it was not described.
fn roots_and_classes(input: &Path) -> Result<(Value, Vec<String>), String> {
    let out = describe(input, None);
    if !out.status.success() {
        return Err(format!("refused: {:?}", err_lines(&out)));
    }
    let d: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let mut keys: Vec<String> = d["classes"].as_object().unwrap().keys().cloned().collect();
    keys.sort();
    Ok((d["rootClasses"].clone(), keys))
}

#[test]
fn components_beside_a_first_pass_root_are_roots_and_listed() {
    // R contains Item; A and B contain each other, and nothing contains them.
    assert_eq!(
        roots_and_classes(&fixture("ecm1m/beside.ecore")),
        Ok((
            json!(["A", "B", "R"]),
            ["A", "B", "Item", "R"].map(String::from).to_vec()
        ))
    );
    // As beside, and B contains C, which contains D and D C: {C, D} is entered from B, no root.
    assert_eq!(
        roots_and_classes(&fixture("ecm1n/chain.ecore")),
        Ok((
            json!(["A", "B", "R"]),
            ["A", "B", "C", "D", "Item", "R"].map(String::from).to_vec()
        ))
    );
    // As beside, and Item references A: the classes were listed already, the roots move.
    assert_eq!(
        roots_and_classes(&fixture("ecm1n/referenced.ecore")),
        Ok((
            json!(["A", "B", "R"]),
            ["A", "B", "Item", "R"].map(String::from).to_vec()
        ))
    );
}

#[test]
fn a_component_the_abstract_roots_do_not_list_refuses_with_its_classes() {
    // Pass 2 finds the abstract Node (Leaf and Group contain each other through it); A and B
    // contain each other and no root lists them.
    let input = fixture("ecm1n/abstract_beside.ecore");
    let file = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ecm1n-{}-abstractbeside.json", std::process::id()));
    let _ = fs::remove_file(&file);
    let with_o = describe(&input, Some(&file));
    let out = describe(&input, None);
    let lines = err_lines(&out);
    assert!(
        !out.status.success()
            && !with_o.status.success()
            && out.stdout.is_empty()
            && !file.exists(),
        "describe must refuse: {lines:?}"
    );
    assert_eq!(
        lines.first().map(String::as_str),
        Some("[ERROR arachne] Error: Failed to find a root class in `abstractbeside` package"),
        "{lines:?}"
    );
    let causes = lines
        .iter()
        .skip(1)
        .filter(|l| l.contains("`A`") && l.contains("`B`"))
        .count();
    assert_eq!(causes, 1, "one later line names `A` and `B`: {lines:?}");
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("no concrete") || l.contains("declares no EClass")),
        "{lines:?}"
    );
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
fn the_registered_corpus_inputs_take_their_registered_roots_and_classes() {
    let registered = fs::read_to_string(tests_dir().join("ecm1n/moved.tsv")).unwrap();
    let rows: Vec<(&str, Value, Vec<String>)> = registered
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let mut f = l.split('\t');
            let (id, roots, classes) = (f.next().unwrap(), f.next().unwrap(), f.next().unwrap());
            (
                id,
                serde_json::from_str(roots).unwrap(),
                serde_json::from_str(classes).unwrap(),
            )
        })
        .collect();
    assert_eq!(rows.len(), MOVED);
    let failures: Vec<String> = rows
        .iter()
        .filter_map(|(id, roots, classes)| {
            let got = roots_and_classes(&input(id));
            (got.as_ref() != Ok(&(roots.clone(), classes.clone())))
                .then(|| format!("{id}: got {got:?}"))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {MOVED} registered inputs not as registered:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The registered inputs (moved.tsv): every described input of 643ff52 whose roots move.
const MOVED: usize = 62;
