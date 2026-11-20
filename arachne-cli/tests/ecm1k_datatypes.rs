//! EC.M1k: `describe` never writes a user EDataType or a zero-literal EEnum as a class or a root
//! (spec: cea-cdrt-knowledge/spec/items/EC.M1k.md).
//!
//! A datatype is a value type, described by EMF's instance class (`instanceClassName`, else the
//! erasure of `instanceTypeName`): the eight primitives and `java.lang.String` as the built-in
//! Ecore type of that class, a wrapper (`java.lang.Integer`, ...) as its primitive, anything else as
//! `string` on a single-value register, as Ecore's `Object` now is too. A zero-literal enum is an
//! enum with no literal. A package that declares no EClass refuses, its first stderr line the
//! root-class line and a later one saying it declares no EClass; a map keyed by an enum refuses
//! with a line naming the feature.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

use serde_json::Value;

const NO_ECLASS: &str = "declares no EClass";
const CORPUS: &str =
    "/volatile/home/vp242103/datasets/modelset/unpacked/modelset/raw-data/repo-ecore-all/data";

fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn fixture(name: &str) -> PathBuf {
    tests_dir().join("fixtures/ecm1k").join(name)
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

/// Describe refuses `input` with nothing on stdout and no file written with `-o`; returns stderr.
fn refused(input: &Path) -> Vec<String> {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "ecm1k-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("metamodel.json");
    let with_o = describe(input, Some(&file));
    let out = describe(input, None);
    assert!(
        !out.status.success() && !with_o.status.success(),
        "describe should refuse {}",
        input.display()
    );
    assert!(
        out.stdout.is_empty() && with_o.stdout.is_empty(),
        "no descriptor on stdout for {}",
        input.display()
    );
    assert!(!file.exists(), "no file with -o for {}", input.display());
    err_lines(&out)
}

fn attributes(d: &Value, class: &str) -> Vec<Value> {
    d["classes"][class]["attributes"]
        .as_array()
        .unwrap_or_else(|| panic!("class {class} is written"))
        .clone()
}

#[test]
fn each_datatype_attribute_is_described_as_its_builtin_twin() {
    let d = described(&fixture("twins.ecore"));
    let (typed, builtin) = (attributes(&d, "Typed"), attributes(&d, "Builtin"));
    assert_eq!(typed.len(), 32, "Typed's attributes");
    assert_eq!(builtin.len(), 32, "Builtin's attributes");
    let mut differ = Vec::new();
    for (t, b) in typed.iter().zip(&builtin) {
        assert_eq!(
            t["name"], b["name"],
            "the twins are declared in the same order"
        );
        if t["name"] == "state" {
            continue;
        }
        if t != b {
            differ.push(format!("{}: {t} but its twin is {b}", t["name"]));
        }
    }
    assert!(
        differ.is_empty(),
        "{} of 31 twins differ:\n{}",
        differ.len(),
        differ.join("\n")
    );
}

#[test]
fn datatypes_are_neither_classes_nor_roots_and_an_empty_enum_is_an_enum() {
    let d = described(&fixture("twins.ecore"));
    let classes: Vec<&String> = d["classes"].as_object().unwrap().keys().collect();
    assert_eq!(classes, ["Builtin", "Typed"], "classes");
    assert_eq!(d["rootClasses"], serde_json::json!(["Builtin", "Typed"]));
    assert_eq!(
        d["enums"],
        serde_json::json!({"Empty": [], "Pair": ["A", "B"]})
    );
    let state = |class: &str| {
        attributes(&d, class)
            .into_iter()
            .find(|a| a["name"] == "state")
            .expect("state")
    };
    // `Typed.state` is `Builtin.state` with the zero-literal enum in place of the two-literal one.
    let want: Value = serde_json::from_str(
        &state("Builtin")
            .to_string()
            .replace("\"Pair\"", "\"Empty\""),
    )
    .unwrap();
    assert_eq!(state("Typed"), want);
    if let Err(e) = moirai_semantics::from_descriptor(&d) {
        panic!("moirai-semantics reads the twins: {e:?}");
    }
}

#[test]
fn a_package_with_no_eclass_refuses_with_that_cause() {
    for (file, package) in [("no_eclass.ecore", "typelib"), ("enum_only.ecore", "kinds")] {
        let lines = refused(&fixture(file));
        assert_eq!(
            lines.first().map(String::as_str),
            Some(
                format!(
                    "[ERROR arachne] Error: Failed to find a root class in `{package}` package"
                )
                .as_str()
            ),
            "{file}: the first line is today's root-class line: {lines:?}"
        );
        assert_eq!(
            lines[1..].iter().filter(|l| l.contains(NO_ECLASS)).count(),
            1,
            "{file}: one later line says `{NO_ECLASS}`: {lines:?}"
        );
    }
}

#[test]
fn an_enum_keyed_map_refuses_with_its_cause() {
    for file in ["enum_key.ecore", "empty_enum_key.ecore"] {
        let lines = refused(&fixture(file));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("Root.entries") && l.contains("enum") && l.contains("key")),
            "{file}: no line naming `Root.entries` and its enum key: {lines:?}"
        );
    }
}

fn ids(name: &str) -> Vec<String> {
    fs::read_to_string(tests_dir().join("ecm1k").join(name))
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
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
fn the_registered_corpus_files_move_as_registered() {
    let (no_eclass, no_root) = (ids("no-eclass.txt"), ids("no-root.txt"));
    let read = [ids("changed.txt"), ids("abstract-roots.txt")].concat();
    assert_eq!((no_eclass.len(), no_root.len(), read.len()), (34, 7, 145));
    let mut failures = Vec::new();
    for id in &no_eclass {
        let out = describe(&input(id), None);
        let lines = err_lines(&out);
        if out.status.success()
            || !out.stdout.is_empty()
            || !lines.iter().skip(1).any(|l| l.contains(NO_ECLASS))
        {
            failures.push(format!("not refused as declaring no EClass: {id}"));
        }
    }
    for id in &no_root {
        let out = describe(&input(id), None);
        let lines = err_lines(&out);
        let first = lines.first().map(String::as_str).unwrap_or("");
        if out.status.success()
            || !out.stdout.is_empty()
            || !first.starts_with("[ERROR arachne] Error: Failed to find a root class in `")
            || lines.iter().any(|l| l.contains(NO_ECLASS))
        {
            failures.push(format!("not refused for want of a root: {id}"));
        }
    }
    // moirai-semantics requires an nsURI, which the four EC.01 fixtures among them do not declare.
    for id in &read {
        let out = describe(&input(id), None);
        let ok = out.status.success()
            && serde_json::from_slice::<Value>(&out.stdout).is_ok_and(|d| {
                d["nsURI"].is_null() || moirai_semantics::from_descriptor(&d).is_ok()
            });
        if !ok {
            failures.push(format!(
                "not described or not read by moirai-semantics: {id}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of 186 registered files: {failures:#?}",
        failures.len()
    );
}
