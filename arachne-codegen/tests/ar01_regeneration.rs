//! AR.01 (`cea-cdrt-knowledge/spec/items/AR.01.md`), oracle A4: the generator at this checkout,
//! run on the metamodel of each of the five crates checked in under `generated/`, writes that
//! crate's generated files byte for byte as they are checked in.
//!
//! The paper's conflict matrix (123 cells, 10,312 schedules, 458,520 twin and 137,536 replica
//! comparisons) is computed on those five crates, so a generator change that would write any of
//! them differently changes the code the figure was measured on. Compared: every file the
//! generator writes whose bytes do not depend on where it writes them (`COMPARED`). Not
//! compared: `Cargo.toml` and `Dockerfile`, which carry the output directory's path (and, as
//! checked in, `Cargo.toml` carries dev-dependencies added by hand); the set of files written
//! must still be exactly those nine.
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-codegen --test ar01_regeneration -- --nocapture
//! ```
//!
//! AR.02 (`cea-cdrt-knowledge/spec/items/AR.02.md`) amends one of the 35 comparisons, and only
//! the bytes it expects. From AR.02 on the descriptor carries the `eOpposite`, `subsets` and
//! `redefines` an `.ecore` declares, and `SimpleUML.ecore` declares four `eOpposite`, so
//! `simpleuml_crdt/metamodel.json` is written with them. The checked-in crate is not
//! regenerated: that file is compared against the checked-in one with exactly those four keys
//! set (`AR02_OPPOSITES`), rendered as the generator renders a descriptor (`{:#}` and a
//! newline). The other 34 files are compared byte for byte as before.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use arachne_codegen::{Config, generate_with_report};
use serde_json::{Value, json};

// SI.05 (`cea-cdrt-knowledge/spec/items/SI.05.md`, section 6.2): see `si05_support`.
#[path = "si05_support/mod.rs"]
mod si05_support;

/// Each checked-in crate, under `generated/`, and the metamodel it was generated from.
const CRATES: [(&str, &str); 5] = [
    ("bt_crdt", "examples/behavior_tree.ecore"),
    ("classdiagram_crdt", "examples/class_diagram.ecore"),
    ("json_crdt", "examples/json.ecore"),
    ("kitchen_crdt", "examples/pet_metamodels/kitchen_sink.ecore"),
    ("simpleuml_crdt", "examples/SimpleUML.ecore"),
];

/// The files the generator writes whose bytes do not depend on where it writes them.
const COMPARED: [&str; 7] = [
    "src/lib.rs",
    "src/classifiers.rs",
    "src/references.rs",
    "src/package.rs",
    "metamodel.json",
    "examples/network_node.rs",
    "examples/conformance/mod.rs",
];

/// The files the generator writes that carry the output directory's path.
const LOCATED: [&str; 2] = ["Cargo.toml", "Dockerfile"];

/// Every file under `root`, as a path relative to it with `/` separators.
fn files(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("listing {}: {e}", dir.display()))
        {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path.strip_prefix(root).expect("under the root");
                out.insert(
                    relative
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/"),
                );
            }
        }
    }
    out
}

/// AR.02: the `eOpposite` declarations of `SimpleUML.ecore` (lines 13, 23, 34, 62), as
/// `(crate, class, feature, opposite)`, read by hand.
const AR02_OPPOSITES: [(&str, &str, &str, &str); 4] = [
    ("simpleuml_crdt", "Package", "ownedElements", "owner"),
    ("simpleuml_crdt", "Packageable", "owner", "ownedElements"),
    ("simpleuml_crdt", "DataType", "attributes", "owner"),
    ("simpleuml_crdt", "Property", "owner", "attributes"),
];

/// The bytes the generator must write for `file` of `krate`: the checked-in bytes, except for a
/// descriptor AR.02 adds declarations to, which is the checked-in descriptor with them set.
fn expected_bytes(krate: &str, file: &str, checked_in: Vec<u8>) -> Vec<u8> {
    let declared: Vec<_> = AR02_OPPOSITES
        .iter()
        .filter(|(k, ..)| *k == krate)
        .collect();
    if file != "metamodel.json" || declared.is_empty() {
        return checked_in;
    }
    let mut descriptor: Value = serde_json::from_slice(&checked_in)
        .unwrap_or_else(|e| panic!("set-up: {krate}/{file} is not JSON: {e}"));
    for (_, class, feature, opposite) in declared {
        let entry = ["containments", "references"]
            .iter()
            .find_map(|array| {
                let position = descriptor["classes"][class][*array]
                    .as_array()?
                    .iter()
                    .position(|entry| entry["name"] == json!(feature))?;
                Some((*array, position))
            })
            .unwrap_or_else(|| panic!("set-up: {krate}/{file} has no `{class}.{feature}`"));
        descriptor["classes"][class][entry.0][entry.1]["eOpposite"] = json!(opposite);
    }
    format!("{descriptor:#}\n").into_bytes()
}

/// Where two byte strings first differ, as a 1-based line number.
fn first_differing_line(left: &[u8], right: &[u8]) -> usize {
    let at = left
        .iter()
        .zip(right)
        .position(|(l, r)| l != r)
        .unwrap_or(left.len().min(right.len()));
    1 + left[..at].iter().filter(|&&b| b == b'\n').count()
}

#[test]
fn ar01_the_five_checked_in_crates_regenerate_byte_for_byte() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest_dir.join("..");
    let moirai = fs::canonicalize(manifest_dir.join("../../moirai-model-plane"))
        .expect("the model-plane Moirai checkout sits beside this one");
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ar01-regenerated");
    let expected: BTreeSet<String> = COMPARED
        .iter()
        .chain(LOCATED.iter())
        .map(|f| f.to_string())
        .collect();

    let mut compared = 0;
    let mut differing = Vec::new();
    for (krate, ecore) in CRATES {
        let output = tmp.join(krate);
        if output.exists() {
            fs::remove_dir_all(&output).expect("removing the previous regeneration");
        }
        let config = Config::new(repo.join(ecore))
            .with_output_dir(&output)
            .with_project_name(krate)
            .with_moirai_root(&moirai);
        if let Err(error) = generate_with_report(config) {
            panic!("AR01 claim: generating `{ecore}` failed: {error:#}");
        }
        let written = files(&output);
        if written != expected {
            differing.push(format!(
                "{krate}: the generator writes {written:?}, not {expected:?}"
            ));
        }
        for file in COMPARED {
            compared += 1;
            let checked_in = repo.join("generated").join(krate).join(file);
            let checked_in = fs::read(&checked_in)
                .unwrap_or_else(|e| panic!("reading {}: {e}", checked_in.display()));
            let checked_in = expected_bytes(krate, file, checked_in);
            let regenerated = fs::read(output.join(file)).unwrap_or_default();
            // SI.05: the generator names the identity rule on the in-scope references of a
            // crate's descriptor, and the checked-in crates are not regenerated (scoping decision
            // S1); everything else of the file is held byte for byte as before.
            let regenerated = if file == "metamodel.json" {
                si05_support::read_back_bytes(regenerated)
            } else {
                regenerated
            };
            if regenerated != checked_in {
                differing.push(format!(
                    "{krate}/{file}: regenerated {} bytes, checked in {}, first differing at \
                     line {}",
                    regenerated.len(),
                    checked_in.len(),
                    first_differing_line(&regenerated, &checked_in)
                ));
            }
        }
    }
    println!(
        "AR01-A4 crates {}, files compared {compared}, differing {}",
        CRATES.len(),
        differing.len()
    );
    for line in &differing {
        println!("AR01-A4 {line}");
    }
    assert!(
        differing.is_empty(),
        "AR01 claim: {} of {compared} generated files differ from the checked-in crates:\n  {}",
        differing.len(),
        differing.join("\n  ")
    );
}
