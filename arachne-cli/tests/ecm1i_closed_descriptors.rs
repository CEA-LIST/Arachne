//! EC.M1i: `describe` prints only closed descriptors (spec: cea-cdrt-knowledge/spec/items/EC.M1i.md).
//!
//! Closed: every supertype, reference or containment target, attribute enum and merge-level name a
//! written class names is a class or enum the descriptor writes, by class identity. A class from a
//! subpackage (describe writes the document-root package only) is refused with one line that says
//! `descriptor not closed` and names the class and the missing name, nothing on stdout and no file
//! with `-o`; `ecore::EObject` is listed when a written name refers to it; a plain reference target
//! no root reaches is written. When the resource loader refuses, the first stderr line stays the
//! parser's and the loader's cause follows on one `caused by: ` line.
//!
//! EC.M1j (spec: cea-cdrt-knowledge/spec/items/EC.M1j.md): describe now writes the whole package tree,
//! so the subpackage fixtures and the registered `refused.txt` inputs below are described, closed,
//! with the subpackage classes keyed by their `::` path.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::{Value, json};

const CORPUS: &str =
    "/volatile/home/vp242103/datasets/modelset/unpacked/modelset/raw-data/repo-ecore-all/data";

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
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

/// Every name the descriptor writes and does not define.
fn undefined(d: &Value) -> Vec<String> {
    let classes = d["classes"].as_object().expect("classes");
    let enums = d["enums"].as_object().expect("enums");
    let mut bad = Vec::new();
    for (name, class) in classes {
        for sup in class["superTypes"].as_array().unwrap() {
            if !classes.contains_key(sup.as_str().unwrap()) {
                bad.push(format!("{name}.superTypes {sup}"));
            }
        }
        for key in ["containments", "references"] {
            for f in class[key].as_array().unwrap() {
                for n in [&f["target"], &f["merge"]["target"]] {
                    if let Some(n) = n.as_str().filter(|n| !classes.contains_key(*n)) {
                        bad.push(format!("{name}.{} -> {n}", f["name"]));
                    }
                }
            }
        }
        for a in class["attributes"].as_array().unwrap() {
            if let Some(e) = a["enum"].as_str().filter(|e| !enums.contains_key(*e)) {
                bad.push(format!("{name}.{} enum {e}", a["name"]));
            }
            if let Some(c) = a["merge"]["leaf"]["class"]
                .as_str()
                .filter(|c| !classes.contains_key(*c) && !enums.contains_key(*c))
            {
                bad.push(format!("{name}.{} leaf {c}", a["name"]));
            }
        }
    }
    bad
}

/// Describe writes a closed descriptor `moirai-semantics` reads; returns it.
fn closed(input: &Path) -> Value {
    let out = describe(input, None);
    assert!(
        out.status.success(),
        "describe should read {}: {:?}",
        input.display(),
        err_lines(&out)
    );
    let d: Value = serde_json::from_slice(&out.stdout).expect("a JSON descriptor");
    assert_eq!(undefined(&d), Vec::<String>::new(), "{}", input.display());
    moirai_semantics::from_descriptor(&d)
        .unwrap_or_else(|e| panic!("moirai-semantics reads {}: {e:?}", input.display()));
    d
}

#[test]
fn a_supertype_from_a_subpackage_is_written_not_dropped() {
    let d = closed(&fixture("ecm1i/sub_supertype.ecore"));
    assert_eq!(
        d["classes"]["Term"]["superTypes"],
        json!(["expressions::Expression"])
    );
}

#[test]
fn a_containment_target_from_a_subpackage_is_written() {
    let d = closed(&fixture("ecm1i/sub_containment.ecore"));
    assert_eq!(
        d["classes"]["System"]["containments"][0]["target"],
        "nodes::Node"
    );
}

#[test]
fn an_enum_from_a_subpackage_is_written() {
    let d = closed(&fixture("ecm1i/sub_enum.ecore"));
    assert_eq!(d["enums"], json!({"kinds::State": ["OPEN", "DONE"]}));
    assert_eq!(
        d["classes"]["Task"]["attributes"][0]["enum"],
        "kinds::State"
    );
}

#[test]
fn a_target_named_like_a_written_class_is_kept_apart_by_its_path() {
    // `#//sub/Node` beside the root's own `Node`: two classes, two keys.
    let d = closed(&fixture("ecm1i/same_name_target.ecore"));
    let refs = &d["classes"]["Root"]["references"];
    assert_eq!(refs[0]["target"], "sub::Node", "{refs}");
    assert!(d["classes"].get("Node").is_some() && d["classes"].get("sub::Node").is_some());
}

#[test]
fn a_supertype_named_like_a_written_class_is_kept_apart_by_its_path() {
    let d = closed(&fixture("ecm1i/same_name_supertype.ecore"));
    assert_eq!(d["classes"]["Leaf"]["superTypes"], json!(["sub::Node"]));
    assert!(d["classes"].get("Node").is_some());
}

#[test]
fn an_eobject_target_lists_ecore_eobject_and_nothing_more() {
    let d = closed(&fixture("ecm1i/eobject_reference.ecore"));
    let names: Vec<&String> = d["classes"].as_object().unwrap().keys().collect();
    // `Loose` extends `EObject` and nothing reaches it: listing `EObject` must not pull it in.
    assert_eq!(names, ["Trace", "ecore::EObject"]);
    assert_eq!(
        d["classes"]["ecore::EObject"],
        // EC.05 (`spec/items/EC.05.md`): `interface` and `superTypeOrder` added.
        json!({"abstract": false, "superTypes": [], "attributes": [], "containments": [], "references": [],
               "interface": false, "superTypeOrder": []})
    );
    assert_eq!(
        d["classes"]["Trace"]["references"][0]["target"],
        "ecore::EObject"
    );
    assert_eq!(d["rootClasses"], json!(["Trace"]));
}

#[test]
fn a_reference_target_no_root_reaches_is_written() {
    let d = closed(&fixture("ecm1i/unreached_target.ecore"));
    let names: Vec<&String> = d["classes"].as_object().unwrap().keys().collect();
    assert_eq!(names, ["Item", "Kind", "Model"]);
    assert_eq!(d["classes"]["Kind"]["abstract"], true);
    assert_eq!(d["classes"]["Kind"]["attributes"][0]["name"], "name");
    assert_eq!(d["classes"]["Item"]["references"][0]["target"], "Kind");
    assert_eq!(d["rootClasses"], json!(["Model"]));
}

#[test]
fn a_dependency_with_a_subpackage_is_described_whole() {
    // EC.M1h's c2 and c5: the dependency names a class of its own subpackage, which it now writes.
    for root in ["ecm1h/c2/forward/root.ecore", "ecm1h/c5/forward/root.ecore"] {
        closed(&fixture(root));
    }
    let d = closed(&fixture("ecm1h/c2/forward/dep.ecore"));
    assert!(d["classes"].get("inner::F").is_some(), "{}", d["classes"]);
}

fn is_cause(line: &str) -> bool {
    line.strip_prefix("[ERROR arachne] ")
        .unwrap_or(line)
        .trim_start()
        .strip_prefix("caused by: ")
        .is_some_and(|rest| !rest.trim().is_empty())
}

#[test]
fn the_loaders_cause_follows_the_unchanged_first_line() {
    const P: &str =
        "[ERROR arachne] Error: Failed to parse Ecore metamodel: unsupported `eType` path ";
    for (rel, line1) in [
        ("ecm1h/refuse/twice.ecore", format!("{P}`dep.ecore#//D`")),
        ("ecm1h/refuse/missing.ecore", format!("{P}`dep.ecore#//D`")),
        ("ecm1h/refuse/ghost.ecore", format!("{P}`dep.ecore#//D`")),
        (
            "ecm1h/refuse/dep_missing_root.ecore",
            format!("{P}`dep_missing.ecore#//E`"),
        ),
        (
            "ecm1h/refuse/pkg_x_forward.ecore",
            format!("{P}`both_forward.ecore#//pkg/X`"),
        ),
        (
            "ecm1h/refuse/pkg_x_reordered.ecore",
            format!("{P}`both_reordered.ecore#//pkg/X`"),
        ),
        (
            "ecm1h/refuse/a_b_x.ecore",
            format!("{P}`nest.ecore#//a/b/X`"),
        ),
        (
            "ecm1h/refuse/dep_missing.ecore",
            "[ERROR ecore_rs::ctx] some forward-referenced classes have not been defined:".into(),
        ),
    ] {
        let out = describe(&fixture(rel), None);
        assert!(!out.status.success() && out.stdout.is_empty(), "{rel}");
        let lines = err_lines(&out);
        assert_eq!(lines[0], line1, "{rel}: the first line stays as today");
        let causes: Vec<&String> = lines.iter().filter(|l| is_cause(l)).collect();
        assert_eq!(causes.len(), 1, "{rel}: one `caused by: ` line: {lines:?}");
        if rel.ends_with("twice.ecore") {
            // The package defined twice, not the path the parser stopped at.
            assert!(
                !causes[0].contains("unsupported `eType` path"),
                "{rel}: {causes:?}"
            );
        }
    }
}

fn corpus_ids(name: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/ecm1i")
        .join(name);
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

fn corpus_path(id: &str) -> PathBuf {
    let rel = id.strip_prefix("modelset/").expect("a ModelSet id");
    let path = Path::new(CORPUS).join(rel);
    assert!(
        path.is_file(),
        "the ModelSet corpus must be at {CORPUS}: {id}"
    );
    path
}

#[test]
fn the_registered_modelset_files_refuse_or_load_closed() {
    let (refused, changed) = (corpus_ids("refused.txt"), corpus_ids("changed.txt"));
    assert_eq!((refused.len(), changed.len()), (42, 68));
    let mut failures = Vec::new();
    // EC.M1j: the refused ones named a subpackage class; the tree is now described, closed.
    for id in refused.iter().chain(&changed) {
        let out = describe(&corpus_path(id), None);
        let ok = out.status.success()
            && serde_json::from_slice::<Value>(&out.stdout).is_ok_and(|d| {
                undefined(&d).is_empty() && moirai_semantics::from_descriptor(&d).is_ok()
            });
        if !ok {
            failures.push(format!("not closed or not read: {id}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of 110 registered files: {failures:#?}",
        failures.len()
    );
}
