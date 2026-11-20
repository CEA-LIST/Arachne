//! EC.M1j: `describe` describes the whole package tree of an Ecore file as one descriptor
//! (spec: cea-cdrt-knowledge/spec/items/EC.M1j.md).
//!
//! A subpackage classifier is keyed by its root-relative `::` path (`catalog::detail::Tag`) wherever
//! a name is written; the root package's names stay bare; `packages` maps every subpackage path to
//! its `nsURI` and `nsPrefix`, written only when a subpackage classifier is; roots and the
//! no-EClass rule run over the whole tree. The goldens (`fixtures/ecm1j/*.metamodel.json`) are the
//! flat twin's descriptor (`ecm1j/flat.py`: every subpackage classifier moved into the root package
//! as `a__b__Name`, described by 18ccade, renamed back), so they come from the single-package
//! describe, not from the code under test.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

use serde_json::{Value, json};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ecm1j")
        .join(rel)
}

fn describe(input: &Path, out: Option<&Path>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_arachne"));
    cmd.arg("describe").arg(input);
    if let Some(out) = out {
        cmd.arg("-o").arg(out.join("metamodel.json"));
    }
    cmd.output().expect("run arachne describe")
}

fn scratch(label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "ecm1j-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
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

/// Describe reads `rel`; returns the descriptor and the identity line.
fn described(rel: &str) -> (Value, Value) {
    let out = describe(&fixture(rel), None);
    assert!(
        out.status.success(),
        "describe should read {rel}: {:?}",
        err_lines(&out)
    );
    let d: Value = serde_json::from_slice(&out.stdout).expect("a JSON descriptor");
    let id: Value = serde_json::from_str(err_lines(&out).last().expect("an identity line"))
        .expect("the identity line is JSON");
    (d, id)
}

fn golden(rel: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(fixture(rel)).unwrap()).unwrap()
}

/// The descriptor equals its golden, its identity line names `ns_uri` and `digest`, `-o` writes the
/// same descriptor, and moirai-semantics reads it with the golden's roots.
fn as_golden(rel: &str, golden_rel: &str, ns_uri: &str, digest: &str) {
    let (d, id) = described(rel);
    assert_eq!(d, golden(golden_rel), "{rel}: the descriptor");
    assert_eq!(
        id,
        json!({"digest": digest, "nsURI": ns_uri}),
        "{rel}: identity"
    );
    let dir = scratch("o");
    assert!(describe(&fixture(rel), Some(&dir)).status.success());
    let written: Value =
        serde_json::from_slice(&fs::read(dir.join("metamodel.json")).unwrap()).unwrap();
    assert_eq!(written, d, "{rel}: -o");
    let s = moirai_semantics::from_descriptor(&d)
        .unwrap_or_else(|e| panic!("moirai-semantics reads {rel}: {e:?}"));
    let roots: Vec<&str> = s
        .roots
        .iter()
        .map(|r| &*s.classes[r.0 as usize].name)
        .collect();
    assert_eq!(json!(roots), d["rootClasses"], "{rel}: moirai roots");
}

#[test]
fn the_tree_is_one_descriptor_with_root_relative_names() {
    // Every name site: a subpackage classifier as key, root, supertype (both ways), containment and
    // reference target, merge target, attribute enum and enum leaf, subsets class; a root-package
    // class contained by a subpackage class is no root; two `Note`s; an unused subpackage enum; a
    // subpackage datatype as an attribute type; an empty subpackage in `packages`.
    as_golden(
        "tree.ecore",
        "tree.metamodel.json",
        "urn:shop",
        "dfd6ecbf4d21504e6aebb5ebc9061c9880fa98c88e7bf762aecd8de46418b33d",
    );
    let d = golden("tree.metamodel.json");
    assert_eq!(d["rootClasses"], json!(["Shop", "catalog::Inventory"]));
    assert_eq!(
        d["packages"],
        json!({
            "catalog": {"nsURI": "urn:shop/catalog", "nsPrefix": "cat"},
            "catalog::detail": {"nsURI": "urn:shop/catalog/detail", "nsPrefix": "det"},
            "misc": {"nsURI": "urn:shop/misc", "nsPrefix": "misc"}
        })
    );
}

#[test]
fn a_root_package_without_an_eclass_takes_its_roots_from_the_tree() {
    as_golden(
        "classless_root.ecore",
        "classless_root.metamodel.json",
        "urn:lib",
        "07e682407f41bd669bcace1dfd9bc2fd2b73e291fc7d33d3d65a30a76fce4035",
    );
    assert_eq!(
        golden("classless_root.metamodel.json")["rootClasses"],
        json!(["model::Model"])
    );
}

#[test]
fn a_tree_with_no_eclass_anywhere_still_refuses_with_that_cause() {
    let out = describe(&fixture("no_eclass_tree.ecore"), None);
    let lines = err_lines(&out);
    assert!(!out.status.success() && out.stdout.is_empty(), "{lines:?}");
    assert_eq!(
        lines[0],
        "[ERROR arachne] Error: Failed to find a root class in `types` package"
    );
    assert!(
        lines[1..].iter().any(|l| l.contains("declares no EClass")),
        "{lines:?}"
    );
}

/// Refused: nothing on stdout, nothing written with `-o`, and one stderr line, not a parser position
/// line (`at ...`), says `subpackage` (any case) and holds every token.
fn refused_with(rel: &str, tokens: &[&str]) {
    let out = describe(&fixture(rel), None);
    let lines = err_lines(&out);
    assert!(
        !out.status.success() && out.stdout.is_empty(),
        "{rel} should be refused: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| !l.trim_start().starts_with("at `")
            && l.to_lowercase().contains("subpackage")
            && tokens.iter().all(|t| l.contains(t))),
        "{rel}: no line saying `subpackage` with {tokens:?}: {lines:?}"
    );
    let dir = scratch("refused");
    assert!(!describe(&fixture(rel), Some(&dir)).status.success());
    assert_eq!(
        fs::read_dir(&dir).unwrap().count(),
        0,
        "{rel}: -o wrote a file"
    );
}

#[test]
fn a_subpackage_without_an_nsuri_refuses() {
    refused_with("sub_no_nsuri.ecore", &["nsURI", "nouri"]);
}

#[test]
fn a_subpackage_sharing_an_nsuri_refuses() {
    refused_with("sub_root_nsuri.ecore", &["nsURI", "urn:dup"]);
    refused_with("sub_same_nsuri.ecore", &["nsURI", "urn:same"]);
}

#[test]
fn a_subpackage_named_ecore_refuses() {
    refused_with("sub_ecore.ecore", &["ecore"]);
}

#[test]
fn a_subpackage_path_colliding_with_an_import_refuses() {
    // `dep.ecore#//D` is imported as `dep::D`; the file's own subpackage `dep` holds `Y`, then `D`.
    refused_with("import/prefix.ecore", &["dep"]);
    refused_with("import/key.ecore", &["dep"]);
}

#[test]
fn a_dependency_is_described_whole_and_its_subpackages_are_never_imported() {
    as_golden(
        "import/deptree.ecore",
        "import/deptree.metamodel.json",
        "urn:deptree",
        "01ca7278bdce109c4122440459d21963a8124b98f992b323a93a9ee16661cafd",
    );
    let (d, _) = described("import/clean.ecore");
    let keys: Vec<&String> = d["classes"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["A", "deptree::U"]);
    assert!(
        d.get("packages").is_none(),
        "clean.ecore writes no subpackage classifier"
    );
    assert_eq!(
        d["dependsOn"],
        json!([{"nsURI": "urn:deptree", "digest": "01ca7278bdce109c4122440459d21963a8124b98f992b323a93a9ee16661cafd"}])
    );
    // A reference into the dependency's subpackage, directly or through an imported class.
    for rel in ["import/into.ecore", "import/reach.ecore"] {
        let out = describe(&fixture(rel), None);
        assert!(
            !out.status.success() && out.stdout.is_empty(),
            "{rel} should be refused: {:?}",
            err_lines(&out)
        );
    }
}
