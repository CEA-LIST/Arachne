//! EC.M1f: `describe` reads an Ecore file whose package is referenced (`#//pkg/X`) before it is
//! defined, and emits the descriptor it emits for the same metamodel with that package defined
//! first; `generate` keeps refusing such a file with Léo's line (Cam, 2026-11-20).
//!
//! The reference is the reordered file: EMF reads both orders as one metamodel, and describe
//! already reads the reordered one. Describe describes the document-root package only, so the
//! other package's classes are named as targets and not described, in both orders alike.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LEO_LINE: &str = "failed to define previously forward-referenced package: not empty";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ecm1f")
        .join(name)
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

/// Describe reads `forward` and prints exactly what it prints for `reordered`, stdout and stderr.
fn reads_as_reordered(forward: &str, reordered: &str) {
    let want = describe(reordered);
    assert!(
        want.status.success(),
        "the reference {reordered} must describe"
    );
    let got = describe(forward);
    assert!(
        got.status.success(),
        "describe should read {forward}, got: {}",
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
    // leo_forward.ecore is the text of Léo's `rejects_interpackage_links`.
    reads_as_reordered("leo_forward.ecore", "leo_reordered.ecore");
}

#[test]
fn describe_reads_a_nested_forward_package_as_its_reordered_twin() {
    // `#//catalog/Item` and `#//catalog/detail/Tag` before `catalog` and `detail` are defined.
    reads_as_reordered("nested_forward.ecore", "nested_reordered.ecore");
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
