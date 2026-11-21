//! EC.M1f: `describe` reads an Ecore file whose package is referenced (`#//pkg/X`) before it is
//! defined, and emits the descriptor it emits for the same metamodel with that package defined
//! first; `generate` keeps refusing such a file with Léo's line (Cam, 2026-11-20).
//!
//! The reference is the reordered file: EMF reads both orders as one metamodel, and describe
//! already reads the reordered one. Describe describes the document-root package only.
//!
//! EC.M1i (spec: cea-cdrt-knowledge/spec/items/EC.M1i.md): the two twin pairs below named a
//! subpackage class from a written class, so their descriptors were not closed; they now refuse
//! alike, and each twin claim holds on a closed variant (`fixtures/ecm1i/closed/`) whose forward
//! reference sits in a class describe does not write.
//!
//! EC.M1j (spec: cea-cdrt-knowledge/spec/items/EC.M1j.md): describe writes the whole package tree, so
//! the open twins now describe alike, the subpackage class keyed by its `::` path.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LEO_LINE: &str = "failed to define previously forward-referenced package: not empty";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ecm1f")
        .join(name)
}

fn closed_fixture(case: &str, side: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ecm1i/closed")
        .join(case)
        .join(side)
        .join("root.ecore")
}

fn arachne(args: &[&str], input: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arachne"))
        .args(args)
        .arg(input)
        .output()
        .expect("run arachne")
}

fn describe(name: &str) -> Output {
    arachne(&["describe"], &fixture(name))
}

/// Describe reads the closed `case`'s forward file and prints exactly what it prints for its
/// reordered twin, stdout and stderr.
fn reads_as_reordered(case: &str) {
    let want = arachne(&["describe"], &closed_fixture(case, "reordered"));
    assert!(
        want.status.success(),
        "the reference {case}/reordered must describe"
    );
    let got = arachne(&["describe"], &closed_fixture(case, "forward"));
    assert!(
        got.status.success(),
        "describe should read {case}/forward, got: {}",
        String::from_utf8_lossy(&got.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&got.stdout),
        String::from_utf8_lossy(&want.stdout)
    );
    assert_eq!(
        String::from_utf8_lossy(&got.stderr),
        String::from_utf8_lossy(&want.stderr)
    );
}

/// The open twins describe alike, stdout and stderr, and the descriptor writes `key`.
fn describe_alike(forward: &str, reordered: &str, key: &str) {
    let (got, want) = (describe(forward), describe(reordered));
    assert!(
        got.status.success() && want.status.success(),
        "{forward} and {reordered} describe: {}",
        String::from_utf8_lossy(&got.stderr)
    );
    assert_eq!(got.stdout, want.stdout, "{forward}: stdout");
    assert_eq!(got.stderr, want.stderr, "{forward}: stderr");
    let d: serde_json::Value = serde_json::from_slice(&got.stdout).expect("a JSON descriptor");
    assert!(d["classes"].get(key).is_some(), "{forward}: no `{key}`");
}

fn describe_refuses(name: &str) {
    let out = describe(name);
    assert!(!out.status.success(), "describe should refuse {name}");
    assert!(out.stdout.is_empty(), "no descriptor for {name}");
}

fn generate_refuses_with_leo_line(name: &str) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ecm1f-gen-{name}"));
    let dir = dir.to_str().unwrap();
    let out = arachne(&["generate", "-o", dir], &fixture(name));
    assert!(!out.status.success(), "generate should refuse {name}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains(LEO_LINE), "generate on {name}: {stderr}");
}

#[test]
fn describe_reads_leos_fixture_as_its_reordered_twin() {
    // The text of Léo's `rejects_interpackage_links`, `A.toB` moved into `early/H`.
    reads_as_reordered("leo");
}

#[test]
fn describe_reads_a_nested_forward_package_as_its_reordered_twin() {
    // `#//catalog/Item` and `#//catalog/detail/Tag` before `catalog` and `detail` are defined,
    // held by `early/H`.
    reads_as_reordered("nested");
}

#[test]
fn leos_fixture_and_its_twin_describe_alike() {
    // leo_forward.ecore is the text of Léo's `rejects_interpackage_links`: `A.toB` names `sub/B`.
    describe_alike("leo_forward.ecore", "leo_reordered.ecore", "sub::B");
}

#[test]
fn the_nested_forward_package_and_its_twin_describe_alike() {
    describe_alike(
        "nested_forward.ecore",
        "nested_reordered.ecore",
        "catalog::detail::Tag",
    );
}

#[test]
fn describe_still_refuses_a_forward_class_its_package_never_defines() {
    describe_refuses("missing_forward.ecore");
}

#[test]
fn describe_still_refuses_a_forward_package_defined_twice() {
    describe_refuses("twice_forward.ecore");
}

#[test]
fn generate_keeps_refusing_forward_packages() {
    generate_refuses_with_leo_line("leo_forward.ecore");
    generate_refuses_with_leo_line("nested_forward.ecore");
}
