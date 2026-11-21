//! EC.05b (`cea-cdrt-knowledge/spec/items/EC.05b.md`): `describe` reads `upperBound="-2"` (Ecore's
//! unspecified multiplicity) as unbounded. `fixtures/ec05b/unspecified.ecore` is `unbounded.ecore`
//! with every `-1` written `-2`, so its run must be the twin's, byte for byte; `-3` still refuses.

use std::{path::PathBuf, process::Command};

fn describe(file: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_arachne"))
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ec05b"))
        .env_remove("RUST_LOG")
        .arg("describe")
        .arg(file)
        .output()
        .expect("run arachne describe")
}

#[test]
fn minus_two_reads_as_minus_one() {
    let twin = describe("unbounded.ecore");
    assert_eq!(twin.status.code(), Some(0), "the -1 twin describes");
    let run = describe("unspecified.ecore");
    assert_eq!(
        run.status.code(),
        Some(0),
        "-2 refused:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(run.stdout == twin.stdout, "stdout differs from the -1 twin");
    assert_eq!(
        String::from_utf8_lossy(&run.stderr),
        String::from_utf8_lossy(&twin.stderr),
        "stderr differs from the -1 twin"
    );
}

#[test]
fn minus_three_still_refuses() {
    let run = describe("minus_three.ecore");
    let err = String::from_utf8_lossy(&run.stderr);
    assert_eq!(run.status.code(), Some(1));
    assert!(run.stdout.is_empty());
    assert!(err.contains("unexpected upper bound") && err.contains("`-3`"), "{err}");
}
