//! EC.01: `arachne describe` on Léo's parser writes, for every fixture metamodel, the byte-identical
//! descriptor and identity line that `arachne describe` writes on our interpreted line at
//! `akira/interpreter-modelstore` f90795e.
//!
//! The goldens under `fixtures/ec01/golden/` were written by that build (release, archive of
//! f90795e against `moirai-model-plane` 748a185) and are the ground truth; `fixtures/ec01/SOURCES.txt`
//! names where each input came from and its SHA-256. `<name>.metamodel.json` is describe's stdout,
//! byte for byte; `<name>.id` is the last line describe writes on stderr, `{"digest", "nsURI"}`.
//!
//! No fixture may be refused: every one of them is described at f90795e, and losing one is a
//! semantic change for the interpreted path.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ec01")
}

fn names() -> Vec<String> {
    let sources = fs::read_to_string(fixtures().join("SOURCES.txt")).expect("SOURCES.txt");
    sources
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split_whitespace()
                .next()
                .unwrap()
                .trim_end_matches(".ecore")
                .to_string()
        })
        .collect()
}

#[test]
fn the_fixture_set_is_the_registered_one() {
    assert_eq!(
        names().len(),
        33,
        "33 fixtures were registered at the spec commit"
    );
}

#[test]
fn describe_matches_f90795e_byte_for_byte_on_every_fixture() {
    let mut failures = Vec::new();
    for name in names() {
        let input = fixtures().join("ecore").join(format!("{name}.ecore"));
        let output = Command::new(env!("CARGO_BIN_EXE_arachne"))
            .arg("describe")
            .arg(&input)
            .output()
            .expect("arachne runs");
        if !output.status.success() {
            failures.push(format!(
                "{name}: refused ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
            continue;
        }
        let golden = fs::read(
            fixtures()
                .join("golden")
                .join(format!("{name}.metamodel.json")),
        )
        .expect("golden descriptor");
        if output.stdout != golden {
            failures.push(format!("{name}: descriptor differs from f90795e's"));
        }
        let id = fs::read_to_string(fixtures().join("golden").join(format!("{name}.id")))
            .expect("golden id");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let last = stderr.lines().last().unwrap_or("");
        if last != id.trim_end() {
            failures.push(format!(
                "{name}: identity line `{last}` differs from f90795e's `{}`",
                id.trim_end()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} fixtures differ:\n{}",
        failures.len(),
        names().len(),
        failures.join("\n")
    );
}
