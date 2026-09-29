//! AR.01 (`cea-cdrt-knowledge/spec/items/AR.01.md`), oracle A1: the crate Arachne generates for
//! `st05rec.ecore` (`Holder.bag` holds `Dir`, `Dir.sub` holds `Mid`, `Mid.sub` holds `Mid` and
//! `Mid.dirs` holds `Dir`, all keyed, neither `Dir` nor `Mid` with a subclass: the direct and the
//! indirect cycle) builds, and a model of it written through both cycles reads back what was
//! written, on the writer and on a second replica that delivered the writes.
//!
//! `generated_crates.rs` generates the crate `st05rec` from `st05rec.ecore` (sha256 in the spec,
//! the metamodel of ST.05's OB4 byte for byte) with the generator at this checkout, against the
//! model-plane Moirai checkout, and copies this file into the crate's `tests/`. At Arachne
//! `cc2e3f1` the crate does not build: E0391, a cycle computing the layout of `<UWMapLog<String,
//! MidLog> as IsLog>::Op` (ST.05, R3.10).
//!
//! The operations are written in their serde form, the form in which they travel between nodes, and
//! deserialized into the crate's operation type; the read is compared in its serde form. A
//! `BoxedLog` wraps its operation and its read in a `Box`, which serde writes as the value it
//! holds, so the same texts hold whichever keyed collections the generator chooses to box.
use moirai_crdt::utils::membership::twins_log;
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::IsReplica;
use serde_json::{Value, json};
use st05rec::package::{St05rec as GenOp, St05recLog as GenLog, St05recValue as GenValue};

/// The generated operation whose serde form is `spelled`.
fn op(spelled: &Value) -> GenOp {
    let generated: GenOp = serde_json::from_value(spelled.clone()).unwrap_or_else(|why| {
        panic!("AR01 claim: the generated runtime has no operation {spelled}: {why}")
    });
    let back = serde_json::to_value(&generated).expect("a generated operation serializes");
    assert_eq!(
        &back, spelled,
        "AR01 claim: the generated operation {spelled} serializes as {back}"
    );
    generated
}

/// `k` +`by` on the object at the end of `path`, a list of (keyed feature, key) steps from
/// `Holder`, spelled as the generated operation type's serde form: `{"Holder": {"Bag":
/// {"Update": ["a", {"Sub": {"Update": ["b", {"K": {"Inc": 1}}]}}]}}}`.
fn inc_at(path: &[(&str, &str)], by: i64) -> Value {
    let mut spelled = json!({"K": {"Inc": by}});
    for (feature, key) in path.iter().rev() {
        spelled = json!({ *feature: {"Update": [key, spelled]} });
    }
    json!({ "Holder": spelled })
}

#[test]
fn ar01_st05rec_a_model_written_through_both_cycles_reads_back() {
    let writes = [
        inc_at(&[("Bag", "a"), ("Sub", "b"), ("Sub", "c")], 1),
        inc_at(&[("Bag", "a"), ("Sub", "b"), ("Dirs", "d")], 2),
        inc_at(
            &[
                ("Bag", "a"),
                ("Sub", "b"),
                ("Dirs", "d"),
                ("Sub", "e"),
                ("Sub", "f"),
            ],
            3,
        ),
    ];
    let want = json!({
        "tick": 0,
        "bag": {
            "a": {
                "k": 0,
                "sub": {
                    "b": {
                        "k": 0,
                        "sub": {"c": {"k": 1, "sub": {}, "dirs": {}}},
                        "dirs": {
                            "d": {
                                "k": 2,
                                "sub": {
                                    "e": {
                                        "k": 0,
                                        "sub": {"f": {"k": 3, "sub": {}, "dirs": {}}},
                                        "dirs": {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });
    let (mut a, mut b) = twins_log::<GenLog>();
    for spelled in &writes {
        let event = a.send(op(spelled)).unwrap_or_else(|why| {
            panic!("AR01 claim: the generated replica refused {spelled}: {why}")
        });
        b.receive(event);
    }
    for (name, replica) in [("the writer", &a), ("the second replica", &b)] {
        let read = serde_json::to_value(replica.query(&Read::<GenValue>::new()))
            .expect("the generated read serializes");
        let holder = read.get("holder").cloned().unwrap_or(Value::Null);
        assert_eq!(
            holder, want,
            "AR01 claim: {name} reads {read}, whose `holder` is not what was written, {want}"
        );
    }
    println!(
        "AR01-A1 st05rec: {} writes read back on two replicas",
        writes.len()
    );
}
