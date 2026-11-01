//! MD.08 validation-first oracle. See vault spec/items/MD.08.md.
#[path = "md08_common.rs"]
mod common;
use arachne_codegen::{EcoreParser, descriptor_json, find_user_package, metamodel_digest};
use common::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;

#[test]
fn g1_dependency_output_is_standalone_bytes() {
    let out = generate("Vehicle.ecore", "G1");
    let standalone = generate("Components.ecore", "G1-alone");
    let d2 = fixture("Components.base.json");
    let path = out
        .join("dependencies")
        .join(format!("{}.metamodel.json", metamodel_digest(&d2)));
    let bytes = fs::read(path).expect("MD08 G1: D2 must be generated beside D1");
    assert_eq!(bytes, fs::read(standalone.join("metamodel.json")).unwrap());
    assert_eq!(
        bytes,
        fs::read(fixtures().join("Components.base.json")).unwrap()
    );
    assert_eq!(fs::read_dir(out.join("dependencies")).unwrap().count(), 1);
}
#[test]
fn g2_origins_closure_and_dependency_digest() {
    let d1 = generated_vehicle("G2");
    assert_eq!(
        d1,
        fixture("Vehicle.expected.json"),
        "MD08 G2: exact independent closure and metadata"
    );
    assert_eq!(
        d1["dependsOn"],
        json!([{"nsURI":"urn:md08:Components", "digest":metamodel_digest(&fixture("Components.base.json"))}])
    );
    let mut changed = d1.clone();
    changed["classes"]["Components::Part"]["origin"] = json!("urn:other");
    assert_ne!(metamodel_digest(&changed), metamodel_digest(&d1));
    changed = d1.clone();
    changed["dependsOn"][0]["digest"] = json!("0".repeat(64));
    assert_ne!(metamodel_digest(&changed), metamodel_digest(&d1));
}
#[test]
fn g3_no_dependency_bytes_and_shipped_digests() {
    let out = generate("Components.ecore", "G3");
    let bytes = fs::read(out.join("metamodel.json")).unwrap();
    assert_eq!(
        bytes,
        fs::read(fixtures().join("Components.base.json")).unwrap(),
        "MD08 G3: no-dependency bytes changed"
    );
    assert!(read(out.join("metamodel.json")).get("dependsOn").is_none());
    let repo = fixtures().ancestors().nth(4).unwrap().to_owned();
    for row in fixture("shipped-hashes.json").as_array().unwrap() {
        let path = repo.join(row["path"].as_str().unwrap());
        let bytes = fs::read(path).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            row["sha256"].as_str().unwrap()
        );
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(metamodel_digest(&value), row["digest"].as_str().unwrap());
    }
}
#[test]
fn g4_generated_descriptor_metadata_does_not_change_rules() {
    assert_tables(&generated_vehicle("G4"));
}
#[test]
fn g4_isolated_metadata_acceptance_and_rules() {
    assert_tables(&fixture("Vehicle.expected.json"));
}
#[test]
fn g6_existing_behavior_and_fixture_sanity() {
    let p = EcoreParser::from_file(fixtures().join("Components.ecore")).unwrap();
    let d = descriptor_json(&p.ctx, find_user_package(&p.ctx).unwrap()).unwrap();
    assert_eq!(d, fixture("Components.base.json"));
    assert!(moirai_semantics::from_descriptor(&strip(fixture("Vehicle.expected.json"))).is_ok());
    let mut bad = strip(fixture("Vehicle.expected.json"));
    bad["classes"]["Car"]["superTypes"] = json!(["Absent"]);
    assert!(
        moirai_semantics::from_descriptor(&bad).is_err(),
        "MD08 G6: unknown classes still rejected"
    );
    bad = strip(fixture("Vehicle.expected.json"));
    bad["classes"]["Car"]["references"][0]
        .as_object_mut()
        .unwrap()
        .remove("merge");
    assert!(
        moirai_semantics::from_descriptor(&bad).is_err(),
        "MD08 G6: missing merge rules still rejected"
    );
}
