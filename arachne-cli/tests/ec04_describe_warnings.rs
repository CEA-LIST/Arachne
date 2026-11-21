//! EC.04: `describe` prints one stderr line, `warning: dropped <kind> at <location>`, for every
//! construct it reads and does not carry into the descriptor, sorted and unique, right before the
//! identity line; stdout, the identity line, the exit status and refusals stay as at 76c9caf (spec:
//! cea-cdrt-knowledge/spec/items/EC.04.md).
//!
//! `fixtures/ec04/expected/warnings.tsv` holds the hand-written warning lines of each fixture, one
//! fixture per construct kind plus `clean` (explicit Ecore defaults, nothing dropped), `combined`
//! (sort, dedup, a subpackage class), `refused`, `cross/root` (a class imported from another file),
//! `unlisted/root` (an imported class its own file's descriptor does not list) and `same_key/root`
//! (a subpackage class under the key of a class a transitively loaded file declares).
//! `<name>.out`, `<name>.err` and `exit.tsv` are what the 76c9caf build printed for the same file.

use std::{fs, path::PathBuf, process::Command};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ec04")
}

fn table(name: &str) -> Vec<Vec<String>> {
    fs::read_to_string(dir().join("expected").join(name))
        .unwrap()
        .lines()
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

#[test]
fn describe_names_every_dropped_construct_and_keeps_its_output() {
    let exits = table("exit.tsv");
    let rows = table("warnings.tsv");
    assert_eq!(rows.len(), 23, "warnings.tsv registers 23 fixtures");
    let mut failures = Vec::new();
    for row in &rows {
        let name = &row[0];
        let stem = name.replace('/', "_");
        let exit: i32 = exits.iter().find(|e| &e[0] == name).unwrap()[1]
            .parse()
            .unwrap();
        let mut want = String::new();
        for line in row[1..].iter().filter(|l| !l.is_empty()) {
            want.push_str(line);
            want.push('\n');
        }
        let want = [
            want.into_bytes(),
            fs::read(dir().join(format!("expected/{stem}.err"))).unwrap(),
        ]
        .concat();
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
            failures.push(format!("{name}: stdout differs from 76c9caf's"));
        }
        if out.stderr != want {
            failures.push(format!(
                "{name}: stderr\n{}expected\n{}",
                String::from_utf8_lossy(&out.stderr),
                String::from_utf8_lossy(&want)
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
