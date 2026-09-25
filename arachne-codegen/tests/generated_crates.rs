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
    generate_build_and_test_with(metamodel, project, &[]);
}

/// [`generate_build_and_test`], with `test_args` passed to `cargo test` after the manifest path
/// (`["--", "--nocapture"]`, say), answering with what `cargo test` printed.
fn generate_build_and_test_with(
    metamodel: &str,
    project: &str,
    test_args: &[&str],
) -> std::process::Output {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let output = tmp.join("generated").join(project);
    if output.exists() {
        fs::remove_dir_all(&output).expect("removing the previous generated crate failed");
    }

    // The model-plane Moirai checkout, which is the one `arachne-codegen`'s own manifest depends
    // on: the generator and what it generates cannot be allowed to see two versions of the
    // interface, and `Config`'s default `../moirai` is a sibling of the *working directory* and
    // so names whichever checkout happens to sit there.
    let moirai = fs::canonicalize(manifest_dir.join("../../moirai-model-plane"))
        .expect("the model-plane Moirai checkout sits beside this one");

    let config = Config::new(manifest_dir.join(metamodel))
        .with_output_dir(&output)
        .with_project_name(project)
        .with_moirai_root(&moirai);
    if let Err(error) = generate_with_report(config) {
        panic!("generating `{metamodel}` failed: {error:#}");
    }

    // The crate is written inside this workspace's target directory; an empty workspace table
    // keeps Cargo from taking it for a member of this workspace. The dev-dependencies are the
    // ones the five checked-in crates carry by hand: the equivalence oracle drives the
    // interpreted `ModelLog` beside the generated log, and `heck` spells a field the way the
    // generator spelled it.
    let manifest = output.join("Cargo.toml");
    let mut cargo_toml = fs::read_to_string(&manifest).expect("reading the generated manifest");
    let moirai = moirai.display();
    cargo_toml.push_str(&format!(
        "\n[dev-dependencies]\n\
         moirai-interp = {{ path = \"{moirai}/moirai-interp\", features = [\"sink\"] }}\n\
         moirai-semantics = {{ path = \"{moirai}/moirai-semantics\" }}\n\
         heck = \"0.5.0\"\n\
         \n[workspace]\n"
    ));
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
        .args(test_args)
        .env("CARGO_TARGET_DIR", tmp.join("generated-target"))
        .output()
        .expect("running cargo on the generated crate");
    assert!(
        result.status.success(),
        "`cargo test` on the crate generated from `{metamodel}` failed\n--- stdout\n{}\n--- stderr\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
    );
    result
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

/// ST.01 (`cea-cdrt-knowledge/spec/items/ST.01.md`), oracle O5: the runtime this generator writes
/// for `tests/generated/st01/st01.ecore`, an optional containment of an abstract class, against
/// the interpreted runtime, on every schedule of an unset raced by a write to one member of a
/// conflict. The tests are `tests/generated/st01/twin.rs`; each prints its counts on lines
/// starting `ST01-`, repeated here.
///
/// First, the descriptor this generator writes for `st01.ecore` must be, value for value, the
/// fixture the interpreted half of the oracle reads in the model-plane Moirai checkout
/// (`moirai-interp/tests/fixtures/st01.metamodel.json`), so that both halves of ST.01 are about
/// one metamodel.
#[test]
fn st01_optional_conflict_twin() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ecore = manifest_dir.join("tests/generated/st01/st01.ecore");
    let parser = arachne_codegen::EcoreParser::from_file(&ecore)
        .unwrap_or_else(|error| panic!("parsing `{}` failed: {error}", ecore.display()));
    let pack = arachne_codegen::find_user_package(&parser.ctx).expect("st01.ecore has a package");
    let described =
        arachne_codegen::descriptor_json(&parser.ctx, pack).expect("st01.ecore is described");
    let fixture_path = manifest_dir
        .join("../../moirai-model-plane/moirai-interp/tests/fixtures/st01.metamodel.json");
    let fixture: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&fixture_path)
            .unwrap_or_else(|error| panic!("reading `{}`: {error}", fixture_path.display())),
    )
    .expect("the fixture is JSON");
    assert_eq!(
        described, fixture,
        "the descriptor of st01.ecore is not the fixture the interpreted half of ST.01 reads"
    );

    let output = generate_build_and_test_with(
        "tests/generated/st01/st01.ecore",
        "st01",
        &["--", "--nocapture", "--test-threads=1"],
    );
    for line in String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.starts_with("ST01-"))
    {
        eprintln!("{line}");
    }
}
