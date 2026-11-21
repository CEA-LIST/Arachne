//! EC.07: `describe` accepts and ignores what EMF writes routinely (`xmi:id` and
//! `eFactoryInstance` on a package, `eExceptions` on an operation, an annotation `details` with no
//! value, `contents` and `references` children of an annotation), and names each ignored
//! construct in an EC.04 line, `warning: dropped <kind> at <location>` (spec:
//! cea-cdrt-knowledge/spec/items/EC.07.md).
//!
//! Each fixture in `fixtures/ec07` has a twin in `fixtures/ec07/twin`: the same bytes with every
//! ignored construct replaced by spaces, so lines and columns are kept. `expected/<name>.out`,
//! `<name>.err` and `exit.tsv` are what the 487d1d1 build printed for the twin. The candidate on the
//! fixture must print the twin's stdout and exit; its stderr is the twin's, with the hand-written
//! lines of `expected/warnings.tsv` merged into the sorted warning block before the identity line
//! when the file is described, and unchanged when it is refused.

use std::{collections::BTreeSet, fs, path::PathBuf, process::Command};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ec07")
}

fn table(name: &str) -> Vec<Vec<String>> {
    fs::read_to_string(dir().join("expected").join(name))
        .unwrap()
        .lines()
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

/// The twin differs from the fixture only where a byte that is not a line break became a space.
fn twin_is_blanked(name: &str) -> Result<(), String> {
    let file = fs::read(dir().join(format!("{name}.ecore"))).unwrap();
    let twin = fs::read(dir().join(format!("twin/{name}.ecore"))).unwrap();
    if file.len() != twin.len() {
        return Err(format!("{name}: twin length differs"));
    }
    let changed: Vec<_> = (0..file.len()).filter(|&i| file[i] != twin[i]).collect();
    if changed.is_empty() {
        return Err(format!("{name}: the twin blanks nothing"));
    }
    if changed
        .iter()
        .any(|&i| twin[i] != b' ' || file[i] == b'\n' || file[i] == b'\r')
    {
        return Err(format!("{name}: the twin changes more than blanks"));
    }
    Ok(())
}

/// `err` with `lines` merged into its warning block, the lines right before the identity line.
fn merged(err: &str, lines: &[String]) -> String {
    let mut all: Vec<&str> = err.split('\n').collect();
    let ident = all.iter().rposition(|l| !l.is_empty()).unwrap();
    let mut first = ident;
    while first > 0 && all[first - 1].starts_with("warning: dropped ") {
        first -= 1;
    }
    let block: BTreeSet<&str> = all[first..ident]
        .iter()
        .copied()
        .chain(lines.iter().map(String::as_str))
        .collect();
    all.splice(first..ident, block);
    all.join("\n")
}

#[test]
fn describe_ignores_routine_emf_constructs_and_names_each() {
    let exits = table("exit.tsv");
    let rows = table("warnings.tsv");
    assert_eq!(rows.len(), 9, "warnings.tsv registers 9 fixtures");
    let mut failures = Vec::new();
    for name in rows.iter().map(|r| r[0].as_str()).chain(["cross/dep"]) {
        if let Err(e) = twin_is_blanked(name) {
            failures.push(e);
        }
    }
    for row in &rows {
        let name = &row[0];
        let stem = name.replace('/', "_");
        let lines: Vec<String> = row[1..].iter().filter(|l| !l.is_empty()).cloned().collect();
        let exit: i32 = exits.iter().find(|e| &e[0] == name).unwrap()[1]
            .parse()
            .unwrap();
        let err = fs::read_to_string(dir().join(format!("expected/{stem}.err"))).unwrap();
        let want = if exit == 0 {
            merged(&err, &lines)
        } else {
            assert!(
                lines.is_empty(),
                "{name}: a refused fixture registers no line"
            );
            err
        };
        let out = Command::new(env!("CARGO_BIN_EXE_arachne"))
            .current_dir(dir())
            .env_remove("RUST_LOG")
            .arg("describe")
            .arg(format!("{name}.ecore"))
            .output()
            .expect("run arachne describe");
        if out.status.code() != Some(exit) {
            failures.push(format!(
                "{name}: exit {:?}, expected {exit}",
                out.status.code()
            ));
        }
        if out.stdout != fs::read(dir().join(format!("expected/{stem}.out"))).unwrap() {
            failures.push(format!("{name}: stdout differs from the twin's"));
        }
        if out.stderr != want.as_bytes() {
            failures.push(format!(
                "{name}: stderr\n{}expected\n{want}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures over {} fixtures:\n{}",
        failures.len(),
        rows.len(),
        failures.join("\n")
    );
}
