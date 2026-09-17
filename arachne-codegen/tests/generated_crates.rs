//! Generates a crate from a metamodel, builds it and runs the tests written for it.
//!
//! The tests of a generated crate live in `tests/generated/<crate>/`, beside this file, and are
//! copied into the generated crate's own `tests/` directory before `cargo test` runs there. The
//! generated crates are written under Cargo's temporary directory for integration tests and share
//! one target directory, so a second run only rebuilds what changed. Building a generated crate
//! needs its dependencies (Moirai at the tag the generator names, and the crates it uses), from
//! the network or from Cargo's cache.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use arachne_codegen::{Config, generate_with_report};

/// Generates `metamodel` as the crate `project`, copies the tests of `tests/generated/<project>/`
/// into it, and runs `cargo test` on it.
fn generate_build_and_test(metamodel: &str, project: &str) {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let output = tmp.join("generated").join(project);
    if output.exists() {
        fs::remove_dir_all(&output).expect("removing the previous generated crate failed");
    }

    let config = Config::new(manifest_dir.join(metamodel))
        .with_output_dir(&output)
        .with_project_name(project);
    if let Err(error) = generate_with_report(config) {
        panic!("generating `{metamodel}` failed: {error:#}");
    }

    // The crate is written inside this workspace's target directory; an empty workspace table
    // keeps Cargo from taking it for a member of this workspace.
    let manifest = output.join("Cargo.toml");
    let mut cargo_toml = fs::read_to_string(&manifest).expect("reading the generated manifest");
    cargo_toml.push_str("\n[workspace]\n");
    fs::write(&manifest, cargo_toml).expect("writing the generated manifest");

    let tests = manifest_dir.join("tests").join("generated").join(project);
    if tests.is_dir() {
        let destination = output.join("tests");
        fs::create_dir_all(&destination).expect("creating the generated crate's tests directory");
        for entry in fs::read_dir(&tests).expect("listing the generated crate's tests") {
            let path = entry.expect("reading a test file").path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                fs::copy(&path, destination.join(path.file_name().unwrap()))
                    .expect("copying a test file");
            }
        }
    }

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let result = Command::new(cargo)
        .arg("test")
        .arg("--manifest-path")
        .arg(&manifest)
        .env("CARGO_TARGET_DIR", tmp.join("generated-target"))
        .output()
        .expect("running cargo on the generated crate");
    assert!(
        result.status.success(),
        "`cargo test` on the crate generated from `{metamodel}` failed\n--- stdout\n{}\n--- stderr\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
    );
}

/// A metamodel whose classes extend `EObject`, `EModelElement` and `ENamedElement`, and whose
/// features are typed by `EObject` and `EModelElement`, generates a crate that builds and passes
/// the tests of `tests/generated/annotated/`.
#[test]
fn ecore_builtins() {
    generate_build_and_test(
        "../examples/pet_metamodels/ecore_builtins.ecore",
        "annotated",
    );
}
