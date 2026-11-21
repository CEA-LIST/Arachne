//! EC.M1j2: three follow-ups of EC.M1j's describe of the package tree
//! (spec: cea-cdrt-knowledge/spec/items/EC.M1j2.md).
//!
//! (1) Names are relative to the package `descriptor_json` describes, not to the first user package
//! of the context. (2) The subpackage refusals (no nsURI, a shared nsURI, a first-level subpackage
//! named `ecore`) answer before any refusal of the class loop, so the line says `subpackage`.
//! (3) An in-document `subsets` or `redefines` fragment becomes a `::` path only when its leading
//! segments name subpackages; otherwise the class is written as 18ccade wrote it.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use arachne_codegen::{EcoreParser, descriptor_json, find_user_package};
use serde_json::{Value, json};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ecm1j2")
        .join(rel)
}

fn describe(rel: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arachne"))
        .arg("describe")
        .arg(fixture(rel))
        .output()
        .expect("run arachne describe")
}

/// Stderr lines with ANSI colour codes removed.
fn err_lines(out: &Output) -> Vec<String> {
    let text = String::from_utf8_lossy(&out.stderr);
    let mut plain = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c == 'm' {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    plain.lines().map(str::to_owned).collect()
}

fn parse(rel: &str) -> EcoreParser {
    EcoreParser::from_file(fixture(rel)).unwrap_or_else(|e| panic!("parse {rel}: {e:?}"))
}

fn keys(d: &Value) -> Vec<&str> {
    d["classes"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn a_described_subpackage_names_its_classifiers_from_itself() {
    let p = parse("nested.ecore");
    // Guard: the user package keeps EC.M1j's names.
    let whole = descriptor_json(&p.ctx, find_user_package(&p.ctx).unwrap()).unwrap();
    assert_eq!(keys(&whole), ["R", "a::A", "a::b::B"], "the whole tree");

    let a = p
        .ctx
        .packs()
        .iter()
        .find(|pack| pack.name() == "a")
        .expect("subpackage a");
    let d = descriptor_json(&p.ctx, a).unwrap_or_else(|e| panic!("describe a: {e:?}"));
    assert_eq!(
        keys(&d),
        ["A", "b::B"],
        "a's classes, by their path below a"
    );
    assert_eq!(d["rootClasses"], json!(["A"]));
    assert_eq!(
        (&d["package"], &d["nsURI"]),
        (&json!("a"), &json!("urn:r/a"))
    );
    assert_eq!(
        d["packages"],
        json!({"b": {"nsURI": "urn:r/a/b", "nsPrefix": "b"}})
    );
    assert!(
        d.to_string().contains("\"b::B\"") && !d.to_string().contains("a::"),
        "a target names b::B, nothing is named from r: {d}"
    );
}

#[test]
fn a_class_outside_the_described_package_is_refused() {
    let p = parse("outside.ecore");
    let a = p
        .ctx
        .packs()
        .iter()
        .find(|pack| pack.name() == "a")
        .expect("subpackage a");
    // `A` extends the root package's `X`, which `a` does not hold: refused, never written bare.
    let refused = descriptor_json(&p.ctx, a);
    assert!(refused.is_err(), "describing a wrote {refused:?}");
}

/// Refused: nothing on stdout, and one stderr line, not a parser position line (`at ...`), says
/// `subpackage` (any case) and holds every token.
fn refused_with(rel: &str, tokens: &[&str]) {
    let out = describe(rel);
    let lines = err_lines(&out);
    assert!(
        !out.status.success() && out.stdout.is_empty(),
        "{rel} should be refused: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| !l.trim_start().starts_with("at `")
            && l.to_lowercase().contains("subpackage")
            && tokens.iter().all(|t| l.contains(t))),
        "{rel}: no line saying `subpackage` with {tokens:?}: {lines:?}"
    );
}

#[test]
fn a_subpackage_named_ecore_refuses_before_its_classes_collide() {
    // `ecore::EAnnotation` is both the subpackage's class and Ecore's own.
    refused_with("ecore_collides.ecore", &["ecore"]);
}

#[test]
fn the_nsuri_refusals_answer_before_the_class_loop() {
    // `Root.entries` is keyed by an enum, which the class loop refuses.
    refused_with("nouri_enum_key.ecore", &["nsURI", "nouri"]);
    refused_with(
        "shared_enum_key.ecore",
        &["nsURI", "http://example.org/ecm1j2/palette"],
    );
}

/// Every `subsets` array of the descriptor.
fn subsets(v: &Value, found: &mut Vec<Value>) {
    match v {
        Value::Object(map) => {
            for (k, v) in map {
                if k == "subsets" {
                    found.push(v.clone());
                } else {
                    subsets(v, found);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|v| subsets(v, found)),
        _ => {}
    }
}

#[test]
fn a_fragment_whose_segments_name_no_subpackage_keeps_18ccade_class() {
    let out = describe("fragment.ecore");
    assert!(out.status.success(), "{:?}", err_lines(&out));
    let d: Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut found = Vec::new();
    subsets(&d, &mut found);
    assert_eq!(
        found,
        [json!([{"class": "op", "feature": "p"}])],
        "#//C/op/p"
    );
    assert!(d.get("packages").is_none());
}
