//! AR.06 (`cea-cdrt-knowledge/spec/items/AR.06.md`): two module docs that AR.04 and AR.05b left
//! saying what the code no longer does, and a guard that only doc comments move.
//!
//! - D1: the version 2 key list in `descriptor.rs`'s module doc no longer says `annotation` is
//!   "the `urn:arachne:semantics` `datatype` string it wrote, or `null`". Its `annotation` bullet
//!   points to `feature_datatype` and to the package-level detail that governs an attribute of
//!   one of Ecore's own classes.
//! - D2: the paragraph of `semantics.rs`'s module doc that names the generator's matches no
//!   longer says `annotation.rs`'s `datatype_override` is what the attribute generator applies
//!   (since `ca22705`, `feature/attribute.rs` calls `datatype_override_in`); it names
//!   `datatype_override_in` and the package-level detail.
//! - G1: guards, green before and after: the code of both files (every line that is a `//`
//!   comment, and every blank line, removed) is the code at `6606ed9`;
//!   `datatype/to_crdt.rs` and every file under `examples/` are byte for byte as at `6606ed9`.
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-codegen --test ar06_doc_comments
//! ```

use std::{
    fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

const DESCRIPTOR_RS: &str = "arachne-codegen/src/codegen/descriptor.rs";
const SEMANTICS_RS: &str = "arachne-codegen/src/codegen/semantics.rs";
const TO_CRDT_RS: &str = "arachne-codegen/src/codegen/datatype/to_crdt.rs";

/// sha256 of each file's code at `6606ed9`, as [`code_only`] reads it.
const DESCRIPTOR_CODE_SHA256: &str =
    "ba22340b161d4e515d622bfe3fd2c6ebf2db47e13eb97a530252065a53c78f4f";
const SEMANTICS_CODE_SHA256: &str =
    "f1764fa68c791c6fb856cbf0e75c65d0ae3d7262f771b89795c9c7b7a101c24f";
/// sha256 of `datatype/to_crdt.rs` at `6606ed9`.
const TO_CRDT_SHA256: &str = "2de8d2e8927b67d61f81d5055ac6a92809991aa259a96227e216602411dd4218";
/// sha256 of [`examples_listing`] at `6606ed9`.
const EXAMPLES_SHA256: &str = "18f1dba27d1cf7cc07f8bf92040c730ede266efec2a666b94277819cc840d92c";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(relative: &str) -> String {
    let path = repo().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("set-up: {}: {e}", path.display()))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The file's `//!` module doc, one string per paragraph (a blank doc line ends one), each with
/// its whitespace collapsed.
fn module_doc_paragraphs(source: &str) -> Vec<String> {
    let lines: Vec<&str> = source
        .lines()
        .take_while(|line| line.starts_with("//!"))
        .map(|line| line.trim_start_matches("//!").trim())
        .collect();
    lines
        .split(|line| line.is_empty())
        .map(|lines| lines.join(" "))
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|text| !text.is_empty())
        .collect()
}

/// The bullet of the module doc that starts with `start`, through the line before the next
/// bullet or the end of its paragraph, whitespace collapsed.
fn module_doc_bullet(source: &str, start: &str) -> String {
    let lines: Vec<&str> = source
        .lines()
        .take_while(|line| line.starts_with("//!"))
        .map(|line| line.trim_start_matches("//!").trim())
        .collect();
    let at = lines
        .iter()
        .position(|line| line.starts_with(start))
        .unwrap_or_else(|| panic!("AR.06 claim: no module-doc bullet starts with `{start}`"));
    let mut bullet = vec![lines[at]];
    bullet.extend(
        lines[at + 1..]
            .iter()
            .take_while(|line| !line.is_empty() && !line.starts_with("- ")),
    );
    bullet
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The file with every blank line and every line that is a `//` comment (`//`, `///`, `//!`)
/// removed, lines kept as written otherwise, joined by `\n`.
#[allow(dead_code)] // SI.05 retired the two code pins that read it (section 6.2).
fn code_only(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            !line.is_empty() && !line.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `<relative path> <sha256>` for every file under `examples/`, sorted by path, one per line.
fn examples_listing() -> String {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|e| panic!("set-up: {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("set-up: a directory entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let root = repo();
    let mut files = Vec::new();
    walk(&root.join("examples"), &mut files);
    let mut lines: Vec<String> = files
        .iter()
        .map(|path| {
            let bytes =
                fs::read(path).unwrap_or_else(|e| panic!("set-up: {}: {e}", path.display()));
            let relative = path.strip_prefix(&root).expect("set-up: under the repo");
            format!("{} {}", relative.display(), sha256(&bytes))
        })
        .collect();
    lines.sort();
    lines.join("\n")
}

/// D1: the `annotation` bullet of `descriptor.rs`'s version 2 key list.
#[test]
fn ar06_d1_descriptor_key_list_says_annotation_is_the_governing_datatype() {
    let source = read(DESCRIPTOR_RS);
    let bullet = module_doc_bullet(&source, "- `annotation`");
    let mut failures = Vec::new();
    if bullet.contains("string it wrote") {
        failures.push("it still says the value is the string the file wrote on the feature");
    }
    if !bullet.contains("feature_datatype") {
        failures.push("it does not point to `feature_datatype`");
    }
    if !bullet.to_lowercase().contains("package") {
        failures.push("it does not say a package-level detail can govern the value");
    }
    if !bullet.contains("Ecore") {
        failures.push("it does not say the package-level detail is for Ecore's own classes");
    }
    if module_doc_paragraphs(&source)
        .iter()
        .any(|paragraph| paragraph.contains("string it wrote"))
    {
        failures.push("the module doc still says `string it wrote` elsewhere");
    }
    println!("\nAR06 D1 bullet: {bullet}");
    assert!(
        failures.is_empty(),
        "AR.06 claim: descriptor.rs's `annotation` key: {}\nbullet: {bullet}",
        failures.join("; ")
    );
}

/// D2: the paragraph of `semantics.rs`'s module doc that names the generator's matches.
#[test]
fn ar06_d2_semantics_module_doc_names_datatype_override_in() {
    let source = read(SEMANTICS_RS);
    let paragraphs = module_doc_paragraphs(&source);
    let matches: Vec<&String> = paragraphs
        .iter()
        .filter(|paragraph| paragraph.contains("feature/attribute.rs"))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "AR.06 claim: semantics.rs's module doc has not exactly one paragraph naming \
         `feature/attribute.rs`: {matches:?}"
    );
    let paragraph = matches[0];
    let mut failures = Vec::new();
    if paragraph.contains("`datatype_override` lets") {
        failures.push("it still says `datatype_override` is what replaces the family");
    }
    if !paragraph.contains("`datatype_override_in`") {
        failures.push("it does not name `datatype_override_in`");
    }
    if !paragraph.to_lowercase().contains("package") {
        failures.push("it does not say the package-level detail is followed");
    }
    println!("\nAR06 D2 paragraph: {paragraph}");
    assert!(
        failures.is_empty(),
        "AR.06 claim: semantics.rs's module doc: {}\nparagraph: {paragraph}",
        failures.join("; ")
    );
}

/// G1: only doc comments move; `to_crdt.rs` and `examples/` do not.
#[test]
fn ar06_g1_only_comments_move() {
    // SI.05 (`cea-cdrt-knowledge/spec/items/SI.05.md`, section 6.2) changes the code of
    // `descriptor.rs` and `semantics.rs`: the emitter names the identity rule. The two code pins
    // were AR.06's guard for its own round (its record, round 1, holds them green at its
    // commit) and are retired here; `to_crdt.rs` and `examples/` stay pinned, which SI.05's
    // scoping decision S1 also requires.
    let _ = (
        DESCRIPTOR_CODE_SHA256,
        SEMANTICS_CODE_SHA256,
        DESCRIPTOR_RS,
        SEMANTICS_RS,
    );
    let observed = [
        (
            TO_CRDT_RS.to_string(),
            sha256(read(TO_CRDT_RS).as_bytes()),
            TO_CRDT_SHA256,
        ),
        (
            "examples/".to_string(),
            sha256(examples_listing().as_bytes()),
            EXAMPLES_SHA256,
        ),
    ];
    let moved: Vec<String> = observed
        .iter()
        .filter(|(_, got, want)| got != want)
        .map(|(what, got, want)| format!("{what}: sha256 {got}, expected {want}"))
        .collect();
    for (what, got, _) in &observed {
        println!("AR06 G1 {what} {got}");
    }
    assert!(
        moved.is_empty(),
        "AR.06 claim: something besides a comment moved:\n  {}",
        moved.join("\n  ")
    );
}
