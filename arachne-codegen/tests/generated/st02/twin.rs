//! ST.02's twin, A5 of `cea-cdrt-knowledge/spec/items/ST.02.md`: the runtime
//! Arachne generates for `st02.ecore` against the interpreted one, on every
//! schedule of claim A and of its guards.
//!
//! `generated_crates.rs` generates the crate `st02` from `st02.ecore` with the
//! generator at this checkout, against the model-plane Moirai checkout, and
//! copies this file into its `tests/`. So the generated side is `OptionLog`
//! and `record!` as the library holds them now, composed as Arachne composes
//! an optional containment of a concrete class (`OptionLog<CellLog>`,
//! `OptionLog<MidLog>`, `OptionLog<BoxLog>`), and the interpreted side is
//! `moirai_interp::ModelLog` on the descriptor Arachne wrote beside the crate
//! (`metamodel.json`, which `generated_crates.rs` holds equal to
//! `moirai-interp/tests/fixtures/st02.metamodel.json`).
//!
//! Each seat is a pair of replicas, one per runtime, fed the same operation
//! in the same order. From the moment T (the root's first write) has been
//! delivered everywhere, after every send and every delivery the seat that
//! moved is read on both runtimes, the generated `Read` projected onto the
//! canonical form by [`project`], written here by hand for this metamodel,
//! and the two must be equal (the spec's P7 counts the differences). In the
//! `Keyed` place the seats are compared at checkpoints only, once every
//! operation sent has been delivered everywhere (the end of a schedule, each
//! later write, the joiner): in between, a replica that has delivered the
//! key's removal and not W holds a removed key whose `Box` is emptied, which
//! the two runtimes read differently for a reason outside the claim (the
//! keyed collection's own emptiness rule, the spec's OB2). At the
//! end of every schedule the generated replicas must also serve the value
//! the rule fixes, which the spec derives by hand (P8): an object the
//! removal emptied and nothing wrote after it is dropped; an object W
//! reached survives holding W's 10.
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
use st02::classifiers::{Box as Bx, Cell, Holder, Mid};
use st02::package::{St02, St02Log, St02Value};

type Interp = Replica<ModelLog, Tcsb<ModelOp>>;
type Gen = Replica<St02Log, Tcsb<St02>>;

/// The descriptor Arachne wrote beside this crate.
const DESCRIPTOR: &str = include_str!("../metamodel.json");

fn descriptor() -> Value {
    serde_json::from_str(DESCRIPTOR).expect("metamodel.json is JSON")
}

// ---------------------------------------------------------------------------
// The projection of the generated read onto the canonical form, by hand
// ---------------------------------------------------------------------------

/// `St02Value` in the canonical form: `Holder` with `eClass`; `tick` a
/// number; an optional that is `None` an absent key; a `Box`, a `Mid` and a
/// `Cell` objects with their `eClass` and every single-valued counter (`k`,
/// `n`) whatever it holds; `Box.note`, an optional counter, a number or an
/// absent key; the keyed `bag` an object of its keys (the generated read
/// already leaves out a key whose value reads as the default).
fn project(value: &St02Value) -> Value {
    let raw = serde_json::to_value(value).expect("the generated read serializes");
    let holder = &raw["holder"];
    let mut out = Map::new();
    out.insert("eClass".into(), json!("Holder"));
    out.insert("tick".into(), holder["tick"].clone());
    if !holder["inner"].is_null() {
        out.insert("inner".into(), boxed_of(&holder["inner"]));
    }
    let mut bag = Map::new();
    for (key, held) in holder["bag"]
        .as_object()
        .unwrap_or_else(|| panic!("`bag` reads as a map: {raw}"))
    {
        bag.insert(key.clone(), boxed_of(held));
    }
    out.insert("bag".into(), Value::Object(bag));
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
    if !value["cell"].is_null() {
        out.insert("cell".into(), cell_of(&value["cell"]));
    }
    if !value["mid"].is_null() {
        let held = &value["mid"];
        let mut mid = Map::new();
        mid.insert("eClass".into(), json!("Mid"));
        mid.insert("k".into(), held["k"].clone());
        if !held["cell"].is_null() {
            mid.insert("cell".into(), cell_of(&held["cell"]));
        }
        out.insert("mid".into(), Value::Object(mid));
    }
    Value::Object(out)
}

/// One `CellValue`, projected.
fn cell_of(value: &Value) -> Value {
    json!({"eClass": "Cell", "n": value["n"].clone()})
}

#[test]
fn st02_a5_the_projection_reads_a_hand_written_generated_value() {
    let raw = json!({"holder": {
        "tick": 1,
        "inner": {"k": 10, "note": null, "cell": {"n": 0},
                  "mid": {"k": 3, "cell": null}},
        "bag": {"k": {"k": 0, "note": 4, "cell": null, "mid": null}}
    }});
    let value: St02Value = serde_json::from_value(raw).expect("the hand-written value parses");
    assert_eq!(
        project(&value),
        json!({
            "eClass": "Holder", "tick": 1,
            "inner": {"eClass": "Box", "k": 10, "cell": {"eClass": "Cell", "n": 0},
                      "mid": {"eClass": "Mid", "k": 3}},
            "bag": {"k": {"eClass": "Box", "k": 0, "note": 4}}
        })
    );
    let empty: St02Value =
        serde_json::from_value(json!({"holder": {"tick": 0, "inner": null, "bag": {}}}))
            .expect("parses");
    assert_eq!(
        project(&empty),
        json!({"eClass": "Holder", "tick": 0, "bag": {}})
    );
}

// ---------------------------------------------------------------------------
// The operations, on both runtimes
// ---------------------------------------------------------------------------

/// Where the optional containment of `Cell` sits and what removes it, as in
/// the spec: `Flat` (`Box.cell`, U unsets `Holder.inner`), `Deep`
/// (`Mid.cell` in `Box.mid`, U unsets `Holder.inner`), `Keyed` (`Box.cell`
/// of the `Box` at key `k` of `Holder.bag`, U removes the key), `Direct`
/// (the guard: `Box.cell`, U unsets it itself).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Flat,
    Deep,
    Direct,
    Keyed,
}

impl Place {
    fn targets(self) -> &'static [Target] {
        match self {
            Place::Deep => &[Target::Outer, Target::Sibling, Target::Object],
            _ => &[Target::Sibling, Target::Object],
        }
    }
}

/// The places of claim A and of its guard; those of the removal-alone
/// guard.
const TWIN: [Place; 4] = [Place::Flat, Place::Deep, Place::Keyed, Place::Direct];
const CLAIM: [Place; 3] = [Place::Flat, Place::Deep, Place::Keyed];
const ALONE: [Place; 3] = [Place::Flat, Place::Deep, Place::Direct];

/// What W increments: the owner's `k` (`Sibling`), the `Cell`'s `n`
/// (`Object`), or, in `Deep`, `Box.k` (`Outer`). `Note`, `Box.note`, serves
/// the observation OB1 only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Outer,
    Sibling,
    Object,
    Note,
}

/// One operation, as each runtime spells it.
#[derive(Clone, Debug)]
struct Op {
    interp: ModelOp,
    generated: St02,
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
            generated: St02::Holder(Holder::Tick(Counter::Inc(1))),
        }
    }

    fn cell(&self, by: i32) -> (InstanceOp, Optional<Cell>) {
        (
            InstanceOp::set(self.is("Cell", self.at("Cell", "n", Self::inc(by)))),
            Optional::Set(Cell::N(Counter::Inc(by))),
        )
    }

    /// The operation on the `Box` that increments `target` by `by`.
    fn on_box(&self, place: Place, target: Target, by: i32) -> (InstanceOp, Bx) {
        match (place, target) {
            (Place::Deep, Target::Object) => {
                let (interp, generated) = self.cell(by);
                (
                    self.at(
                        "Box",
                        "mid",
                        InstanceOp::set(self.is("Mid", self.at("Mid", "cell", interp))),
                    ),
                    Bx::Mid(Optional::Set(Mid::Cell(generated))),
                )
            }
            (Place::Deep, Target::Sibling) => (
                self.at(
                    "Box",
                    "mid",
                    InstanceOp::set(self.is("Mid", self.at("Mid", "k", Self::inc(by)))),
                ),
                Bx::Mid(Optional::Set(Mid::K(Counter::Inc(by)))),
            ),
            (_, Target::Object) => {
                let (interp, generated) = self.cell(by);
                (self.at("Box", "cell", interp), Bx::Cell(generated))
            }
            (_, Target::Note) => (
                self.at("Box", "note", InstanceOp::set(Self::inc(by))),
                Bx::Note(Optional::Set(Counter::Inc(by))),
            ),
            (Place::Deep, Target::Outer) | (_, Target::Sibling) => {
                (self.at("Box", "k", Self::inc(by)), Bx::K(Counter::Inc(by)))
            }
            (place, Target::Outer) => panic!("set-up: no Outer in {place:?}"),
        }
    }

    /// A write at the place that increments `target` by `by`.
    fn write(&self, place: Place, target: Target, by: i32) -> Op {
        let (interp, generated) = self.on_box(place, target, by);
        match place {
            Place::Keyed => Op {
                interp: self.holder(self.at(
                    "Holder",
                    "bag",
                    InstanceOp::entry(Scalar::text("k"), self.is("Box", interp)),
                )),
                generated: St02::Holder(Holder::Bag(UWMap::Update("k".to_string(), generated))),
            },
            _ => Op {
                interp: self.holder(self.at(
                    "Holder",
                    "inner",
                    InstanceOp::set(self.is("Box", interp)),
                )),
                generated: St02::Holder(Holder::Inner(Optional::Set(generated))),
            },
        }
    }

    /// U, the removal of the place.
    fn removal(&self, place: Place) -> Op {
        match place {
            Place::Flat | Place::Deep => Op {
                interp: self.holder(self.at("Holder", "inner", InstanceOp::unset())),
                generated: St02::Holder(Holder::Inner(Optional::Unset)),
            },
            Place::Direct => Op {
                interp: self.holder(self.at(
                    "Holder",
                    "inner",
                    InstanceOp::set(self.is("Box", self.at("Box", "cell", InstanceOp::unset()))),
                )),
                generated: St02::Holder(Holder::Inner(Optional::Set(Bx::Cell(Optional::Unset)))),
            },
            Place::Keyed => Op {
                interp: self.holder(self.at(
                    "Holder",
                    "bag",
                    InstanceOp::remove(Scalar::text("k")),
                )),
                generated: St02::Holder(Holder::Bag(UWMap::Remove("k".to_string()))),
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
        project(&self.generated.query(&Read::<St02Value>::new()))
    }
}

/// Three seats, what was sent on both runtimes, and every difference seen.
struct Twins {
    seats: Vec<Seat>,
    sent: Vec<(EventMessage<ModelOp>, EventMessage<St02>)>,
    armed: bool,
    /// Compared after every step once armed (`true`), or at checkpoints only.
    per_step: bool,
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
        Self::at(desc, label, true)
    }

    /// Seats for `place`: compared after every step, or, in `Keyed`, at
    /// checkpoints only.
    fn for_place(desc: &Value, label: String, place: Place) -> Self {
        Self::at(desc, label, place != Place::Keyed)
    }

    fn at(desc: &Value, label: String, per_step: bool) -> Self {
        let (mut ia, mut ib, mut ic) = triplet_log::<ModelLog>();
        let opening = ia
            .send(install("m", desc))
            .expect("a fresh log takes its `Install`");
        ib.receive(opening.clone());
        ic.receive(opening);
        let (ga, gb, gc) = triplet_log::<St02Log>();
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
            per_step,
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
        if !self.per_step {
            return;
        }
        self.armed = true;
        for seat in 0..self.seats.len() {
            self.compare(seat, "once T was delivered everywhere");
        }
    }

    /// Once every operation sent has been delivered everywhere: every seat
    /// compared, where seats are compared at checkpoints only.
    fn checkpoint(&mut self, when: &str) {
        if self.per_step {
            return;
        }
        self.armed = true;
        for seat in 0..self.seats.len() {
            self.compare(seat, when);
        }
        self.armed = false;
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

    /// The interpreted seats' reads, all of them.
    fn interp_docs(&self) -> Vec<Value> {
        self.seats.iter().map(Seat::interp_doc).collect()
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

    fn drops_cell(&self) -> bool {
        self.target != Target::Object
    }
}

fn schedules(place: Place, targets: &[Target]) -> Vec<Schedule> {
    let mut out = Vec::new();
    for xy in SEATS {
        for x_first in [true, false] {
            for uw in SEATS {
                for u_first in [true, false] {
                    for &target in targets {
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

/// T, then X (the `Cell` +1) and Y (the owner's `k` +2) raced, then U and W
/// raced, on both runtimes.
fn play(ops: &Ops, twins: &mut Twins, schedule: &Schedule) {
    let place = schedule.place;
    twins.opening(ops);
    twins.race(
        schedule.xy,
        &ops.write(place, Target::Object, 1),
        &ops.write(place, Target::Sibling, 2),
        schedule.x_first,
    );
    twins.race(
        schedule.uw,
        &ops.removal(place),
        &ops.write(place, schedule.target, 10),
        schedule.u_first,
    );
    twins.checkpoint("once every operation was delivered");
}

fn cell(n: i64) -> Value {
    json!({"eClass": "Cell", "n": n})
}

fn mid(k: i64, held: Option<Value>) -> Value {
    let mut out = json!({"eClass": "Mid", "k": k});
    if let Some(held) = held {
        out["cell"] = held;
    }
    out
}

fn boxed(k: i64, held: Option<Value>, below: Option<Value>) -> Value {
    let mut out = json!({"eClass": "Box", "k": k});
    if let Some(held) = held {
        out["cell"] = held;
    }
    if let Some(below) = below {
        out["mid"] = below;
    }
    out
}

/// The whole document, the place's `Box` holding `content` (under
/// `Holder.inner`, or at key `k` of `Holder.bag` in `Keyed`), or no `Box`.
fn document(place: Place, content: Option<Value>) -> Value {
    let mut doc = json!({"bag": {}, "eClass": "Holder", "tick": 1});
    if let Some(content) = content {
        match place {
            Place::Keyed => doc["bag"] = json!({"k": content}),
            _ => doc["inner"] = content,
        }
    }
    doc
}

/// The rule's value once every seat has delivered T, X, Y, U and W (spec
/// section 2): the object W reached holds 10; what U emptied and W did not
/// reach is dropped; in `Direct` the `Box` keeps Y's 2 beside W's 10.
fn expected(schedule: &Schedule) -> Value {
    let content = match (schedule.place, schedule.target) {
        (Place::Flat | Place::Keyed, Target::Sibling) => boxed(10, None, None),
        (Place::Flat | Place::Keyed, Target::Object) => boxed(0, Some(cell(10)), None),
        (Place::Deep, Target::Outer) => boxed(10, None, None),
        (Place::Deep, Target::Sibling) => boxed(0, None, Some(mid(10, None))),
        (Place::Deep, Target::Object) => boxed(0, None, Some(mid(0, Some(cell(10))))),
        (Place::Direct, Target::Sibling) => boxed(12, None, None),
        (Place::Direct, Target::Object) => boxed(2, Some(cell(10)), None),
        (place, target) => panic!("set-up: no expected value for {target:?} in {place:?}"),
    };
    document(schedule.place, Some(content))
}

/// U alone: `Holder.inner` unset (`Flat`, `Deep`); the `Box` holding Y's 2
/// and no `Cell` (`Direct`).
fn unset_alone(place: Place) -> Value {
    match place {
        Place::Flat | Place::Deep => document(place, None),
        Place::Direct => document(place, Some(boxed(2, None, None))),
        Place::Keyed => panic!("set-up: a removed key alone is outside the claim (OB2)"),
    }
}

/// The later writes of the spec's A2, from one writer: W's target +1, then,
/// where the race dropped the `Cell`, the `Cell` +5. Each is followed by what
/// every seat must then serve.
fn later_writes(ops: &Ops, schedule: &Schedule) -> Vec<(Op, Value)> {
    let place = schedule.place;
    let again = match (place, schedule.target) {
        (Place::Flat | Place::Keyed, Target::Sibling) => boxed(11, None, None),
        (Place::Flat | Place::Keyed, Target::Object) => boxed(0, Some(cell(11)), None),
        (Place::Deep, Target::Outer) => boxed(11, None, None),
        (Place::Deep, Target::Sibling) => boxed(0, None, Some(mid(11, None))),
        (Place::Deep, Target::Object) => boxed(0, None, Some(mid(0, Some(cell(11))))),
        (Place::Direct, Target::Sibling) => boxed(13, None, None),
        (Place::Direct, Target::Object) => boxed(2, Some(cell(11)), None),
        (place, target) => panic!("set-up: no later writes for {target:?} in {place:?}"),
    };
    let mut out = vec![(
        ops.write(place, schedule.target, 1),
        document(place, Some(again)),
    )];
    if schedule.drops_cell() {
        let rebuilt = match (place, schedule.target) {
            (Place::Flat | Place::Keyed, Target::Sibling) => boxed(11, Some(cell(5)), None),
            (Place::Deep, Target::Outer) => boxed(11, None, Some(mid(0, Some(cell(5))))),
            (Place::Deep, Target::Sibling) => boxed(0, None, Some(mid(11, Some(cell(5))))),
            (Place::Direct, Target::Sibling) => boxed(13, Some(cell(5)), None),
            (place, target) => panic!("set-up: nothing rebuilt for {target:?} in {place:?}"),
        };
        out.push((
            ops.write(place, Target::Object, 5),
            document(place, Some(rebuilt)),
        ));
    }
    out
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

fn summary(
    test: &str,
    places: &[Place],
    runs: usize,
    comparisons: usize,
    differences: &[String],
    values: &[String],
    parted: &[String],
) {
    println!(
        "ST02-A5 {test}: runs {runs}, twin comparisons {comparisons}, twin differences {} (P7), \
         generated values other than expected {} (P8), generated runs parted {}",
        differences.len(),
        values.len(),
        parted.len()
    );
    for place in places {
        let prefix = format!("{place:?}:");
        let count = |lines: &[String]| lines.iter().filter(|l| l.starts_with(&prefix)).count();
        println!(
            "ST02-A5 {test}: {place:?}: twin differences {}, generated values other than \
             expected {}, generated runs parted {}",
            count(differences),
            count(values),
            count(parted)
        );
    }
    for line in differences.iter().chain(values).take(6) {
        println!("ST02-A5 {line}");
    }
}

fn verdict(test: &str, runs: usize, differences: &[String], values: &[String], parted: &[String]) {
    assert!(
        differences.is_empty() && values.is_empty() && parted.is_empty(),
        "ST.02 A5 {test}: {} twin differences, {} generated values other than expected and {} \
         generated runs parted, over {runs} runs; first:\n  {}",
        differences.len(),
        values.len(),
        parted.len(),
        differences
            .iter()
            .chain(values)
            .chain(parted)
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// A5
// ---------------------------------------------------------------------------

/// Every schedule of claim A (Flat 288, Deep 432, Keyed 288) and of the
/// guard (Direct 288): twin equality after every step once T is everywhere
/// (in `Keyed`, once every operation is delivered); at the end the generated
/// seats serve one document, the rule's.
#[test]
fn st02_a5_every_schedule_both_runtimes_agree_after_every_step_and_serve_the_rule() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let (mut runs, mut comparisons) = (0, 0);
    let (mut differences, mut values, mut parted) = (Vec::new(), Vec::new(), Vec::new());
    for place in TWIN {
        for schedule in schedules(place, place.targets()) {
            runs += 1;
            let label = schedule.label();
            let mut twins = Twins::for_place(&desc, label.clone(), place);
            play(&ops, &mut twins, &schedule);
            values.extend(generated_against(&twins, &expected(&schedule), &label));
            if generated_parted(&twins) {
                parted.push(format!(
                    "{label}: the generated seats part: {:?}",
                    twins.generated_docs()
                ));
            }
            comparisons += twins.comparisons;
            differences.append(&mut twins.differences);
        }
    }
    summary(
        "schedules",
        &TWIN,
        runs,
        comparisons,
        &differences,
        &values,
        &parted,
    );
    verdict("schedules", runs, &differences, &values, &parted);
}

/// U alone, in `Flat`, `Deep` and `Direct`: 12 formations x 3 seats of U. A
/// removed key alone is outside the claim (OB2).
#[test]
fn st02_a5_a_removal_alone_agrees_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let (mut runs, mut comparisons) = (0, 0);
    let (mut differences, mut values) = (Vec::new(), Vec::new());
    for place in ALONE {
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
                        &ops.write(place, Target::Object, 1),
                        &ops.write(place, Target::Sibling, 2),
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
    summary(
        "removal alone",
        &ALONE,
        runs,
        comparisons,
        &differences,
        &values,
        &[],
    );
    verdict("removal alone", runs, &differences, &values, &[]);
}

/// After the race, one seat makes the later writes of [`later_writes`], each
/// delivered everywhere; each seat takes that turn in its own run. X on 0,
/// Y on 1, the third taking X first; U and W in every seating, the third
/// taking U first; every target; the four places.
#[test]
fn st02_a5_writes_after_the_race_agree_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let (mut runs, mut comparisons) = (0, 0);
    let (mut differences, mut values) = (Vec::new(), Vec::new());
    for place in TWIN {
        for uw in SEATS {
            for &target in place.targets() {
                for writer in 0..3 {
                    runs += 1;
                    let schedule = Schedule {
                        place,
                        xy: (0, 1),
                        x_first: true,
                        uw,
                        u_first: true,
                        target,
                    };
                    let label = format!("{}; then seat {writer} writes", schedule.label());
                    let mut twins = Twins::for_place(&desc, label.clone(), place);
                    play(&ops, &mut twins, &schedule);
                    for (op, want) in later_writes(&ops, &schedule) {
                        let index = twins.send(writer, &op);
                        for seat in 0..3 {
                            if seat != writer {
                                twins.deliver(seat, index);
                            }
                        }
                        twins.checkpoint(&format!("once {:?} was delivered", op.generated));
                        values.extend(generated_against(
                            &twins,
                            &want,
                            &format!("{label}, after {:?}", op.generated),
                        ));
                    }
                    comparisons += twins.comparisons;
                    differences.append(&mut twins.differences);
                }
            }
        }
    }
    summary(
        "writes after the race",
        &TWIN,
        runs,
        comparisons,
        &differences,
        &values,
        &[],
    );
    verdict("writes after the race", runs, &differences, &values, &[]);
}

/// A joiner `d` on each runtime adopts, after the same step, the same seat's
/// causal snapshot and log (each through a serde round trip), delivers every
/// operation sent, in the order sent, and must serve on each runtime what
/// its twin serves and the rule's document; then it makes the later writes
/// of [`later_writes`], delivered to the three, and all four seats agree
/// across runtimes and serve what those say. X on 0, Y on 1; U and W on 0
/// and 1 in both assignments; every target; the three places of the claim;
/// each seat as donor after each of the 15 steps.
#[test]
fn st02_a5_a_joiner_by_state_transfer_agrees_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let (mut runs, mut comparisons) = (0, 0);
    let (mut differences, mut values) = (Vec::new(), Vec::new());
    for place in CLAIM {
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
                        runs += 1;
                        let label = format!(
                            "{}; d joins from seat {donor} after step {at}",
                            schedule.label()
                        );
                        let taken: std::rc::Rc<std::cell::RefCell<Option<Seat>>> =
                            Default::default();
                        let mut twins = Twins::for_place(&desc, label.clone(), place);
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
                        twins.checkpoint("once the joiner delivered everything");
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
                            twins.checkpoint(&format!("once d's {:?} was delivered", op.generated));
                            values.extend(generated_against(
                                &twins,
                                &want,
                                &format!("{label}, d wrote {:?}", op.generated),
                            ));
                        }
                        comparisons += twins.comparisons;
                        differences.append(&mut twins.differences);
                    }
                }
            }
        }
    }
    summary(
        "joiner",
        &CLAIM,
        runs,
        comparisons,
        &differences,
        &values,
        &[],
    );
    verdict("joiner", runs, &differences, &values, &[]);
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

    let log: St02Log = serde_json::from_str(
        &serde_json::to_string(donor.generated.log()).expect("the generated log serializes"),
    )
    .expect("the generated log comes back");
    let snapshot: StateSnapshot<St02> = serde_json::from_str(
        &serde_json::to_string(&donor.generated.snapshot()).expect("the snapshot serializes"),
    )
    .expect("the snapshot comes back");
    let mut generated: Gen =
        Replica::bootstrap_with_log_id("d".to_string(), &["d"], donor.generated.log_id().clone());
    generated.adopt(snapshot, log);
    Seat { interp, generated }
}

// ---------------------------------------------------------------------------
// Observations, outside the claim: not pass criteria
// ---------------------------------------------------------------------------

/// OB1 and OB2 of the spec, printed and not asserted. OB1: X writes
/// `Box.note` (an optional attribute, a leaf) instead of the `Cell`, and W
/// increments `Box.k`, in `Flat`. OB2: the `Keyed` place compared at every
/// step (A5 compares it at checkpoints), and U alone at a removed key. Each
/// line counts the twin differences at every step, the runs whose generated
/// seats part, and the runs whose interpreted seats part.
#[test]
#[ignore = "ST.02 observations OB1 and OB2, not pass criteria: run with --ignored --nocapture"]
fn st02_observation_an_optional_attribute_and_a_keyed_collection_reached_by_a_removal() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let cases: [(&str, Place, Target, &[Target]); 3] = [
        ("OB1 note", Place::Flat, Target::Note, &[Target::Sibling]),
        (
            "OB2 keyed, every step",
            Place::Keyed,
            Target::Object,
            &[Target::Sibling, Target::Object],
        ),
        ("OB2 keyed, U alone", Place::Keyed, Target::Object, &[]),
    ];
    for (name, place, x_target, w_targets) in cases {
        let (mut runs, mut differences, mut gen_parted, mut int_parted) = (0, 0, 0, 0);
        let mut first = Vec::new();
        let mut shown = Vec::new();
        for xy in SEATS {
            for x_first in [true, false] {
                let races: Vec<Option<((usize, usize), bool, Target)>> = if w_targets.is_empty() {
                    vec![None; 3]
                } else {
                    let mut out = Vec::new();
                    for uw in SEATS {
                        for u_first in [true, false] {
                            for &t in w_targets {
                                out.push(Some((uw, u_first, t)));
                            }
                        }
                    }
                    out
                };
                for (index, race) in races.into_iter().enumerate() {
                    runs += 1;
                    let label = format!("{name}: X on {} Y on {}, {race:?} #{index}", xy.0, xy.1);
                    let mut twins = Twins::new(&desc, label.clone());
                    twins.opening(&ops);
                    twins.race(
                        xy,
                        &ops.write(place, x_target, 1),
                        &ops.write(place, Target::Sibling, 2),
                        x_first,
                    );
                    match race {
                        Some((uw, u_first, t)) => {
                            twins.race(uw, &ops.removal(place), &ops.write(place, t, 10), u_first)
                        }
                        None => {
                            let removal = twins.send(index, &ops.removal(place));
                            for seat in 0..3 {
                                if seat != index {
                                    twins.deliver(seat, removal);
                                }
                            }
                        }
                    }
                    if generated_parted(&twins) {
                        gen_parted += 1;
                    }
                    if twins.interp_docs().windows(2).any(|p| p[0] != p[1]) {
                        int_parted += 1;
                    }
                    differences += twins.differences.len();
                    if first.is_empty() {
                        first = twins.differences.clone();
                    }
                    if shown.len() < 2 {
                        shown.push(format!(
                            "{label}: interpreted {:?} | generated {:?}",
                            twins.interp_docs(),
                            twins.generated_docs()
                        ));
                    }
                }
            }
        }
        println!(
            "ST02-OB {name}: runs {runs}, twin differences {differences}, generated runs parted \
             {gen_parted}, interpreted runs parted {int_parted}"
        );
        for line in first.iter().take(2).chain(shown.iter()) {
            println!("ST02-OB {line}");
        }
    }
}
