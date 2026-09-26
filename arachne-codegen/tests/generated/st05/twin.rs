//! ST.05's twin, K6 of `cea-cdrt-knowledge/spec/items/ST.05.md`: the
//! runtime Arachne generates for `st05.ecore` against the interpreted one,
//! when a key of a keyed collection whose values are objects is removed,
//! alone or raced by a write into the removed key's object.
//!
//! `generated_crates.rs` generates the crate `st05` from `st05.ecore` with the
//! generator at this checkout, against the model-plane Moirai checkout, and
//! copies this file into its `tests/`. So the generated side is `UWMapLog`,
//! `record!`, `union!` and `OptionLog` as the library holds them now,
//! composed as Arachne composes a keyed containment: `UWMapLog<String,
//! BoxLog>` for `Box`, which has no subclass, `UWMapLog<String,
//! ShapeKindLog>` for the abstract `Shape`, `UWMapLog<String, BinKindLog>`
//! for `Bin`, which has one, and `UWMapLog<String, MidLog>` one level deeper;
//! and the interpreted side is `moirai_interp::ModelLog` on the descriptor
//! Arachne wrote beside the crate (`metamodel.json`, which
//! `generated_crates.rs` holds equal to
//! `moirai-interp/tests/fixtures/st05.metamodel.json`).
//!
//! Each seat is a pair of replicas, one per runtime, fed the same operation
//! in the same order. At every checkpoint, once every operation sent has
//! been delivered everywhere (the end of a schedule, each later write, the
//! joiner), each seat is read on both runtimes, the generated `Read`
//! projected onto the canonical form by [`project`], written here by hand
//! for this metamodel, and the two must be equal (the spec's P7 counts the
//! differences). At the end of every schedule the generated replicas must
//! also serve the value the rule fixes, which the spec derives by hand (P8),
//! and one document (P9). Between checkpoints the seats are compared too,
//! from the moment T has been delivered everywhere, and those differences
//! are counted and printed, not asserted: the claim is about quiescence.
//!
//! The canonical form of the interpreted side is its default read, what
//! `GET /api/model/{id}/state` serves: `Replica::query(&Read::<Value>::new())`.
use std::sync::Arc;

use moirai_crdt::counter::resettable_counter::Counter;
use moirai_crdt::map::uw_map::UWMap;
use moirai_crdt::option::Optional;
use moirai_crdt::utils::membership::triplet_log;
use moirai_interp::testing::{class_slot, feature_slot, install};
use moirai_interp::{InstanceOp, LeafOp, ModelLog, ModelOp, Scalar};
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::{StateSnapshot, Tcsb};
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::{IsReplica, Replica};
use moirai_semantics::{MetamodelSemantics, from_descriptor};
use serde_json::{Map, Value, json};
use st05::classifiers::{A, B, Bin, BinKind, Box as Bx, Holder, Mid, Shape, ShapeKind};
use st05::package::{St05, St05Log, St05Value};

type Interp = Replica<ModelLog, Tcsb<ModelOp>>;
type Gen = Replica<St05Log, Tcsb<St05>>;

/// The descriptor Arachne wrote beside this crate.
const DESCRIPTOR: &str = include_str!("../metamodel.json");

fn descriptor() -> Value {
    serde_json::from_str(DESCRIPTOR).expect("metamodel.json is JSON")
}

// ---------------------------------------------------------------------------
// The projection of the generated read onto the canonical form, by hand
// ---------------------------------------------------------------------------

/// `St05Value` in the canonical form: `Holder` with `eClass`; `tick` a
/// number; an optional that is `None` an absent key; a record (`Box`, `Mid`)
/// an object with its `eClass` and every single-valued counter whatever it
/// holds; `Box.note`, an optional counter, a number or an absent key; a
/// keyed collection an object of the keys the generated read serves; a
/// `union!` value its member directly (`Value`), a conflict `{"__conflict":
/// [...]}` ordered by class name (`Conflict`), nothing (`Unset`) an absent
/// key; `A`'s and `B`'s inherited `n` and `parts` read through their
/// `shape_super`, `BigBin`'s `k` through its `bin_super`.
fn project(value: &St05Value) -> Value {
    let raw = serde_json::to_value(value).expect("the generated read serializes");
    let holder = &raw["holder"];
    let mut out = Map::new();
    out.insert("eClass".into(), json!("Holder"));
    out.insert("tick".into(), holder["tick"].clone());
    if !holder["inner"].is_null() {
        out.insert("inner".into(), boxed_of(&holder["inner"]));
    }
    out.insert(
        "bag".into(),
        keyed(&holder["bag"], |held| Some(boxed_of(held))),
    );
    out.insert("shapes".into(), keyed(&holder["shapes"], kind));
    out.insert("bins".into(), keyed(&holder["bins"], kind));
    Value::Object(out)
}

/// A generated keyed collection, each value projected by `each`; a value
/// `each` reads as nothing is an absent key.
fn keyed(map: &Value, each: impl Fn(&Value) -> Option<Value>) -> Value {
    let mut out = Map::new();
    for (key, held) in map
        .as_object()
        .unwrap_or_else(|| panic!("a keyed collection reads as a map: {map}"))
    {
        if let Some(value) = each(held) {
            out.insert(key.clone(), value);
        }
    }
    Value::Object(out)
}

/// One `BoxValue`, projected.
fn boxed_of(value: &Value) -> Value {
    let mut out = Map::new();
    out.insert("eClass".into(), json!("Box"));
    out.insert("k".into(), value["k"].clone());
    if !value["note"].is_null() {
        out.insert("note".into(), value["note"].clone());
    }
    if !value["mid"].is_null() {
        out.insert("mid".into(), mid_of(&value["mid"]));
    }
    out.insert(
        "sub".into(),
        keyed(&value["sub"], |held| Some(mid_of(held))),
    );
    Value::Object(out)
}

/// One `MidValue`, projected.
fn mid_of(value: &Value) -> Value {
    json!({"eClass": "Mid", "k": value["k"].clone()})
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
                        .unwrap_or_else(|| panic!("a `Conflict` holds an array: {value}"))
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
                other => panic!("`{other}` is not a `union!` state: {value}"),
            }
        }
        other => panic!("`{other}` is not a `union!` value"),
    }
}

/// One member of a `union!`: `{"A": {"shape_super": {"n": 1, "parts": {}}}}`
/// as `{"eClass": "A", "n": 1, "parts": {}}`, `{"Bin": {"k": 3}}` as
/// `{"eClass": "Bin", "k": 3}`, `{"BigBin": {"bin_super": {"k": 3}}}` as
/// `{"eClass": "BigBin", "k": 3}`.
fn member(child: &Value) -> Value {
    let (class, payload) = child
        .as_object()
        .and_then(|variant| variant.iter().next())
        .unwrap_or_else(|| panic!("a `union!` child is its variant: {child}"));
    match class.as_str() {
        "A" | "B" => {
            let shape = &payload["shape_super"];
            json!({
                "eClass": class,
                "n": shape["n"].clone(),
                "parts": keyed(&shape["parts"], |held| Some(mid_of(held))),
            })
        }
        "Bin" => json!({"eClass": "Bin", "k": payload["k"].clone()}),
        "BigBin" => json!({"eClass": "BigBin", "k": payload["bin_super"]["k"].clone()}),
        other => panic!("`{other}` is not a class of st05.ecore: {child}"),
    }
}

#[test]
fn st05_k6_the_projection_reads_a_hand_written_generated_value() {
    let raw = json!({"holder": {
        "tick": 1,
        "inner": {"k": 2, "note": null, "mid": null, "sub": {"j": {"k": 0}}},
        "bag": {"k": {"k": 10, "note": 4, "mid": {"k": 3}, "sub": {}}},
        "shapes": {
            "k": {"Conflict": [{"B": {"shape_super": {"n": 0, "parts": {}}}},
                               {"A": {"shape_super": {"n": 10, "parts": {"j": {"k": 1}}}}}]},
            "u": "Unset"
        },
        "bins": {"k": {"Value": {"BigBin": {"bin_super": {"k": 7}}}}, "v": {"Value": {"Bin": {"k": 0}}}}
    }});
    let value: St05Value = serde_json::from_value(raw).expect("the hand-written value parses");
    assert_eq!(
        project(&value),
        json!({
            "eClass": "Holder", "tick": 1,
            "inner": {"eClass": "Box", "k": 2, "sub": {"j": {"eClass": "Mid", "k": 0}}},
            "bag": {"k": {"eClass": "Box", "k": 10, "note": 4,
                          "mid": {"eClass": "Mid", "k": 3}, "sub": {}}},
            "shapes": {"k": {"__conflict": [
                {"eClass": "A", "n": 10, "parts": {"j": {"eClass": "Mid", "k": 1}}},
                {"eClass": "B", "n": 0, "parts": {}}
            ]}},
            "bins": {"k": {"eClass": "BigBin", "k": 7}, "v": {"eClass": "Bin", "k": 0}}
        })
    );
    let empty: St05Value = serde_json::from_value(json!({"holder": {
        "tick": 0, "inner": null, "bag": {}, "shapes": {}, "bins": {}
    }}))
    .expect("parses");
    assert_eq!(
        project(&empty),
        json!({"eClass": "Holder", "tick": 0, "bag": {}, "shapes": {}, "bins": {}})
    );
}

// ---------------------------------------------------------------------------
// The operations, on both runtimes
// ---------------------------------------------------------------------------

/// Which keyed collection a key is removed from, and what the removed key's
/// object holds, as in the spec: `Record` (`Holder.bag`, a `Box`; X `Box.k`,
/// Y `Box.note`), `RecordDeep` (`Holder.bag`, a `Box` holding the `Mid` at
/// key `j` of `Box.sub` (X) and the `Mid` of `Box.mid` (Y)), `RecordInner`
/// (key `j` of `Box.sub` of the `Box` in `Holder.inner`; X that `Mid`, Y the
/// `Box`'s `k`), `Union` (`Holder.shapes`, the conflict of an `A` (X) and a
/// `B` (Y)), `UnionOne` (`Holder.shapes`, one `A`), `UnionDeep`
/// (`Holder.shapes`, an `A` holding the `Mid` at key `j` of `A.parts` (X);
/// Y the `A`'s `n`), `Family` (`Holder.bins`, a `Bin`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Record,
    RecordDeep,
    RecordInner,
    Union,
    UnionOne,
    UnionDeep,
    Family,
}

const PLACES: [Place; 7] = [
    Place::Record,
    Place::RecordDeep,
    Place::RecordInner,
    Place::Union,
    Place::UnionOne,
    Place::UnionDeep,
    Place::Family,
];

impl Place {
    fn targets(self) -> &'static [Target] {
        match self {
            Place::RecordDeep => &[Target::Member, Target::Sub, Target::Mid],
            Place::Union => &[Target::Member, Target::Other],
            Place::UnionDeep => &[Target::Member, Target::Sub],
            _ => &[Target::Member],
        }
    }

    fn formation(self) -> (Target, Target) {
        match self {
            Place::Record => (Target::Member, Target::Note),
            Place::RecordDeep => (Target::Sub, Target::Mid),
            Place::RecordInner => (Target::Member, Target::Owner),
            Place::Union => (Target::Member, Target::Other),
            Place::UnionOne | Place::Family => (Target::Member, Target::Member),
            Place::UnionDeep => (Target::Sub, Target::Member),
        }
    }
}

/// What a write increments: the removed key's object (`Member`), the `Mid`
/// at key `j` one level deeper (`Sub`), the `Mid` in `Box.mid` (`Mid`), the
/// `B` (`Other`), `Box.note` (`Note`), the `k` of the `Box` holding
/// `RecordInner`'s collection (`Owner`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Member,
    Sub,
    Mid,
    Other,
    Note,
    Owner,
}

/// One operation, as each runtime spells it.
#[derive(Clone, Debug)]
struct Op {
    interp: ModelOp,
    generated: St05,
}

struct Ops {
    sem: Arc<MetamodelSemantics>,
}

impl Ops {
    fn new(desc: &Value) -> Self {
        Ops {
            sem: Arc::new(from_descriptor(desc).expect("Arachne's descriptor parses")),
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

    fn holder(&self, inner: InstanceOp) -> ModelOp {
        self.is("Holder", inner).into_model_op()
    }

    fn inc(by: i32) -> InstanceOp {
        InstanceOp::Leaf(LeafOp::Inc(Scalar::Int(by as i64)))
    }

    fn tick(&self) -> Op {
        Op {
            interp: self.holder(self.at("Holder", "tick", Self::inc(1))),
            generated: St05::Holder(Holder::Tick(Counter::Inc(1))),
        }
    }

    /// A `Mid` whose `k` is incremented by `by`, as an entry `j`.
    fn mid_entry(&self, by: i32) -> (InstanceOp, UWMap<String, Mid>) {
        (
            InstanceOp::entry(
                Scalar::text("j"),
                self.is("Mid", self.at("Mid", "k", Self::inc(by))),
            ),
            UWMap::Update("j".to_string(), Mid::K(Counter::Inc(by))),
        )
    }

    fn in_bag(&self, interp: InstanceOp, generated: Bx) -> Op {
        Op {
            interp: self.holder(self.at(
                "Holder",
                "bag",
                InstanceOp::entry(Scalar::text("k"), self.is("Box", interp)),
            )),
            generated: St05::Holder(Holder::Bag(UWMap::Update("k".to_string(), generated))),
        }
    }

    fn in_inner(&self, interp: InstanceOp, generated: Bx) -> Op {
        Op {
            interp: self.holder(self.at(
                "Holder",
                "inner",
                InstanceOp::set(self.is("Box", interp)),
            )),
            generated: St05::Holder(Holder::Inner(Optional::Set(generated))),
        }
    }

    fn in_shapes(&self, class: &str, interp: InstanceOp, generated: Shape) -> Op {
        let kind = if class == "A" {
            ShapeKind::A(A::ShapeSuper(generated))
        } else {
            ShapeKind::B(B::ShapeSuper(generated))
        };
        Op {
            interp: self.holder(self.at(
                "Holder",
                "shapes",
                InstanceOp::entry(Scalar::text("k"), self.is(class, interp)),
            )),
            generated: St05::Holder(Holder::Shapes(UWMap::Update("k".to_string(), kind))),
        }
    }

    /// A write at the place that increments `target` by `by`.
    fn write(&self, place: Place, target: Target, by: i32) -> Op {
        match (place, target) {
            (Place::Record | Place::RecordDeep, Target::Member) => {
                self.in_bag(self.at("Box", "k", Self::inc(by)), Bx::K(Counter::Inc(by)))
            }
            (Place::Record, Target::Note) => self.in_bag(
                self.at("Box", "note", InstanceOp::set(Self::inc(by))),
                Bx::Note(Optional::Set(Counter::Inc(by))),
            ),
            (Place::RecordDeep, Target::Sub) => {
                let (interp, generated) = self.mid_entry(by);
                self.in_bag(self.at("Box", "sub", interp), Bx::Sub(generated))
            }
            (Place::RecordDeep, Target::Mid) => self.in_bag(
                self.at(
                    "Box",
                    "mid",
                    InstanceOp::set(self.is("Mid", self.at("Mid", "k", Self::inc(by)))),
                ),
                Bx::Mid(Optional::Set(Mid::K(Counter::Inc(by)))),
            ),
            (Place::RecordInner, Target::Member) => {
                let (interp, generated) = self.mid_entry(by);
                self.in_inner(self.at("Box", "sub", interp), Bx::Sub(generated))
            }
            (Place::RecordInner, Target::Owner) => {
                self.in_inner(self.at("Box", "k", Self::inc(by)), Bx::K(Counter::Inc(by)))
            }
            (Place::Union | Place::UnionOne | Place::UnionDeep, Target::Member) => self.in_shapes(
                "A",
                self.at("A", "n", Self::inc(by)),
                Shape::N(Counter::Inc(by)),
            ),
            (Place::Union, Target::Other) => self.in_shapes(
                "B",
                self.at("B", "n", Self::inc(by)),
                Shape::N(Counter::Inc(by)),
            ),
            (Place::UnionDeep, Target::Sub) => {
                let (interp, generated) = self.mid_entry(by);
                self.in_shapes("A", self.at("A", "parts", interp), Shape::Parts(generated))
            }
            (Place::Family, Target::Member) => Op {
                interp: self.holder(self.at(
                    "Holder",
                    "bins",
                    InstanceOp::entry(
                        Scalar::text("k"),
                        self.is("Bin", self.at("Bin", "k", Self::inc(by))),
                    ),
                )),
                generated: St05::Holder(Holder::Bins(UWMap::Update(
                    "k".to_string(),
                    BinKind::Bin(Bin::K(Counter::Inc(by))),
                ))),
            },
            (place, target) => panic!("set-up: no {target:?} in {place:?}"),
        }
    }

    /// U, the key removal.
    fn removal(&self, place: Place) -> Op {
        let at_root = |feature: &str, generated: Holder| Op {
            interp: self.holder(self.at("Holder", feature, InstanceOp::remove(Scalar::text("k")))),
            generated: St05::Holder(generated),
        };
        match place {
            Place::Record | Place::RecordDeep => {
                at_root("bag", Holder::Bag(UWMap::Remove("k".to_string())))
            }
            Place::RecordInner => self.in_inner(
                self.at("Box", "sub", InstanceOp::remove(Scalar::text("j"))),
                Bx::Sub(UWMap::Remove("j".to_string())),
            ),
            Place::Union | Place::UnionOne | Place::UnionDeep => {
                at_root("shapes", Holder::Shapes(UWMap::Remove("k".to_string())))
            }
            Place::Family => at_root("bins", Holder::Bins(UWMap::Remove("k".to_string()))),
        }
    }
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
        project(&self.generated.query(&Read::<St05Value>::new()))
    }
}

/// Three seats, what was sent on both runtimes, and every difference seen:
/// at checkpoints (asserted) and between them (counted only).
struct Twins {
    seats: Vec<Seat>,
    sent: Vec<(EventMessage<ModelOp>, EventMessage<St05>)>,
    armed: bool,
    label: String,
    differences: Vec<String>,
    comparisons: usize,
    between: usize,
    between_comparisons: usize,
    steps: usize,
    /// Called with the seats after every step; the joiner test takes its
    /// snapshot there.
    on_step: Option<Box<dyn FnMut(usize, &[Seat])>>,
}

impl Twins {
    fn new(desc: &Value, label: String) -> Self {
        let (mut ia, mut ib, mut ic) = triplet_log::<ModelLog>();
        let opening = ia
            .send(install("m", desc))
            .expect("a fresh log takes its `Install`");
        ib.receive(opening.clone());
        ic.receive(opening);
        let (ga, gb, gc) = triplet_log::<St05Log>();
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
            differences: Vec::new(),
            comparisons: 0,
            between: 0,
            between_comparisons: 0,
            steps: 0,
            on_step: None,
        }
    }

    /// Between checkpoints: counted, not asserted.
    fn compare_between(&mut self, seat: usize) {
        if !self.armed {
            return;
        }
        self.between_comparisons += 1;
        if self.seats[seat].interp_doc() != self.seats[seat].generated_doc() {
            self.between += 1;
        }
    }

    fn compare(&mut self, seat: usize, when: &str) {
        self.comparisons += 1;
        let interp = self.seats[seat].interp_doc();
        let generated = self.seats[seat].generated_doc();
        if interp != generated {
            self.differences.push(format!(
                "{}: seat {seat} {when}: interpreted {interp} | generated {generated}",
                self.label
            ));
        }
    }

    fn stepped(&mut self) {
        self.steps += 1;
        if let Some(hook) = self.on_step.as_mut() {
            hook(self.steps, &self.seats);
        }
    }

    fn send(&mut self, seat: usize, op: &Op) -> usize {
        let interp = self.seats[seat]
            .interp
            .send(op.interp.clone())
            .unwrap_or_else(|why| {
                panic!(
                    "{}: the interpreted seat {seat} refused {op:?}: {why:?}",
                    self.label
                )
            });
        let generated = self.seats[seat]
            .generated
            .send(op.generated.clone())
            .unwrap_or_else(|why| {
                panic!(
                    "{}: the generated seat {seat} refused {op:?}: {why}",
                    self.label
                )
            });
        self.sent.push((interp, generated));
        self.compare_between(seat);
        self.stepped();
        self.sent.len() - 1
    }

    fn deliver(&mut self, seat: usize, index: usize) {
        let (interp, generated) = self.sent[index].clone();
        self.seats[seat].interp.receive(interp);
        self.seats[seat].generated.receive(generated);
        self.compare_between(seat);
        self.stepped();
    }

    /// Once every operation sent has been delivered everywhere: every seat
    /// compared.
    fn checkpoint(&mut self, when: &str) {
        for seat in 0..self.seats.len() {
            self.compare(seat, when);
        }
    }

    /// Two concurrent operations on seats `p` and `q`, each delivered to the
    /// other writer, and to the third seat in the order `first_p`.
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

    /// T from seat 0, delivered everywhere; the comparisons between
    /// checkpoints start then.
    fn opening(&mut self, ops: &Ops) {
        let t = self.send(0, &ops.tick());
        self.deliver(1, t);
        self.deliver(2, t);
        self.armed = true;
    }

    /// T, then X and Y raced, on both runtimes.
    fn form(&mut self, ops: &Ops, place: Place, xy: (usize, usize), x_first: bool) {
        self.opening(ops);
        let (x, y) = place.formation();
        self.race(
            xy,
            &ops.write(place, x, 1),
            &ops.write(place, y, 2),
            x_first,
        );
        self.checkpoint("once X and Y were delivered");
    }

    /// The generated seats' reads, all of them.
    fn generated_docs(&self) -> Vec<Value> {
        self.seats.iter().map(Seat::generated_doc).collect()
    }
}

// ---------------------------------------------------------------------------
// The schedules and the documents, as in the spec
// ---------------------------------------------------------------------------

const SEATS: [(usize, usize); 6] = [(0, 1), (1, 0), (0, 2), (2, 0), (1, 2), (2, 1)];

#[derive(Clone, Copy, Debug)]
struct Schedule {
    place: Place,
    xy: (usize, usize),
    x_first: bool,
    uw: (usize, usize),
    u_first: bool,
    target: Target,
}

impl Schedule {
    fn label(&self) -> String {
        format!(
            "{:?}: X on {} Y on {}, third X first {}; U on {} W({:?}) on {}, third U first {}",
            self.place,
            self.xy.0,
            self.xy.1,
            self.x_first,
            self.uw.0,
            self.target,
            self.uw.1,
            self.u_first
        )
    }

    /// Whether the race leaves out the `Mid` at key `j`.
    fn drops_sub(&self) -> bool {
        matches!(
            (self.place, self.target),
            (Place::RecordDeep, Target::Member | Target::Mid) | (Place::UnionDeep, Target::Member)
        )
    }
}

fn schedules(place: Place) -> Vec<Schedule> {
    let mut out = Vec::new();
    for xy in SEATS {
        for x_first in [true, false] {
            for uw in SEATS {
                for u_first in [true, false] {
                    for &target in place.targets() {
                        out.push(Schedule {
                            place,
                            xy,
                            x_first,
                            uw,
                            u_first,
                            target,
                        });
                    }
                }
            }
        }
    }
    out
}

/// T, X and Y, then U and W raced, on both runtimes.
fn play(ops: &Ops, twins: &mut Twins, schedule: &Schedule) {
    let place = schedule.place;
    twins.form(ops, place, schedule.xy, schedule.x_first);
    twins.race(
        schedule.uw,
        &ops.removal(place),
        &ops.write(place, schedule.target, 10),
        schedule.u_first,
    );
    twins.checkpoint("once every operation was delivered");
}

/// T, X and Y, then U alone from seat `u`, delivered everywhere.
fn play_alone(
    ops: &Ops,
    twins: &mut Twins,
    place: Place,
    xy: (usize, usize),
    x_first: bool,
    u: usize,
) {
    twins.form(ops, place, xy, x_first);
    let removal = twins.send(u, &ops.removal(place));
    for seat in 0..3 {
        if seat != u {
            twins.deliver(seat, removal);
        }
    }
    twins.checkpoint("once the removal was delivered");
}

fn mid(k: i64) -> Value {
    json!({"eClass": "Mid", "k": k})
}

fn keyed_doc(entries: &[(&str, Value)]) -> Value {
    Value::Object(
        entries
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect(),
    )
}

fn boxed(k: i64, note: Option<i64>, held: Option<Value>, sub: &[(&str, Value)]) -> Value {
    let mut out = json!({"eClass": "Box", "k": k, "sub": keyed_doc(sub)});
    if let Some(note) = note {
        out["note"] = json!(note);
    }
    if let Some(held) = held {
        out["mid"] = held;
    }
    out
}

fn shape(class: &str, n: i64, parts: &[(&str, Value)]) -> Value {
    json!({"eClass": class, "n": n, "parts": keyed_doc(parts)})
}

fn conflict(first: Value, second: Value) -> Value {
    let mut members = vec![first, second];
    members.sort_by(|l, r| l["eClass"].as_str().cmp(&r["eClass"].as_str()));
    json!({"__conflict": members})
}

fn bin(k: i64) -> Value {
    json!({"eClass": "Bin", "k": k})
}

/// The whole document, the place's object holding `content` (at key `k` of
/// `bag`, `shapes` or `bins`, or as `Holder.inner` in `RecordInner`), or
/// absent.
fn document(place: Place, content: Option<Value>) -> Value {
    let mut doc = json!({"bag": {}, "bins": {}, "eClass": "Holder", "shapes": {}, "tick": 1});
    if let Some(content) = content {
        match place {
            Place::Record | Place::RecordDeep => doc["bag"] = json!({"k": content}),
            Place::RecordInner => doc["inner"] = content,
            Place::Union | Place::UnionOne | Place::UnionDeep => {
                doc["shapes"] = json!({"k": content})
            }
            Place::Family => doc["bins"] = json!({"k": content}),
        }
    }
    doc
}

/// What every seat serves once X and Y are delivered.
fn formed_doc(place: Place) -> Value {
    let content = match place {
        Place::Record => boxed(1, Some(2), None, &[]),
        Place::RecordDeep => boxed(0, None, Some(mid(2)), &[("j", mid(1))]),
        Place::RecordInner => boxed(2, None, None, &[("j", mid(1))]),
        Place::Union => conflict(shape("A", 1, &[]), shape("B", 2, &[])),
        Place::UnionOne => shape("A", 3, &[]),
        Place::UnionDeep => shape("A", 2, &[("j", mid(1))]),
        Place::Family => bin(3),
    };
    document(place, Some(content))
}

/// The rule's value once every seat has delivered T, X, Y, U and W, W's
/// target holding `w` (spec section 2): what U emptied and W did not reach
/// is left out at a key of a class with no subclass and by an optional, and
/// stays, emptied, at a key of a class with subclasses.
fn race_value(schedule: &Schedule, w: i64) -> Value {
    let content = match (schedule.place, schedule.target) {
        (Place::Record, Target::Member) => boxed(w, None, None, &[]),
        (Place::RecordDeep, Target::Member) => boxed(w, None, None, &[]),
        (Place::RecordDeep, Target::Sub) => boxed(0, None, None, &[("j", mid(w))]),
        (Place::RecordDeep, Target::Mid) => boxed(0, None, Some(mid(w)), &[]),
        (Place::RecordInner, Target::Member) => boxed(2, None, None, &[("j", mid(w))]),
        (Place::Union, Target::Member) => conflict(shape("A", w, &[]), shape("B", 0, &[])),
        (Place::Union, Target::Other) => conflict(shape("A", 0, &[]), shape("B", w, &[])),
        (Place::UnionOne, Target::Member) => shape("A", w, &[]),
        (Place::UnionDeep, Target::Member) => shape("A", w, &[]),
        (Place::UnionDeep, Target::Sub) => shape("A", 0, &[("j", mid(w))]),
        (Place::Family, Target::Member) => bin(w),
        (place, target) => panic!("set-up: no {target:?} in {place:?}"),
    };
    document(schedule.place, Some(content))
}

fn expected(schedule: &Schedule) -> Value {
    race_value(schedule, 10)
}

/// What the removal alone leaves (spec section 2).
fn alone(place: Place) -> Value {
    let content = match place {
        Place::Record | Place::RecordDeep => None,
        Place::RecordInner => Some(boxed(2, None, None, &[])),
        Place::Union => Some(conflict(shape("A", 0, &[]), shape("B", 0, &[]))),
        Place::UnionOne | Place::UnionDeep => Some(shape("A", 0, &[])),
        Place::Family => Some(bin(0)),
    };
    document(place, content)
}

/// The later writes after the race, from one writer: W's target +1, then,
/// where the race left out the `Mid` at key `j`, that `Mid` +5. Each is
/// followed by what every seat must then serve.
fn later_writes(ops: &Ops, schedule: &Schedule) -> Vec<(Op, Value)> {
    let place = schedule.place;
    let mut out = vec![(
        ops.write(place, schedule.target, 1),
        race_value(schedule, 11),
    )];
    if schedule.drops_sub() {
        let content = match (place, schedule.target) {
            (Place::RecordDeep, Target::Member) => boxed(11, None, None, &[("j", mid(5))]),
            (Place::RecordDeep, Target::Mid) => boxed(0, None, Some(mid(11)), &[("j", mid(5))]),
            (Place::UnionDeep, Target::Member) => shape("A", 11, &[("j", mid(5))]),
            (place, target) => panic!("set-up: nothing rebuilt for {target:?} in {place:?}"),
        };
        out.push((
            ops.write(place, Target::Sub, 5),
            document(place, Some(content)),
        ));
    }
    out
}

/// The later writes after a removal alone: the removed key's object +5,
/// then +1.
fn later_writes_alone(ops: &Ops, place: Place) -> Vec<(Op, Value)> {
    let value = |m: i64| {
        let content = match place {
            Place::Record | Place::RecordDeep => boxed(m, None, None, &[]),
            Place::RecordInner => boxed(2, None, None, &[("j", mid(m))]),
            Place::Union => conflict(shape("A", m, &[]), shape("B", 0, &[])),
            Place::UnionOne | Place::UnionDeep => shape("A", m, &[]),
            Place::Family => bin(m),
        };
        document(place, Some(content))
    };
    vec![
        (ops.write(place, Target::Member, 5), value(5)),
        (ops.write(place, Target::Member, 1), value(6)),
    ]
}

/// Every generated seat's read against `want`; one line per seat that
/// serves something else.
fn generated_against(twins: &Twins, want: &Value, label: &str) -> Vec<String> {
    twins
        .generated_docs()
        .iter()
        .enumerate()
        .filter(|(_, served)| *served != want)
        .map(|(seat, served)| format!("{label}: generated seat {seat} serves {served}, not {want}"))
        .collect()
}

/// Whether the generated seats part.
fn generated_parted(twins: &Twins) -> bool {
    twins.generated_docs().windows(2).any(|p| p[0] != p[1])
}

/// Counts over one test.
#[derive(Default)]
struct Counts {
    runs: usize,
    comparisons: usize,
    differences: Vec<String>,
    values: Vec<String>,
    parted: Vec<String>,
    between: usize,
    between_comparisons: usize,
}

impl Counts {
    fn absorb(&mut self, twins: &mut Twins) {
        self.comparisons += twins.comparisons;
        self.between += twins.between;
        self.between_comparisons += twins.between_comparisons;
        self.differences.append(&mut twins.differences);
    }

    fn summary(&self, test: &str) {
        println!(
            "ST05-K6 {test}: runs {}, twin comparisons {}, twin differences {} (P7), generated \
             values other than expected {} (P8), generated runs parted {} (P9); between \
             checkpoints, not asserted: comparisons {}, differences {}",
            self.runs,
            self.comparisons,
            self.differences.len(),
            self.values.len(),
            self.parted.len(),
            self.between_comparisons,
            self.between
        );
        for place in PLACES {
            let prefix = format!("{place:?}:");
            let count = |lines: &[String]| lines.iter().filter(|l| l.starts_with(&prefix)).count();
            println!(
                "ST05-K6 {test}: {place:?}: twin differences {}, generated values other than \
                 expected {}, generated runs parted {}",
                count(&self.differences),
                count(&self.values),
                count(&self.parted)
            );
        }
        for line in self.differences.iter().chain(&self.values).take(6) {
            println!("ST05-K6 {line}");
        }
    }

    fn verdict(&self, test: &str) {
        assert!(
            self.differences.is_empty() && self.values.is_empty() && self.parted.is_empty(),
            "ST.05 K6 {test}: {} twin differences, {} generated values other than expected and \
             {} generated runs parted, over {} runs; first:\n  {}",
            self.differences.len(),
            self.values.len(),
            self.parted.len(),
            self.runs,
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

// ---------------------------------------------------------------------------
// K6
// ---------------------------------------------------------------------------

/// Every schedule of the race (1584): twin equality once X and Y and once
/// every operation are delivered; at the end the generated seats serve one
/// document, the rule's.
#[test]
fn st05_k6_a_key_removal_raced_by_a_write_agrees_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let mut counts = Counts::default();
    for place in PLACES {
        for schedule in schedules(place) {
            counts.runs += 1;
            let label = schedule.label();
            let mut twins = Twins::new(&desc, label.clone());
            play(&ops, &mut twins, &schedule);
            counts
                .values
                .extend(generated_against(&twins, &expected(&schedule), &label));
            if generated_parted(&twins) {
                counts.parted.push(format!(
                    "{label}: the generated seats part: {:?}",
                    twins.generated_docs()
                ));
            }
            counts.absorb(&mut twins);
        }
    }
    counts.summary("schedules");
    counts.verdict("schedules");
}

/// The removal alone: 12 formations x 3 seats of U, the 7 places (252).
#[test]
fn st05_k6_a_key_removal_alone_agrees_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let mut counts = Counts::default();
    for place in PLACES {
        for xy in SEATS {
            for x_first in [true, false] {
                for u in 0..3 {
                    counts.runs += 1;
                    let label = format!(
                        "{place:?}: X on {} Y on {}, third X first {x_first}; U alone on {u}",
                        xy.0, xy.1
                    );
                    let mut twins = Twins::new(&desc, label.clone());
                    play_alone(&ops, &mut twins, place, xy, x_first, u);
                    counts
                        .values
                        .extend(generated_against(&twins, &alone(place), &label));
                    if generated_parted(&twins) {
                        counts.parted.push(format!(
                            "{label}: the generated seats part: {:?}",
                            twins.generated_docs()
                        ));
                    }
                    counts.absorb(&mut twins);
                }
            }
        }
    }
    counts.summary("removal alone");
    counts.verdict("removal alone");
}

/// After the race (X on 0, Y on 1, the third taking X first; U and W in
/// every seating, the third taking U first; every target), one seat makes
/// the later writes, each delivered everywhere; each seat takes that turn
/// in its own run (198). After a removal alone (U on each seat), each seat
/// in turn writes the removed key's object again, and the next once more
/// (63).
#[test]
fn st05_k6_writes_after_the_removal_agree_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let mut counts = Counts::default();
    let deliver_all = |twins: &mut Twins, writer: usize, op: &Op| {
        let index = twins.send(writer, op);
        for seat in 0..3 {
            if seat != writer {
                twins.deliver(seat, index);
            }
        }
    };
    for place in PLACES {
        for uw in SEATS {
            for &target in place.targets() {
                for writer in 0..3 {
                    counts.runs += 1;
                    let schedule = Schedule {
                        place,
                        xy: (0, 1),
                        x_first: true,
                        uw,
                        u_first: true,
                        target,
                    };
                    let label = format!("{}; then seat {writer} writes", schedule.label());
                    let mut twins = Twins::new(&desc, label.clone());
                    play(&ops, &mut twins, &schedule);
                    for (op, want) in later_writes(&ops, &schedule) {
                        deliver_all(&mut twins, writer, &op);
                        twins.checkpoint(&format!("once {:?} was delivered", op.generated));
                        counts.values.extend(generated_against(
                            &twins,
                            &want,
                            &format!("{label}, after {:?}", op.generated),
                        ));
                    }
                    counts.absorb(&mut twins);
                }
            }
        }
        for u in 0..3 {
            for writer in 0..3 {
                counts.runs += 1;
                let label = format!(
                    "{place:?}: X on 0 Y on 1, third X first true; U alone on {u}; then seat \
                     {writer} writes it again"
                );
                let mut twins = Twins::new(&desc, label.clone());
                play_alone(&ops, &mut twins, place, (0, 1), true, u);
                for (turn, (op, want)) in later_writes_alone(&ops, place).into_iter().enumerate() {
                    let by = (writer + turn) % 3;
                    deliver_all(&mut twins, by, &op);
                    twins.checkpoint(&format!("once {:?} was delivered", op.generated));
                    counts.values.extend(generated_against(
                        &twins,
                        &want,
                        &format!("{label}, after {:?}", op.generated),
                    ));
                }
                counts.absorb(&mut twins);
            }
        }
    }
    counts.summary("writes after the removal");
    counts.verdict("writes after the removal");
}

/// A joiner `d` on each runtime adopts, after the same step, the same seat's
/// causal snapshot and log (each through a serde round trip), delivers every
/// operation sent, in the order sent, and must serve on each runtime what
/// its twin serves and the rule's document; then it makes the later writes,
/// delivered to the three, and all four seats agree across runtimes and
/// serve what those say. The race: X on 0, Y on 1; U and W on 0 and 1 both
/// ways; every target; each seat as donor after each of the 15 steps (990).
/// The removal alone: U on 0; each seat as donor after each of the 12 steps
/// (252).
#[test]
fn st05_k6_a_joiner_by_state_transfer_agrees_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let mut counts = Counts::default();
    let finish = |twins: &mut Twins,
                  counts: &mut Counts,
                  label: &str,
                  want: Value,
                  later: Vec<(Op, Value)>| {
        let sent = twins.sent.clone();
        for (interp, generated) in sent {
            twins.seats[3].interp.receive(interp);
            twins.seats[3].generated.receive(generated);
        }
        twins.checkpoint("once the joiner delivered everything");
        let joined = twins.seats[3].generated_doc();
        if joined != want {
            counts.values.push(format!(
                "{label}: the generated joiner serves {joined}, not {want}"
            ));
        }
        for (op, want) in later {
            let index = twins.send(3, &op);
            for seat in 0..3 {
                twins.deliver(seat, index);
            }
            twins.checkpoint(&format!("once d's {:?} was delivered", op.generated));
            counts.values.extend(generated_against(
                twins,
                &want,
                &format!("{label}, d wrote {:?}", op.generated),
            ));
        }
        counts.absorb(twins);
    };
    let take_at = |twins: &mut Twins, donor: usize, at: usize| {
        let taken: std::rc::Rc<std::cell::RefCell<Option<Seat>>> = Default::default();
        let slot = taken.clone();
        twins.on_step = Some(Box::new(move |step, seats| {
            if step == at {
                *slot.borrow_mut() = Some(join(&seats[donor]));
            }
        }));
        taken
    };
    for place in PLACES {
        for uw in [(0, 1), (1, 0)] {
            for &target in place.targets() {
                let schedule = Schedule {
                    place,
                    xy: (0, 1),
                    x_first: true,
                    uw,
                    u_first: true,
                    target,
                };
                for donor in 0..3 {
                    for at in 1..=15 {
                        counts.runs += 1;
                        let label = format!(
                            "{}; d joins from seat {donor} after step {at}",
                            schedule.label()
                        );
                        let mut twins = Twins::new(&desc, label.clone());
                        let taken = take_at(&mut twins, donor, at);
                        play(&ops, &mut twins, &schedule);
                        twins.on_step = None;
                        let d = taken
                            .borrow_mut()
                            .take()
                            .expect("set-up: the schedule has 15 steps");
                        twins.seats.push(d);
                        finish(
                            &mut twins,
                            &mut counts,
                            &label,
                            expected(&schedule),
                            later_writes(&ops, &schedule),
                        );
                    }
                }
            }
        }
        for donor in 0..3 {
            for at in 1..=12 {
                counts.runs += 1;
                let label = format!(
                    "{place:?}: X on 0 Y on 1, third X first true; U alone on 0; d joins from \
                     seat {donor} after step {at}"
                );
                let mut twins = Twins::new(&desc, label.clone());
                let taken = take_at(&mut twins, donor, at);
                play_alone(&ops, &mut twins, place, (0, 1), true, 0);
                twins.on_step = None;
                let d = taken
                    .borrow_mut()
                    .take()
                    .expect("set-up: the removal alone has 12 steps");
                twins.seats.push(d);
                finish(
                    &mut twins,
                    &mut counts,
                    &label,
                    alone(place),
                    later_writes_alone(&ops, place),
                );
            }
        }
    }
    counts.summary("joiner");
    counts.verdict("joiner");
}

/// A fourth seat joining by state transfer from `donor` on both runtimes:
/// bootstrapped knowing only itself on the donor's log, adopting the donor's
/// snapshot and log, each through a serde round trip.
fn join(donor: &Seat) -> Seat {
    let log: ModelLog = serde_json::from_str(
        &serde_json::to_string(donor.interp.log()).expect("the interpreted log serializes"),
    )
    .expect("the interpreted log comes back");
    let snapshot: StateSnapshot<ModelOp> = serde_json::from_str(
        &serde_json::to_string(&donor.interp.snapshot()).expect("the snapshot serializes"),
    )
    .expect("the snapshot comes back");
    let mut interp: Interp =
        Replica::bootstrap_with_log_id("d".to_string(), &["d"], donor.interp.log_id().clone());
    interp.adopt(snapshot, log);

    let log: St05Log = serde_json::from_str(
        &serde_json::to_string(donor.generated.log()).expect("the generated log serializes"),
    )
    .expect("the generated log comes back");
    let snapshot: StateSnapshot<St05> = serde_json::from_str(
        &serde_json::to_string(&donor.generated.snapshot()).expect("the snapshot serializes"),
    )
    .expect("the snapshot comes back");
    let mut generated: Gen =
        Replica::bootstrap_with_log_id("d".to_string(), &["d"], donor.generated.log_id().clone());
    generated.adopt(snapshot, log);
    Seat { interp, generated }
}

// ---------------------------------------------------------------------------
// Observation, outside the claim: not a pass criterion
// ---------------------------------------------------------------------------

/// What each runtime serves, per place, once the removal alone and the race
/// with W on the removed key's object are delivered (U on 0, W on 1, X on 0,
/// Y on 1), printed and not asserted; and the generated read of a key
/// written with nothing in it (`Box`, `A`, `Bin` created by a write of
/// nothing but their class).
#[test]
#[ignore = "ST.05 observation, not a pass criterion: run with --ignored --nocapture"]
fn st05_observation_what_each_runtime_serves_at_a_removed_key() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    for place in PLACES {
        let mut twins = Twins::new(&desc, format!("{place:?} alone"));
        play_alone(&ops, &mut twins, place, (0, 1), true, 0);
        println!(
            "ST05-OB {place:?}, removal alone: interpreted {} | generated {}",
            twins.seats[0].interp_doc(),
            twins.seats[0].generated_doc()
        );
        for &target in place.targets() {
            let schedule = Schedule {
                place,
                xy: (0, 1),
                x_first: true,
                uw: (0, 1),
                u_first: true,
                target,
            };
            let mut twins = Twins::new(&desc, schedule.label());
            play(&ops, &mut twins, &schedule);
            println!(
                "ST05-OB {place:?}, W on {target:?}: interpreted {} | generated {}",
                twins.seats[0].interp_doc(),
                twins.seats[0].generated_doc()
            );
        }
    }
    let created = [
        (
            "Box",
            Op {
                interp: ops.holder(ops.at(
                    "Holder",
                    "bag",
                    InstanceOp::entry(Scalar::text("n"), ops.is("Box", InstanceOp::New)),
                )),
                generated: St05::Holder(Holder::Bag(UWMap::Update("n".to_string(), Bx::New))),
            },
        ),
        (
            "Bin",
            Op {
                interp: ops.holder(ops.at(
                    "Holder",
                    "bins",
                    InstanceOp::entry(Scalar::text("n"), ops.is("Bin", InstanceOp::New)),
                )),
                generated: St05::Holder(Holder::Bins(UWMap::Update(
                    "n".to_string(),
                    BinKind::Bin(Bin::New),
                ))),
            },
        ),
    ];
    for (class, op) in created {
        let mut twins = Twins::new(&desc, format!("{class} created empty"));
        twins.opening(&ops);
        let index = twins.send(0, &op);
        twins.deliver(1, index);
        twins.deliver(2, index);
        println!(
            "ST05-OB a {class} created with nothing written: interpreted {} | generated {}",
            twins.seats[0].interp_doc(),
            twins.seats[0].generated_doc()
        );
    }
}
