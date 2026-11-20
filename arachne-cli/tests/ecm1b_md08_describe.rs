//! EC.M1b (`cea-cdrt-knowledge/spec/items/EC.M1b.md`), O7: MD.08's model dependencies reach
//! `arachne describe`. MD.08's oracle (`spec/items/MD.08.md`, G1 to G6, formerly
//! `arachne-codegen/tests/md08_dependencies.rs`) went through `generate`, which is now Léo's
//! generator; the same assertions now go through the CLI's `describe`, with its fixtures.
//! `describe FILE -o OUT` writes the descriptor to `OUT` and each dependency's descriptor to
//! `dependencies/<digest>.metamodel.json` beside it, as `generate` wrote them into its project.
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-cli --test ecm1b_md08_describe
//! ```
use arachne_codegen::{EcoreParser, descriptor_json, find_user_package, metamodel_digest};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../arachne-codegen/tests/fixtures/md08")
}
fn fixture(name: &str) -> Value {
    serde_json::from_slice(&fs::read(fixtures().join(name)).unwrap()).unwrap()
}
fn read(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
/// A fresh folder under the target's tmp dir.
fn scratch(label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "ecm1b-md08-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}
/// `arachne describe <fixture> -o <dir>/metamodel.json`; the folder it wrote into.
fn describe(file: &str, label: &str) -> PathBuf {
    let dir = scratch(label);
    let out = Command::new(env!("CARGO_BIN_EXE_arachne"))
        .arg("describe")
        .arg(fixtures().join(file))
        .arg("-o")
        .arg(dir.join("metamodel.json"))
        .output()
        .expect("run arachne describe");
    assert!(
        out.status.success(),
        "MD08 {label}: describing {file} must resolve its resource references: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}
fn described_vehicle(label: &str) -> Value {
    read(describe("Vehicle.ecore", label).join("metamodel.json"))
}
fn strip(mut d: Value) -> Value {
    d.as_object_mut().unwrap().remove("dependsOn");
    for c in d["classes"].as_object_mut().unwrap().values_mut() {
        c.as_object_mut().unwrap().remove("origin");
    }
    d
}
/// MD.08's G4, unchanged: the metadata changes no class, enum, root or rule, and the digest covers it.
fn assert_tables(d: &Value) {
    let actual = moirai_semantics::from_descriptor(d).expect("MD08 G4: metadata accepted");
    let plain = moirai_semantics::from_descriptor(&strip(d.clone())).unwrap();
    assert_eq!(
        actual.classes, plain.classes,
        "MD08 G4: metadata changed classes or rules"
    );
    assert_eq!(actual.enums, plain.enums, "MD08 G4: metadata changed enums");
    assert_eq!(actual.roots, plain.roots, "MD08 G4: metadata changed roots");
    assert_ne!(
        actual.digest, plain.digest,
        "MD08 G4: digest must cover metadata"
    );
    let d2 = fixture("Components.base.json");
    for name in ["Part", "Engine", "Controller", "Device"] {
        let mut expected = d2["classes"][name].clone();
        // EC.05 (`spec/items/EC.05.md`): `superTypeOrder` names supertypes too.
        for key in ["superTypes", "superTypeOrder"] {
            expected[key] = Value::Array(
                expected[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| Value::String(format!("Components::{}", v.as_str().unwrap())))
                    .collect(),
            );
        }
        for field in ["containments", "references"] {
            for f in expected[field].as_array_mut().unwrap() {
                if let Some(v) = f.get("target").and_then(Value::as_str).map(str::to_owned) {
                    f["target"] = Value::String(format!("Components::{v}"));
                }
                if let Some(v) = f["merge"]
                    .get("target")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                {
                    f["merge"]["target"] = Value::String(format!("Components::{v}"));
                }
            }
        }
        let mut copied = d["classes"][format!("Components::{name}")].clone();
        copied
            .as_object_mut()
            .expect("copied class")
            .remove("origin");
        assert_eq!(
            copied, expected,
            "MD08 G4: copied declaration differs from D2: {name}"
        );
    }
}

#[test]
fn g1_dependency_output_is_standalone_bytes() {
    let out = describe("Vehicle.ecore", "G1");
    let standalone = describe("Components.ecore", "G1-alone");
    let d2 = fixture("Components.base.json");
    let path = out
        .join("dependencies")
        .join(format!("{}.metamodel.json", metamodel_digest(&d2)));
    let bytes = fs::read(path).expect("MD08 G1: D2 must be written beside D1");
    assert_eq!(bytes, fs::read(standalone.join("metamodel.json")).unwrap());
    assert_eq!(
        bytes,
        fs::read(fixtures().join("Components.base.json")).unwrap()
    );
    assert_eq!(fs::read_dir(out.join("dependencies")).unwrap().count(), 1);
}
#[test]
fn g2_origins_closure_and_dependency_digest() {
    let d1 = described_vehicle("G2");
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
    let out = describe("Components.ecore", "G3");
    let bytes = fs::read(out.join("metamodel.json")).unwrap();
    assert_eq!(
        bytes,
        fs::read(fixtures().join("Components.base.json")).unwrap(),
        "MD08 G3: no-dependency bytes changed"
    );
    assert!(read(out.join("metamodel.json")).get("dependsOn").is_none());
    assert!(
        !out.join("dependencies").exists(),
        "MD08 G3: no dependency, no dependencies/"
    );
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
    assert_tables(&described_vehicle("G4"));
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
/// EC.M1b: on stdout, `describe` prints D1 and its identity line and writes no file.
#[test]
fn ecm1b_stdout_carries_d1_and_writes_nothing() {
    let cwd = scratch("stdout");
    let out = Command::new(env!("CARGO_BIN_EXE_arachne"))
        .current_dir(&cwd)
        .arg("describe")
        .arg(fixtures().join("Vehicle.ecore"))
        .output()
        .expect("run arachne describe");
    assert!(
        out.status.success(),
        "EC.M1b: describing Vehicle.ecore on stdout: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let d1: Value = serde_json::from_slice(&out.stdout).expect("stdout is a descriptor");
    assert_eq!(d1, fixture("Vehicle.expected.json"));
    let stderr = String::from_utf8_lossy(&out.stderr);
    let id: Value = serde_json::from_str(stderr.lines().last().unwrap_or("")).unwrap_or_default();
    assert_eq!(
        id,
        json!({"nsURI": d1["nsURI"], "digest": metamodel_digest(&d1)}),
        "EC.M1b: the identity line is D1's: {stderr}"
    );
    assert_eq!(
        fs::read_dir(&cwd).unwrap().count(),
        0,
        "EC.M1b: describe on stdout writes no file"
    );
}
