use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

use serde_json::Value;

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(content: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "arachne-json-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("model.ecore"), content).unwrap();
        Self(path)
    }
    fn run(&self, stage: &str) -> (bool, Vec<Value>) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arachne"));
        command
            .args([stage, "--diagnostics-json"])
            .arg(self.0.join("model.ecore"));
        if stage == "parse" {
            command.arg("--quiet");
        } else {
            command.arg("-o").arg(self.0.join("generated"));
        }
        // Machine diagnostics are complete independently of human log filters.
        command.env("RUST_LOG", "off");
        let output = command.output().unwrap();
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let events = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).expect("every stdout line is JSON"))
            .collect();
        (output.status.success(), events)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

const MODEL: &str = r##"<ecore:EPackage xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" name="test">
    <eClassifiers xsi:type="ecore:EClass" name="Root">
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="omitted" derived="true"
            eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
        <eStructuralFeatures xsi:type="ecore:EReference" name="link" eType="#//Root" eOpposite="#//Root/link"/>
    </eClassifiers>
</ecore:EPackage>"##;

#[test]
fn quiet_parse_preserves_warnings_without_pretty_output() {
    let fixture = Fixture::new(MODEL);
    let (ok, events) = fixture.run("parse");
    assert!(ok);
    assert!(events.iter().any(|event| event["level"] == "warning"
        && event["message"].as_str().unwrap().contains("eOpposite")));
    let result = events.last().unwrap();
    assert_eq!(result["reason"], "arachne-result");
    assert_eq!(result["stage"], "parse");
    assert_eq!(result["ok"], true);
    assert!(!events.iter().any(|event| {
        event["message"]
            .as_str()
            .is_some_and(|message| message.starts_with("| "))
    }));
}

#[test]
fn generation_reports_stable_warning_codes() {
    let fixture = Fixture::new(MODEL);
    let (ok, events) = fixture.run("generate");
    assert!(ok, "{events:?}");
    assert!(
        events
            .iter()
            .any(|event| event["code"] == "arachne_codegen::warning::feature_omitted")
    );
    assert_eq!(events.last().unwrap()["ok"], true);
    assert!(fixture.0.join("generated/Cargo.toml").is_file());
}

#[test]
fn invalid_xml_has_a_structured_failure_result() {
    let fixture = Fixture::new("<bad>");
    let (ok, events) = fixture.run("parse");
    assert!(!ok);
    let result = events.last().unwrap();
    assert_eq!(result["ok"], false);
    assert_eq!(result["error"]["code"], "parse_error");
    assert!(
        result["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Failed to parse XML")
    );
}

#[test]
fn file_io_failure_is_an_infrastructure_error() {
    let fixture = Fixture::new(MODEL);
    fs::remove_file(fixture.0.join("model.ecore")).unwrap();
    let (ok, events) = fixture.run("parse");
    assert!(!ok);
    assert_eq!(events.last().unwrap()["error"]["kind"], "infrastructure");
    assert_eq!(events.last().unwrap()["error"]["code"], "io");
}
