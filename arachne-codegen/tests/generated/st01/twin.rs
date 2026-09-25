//! ST.01's twin, O5 of `cea-cdrt-knowledge/spec/items/ST.01.md`: the runtime
//! Arachne generates for `st01.ecore` against the interpreted one, on every
//! schedule of the claim.
//!
//! `generated_crates.rs` generates the crate `st01` from `st01.ecore` with the
//! generator at this checkout, against the model-plane Moirai checkout, and
//! copies this file into its `tests/`. So the generated side is `OptionLog`,
//! `union!` and `record!` as the library holds them now, composed as Arachne
//! composes an optional containment of an abstract class
//! (`OptionLog<ShapeKindLog>`, `ShapeKind = union!(A | B)`), and the
//! interpreted side is `moirai_interp::ModelLog` on the descriptor Arachne
//! wrote beside the crate (`metamodel.json`, which `generated_crates.rs` holds
//! equal to `moirai-interp/tests/fixtures/st01.metamodel.json`).
//!
//! Each seat is a pair of replicas, one per runtime, fed the same operation
//! in the same order. From the moment T (the root's first write) has been
//! delivered everywhere, after every send and every delivery the seat that
//! moved is read on both runtimes, the generated `Read` projected onto the
//! canonical form of `02 Validation Plan` §2 by [`project`], written here by
//! hand for this metamodel, and the two must be equal (C4; prediction P7
//! counts the differences). At the end of every schedule the generated
//! replicas must also serve the decided value, V1, which the spec derives by
//! hand: the reached member alone, holding W's +10. The keyed collection's
//! race is the guard: both runtimes serve today, and must keep serving, both
//! members at the removed key.
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
use st01::classifiers::{A, B, Box as Bx, Holder, Shape, ShapeKind};
use st01::package::{St01, St01Log, St01Value};

type Interp = Replica<ModelLog, Tcsb<ModelOp>>;
type Gen = Replica<St01Log, Tcsb<St01>>;

/// The descriptor Arachne wrote beside this crate.
const DESCRIPTOR: &str = include_str!("../metamodel.json");

fn descriptor() -> Value {
    serde_json::from_str(DESCRIPTOR).expect("metamodel.json is JSON")
}

// ---------------------------------------------------------------------------
// The projection of the generated read onto the canonical form, by hand
// ---------------------------------------------------------------------------

/// `St01Value` in the canonical form: `Holder` with `eClass`; `tick` a
/// number; an optional that is `None` an absent key; a `Box` an object with
/// `eClass`; a `union!` value its member directly (`Value`), a conflict
/// `{"__conflict": [...]}` ordered by class name (`Conflict`), nothing
/// (`Unset`) an absent key; the member's inherited `n` read through its
/// `shape_super`; the keyed `bag` an object of its keys, a key whose value
/// reads as nothing absent.
fn project(value: &St01Value) -> Value {
    let raw = serde_json::to_value(value).expect("the generated read serializes");
    let holder = &raw["holder"];
    let mut out = Map::new();
    out.insert("eClass".into(), json!("Holder"));
    out.insert("tick".into(), holder["tick"].clone());
    if let Some(slot) = kind(&holder["slot"]) {
        out.insert("slot".into(), slot);
    }
    let inner = &holder["inner"];
    if !inner.is_null() {
        let mut boxed = Map::new();
        boxed.insert("eClass".into(), json!("Box"));
        if let Some(slot) = kind(&inner["slot"]) {
            boxed.insert("slot".into(), slot);
        }
        out.insert("inner".into(), Value::Object(boxed));
    }
    let mut bag = Map::new();
    for (key, held) in holder["bag"]
        .as_object()
        .unwrap_or_else(|| panic!("`bag` reads as a map: {raw}"))
    {
        if let Some(held) = kind(held) {
            bag.insert(key.clone(), held);
        }
    }
    out.insert("bag".into(), Value::Object(bag));
    Value::Object(out)
}

/// One `ShapeKindValue` (or an `Option` of one), projected.
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

/// `{"A": {"shape_super": {"n": 1}}}` as `{"eClass": "A", "n": 1}`.
fn member(child: &Value) -> Value {
    let (class, payload) = child
        .as_object()
        .and_then(|variant| variant.iter().next())
        .unwrap_or_else(|| panic!("a `union!` child is its variant: {child}"));
    json!({"eClass": class, "n": payload["shape_super"]["n"].clone()})
}

#[test]
fn st01_o5_the_projection_reads_a_hand_written_generated_value() {
    let raw = json!({"holder": {
        "tick": 1,
        "slot": {"Conflict": [{"B": {"shape_super": {"n": 10}}}, {"A": {"shape_super": {"n": 0}}}]},
        "inner": {"slot": {"Value": {"A": {"shape_super": {"n": 3}}}}},
        "bag": {"k": {"Value": {"B": {"shape_super": {"n": 2}}}}, "j": "Unset"}
    }});
    let value: St01Value = serde_json::from_value(raw).expect("the hand-written value parses");
    assert_eq!(
        project(&value),
        json!({
            "eClass": "Holder", "tick": 1,
            "slot": {"__conflict": [{"eClass": "A", "n": 0}, {"eClass": "B", "n": 10}]},
            "inner": {"eClass": "Box", "slot": {"eClass": "A", "n": 3}},
            "bag": {"k": {"eClass": "B", "n": 2}}
        })
    );
    let empty: St01Value = serde_json::from_value(
        json!({"holder": {"tick": 0, "slot": null, "inner": {"slot": null}, "bag": {}}}),
    )
    .expect("parses");
    assert_eq!(
        project(&empty),
        json!({"eClass": "Holder", "tick": 0, "inner": {"eClass": "Box"}, "bag": {}})
    );
}

// ---------------------------------------------------------------------------
// The operations, on both runtimes
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Flat,
    Inner,
    Outer,
    Keyed,
}

const ALL: [Place; 4] = [Place::Flat, Place::Inner, Place::Outer, Place::Keyed];

/// One operation, as each runtime spells it.
#[derive(Clone, Debug)]
struct Op {
    interp: ModelOp,
    generated: St01,
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

    fn tick(&self) -> Op {
        Op {
            interp: self.holder(self.at(
                "Holder",
                "tick",
                InstanceOp::Leaf(LeafOp::Inc(Scalar::Int(1))),
            )),
            generated: St01::Holder(Holder::Tick(Counter::Inc(1))),
        }
    }

    fn object(&self, class: &str, by: i32) -> (InstanceOp, ShapeKind) {
        let interp = self.is(
            class,
            self.at(
                class,
                "n",
                InstanceOp::Leaf(LeafOp::Inc(Scalar::Int(by as i64))),
            ),
        );
        let shape = Shape::N(Counter::Inc(by));
        let generated = if class == "A" {
            ShapeKind::A(A::ShapeSuper(shape))
        } else {
            ShapeKind::B(B::ShapeSuper(shape))
        };
        (interp, generated)
    }

    fn write(&self, place: Place, class: &str, by: i32) -> Op {
        let (object, kind) = self.object(class, by);
        match place {
            Place::Flat => Op {
                interp: self.holder(self.at("Holder", "slot", InstanceOp::set(object))),
                generated: St01::Holder(Holder::Slot(Optional::Set(kind))),
            },
            Place::Inner | Place::Outer => Op {
                interp: self.holder(self.at(
                    "Holder",
                    "inner",
                    InstanceOp::set(
                        self.is("Box", self.at("Box", "slot", InstanceOp::set(object))),
                    ),
                )),
                generated: St01::Holder(Holder::Inner(Optional::Set(Bx::Slot(Optional::Set(
                    kind,
                ))))),
            },
            Place::Keyed => Op {
                interp: self.holder(self.at(
                    "Holder",
                    "bag",
                    InstanceOp::entry(Scalar::text("k"), object),
                )),
                generated: St01::Holder(Holder::Bag(UWMap::Update("k".to_string(), kind))),
            },
        }
    }

    fn removal(&self, place: Place) -> Op {
        match place {
            Place::Flat => Op {
                interp: self.holder(self.at("Holder", "slot", InstanceOp::unset())),
                generated: St01::Holder(Holder::Slot(Optional::Unset)),
            },
            Place::Inner => Op {
                interp: self.holder(self.at(
                    "Holder",
                    "inner",
                    InstanceOp::set(self.is("Box", self.at("Box", "slot", InstanceOp::unset()))),
                )),
                generated: St01::Holder(Holder::Inner(Optional::Set(Bx::Slot(Optional::Unset)))),
            },
            Place::Outer => Op {
                interp: self.holder(self.at("Holder", "inner", InstanceOp::unset())),
                generated: St01::Holder(Holder::Inner(Optional::Unset)),
            },
            Place::Keyed => Op {
                interp: self.holder(self.at(
                    "Holder",
                    "bag",
                    InstanceOp::remove(Scalar::text("k")),
                )),
                generated: St01::Holder(Holder::Bag(UWMap::Remove("k".to_string()))),
            },
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
        project(&self.generated.query(&Read::<St01Value>::new()))
    }
}

/// Three seats, what was sent on both runtimes, and every difference seen.
struct Twins {
    seats: Vec<Seat>,
    sent: Vec<(EventMessage<ModelOp>, EventMessage<St01>)>,
    armed: bool,
    label: String,
    differences: Vec<String>,
    comparisons: usize,
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
        let (ga, gb, gc) = triplet_log::<St01Log>();
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
            steps: 0,
            on_step: None,
        }
    }

    fn compare(&mut self, seat: usize, when: &str) {
        if !self.armed {
            return;
        }
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
        self.compare(seat, &format!("after sending {:?}", op.generated));
        self.stepped();
        self.sent.len() - 1
    }

    fn deliver(&mut self, seat: usize, index: usize) {
        let (interp, generated) = self.sent[index].clone();
        self.seats[seat].interp.receive(interp);
        self.seats[seat].generated.receive(generated);
        self.compare(seat, &format!("after delivering operation {index}"));
        self.stepped();
    }

    fn arm(&mut self) {
        self.armed = true;
        for seat in 0..self.seats.len() {
            self.compare(seat, "once T was delivered everywhere");
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

    /// T from seat 0, delivered everywhere; the comparisons start then.
    fn opening(&mut self, ops: &Ops) {
        let t = self.send(0, &ops.tick());
        self.deliver(1, t);
        self.deliver(2, t);
        self.arm();
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
    member: &'static str,
}

impl Schedule {
    fn other(&self) -> &'static str {
        if self.member == "A" { "B" } else { "A" }
    }

    fn label(&self) -> String {
        format!(
            "{:?}: X on {} Y on {}, third X first {}; U on {} W({}) on {}, third U first {}",
            self.place,
            self.xy.0,
            self.xy.1,
            self.x_first,
            self.uw.0,
            self.member,
            self.uw.1,
            self.u_first
        )
    }
}

fn schedules(place: Place) -> Vec<Schedule> {
    let mut out = Vec::new();
    for xy in SEATS {
        for x_first in [true, false] {
            for uw in SEATS {
                for u_first in [true, false] {
                    for member in ["A", "B"] {
                        out.push(Schedule {
                            place,
                            xy,
                            x_first,
                            uw,
                            u_first,
                            member,
                        });
                    }
                }
            }
        }
    }
    out
}

/// T, then X and Y raced, then U and W raced, on both runtimes.
fn play(ops: &Ops, twins: &mut Twins, schedule: &Schedule) {
    let place = schedule.place;
    twins.opening(ops);
    twins.race(
        schedule.xy,
        &ops.write(place, "A", 1),
        &ops.write(place, "B", 2),
        schedule.x_first,
    );
    twins.race(
        schedule.uw,
        &ops.removal(place),
        &ops.write(place, schedule.member, 10),
        schedule.u_first,
    );
}

fn shape(class: &str, n: i64) -> Value {
    json!({"eClass": class, "n": n})
}

fn conflict(a: i64, b: i64) -> Value {
    json!({"__conflict": [shape("A", a), shape("B", b)]})
}

fn document(place: Place, content: Option<Value>) -> Value {
    let mut doc = json!({"bag": {}, "eClass": "Holder", "tick": 1});
    if let Some(content) = content {
        match place {
            Place::Flat => doc["slot"] = content,
            Place::Inner | Place::Outer => doc["inner"] = json!({"eClass": "Box", "slot": content}),
            Place::Keyed => doc["bag"] = json!({"k": content}),
        }
    }
    doc
}

/// V1 on an optional place; on the keyed collection, what it serves today.
fn expected(schedule: &Schedule) -> Value {
    match schedule.place {
        Place::Keyed => {
            let (a, b) = if schedule.member == "A" {
                (10, 0)
            } else {
                (0, 10)
            };
            document(Place::Keyed, Some(conflict(a, b)))
        }
        place => document(place, Some(shape(schedule.member, 10))),
    }
}

/// The later writes of the spec's O2, from one writer: the reached member's
/// class +1, then the place removed and the emptied member's class +5. Each
/// is followed by what every seat must then serve: the reached member alone
/// holding 11, then the emptied class alone holding 5. On the keyed
/// collection, where the emptied member stays, the +1 leaves both members,
/// and the removal and the +5 leave both, the reached one emptied.
fn later_writes(ops: &Ops, schedule: &Schedule) -> Vec<(Op, Option<Value>)> {
    let (place, member, other) = (schedule.place, schedule.member, schedule.other());
    let after_one = match place {
        Place::Keyed => {
            let (a, b) = if member == "A" { (11, 0) } else { (0, 11) };
            document(place, Some(conflict(a, b)))
        }
        _ => document(place, Some(shape(member, 11))),
    };
    let after_again = match place {
        Place::Keyed => {
            let (a, b) = if member == "A" { (0, 5) } else { (5, 0) };
            document(place, Some(conflict(a, b)))
        }
        _ => document(place, Some(shape(other, 5))),
    };
    vec![
        (ops.write(place, member, 1), Some(after_one)),
        (ops.removal(place), None),
        (ops.write(place, other, 5), Some(after_again)),
    ]
}

fn unset_alone(place: Place) -> Value {
    match place {
        Place::Keyed => document(Place::Keyed, Some(conflict(0, 0))),
        place => document(place, None),
    }
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

fn summary(test: &str, runs: usize, comparisons: usize, differences: &[String], values: &[String]) {
    println!(
        "ST01-O5 {test}: runs {runs}, twin comparisons {comparisons}, twin differences {} (P7), \
         generated values other than expected {}",
        differences.len(),
        values.len()
    );
    for place in ALL {
        let prefix = format!("{place:?}:");
        println!(
            "ST01-O5 {test}: {place:?}: twin differences {}, generated values other than \
             expected {}",
            differences
                .iter()
                .filter(|line| line.starts_with(&prefix))
                .count(),
            values
                .iter()
                .filter(|line| line.starts_with(&prefix))
                .count()
        );
    }
    for line in differences.iter().chain(values).take(8) {
        println!("ST01-O5 {line}");
    }
}

fn verdict(test: &str, runs: usize, differences: &[String], values: &[String]) {
    assert!(
        differences.is_empty() && values.is_empty(),
        "ST.01 O5 {test}: {} twin differences and {} generated values other than expected over \
         {runs} runs; first:\n  {}",
        differences.len(),
        values.len(),
        differences
            .iter()
            .chain(values)
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// O5
// ---------------------------------------------------------------------------

/// Every schedule of the claim (864) and of the keyed guard (288): twin
/// equality after every step once T is everywhere, and at the end the
/// generated replicas serve V1 (the guard: what it serves today).
#[test]
fn st01_o5_every_schedule_both_runtimes_agree_after_every_step_and_serve_v1() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let (mut runs, mut comparisons) = (0, 0);
    let (mut differences, mut values) = (Vec::new(), Vec::new());
    for place in ALL {
        for schedule in schedules(place) {
            runs += 1;
            let label = schedule.label();
            let mut twins = Twins::new(&desc, label.clone());
            play(&ops, &mut twins, &schedule);
            values.extend(generated_against(&twins, &expected(&schedule), &label));
            comparisons += twins.comparisons;
            differences.append(&mut twins.differences);
        }
    }
    summary("schedules", runs, comparisons, &differences, &values);
    verdict("schedules", runs, &differences, &values);
}

/// U alone, in the four places: 12 conflict schedules x 3 seats of U.
#[test]
fn st01_o5_an_unset_alone_agrees_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let (mut runs, mut comparisons) = (0, 0);
    let (mut differences, mut values) = (Vec::new(), Vec::new());
    for place in ALL {
        for xy in SEATS {
            for x_first in [true, false] {
                for u in 0..3 {
                    runs += 1;
                    let label = format!(
                        "{place:?}: X on {} Y on {}, third X first {x_first}; U alone on {u}",
                        xy.0, xy.1
                    );
                    let mut twins = Twins::new(&desc, label.clone());
                    twins.opening(&ops);
                    twins.race(
                        xy,
                        &ops.write(place, "A", 1),
                        &ops.write(place, "B", 2),
                        x_first,
                    );
                    let removal = twins.send(u, &ops.removal(place));
                    for seat in 0..3 {
                        if seat != u {
                            twins.deliver(seat, removal);
                        }
                    }
                    values.extend(generated_against(&twins, &unset_alone(place), &label));
                    comparisons += twins.comparisons;
                    differences.append(&mut twins.differences);
                }
            }
        }
    }
    summary("unset alone", runs, comparisons, &differences, &values);
    verdict("unset alone", runs, &differences, &values);
}

/// After the race, one seat makes the later writes of [`later_writes`], each
/// delivered everywhere; each seat takes that turn in its own run. The
/// conflict seated X on 0, Y on 1, the third taking X first; U and W in every
/// seating, the third taking U first; both members; all four places.
#[test]
fn st01_o5_writes_after_the_race_agree_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let (mut runs, mut comparisons) = (0, 0);
    let (mut differences, mut values) = (Vec::new(), Vec::new());
    for place in ALL {
        for uw in SEATS {
            for member in ["A", "B"] {
                for writer in 0..3 {
                    runs += 1;
                    let schedule = Schedule {
                        place,
                        xy: (0, 1),
                        x_first: true,
                        uw,
                        u_first: true,
                        member,
                    };
                    let label = format!("{}; then seat {writer} writes", schedule.label());
                    let mut twins = Twins::new(&desc, label.clone());
                    play(&ops, &mut twins, &schedule);
                    for (op, want) in later_writes(&ops, &schedule) {
                        let index = twins.send(writer, &op);
                        for seat in 0..3 {
                            if seat != writer {
                                twins.deliver(seat, index);
                            }
                        }
                        if let Some(want) = want {
                            values.extend(generated_against(
                                &twins,
                                &want,
                                &format!("{label}, after {:?}", op.generated),
                            ));
                        }
                    }
                    comparisons += twins.comparisons;
                    differences.append(&mut twins.differences);
                }
            }
        }
    }
    summary(
        "writes after the race",
        runs,
        comparisons,
        &differences,
        &values,
    );
    verdict("writes after the race", runs, &differences, &values);
}

/// A joiner `d` on each runtime adopts, after the same step, the same seat's
/// causal snapshot and log (each through a serde round trip), delivers every
/// operation sent, in the order sent, and must serve on each runtime what
/// its twin serves and what the three serve; then it makes the later writes
/// of [`later_writes`], delivered to the three, and all four seats agree
/// across runtimes and serve what those say. The conflict seated X
/// on 0, Y on 1; U and W on 0 and 1 in both assignments; both members; the
/// four places; each seat as donor after each of the 15 steps.
#[test]
fn st01_o5_a_joiner_by_state_transfer_agrees_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let (mut runs, mut comparisons) = (0, 0);
    let (mut differences, mut values) = (Vec::new(), Vec::new());
    for place in ALL {
        for uw in [(0, 1), (1, 0)] {
            for member in ["A", "B"] {
                let schedule = Schedule {
                    place,
                    xy: (0, 1),
                    x_first: true,
                    uw,
                    u_first: true,
                    member,
                };
                for donor in 0..3 {
                    for at in 1..=15 {
                        runs += 1;
                        let label = format!(
                            "{}; d joins from seat {donor} after step {at}",
                            schedule.label()
                        );
                        let taken: std::rc::Rc<std::cell::RefCell<Option<Seat>>> =
                            Default::default();
                        let mut twins = Twins::new(&desc, label.clone());
                        let slot = taken.clone();
                        twins.on_step = Some(Box::new(move |step, seats| {
                            if step == at {
                                *slot.borrow_mut() = Some(join(&seats[donor]));
                            }
                        }));
                        play(&ops, &mut twins, &schedule);
                        twins.on_step = None;
                        let d = taken
                            .borrow_mut()
                            .take()
                            .expect("set-up: the schedule has 15 steps");
                        twins.seats.push(d);
                        let sent = twins.sent.clone();
                        for (interp, generated) in sent {
                            twins.seats[3].interp.receive(interp);
                            twins.seats[3].generated.receive(generated);
                        }
                        twins.compare(3, "once the joiner delivered everything");
                        let want = expected(&schedule);
                        let joined = twins.seats[3].generated_doc();
                        if joined != want {
                            values.push(format!(
                                "{label}: the generated joiner serves {joined}, not {want}"
                            ));
                        }
                        for (op, want) in later_writes(&ops, &schedule) {
                            let index = twins.send(3, &op);
                            for seat in 0..3 {
                                twins.deliver(seat, index);
                            }
                            if let Some(want) = want {
                                values.extend(generated_against(
                                    &twins,
                                    &want,
                                    &format!("{label}, d wrote {:?}", op.generated),
                                ));
                            }
                        }
                        comparisons += twins.comparisons;
                        differences.append(&mut twins.differences);
                    }
                }
            }
        }
    }
    summary("joiner", runs, comparisons, &differences, &values);
    verdict("joiner", runs, &differences, &values);
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

    let log: St01Log = serde_json::from_str(
        &serde_json::to_string(donor.generated.log()).expect("the generated log serializes"),
    )
    .expect("the generated log comes back");
    let snapshot: StateSnapshot<St01> = serde_json::from_str(
        &serde_json::to_string(&donor.generated.snapshot()).expect("the snapshot serializes"),
    )
    .expect("the snapshot comes back");
    let mut generated: Gen =
        Replica::bootstrap_with_log_id("d".to_string(), &["d"], donor.generated.log_id().clone());
    generated.adopt(snapshot, log);
    Seat { interp, generated }
}
