//! Shared MD.08 oracle helpers. No implementation of resource resolution.
#![allow(dead_code)]
use arachne_codegen::{Config, generate_with_report};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

pub fn fixtures() -> PathBuf {
    std::env::var_os("MD08_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/md08"))
}
pub fn fixture(name: &str) -> Value {
    serde_json::from_slice(&fs::read(fixtures().join(name)).unwrap()).unwrap()
}
pub fn generate(file: &str, label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let scratch =
        Path::new("/volatile/home/vp242103/.cache/cea-crdt-scratch/MD.08/runs").join(format!(
            "{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
    fs::create_dir_all(&scratch).unwrap();
    let moirai = fixtures()
        .ancestors()
        .nth(5)
        .unwrap()
        .join("moirai-model-plane");
    let config = Config::new(fixtures().join(file))
        .with_output_dir(&scratch)
        .with_project_name("md08_fixture")
        .with_moirai_root(moirai);
    generate_with_report(config).unwrap_or_else(|e| {
        panic!("MD08 {label}: generating {file} must resolve its resource references: {e:#}")
    });
    scratch
}
pub fn read(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
pub fn generated_vehicle(label: &str) -> Value {
    read(generate("Vehicle.ecore", label).join("metamodel.json"))
}
pub fn strip(mut d: Value) -> Value {
    d.as_object_mut().unwrap().remove("dependsOn");
    for c in d["classes"].as_object_mut().unwrap().values_mut() {
        c.as_object_mut().unwrap().remove("origin");
    }
    d
}
pub fn assert_tables(d: &Value) {
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
    // Independent name-based comparison: target slots differ between tables.
    let d2 = fixture("Components.base.json");
    for name in ["Part", "Engine", "Controller", "Device"] {
        let mut expected = d2["classes"][name].clone();
        expected["superTypes"] = Value::Array(
            expected["superTypes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| Value::String(format!("Components::{}", v.as_str().unwrap())))
                .collect(),
        );
        for field in ["containments", "references"] {
            for f in expected[field].as_array_mut().unwrap() {
                for key in ["target"] {
                    if let Some(v) = f.get(key).and_then(Value::as_str).map(str::to_owned) {
                        f[key] = Value::String(format!("Components::{v}"));
                    }
                    if let Some(v) = f["merge"]
                        .get(key)
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                    {
                        f["merge"][key] = Value::String(format!("Components::{v}"));
                    }
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
