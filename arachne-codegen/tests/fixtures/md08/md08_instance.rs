//! Independent in-process component gate, built by the scratch manifest in the spec.
#[path = "../../md08_common.rs"]
mod common;
use common::*;
use moirai_interp::{InstanceOp, LeafOp, ModelLog, ModelOp};
use moirai_protocol::{
    broadcast::tcsb::Tcsb,
    log_id::LogId,
    replica::{IsReplica, Replica},
};
use moirai_semantics::{FeatureSlot, from_descriptor, metamodel_digest};
use serde_json::Value;
type Rep = Replica<ModelLog, Tcsb<ModelOp>>;
const MODEL: &str = "0123456789abcdef0123456789abcdef_Vehicle";
const FOREIGN: &str = "fedcba9876543210fedcba9876543210_Components#dave@17.2";
fn run(d: Value) {
    let sem = from_descriptor(&d).expect("MD08 G5: D1 is self-contained");
    let car = sem.classes.iter().find(|c| &*c.name == "Car").unwrap();
    let field = |name: &str| {
        FeatureSlot(
            car.visible
                .iter()
                .position(|(n, _, _)| &**n == name)
                .unwrap() as u16,
        )
    };
    for reverse in [false, true] {
        let ids = ["alice", "bob", "carol"];
        let mut rs: Vec<Rep> = ids
            .iter()
            .map(|id| {
                Replica::bootstrap_with_log_id((*id).into(), &ids, LogId::parse(MODEL).unwrap())
            })
            .collect();
        let text = d.to_string();
        let install = ModelOp::Install {
            model_id: MODEL.into(),
            metamodel_id: metamodel_digest(&d),
            descriptor: text.clone(),
        };
        let wire = serde_json::to_value(&install).unwrap();
        assert_eq!(wire["Install"]["descriptor"], text);
        assert_eq!(
            wire["Install"].as_object().unwrap().len(),
            3,
            "MD08 G5: no dependency payload in Install"
        );
        let open = rs[0].send(install).expect("MD08 G5: Install D1 without D2");
        rs[1].receive(open.clone());
        rs[2].receive(open);
        let root = rs[0]
            .send(ModelOp::Instance(InstanceOp::variant(
                car.slot,
                InstanceOp::New,
            )))
            .unwrap();
        rs[1].receive(root.clone());
        rs[2].receive(root);
        // Carol now receives nothing until both writers have edited.
        let label = rs[0]
            .send(ModelOp::Instance(InstanceOp::variant(
                car.slot,
                InstanceOp::field(
                    field("label"),
                    InstanceOp::Leaf(LeafOp::InsertChar { pos: 0, ch: 'A' }),
                ),
            )))
            .expect("MD08 G5: inherited Part.label edits work");
        let engine = rs[1]
            .send(ModelOp::Instance(InstanceOp::variant(
                car.slot,
                InstanceOp::field(
                    field("engine"),
                    InstanceOp::Leaf(LeafOp::Link(FOREIGN.into())),
                ),
            )))
            .expect("MD08 G5: foreign engine link needs no Components Install");
        rs[0].receive(engine.clone());
        rs[1].receive(label.clone());
        if reverse {
            rs[2].receive(engine);
            rs[2].receive(label);
        } else {
            rs[2].receive(label);
            rs[2].receive(engine);
        }
        let reads: Vec<_> = rs
            .iter()
            .map(|r| {
                let log = r.state();
                assert_eq!(
                    log.opened_descriptor(),
                    Some(text.as_str()),
                    "MD08 G5: exactly D1 installed"
                );
                assert_eq!(log.semantics().unwrap().ns_uri.as_ref(), "urn:md08:Vehicle");
                moirai_interp::eval::read_identified(log.semantics().unwrap(), log.root())
            })
            .collect();
        assert_eq!(reads[0], reads[1]);
        assert_eq!(reads[1], reads[2]);
        fn has(v: &Value, key: &str, expected: &str) -> bool {
            match v {
                Value::Object(m) => {
                    m.get(key).and_then(Value::as_str) == Some(expected)
                        || m.values().any(|v| has(v, key, expected))
                }
                Value::Array(a) => a.iter().any(|v| has(v, key, expected)),
                _ => false,
            }
        }
        assert!(
            has(&reads[0], "label", "A"),
            "MD08 G5: inherited read: {}",
            reads[0]
        );
        assert!(
            has(&reads[0], "engine", FOREIGN),
            "MD08 G5: exact foreign read: {}",
            reads[0]
        );
        // A separate model may explicitly install D2; no dependency installation was needed above.
        let mut other: Rep = Replica::bootstrap_with_log_id(
            "dave".into(),
            &["dave"],
            LogId::parse("fedcba9876543210fedcba9876543210_Components").unwrap(),
        );
        let d2 = fixture("Components.base.json");
        other
            .send(ModelOp::Install {
                model_id: "fedcba9876543210fedcba9876543210_Components".into(),
                metamodel_id: metamodel_digest(&d2),
                descriptor: d2.to_string(),
            })
            .unwrap();
        assert_eq!(
            other.state().semantics().unwrap().ns_uri.as_ref(),
            "urn:md08:Components"
        );
        assert_eq!(rs[0].state().opened_descriptor(), Some(text.as_str()));
    }
}
#[test]
fn g5_generated_d1_installs_alone_and_accepts_edits() {
    run(generated_vehicle("G5"));
}
#[test]
fn g5_frozen_d1_installs_alone_and_accepts_edits() {
    run(fixture("Vehicle.expected.json"));
}
