//! EC.05 round 2: every identity digest a golden pins is the digest of the golden descriptor
//! beside it (spec: cea-cdrt-knowledge/spec/items/EC.05.md, round 2). Three pin forms under
//! `tests/fixtures`: the last stderr line of `<name>.err` beside `<name>.out`, `<name>.id` beside
//! `<name>.metamodel.json`, and each `identity.tsv` row beside `<stem>.json`. The digest is
//! computed here from the golden's bytes (sha256 of the compact JSON with sorted keys), never by
//! the code under test.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use sha2::{Digest, Sha256};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files(&p, out)
        } else {
            out.push(p)
        }
    }
}

/// sha256 of the compact serialization; serde_json's map is sorted (no `preserve_order` here,
/// and the goldens are written with sorted keys either way).
fn digest_of(golden: &Path) -> String {
    let v: Value = serde_json::from_slice(&fs::read(golden).unwrap()).unwrap();
    format!("{:x}", Sha256::digest(v.to_string()))
}

#[test]
fn every_pinned_identity_digest_is_the_digest_of_the_golden_beside_it() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut all = Vec::new();
    files(&root, &mut all);
    all.sort();
    let mut pins: Vec<(PathBuf, String, String)> = Vec::new(); // (golden, pinned, where)
    for p in &all {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let at = p.strip_prefix(&root).unwrap().display().to_string();
        if let Some(stem) = name.strip_suffix(".err") {
            let out = p.with_file_name(format!("{stem}.out"));
            let text = fs::read_to_string(p).unwrap();
            let last = text.lines().last().unwrap_or("");
            let Ok(Value::Object(id)) = serde_json::from_str::<Value>(last) else {
                continue;
            };
            if out.exists() && id.len() == 2 && id.contains_key("nsURI") {
                pins.push((out, id["digest"].as_str().unwrap().into(), at));
            }
        } else if let Some(stem) = name.strip_suffix(".id") {
            let id: Value = serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap();
            let golden = p.with_file_name(format!("{stem}.metamodel.json"));
            pins.push((golden, id["digest"].as_str().unwrap().into(), at));
        } else if name == "identity.tsv" {
            for row in fs::read_to_string(p).unwrap().lines() {
                let cols: Vec<&str> = row.split('\t').collect();
                let stem = Path::new(cols[0]).file_stem().unwrap().to_string_lossy();
                let golden = p.with_file_name(format!("{stem}.json"));
                pins.push((golden, cols[2].into(), format!("{at}: {}", cols[0])));
            }
        }
    }
    // 83 pins at registration; fewer means the walk lost a form.
    assert!(pins.len() >= 83, "only {} identity pins found", pins.len());
    let wrong: Vec<String> = pins
        .iter()
        .filter_map(|(golden, pinned, at)| {
            let got = digest_of(golden);
            (got != *pinned).then(|| format!("{at}: pins {pinned}, its golden digests to {got}"))
        })
        .collect();
    assert!(
        wrong.is_empty(),
        "{} of {} identity pins are not their golden's digest: {wrong:#?}",
        wrong.len(),
        pins.len()
    );
}
