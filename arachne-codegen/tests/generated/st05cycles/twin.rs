//! AR.01's twin for ST.05's K8 and K9 (`cea-cdrt-knowledge/spec/items/AR.01.md`, oracle A3): the
//! runtime Arachne generates for `st05cycles.ecore` against the interpreted one, on every
//! schedule of K8 and K9 (`cea-cdrt-knowledge/spec/items/ST.05.md`, round 3, R3.2), whose
//! metamodel has record classes (classes with no subclass) that reach themselves through keyed
//! collections: `Mid.sub` holds `Mid` (the direct cycle), and `Box.sub` holds `Mid` while
//! `Mid.boxes` holds `Box` (the indirect one).
//!
//! `generated_crates.rs` generates the crate `st05cycles` from `st05cycles.ecore` with the
//! generator at this checkout, against the model-plane Moirai checkout, after holding the
//! descriptor the generator writes equal to the one K8 and K9 build in code
//! (`moirai-interp/tests/st05_recursive_keyed_class.rs`, `cycles_descriptor`) but for the
//! package's name and nsURI, and copies this file into the crate's `tests/`. The interpreted
//! side is `moirai_interp::ModelLog` on the descriptor written beside the crate
//! (`metamodel.json`).
//!
//! The generated side's operations are written here in their serde form, the form in which
//! operations travel between nodes, and deserialized into the crate's operation type. A
//! `BoxedLog` wraps its operation in a `Box`, which serde writes as the value it holds, so one
//! JSON text is the same operation whichever keyed collections the generator chooses to box;
//! the read is projected from its serde form for the same reason. An operation the crate's type
//! cannot take, or a read it does not serve, fails the claim, not the set-up.
//!
//! Each seat is a pair of replicas, one per runtime, fed the same operation in the same order.
//! At every checkpoint, once every operation sent has been delivered everywhere (after T, after
//! N1, after N2, after the removal or the race), each seat is read on both runtimes, the
//! generated read projected by [`project`], written by hand for this metamodel, and the two
//! must be equal; the generated seats must also serve the document the rule gives, written by
//! hand below from ST.05's R3.2, and one document. After every step once T has been delivered
//! everywhere, the three seats are compared too, and those differences are counted and printed,
//! not asserted: the claim is about quiescence, as ST.05's C4.
//!
//! The canonical form of the interpreted side is its default read, what
//! `GET /api/model/{id}/state` serves: `Replica::query(&Read::<Value>::new())`.
use std::sync::Arc;

use moirai_crdt::utils::membership::triplet_log;
use moirai_interp::testing::{class_slot, feature_slot, install};
use moirai_interp::{InstanceOp, LeafOp, ModelLog, ModelOp, Scalar};
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::Tcsb;
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::{IsReplica, Replica};
use moirai_semantics::{MetamodelSemantics, from_descriptor};
use serde_json::{Map, Value, json};
use st05cycles::package::{
    St05cycles as GenOp, St05cyclesLog as GenLog, St05cyclesValue as GenValue,
};

type Interp = Replica<ModelLog, Tcsb<ModelOp>>;
type Gen = Replica<GenLog, Tcsb<GenOp>>;

/// The descriptor Arachne wrote beside this crate.
const DESCRIPTOR: &str = include_str!("../metamodel.json");

fn descriptor() -> Value {
    serde_json::from_str(DESCRIPTOR).expect("set-up: metamodel.json is JSON")
}

// ---------------------------------------------------------------------------
// The projection of the generated read onto the canonical form, by hand
// ---------------------------------------------------------------------------

/// `St05cyclesValue` in the canonical form: `Holder` with `eClass`; `tick` a number; an
/// optional that is `None` an absent key; a record (`Box`, `Mid`) an object with its `eClass`
/// and every single-valued counter whatever it holds; `Box.note`, an optional counter, a number
/// or an absent key; a keyed collection an object of the keys the generated read serves, each
/// value projected by its class, `Mid`'s own `sub` and `boxes` included, to any depth; a
/// `union!` value its member directly, a conflict `{"__conflict": [...]}` ordered by class
/// name, nothing an absent key.
fn project(value: &GenValue) -> Value {
    let raw = serde_json::to_value(value).expect("the generated read serializes");
    let holder = field(&raw, "holder");
    let mut out = Map::new();
    out.insert("eClass".into(), json!("Holder"));
    out.insert("tick".into(), field(holder, "tick").clone());
    if !field(holder, "inner").is_null() {
        out.insert("inner".into(), box_of(&holder["inner"]));
    }
    out.insert(
        "bag".into(),
        keyed(field(holder, "bag"), |held| Some(box_of(held))),
    );
    out.insert("shapes".into(), keyed(field(holder, "shapes"), kind));
    out.insert("bins".into(), keyed(field(holder, "bins"), kind));
    Value::Object(out)
}

/// A field of a generated record's read, which must be there.
fn field<'a>(record: &'a Value, name: &str) -> &'a Value {
    record
        .get(name)
        .unwrap_or_else(|| panic!("AR01 claim: the generated read has no field `{name}`: {record}"))
}

/// A generated keyed collection, each value projected by `each`; a value `each` reads as
/// nothing is an absent key.
fn keyed(map: &Value, each: impl Fn(&Value) -> Option<Value>) -> Value {
    let mut out = Map::new();
    for (key, held) in map
        .as_object()
        .unwrap_or_else(|| panic!("AR01 claim: a keyed collection reads as a map: {map}"))
    {
        if let Some(value) = each(held) {
            out.insert(key.clone(), value);
        }
    }
    Value::Object(out)
}

/// One `BoxValue`, projected.
fn box_of(value: &Value) -> Value {
    let mut out = Map::new();
    out.insert("eClass".into(), json!("Box"));
    out.insert("k".into(), field(value, "k").clone());
    if !field(value, "note").is_null() {
        out.insert("note".into(), value["note"].clone());
    }
    if !field(value, "mid").is_null() {
        out.insert("mid".into(), mid_of(&value["mid"]));
    }
    out.insert(
        "sub".into(),
        keyed(field(value, "sub"), |held| Some(mid_of(held))),
    );
    Value::Object(out)
}

/// One `MidValue`, projected, its keyed collections to any depth.
fn mid_of(value: &Value) -> Value {
    json!({
        "eClass": "Mid",
        "k": field(value, "k").clone(),
        "sub": keyed(field(value, "sub"), |held| Some(mid_of(held))),
        "boxes": keyed(field(value, "boxes"), |held| Some(box_of(held))),
    })
}

/// One `union!` value (`ShapeKindValue`, `BinKindValue`), projected.
fn kind(value: &Value) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::String(word) if word == "Unset" => None,
        Value::Object(state) if state.len() == 1 => {
            let (tag, payload) = state.iter().next().expect("one entry");
            match tag.as_str() {
                "Value" => Some(member(payload)),
                "Conflict" => {
                    let mut held: Vec<(String, Vec<u8>, Value)> = payload
                        .as_array()
                        .unwrap_or_else(|| {
                            panic!("AR01 claim: a `Conflict` holds an array: {value}")
                        })
                        .iter()
                        .map(|child| {
                            let projected = member(child);
                            let class = projected["eClass"].as_str().unwrap_or("").to_string();
                            let bytes = serde_json::to_vec(&projected).unwrap_or_default();
                            (class, bytes, projected)
                        })
                        .collect();
                    match held.len() {
                        0 => None,
                        1 => Some(held.pop().expect("one").2),
                        _ => {
                            held.sort_by(|l, r| l.0.cmp(&r.0).then_with(|| l.1.cmp(&r.1)));
                            Some(
                                json!({"__conflict": held.into_iter().map(|h| h.2).collect::<Vec<_>>()}),
                            )
                        }
                    }
                }
                other => panic!("AR01 claim: `{other}` is not a `union!` state: {value}"),
            }
        }
        other => panic!("AR01 claim: `{other}` is not a `union!` value"),
    }
}

/// One member of a `union!`: `{"A": {"shape_super": {"n": 1, "parts": {}}}}` as
/// `{"eClass": "A", "n": 1, "parts": {}}`, `{"Bin": {"k": 3}}` as `{"eClass": "Bin", "k": 3}`,
/// `{"BigBin": {"bin_super": {"k": 3}}}` as `{"eClass": "BigBin", "k": 3}`.
fn member(child: &Value) -> Value {
    let (class, payload) = child
        .as_object()
        .and_then(|variant| variant.iter().next())
        .unwrap_or_else(|| panic!("AR01 claim: a `union!` child is its variant: {child}"));
    match class.as_str() {
        "A" | "B" => {
            let shape = field(payload, "shape_super");
            json!({
                "eClass": class,
                "n": field(shape, "n").clone(),
                "parts": keyed(field(shape, "parts"), |held| Some(mid_of(held))),
            })
        }
        "Bin" => json!({"eClass": "Bin", "k": field(payload, "k").clone()}),
        "BigBin" => {
            json!({"eClass": "BigBin", "k": field(field(payload, "bin_super"), "k").clone()})
        }
        other => panic!("AR01 claim: `{other}` is not a class of st05cycles.ecore: {child}"),
    }
}

/// The projection, held to hand-written generated values: one deep inside both cycles, with an
/// optional, a conflict and a subclass, and the empty one.
#[test]
fn ar01_k8_k9_the_projection_reads_a_hand_written_generated_value() {
    let raw = json!({"holder": {
        "tick": 1,
        "inner": {"k": 2, "note": null, "mid": {"k": 3, "sub": {}, "boxes": {}}, "sub": {}},
        "bag": {"k": {"k": 0, "note": 4, "mid": null, "sub": {
            "j": {"k": 5, "sub": {"i": {"k": 6, "sub": {}, "boxes": {}}},
                  "boxes": {"d": {"k": 7, "note": null, "mid": null, "sub": {
                      "e": {"k": 8, "sub": {}, "boxes": {}}}}}}}}},
        "shapes": {
            "k": {"Conflict": [{"B": {"shape_super": {"n": 0, "parts": {}}}},
                               {"A": {"shape_super": {"n": 10, "parts": {
                                   "p": {"k": 1, "sub": {"q": {"k": 2, "sub": {}, "boxes": {}}},
                                         "boxes": {}}}}}}]},
            "u": "Unset"
        },
        "bins": {"k": {"Value": {"BigBin": {"bin_super": {"k": 7}}}}, "v": {"Value": {"Bin": {"k": 0}}}}
    }});
    let value: GenValue =
        serde_json::from_value(raw).expect("set-up: the hand-written value parses");
    let mid = |k: i64, sub: Value, boxes: Value| json!({"eClass": "Mid", "k": k, "sub": sub, "boxes": boxes});
    assert_eq!(
        project(&value),
        json!({
            "eClass": "Holder", "tick": 1,
            "inner": {"eClass": "Box", "k": 2, "mid": mid(3, json!({}), json!({})), "sub": {}},
            "bag": {"k": {"eClass": "Box", "k": 0, "note": 4, "sub": {
                "j": mid(5, json!({"i": mid(6, json!({}), json!({}))}), json!({
                    "d": {"eClass": "Box", "k": 7, "sub": {"e": mid(8, json!({}), json!({}))}}
                }))
            }}},
            "shapes": {"k": {"__conflict": [
                {"eClass": "A", "n": 10, "parts": {
                    "p": mid(1, json!({"q": mid(2, json!({}), json!({}))}), json!({}))}},
                {"eClass": "B", "n": 0, "parts": {}}
            ]}},
            "bins": {"k": {"eClass": "BigBin", "k": 7}, "v": {"eClass": "Bin", "k": 0}}
        })
    );
    let empty: GenValue = serde_json::from_value(json!({"holder": {
        "tick": 0, "inner": null, "bag": {}, "shapes": {}, "bins": {}
    }}))
    .expect("set-up: parses");
    assert_eq!(
        project(&empty),
        json!({"eClass": "Holder", "tick": 0, "bag": {}, "shapes": {}, "bins": {}})
    );
}

// ---------------------------------------------------------------------------
// The operations, on both runtimes
// ---------------------------------------------------------------------------

/// One operation, as each runtime spells it, and the generated one's serde form.
#[derive(Clone, Debug)]
struct Op {
    interp: ModelOp,
    generated: GenOp,
    spelled: Value,
}

/// Where U removes a key, as in ST.05's K8: `Top`, key `k` of `Holder.bag`; `Direct`, key `i`
/// of the `sub` of the `Mid` at `j` (the direct cycle's own collection); `Indirect`, key `d` of
/// that `Mid`'s `boxes` (the collection that closes the indirect cycle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Top,
    Direct,
    Indirect,
}

const PLACES: [Place; 3] = [Place::Top, Place::Direct, Place::Indirect];

/// What W increments by 10, concurrently with U at `Top`, as in ST.05's K9: the `Mid` at `j`
/// (`Joint`), the `Mid` at `i` in its `sub` (`Inner`), the `Box` at `d` in its `boxes`
/// (`Back`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Joint,
    Inner,
    Back,
}

const TARGETS: [Target; 3] = [Target::Joint, Target::Inner, Target::Back];

struct Ops {
    sem: Arc<MetamodelSemantics>,
}

impl Ops {
    fn new(desc: &Value) -> Self {
        Ops {
            sem: Arc::new(from_descriptor(desc).expect("set-up: Arachne's descriptor parses")),
        }
    }

    fn is(&self, class: &str, inner: InstanceOp) -> InstanceOp {
        InstanceOp::variant(class_slot(&self.sem, class), inner)
    }

    fn at(&self, class: &str, feature: &str, inner: InstanceOp) -> InstanceOp {
        InstanceOp::field(
            feature_slot(&self.sem, class_slot(&self.sem, class), feature),
            inner,
        )
    }

    fn inc(by: i64) -> InstanceOp {
        InstanceOp::Leaf(LeafOp::Inc(Scalar::Int(by)))
    }

    /// The interpreted operation on the root, and the generated one from its serde form.
    fn op(&self, interp: InstanceOp, spelled: Value) -> Op {
        let generated: GenOp = serde_json::from_value(spelled.clone()).unwrap_or_else(|why| {
            panic!("AR01 claim: the generated runtime has no operation {spelled}: {why}")
        });
        let back = serde_json::to_value(&generated).expect("a generated operation serializes");
        assert_eq!(
            back, spelled,
            "AR01 claim: the generated operation {spelled} serializes as {back}"
        );
        Op {
            interp: self.is("Holder", interp).into_model_op(),
            generated,
            spelled,
        }
    }

    /// T: `Holder.tick` +1, the first write, which touches no keyed collection.
    fn tick(&self) -> Op {
        self.op(
            self.at("Holder", "tick", Self::inc(1)),
            json!({"Holder": {"Tick": {"Inc": 1}}}),
        )
    }

    /// `interp` / `spelled` on the `Box` at key `k` of `Holder.bag`.
    fn in_box(&self, interp: InstanceOp, spelled: Value) -> Op {
        self.op(
            self.at(
                "Holder",
                "bag",
                InstanceOp::entry(Scalar::text("k"), self.is("Box", interp)),
            ),
            json!({"Holder": {"Bag": {"Update": ["k", spelled]}}}),
        )
    }

    /// `interp` / `spelled` on the `Mid` at key `j` of that `Box`'s `sub`.
    fn in_mid(&self, interp: InstanceOp, spelled: Value) -> Op {
        self.in_box(
            self.at(
                "Box",
                "sub",
                InstanceOp::entry(Scalar::text("j"), self.is("Mid", interp)),
            ),
            json!({"Sub": {"Update": ["j", spelled]}}),
        )
    }

    /// `k` +`by` on the `Mid` at key `i` of that `Mid`'s own `sub`: N1, and W on `Inner`.
    fn inner(&self, by: i64) -> Op {
        self.in_mid(
            self.at(
                "Mid",
                "sub",
                InstanceOp::entry(
                    Scalar::text("i"),
                    self.is("Mid", self.at("Mid", "k", Self::inc(by))),
                ),
            ),
            json!({"Sub": {"Update": ["i", {"K": {"Inc": by}}]}}),
        )
    }

    /// `k` +`by` on the `Box` at key `d` of that `Mid`'s `boxes`: N2, and W on `Back`.
    fn back(&self, by: i64) -> Op {
        self.in_mid(
            self.at(
                "Mid",
                "boxes",
                InstanceOp::entry(
                    Scalar::text("d"),
                    self.is("Box", self.at("Box", "k", Self::inc(by))),
                ),
            ),
            json!({"Boxes": {"Update": ["d", {"K": {"Inc": by}}]}}),
        )
    }

    /// `k` +`by` on the `Mid` at key `j`: W on `Joint`.
    fn joint(&self, by: i64) -> Op {
        self.in_mid(
            self.at("Mid", "k", Self::inc(by)),
            json!({"K": {"Inc": by}}),
        )
    }

    /// U at a place.
    fn removal(&self, place: Place) -> Op {
        match place {
            Place::Top => self.op(
                self.at("Holder", "bag", InstanceOp::remove(Scalar::text("k"))),
                json!({"Holder": {"Bag": {"Remove": "k"}}}),
            ),
            Place::Direct => self.in_mid(
                self.at("Mid", "sub", InstanceOp::remove(Scalar::text("i"))),
                json!({"Sub": {"Remove": "i"}}),
            ),
            Place::Indirect => self.in_mid(
                self.at("Mid", "boxes", InstanceOp::remove(Scalar::text("d"))),
                json!({"Boxes": {"Remove": "d"}}),
            ),
        }
    }

    /// W, +10 on its target.
    fn write(&self, target: Target) -> Op {
        match target {
            Target::Joint => self.joint(10),
            Target::Inner => self.inner(10),
            Target::Back => self.back(10),
        }
    }
}

// ---------------------------------------------------------------------------
// The documents the rule gives, by hand (ST.05, round 3, R3.2)
// ---------------------------------------------------------------------------

/// A `Box`: `note` and `mid` never written, so absent.
fn boxed(k: i64, sub: Value) -> Value {
    json!({"eClass": "Box", "k": k, "sub": sub})
}

/// A `Mid` of this metamodel.
fn mid(k: i64, boxes: Value, sub: Value) -> Value {
    json!({"boxes": boxes, "eClass": "Mid", "k": k, "sub": sub})
}

/// The whole document: `tick` 1, `bag` holding `held` at key `k` or nothing, `inner` never
/// written, `shapes` and `bins` empty.
fn document(held: Option<Value>) -> Value {
    let bag = match held {
        Some(held) => json!({ "k": held }),
        None => json!({}),
    };
    json!({"bag": bag, "bins": {}, "eClass": "Holder", "shapes": {}, "tick": 1})
}

/// Once N1 is delivered everywhere.
fn after_inner() -> Value {
    document(Some(boxed(
        0,
        json!({"j": mid(0, json!({}), json!({"i": mid(1, json!({}), json!({}))}))}),
    )))
}

/// Once N1 and N2 are delivered everywhere: the formed document.
fn formed() -> Value {
    document(Some(boxed(
        0,
        json!({"j": mid(
            0,
            json!({"d": boxed(1, json!({}))}),
            json!({"i": mid(1, json!({}), json!({}))}),
        )}),
    )))
}

/// K8, once U alone is delivered everywhere: U resets what it had seen, the entry stays, and an
/// emptied `Box` or `Mid` reads as a fresh one and is left out, one level after another.
fn alone(place: Place) -> Value {
    match place {
        Place::Top => document(None),
        Place::Direct => document(Some(boxed(
            0,
            json!({"j": mid(0, json!({"d": boxed(1, json!({}))}), json!({}))}),
        ))),
        Place::Indirect => after_inner(),
    }
}

/// K9, once U and W are delivered everywhere: W's +10 survives U, what U had seen is reset, and
/// every object left emptied is left out.
fn raced(target: Target) -> Value {
    let j = match target {
        Target::Joint => mid(10, json!({}), json!({})),
        Target::Inner => mid(0, json!({}), json!({"i": mid(10, json!({}), json!({}))})),
        Target::Back => mid(0, json!({"d": boxed(10, json!({}))}), json!({})),
    };
    document(Some(boxed(0, json!({ "j": j }))))
}

// ---------------------------------------------------------------------------
// The twins
// ---------------------------------------------------------------------------

/// One seat on both runtimes.
struct Seat {
    interp: Interp,
    generated: Gen,
}

impl Seat {
    fn interp_doc(&self) -> Value {
        self.interp.query(&Read::<Value>::new())
    }

    fn generated_doc(&self) -> Value {
        project(&self.generated.query(&Read::<GenValue>::new()))
    }
}

/// Every comparison made and every failure seen over one test.
#[derive(Default)]
struct Counts {
    runs: usize,
    checkpoints: usize,
    comparisons: usize,
    differences: Vec<String>,
    values: Vec<String>,
    parted: Vec<String>,
    between_comparisons: usize,
    between: usize,
}

impl Counts {
    fn line(&self, test: &str) -> String {
        format!(
            "{test} runs {}: checkpoints {}, twin comparisons {}, twin differences {}, generated \
             values other than the rule's {}, generated checkpoints parted {}; between \
             checkpoints, not asserted: comparisons {}, differences {}",
            self.runs,
            self.checkpoints,
            self.comparisons,
            self.differences.len(),
            self.values.len(),
            self.parted.len(),
            self.between_comparisons,
            self.between
        )
    }

    /// The summary, a line per group of runs whose labels start with one of `groups`, and the
    /// verdict.
    fn report(&self, test: &str, groups: &[String]) {
        println!("{}", self.line(test));
        for group in groups {
            let count = |lines: &[String]| lines.iter().filter(|l| l.starts_with(group)).count();
            println!(
                "{test} {group} twin differences {}, generated values other than the rule's {}, \
                 generated checkpoints parted {}",
                count(&self.differences),
                count(&self.values),
                count(&self.parted)
            );
        }
        for line in self
            .differences
            .iter()
            .chain(&self.values)
            .chain(&self.parted)
            .take(6)
        {
            println!("{test} {line}");
        }
        assert!(
            self.differences.is_empty() && self.values.is_empty() && self.parted.is_empty(),
            "{}; first:\n  {}",
            self.line(test),
            self.differences
                .iter()
                .chain(&self.values)
                .chain(&self.parted)
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n  ")
        );
    }
}

/// Three seats, what was sent on both runtimes, and the counts of the test.
struct Twins<'a> {
    seats: Vec<Seat>,
    sent: Vec<(EventMessage<ModelOp>, EventMessage<GenOp>)>,
    armed: bool,
    label: String,
    counts: &'a mut Counts,
}

impl<'a> Twins<'a> {
    fn new(desc: &Value, label: String, counts: &'a mut Counts) -> Self {
        let (mut ia, mut ib, mut ic) = triplet_log::<ModelLog>();
        let opening = ia
            .send(install("m", desc))
            .expect("set-up: a fresh log takes its `Install`");
        ib.receive(opening.clone());
        ic.receive(opening);
        let (ga, gb, gc) = triplet_log::<GenLog>();
        counts.runs += 1;
        Twins {
            seats: vec![
                Seat {
                    interp: ia,
                    generated: ga,
                },
                Seat {
                    interp: ib,
                    generated: gb,
                },
                Seat {
                    interp: ic,
                    generated: gc,
                },
            ],
            sent: Vec::new(),
            armed: false,
            label,
            counts,
        }
    }

    /// After a step, once T has been delivered everywhere: every seat compared, counted and not
    /// asserted.
    fn look(&mut self) {
        if !self.armed {
            return;
        }
        for seat in &self.seats {
            self.counts.between_comparisons += 1;
            if seat.interp_doc() != seat.generated_doc() {
                self.counts.between += 1;
            }
        }
    }

    fn send(&mut self, seat: usize, op: &Op) -> usize {
        let interp = self.seats[seat]
            .interp
            .send(op.interp.clone())
            .unwrap_or_else(|why| {
                panic!(
                    "set-up: {}: the interpreted seat {seat} refused {}: {why:?}",
                    self.label, op.spelled
                )
            });
        let generated = self.seats[seat]
            .generated
            .send(op.generated.clone())
            .unwrap_or_else(|why| {
                panic!(
                    "AR01 claim: {}: the generated seat {seat} refused {}: {why}",
                    self.label, op.spelled
                )
            });
        self.sent.push((interp, generated));
        self.look();
        self.sent.len() - 1
    }

    fn deliver(&mut self, seat: usize, index: usize) {
        let (interp, generated) = self.sent[index].clone();
        self.seats[seat].interp.receive(interp);
        self.seats[seat].generated.receive(generated);
        self.look();
    }

    /// `op` sent on `seat` and delivered to the two others.
    fn broadcast(&mut self, seat: usize, op: &Op) {
        let index = self.send(seat, op);
        for other in 0..3 {
            if other != seat {
                self.deliver(other, index);
            }
        }
    }

    /// Two concurrent operations on seats `p` and `q`, each delivered to the other writer, and
    /// to the third seat in the order `first_p`.
    fn race(&mut self, (p, q): (usize, usize), op_p: &Op, op_q: &Op, first_p: bool) {
        let r = 3 - p - q;
        let ep = self.send(p, op_p);
        let eq = self.send(q, op_q);
        self.deliver(p, eq);
        self.deliver(q, ep);
        if first_p {
            self.deliver(r, ep);
            self.deliver(r, eq);
        } else {
            self.deliver(r, eq);
            self.deliver(r, ep);
        }
    }

    /// Once every operation sent has been delivered everywhere: each seat's two runtimes
    /// compared, each generated seat against `want`, and the generated seats against each
    /// other.
    fn checkpoint(&mut self, at: &str, want: &Value) {
        self.counts.checkpoints += 1;
        let mut generated = Vec::new();
        for (index, seat) in self.seats.iter().enumerate() {
            self.counts.comparisons += 1;
            let interp = seat.interp_doc();
            let served = seat.generated_doc();
            if interp != served {
                self.counts.differences.push(format!(
                    "{}, {at}: seat {index}: interpreted {interp} | generated {served}",
                    self.label
                ));
            }
            if served != *want {
                self.counts.values.push(format!(
                    "{}, {at}: generated seat {index} serves {served}, not {want}",
                    self.label
                ));
            }
            generated.push(served);
        }
        if generated.windows(2).any(|pair| pair[0] != pair[1]) {
            self.counts.parted.push(format!(
                "{}, {at}: the generated seats part: {generated:?}",
                self.label
            ));
        }
    }

    /// The formation: T, N1 and N2, each from seat 0 and delivered to the others, each followed
    /// by a checkpoint; the comparisons after every step start once T is delivered.
    fn form(&mut self, ops: &Ops) {
        self.broadcast(0, &ops.tick());
        self.armed = true;
        self.checkpoint("after T", &document(None));
        self.broadcast(0, &ops.inner(1));
        self.checkpoint("after N1", &after_inner());
        self.broadcast(0, &ops.back(1));
        self.checkpoint("after N2", &formed());
    }
}

/// Every assignment of U and W to two of the three seats.
const SEATS: [(usize, usize); 6] = [(0, 1), (1, 0), (0, 2), (2, 0), (1, 2), (2, 1)];

// ---------------------------------------------------------------------------
// K8 and K9 on both runtimes
// ---------------------------------------------------------------------------

/// ST.05's K8 on both runtimes: the formation, then U at each place (`Top`, `Direct`,
/// `Indirect`) from each seat, delivered to the others (9 runs, 4 checkpoints each).
#[test]
fn ar01_k8_a_removal_alone_in_classes_that_reach_themselves_agrees_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let mut counts = Counts::default();
    for place in PLACES {
        for u in 0..3 {
            let mut twins = Twins::new(&desc, format!("{place:?}: U alone on {u}"), &mut counts);
            twins.form(&ops);
            twins.broadcast(u, &ops.removal(place));
            twins.checkpoint("after U", &alone(place));
        }
    }
    let groups: Vec<String> = PLACES.iter().map(|place| format!("{place:?}:")).collect();
    counts.report("AR01-K8", &groups);
}

/// ST.05's K9 on both runtimes: the formation, then U (key `k` of `bag`) on one seat and W (+10
/// on the `Mid` at `j`, the `Mid` at `i` or the `Box` at `d`) on another, concurrently; each
/// writer delivers the other's, and the third seat delivers both in either order (6 seatings x
/// 2 orders x 3 targets, 36 schedules, 4 checkpoints each).
#[test]
fn ar01_k9_a_removal_raced_by_a_write_inside_either_cycle_agrees_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let mut counts = Counts::default();
    for target in TARGETS {
        for (u, w) in SEATS {
            for u_first in [true, false] {
                let mut twins = Twins::new(
                    &desc,
                    format!("{target:?}: U on {u} W on {w}, third U first {u_first}"),
                    &mut counts,
                );
                twins.form(&ops);
                twins.race(
                    (u, w),
                    &ops.removal(Place::Top),
                    &ops.write(target),
                    u_first,
                );
                twins.checkpoint("after U and W", &raced(target));
            }
        }
    }
    let groups: Vec<String> = TARGETS
        .iter()
        .map(|target| format!("{target:?}:"))
        .collect();
    counts.report("AR01-K9", &groups);
}
