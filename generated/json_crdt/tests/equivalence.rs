//! The equivalence oracle over `json.ecore`: `ip28`, the second driver of
//! criterion I-A1.
//!
//! # What is being claimed
//!
//! The same claim `generated/bt_crdt/tests/equivalence.rs` makes for
//! `bt.ecore`, over the metamodel the interpreted path was *not* designed
//! around. `json.ecore` reaches two constructions `bt.ecore` never does — a
//! `uw-map` keyed containment and five classes the
//! `urn:arachne:representation` annotation makes transparent — and reaches
//! two leaves `bt.ecore` never does, a `Counter<f64>` and an `EWFlag`. Every
//! one of them was out of scope until decision D6 was amended on 2026-09-08,
//! and this file is what says the amendment works rather than compiles.
//!
//! # Why this driver is shorter than the behaviour tree's
//!
//! `classifiers.rs` for this metamodel is one `union!` and five type
//! aliases. There is no `record!` anywhere, so there are no `<Super>Super`
//! hops to walk, no field names to derive with `heck`, and no eagerly
//! materialised record to prune: a `JsonLog` that has just been constructed
//! holds `JsonKindValue::Unset`, which is `null`, and the interpreted model
//! that has just been opened is `null` too. The `without_defaults` pass the
//! behaviour tree's oracle needs on both sides before it can compare them has
//! no work to do here and is not written. The two read-outs are compared
//! **raw**.
//!
//! # The two arms
//!
//! Interpreted: a pair of `moirai_interp::testing::Harness` replicas, which
//! is the node tree under a real `Replica` with no installation ceremony in
//! the way — the same `check` and `apply` a `ModelLog` runs, reached without
//! a `ModelOp::Install` in front of it. That is deliberate: it leaves event
//! *n* as event *n* on both arms, so the two eg-walkers break their
//! concurrency ties on the same sequence numbers, which the behaviour tree's
//! oracle has to buy with a `Root::New` on the generated side and which
//! `json.ecore` has no no-op operation to buy with. The `Install` path over
//! this same metamodel is covered where it belongs, by `moirai-interp`'s own
//! `a_json_document_reads_out_as_a_json_document`, which drives the real
//! `ModelLog` over the real `json.metamodel.json`.
//!
//! Generated: a `twins_log::<JsonLog>()` pair, driven by operations built as
//! JSON and deserialized into `Json`, so a wrong shape is a
//! `serde_json::from_value` error naming the enum and never a silent pass.
//!
//! # The document is the model
//!
//! Because every concrete class is transparent, the canonical read-out of a
//! model under `json.ecore` *is* a JSON document: an `Array` is the array, an
//! `Object` is the object, a `String` is the string. So the script generator
//! reads the document itself to know what it may edit — a JSON array offers
//! an insert and a delete, an object a put and a remove, a string a character
//! — and never needs an `eClass` key, because there is not one anywhere in
//! the read-out.
//!
//! # What the criterion excludes here
//!
//! Nothing. There is no reference in `json.ecore`, no enum, no optional and
//! no eagerly materialised record, so neither of the two exclusions the
//! behaviour tree's oracle carries applies: the raw read-outs are compared
//! after every operation and after every delivery, and the residual is
//! expected to be empty.

use std::collections::BTreeMap;
use std::sync::Arc;

use json_crdt::package::{Json, JsonLog, JsonValue};
use moirai_crdt::utils::membership::twins_log;
use moirai_interp::testing::{JSON_DESCRIPTOR, class_slot, feature_slot, twins};
use moirai_interp::{InstanceOp, LeafOp, Scalar};
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::Tcsb;
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::{IsReplica, Replica};
use moirai_semantics::{ClassSlot, FeatureSlot, MetamodelSemantics, from_descriptor};
use serde_json::{Map, Value, json};

type InterpReplica = Replica<moirai_interp::testing::Harness, Tcsb<InstanceOp>>;
type GenReplica = Replica<JsonLog, Tcsb<Json>>;

/// The key a conflict set is carried under, `02 Validation Plan` §2.
const CONFLICT: &str = "__conflict";

// ---------------------------------------------------------------------------
// 1. The five kinds, which are the whole metamodel
// ---------------------------------------------------------------------------

/// One of `json.ecore`'s five concrete classes.
///
/// They are `JsonKind`'s five variants, and — because every one of them is
/// transparent — they are also the five things a JSON value can be. The
/// oracle never needs any other name out of the metamodel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Array,
    Object,
    String,
    Number,
    Boolean,
}

impl Kind {
    const ALL: [Kind; 5] = [
        Kind::Array,
        Kind::Object,
        Kind::String,
        Kind::Number,
        Kind::Boolean,
    ];

    /// The classifier name, which is both the `union!` variant and the class.
    const fn name(self) -> &'static str {
        match self {
            Kind::Array => "Array",
            Kind::Object => "Object",
            Kind::String => "String",
            Kind::Number => "Number",
            Kind::Boolean => "Boolean",
        }
    }

    /// The feature the class is represented by, as the table records it under
    /// `transparent`. Read from the table rather than written here would be
    /// circular: this is the second implementation.
    const fn field(self) -> &'static str {
        match self {
            Kind::Array => "items",
            Kind::Object => "entry",
            Kind::String | Kind::Number | Kind::Boolean => "value",
        }
    }

    /// Which kind a canonical value is, or `None` for a conflict set, which
    /// the generator never descends into.
    fn of(value: &Value) -> Option<Kind> {
        match value {
            Value::Array(_) => Some(Kind::Array),
            Value::Object(map) if map.contains_key(CONFLICT) => None,
            Value::Object(_) => Some(Kind::Object),
            Value::String(_) => Some(Kind::String),
            Value::Number(_) => Some(Kind::Number),
            Value::Bool(_) => Some(Kind::Boolean),
            Value::Null => None,
        }
    }
}

/// The table, plus the two lookups an encoder needs from it.
struct Meta {
    sem: Arc<MetamodelSemantics>,
}

impl Meta {
    fn new(descriptor: &Value) -> Meta {
        Meta {
            sem: Arc::new(from_descriptor(descriptor).expect("the checked-in descriptor parses")),
        }
    }

    fn slot(&self, kind: Kind) -> ClassSlot {
        class_slot(&self.sem, kind.name())
    }

    /// The visible slot of the feature the class is represented by, checked
    /// against what the table itself says is the transparent one.
    fn field_slot(&self, kind: Kind) -> FeatureSlot {
        let class = self.slot(kind);
        let slot = feature_slot(&self.sem, class, kind.field());
        assert_eq!(
            self.sem.classes[class.index()].transparent,
            Some(slot),
            "`{}` is represented by `{}` on both sides of the oracle",
            kind.name(),
            kind.field()
        );
        slot
    }
}

// ---------------------------------------------------------------------------
// 2. The edit script
// ---------------------------------------------------------------------------

/// One step down a JSON document.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    /// Into element `pos` of an array.
    Index(usize),
    /// Into the value at `key` of an object.
    Key(String),
}

/// Where an edit happens, from the root of the document.
#[derive(Clone, Debug, Default, PartialEq)]
struct Path {
    steps: Vec<Step>,
}

impl Path {
    fn child(&self, step: Step) -> Path {
        let mut steps = self.steps.clone();
        steps.push(step);
        Path { steps }
    }

    fn show(&self) -> String {
        let mut out = String::from("$");
        for step in &self.steps {
            match step {
                Step::Index(pos) => out.push_str(&format!("[{pos}]")),
                Step::Key(key) => out.push_str(&format!(".{key}")),
            }
        }
        out
    }
}

/// What an edit does to the value its path names.
#[derive(Clone, Debug, PartialEq)]
enum Action {
    /// Put a character into the string here.
    InsertChar { pos: usize, ch: char },
    /// Take one out.
    DeleteChar { pos: usize },
    /// Add to the number here.
    Inc(i64),
    /// Write the flag here.
    Flag { on: bool },
    /// Insert a seeded value of `kind` at `pos` of the array here.
    InsertItem { pos: usize, kind: Kind },
    /// Take element `pos` of the array here out.
    DeleteItem { pos: usize },
    /// Put a seeded value of `kind` at `key` of the object here.
    PutKey { key: String, kind: Kind },
    /// Take `key` out of the object here.
    RemoveKey { key: String },
    /// Take every key out of the object here. The seeded scripts never
    /// propose it; the conflict matrix below does.
    ClearKeys,
    /// Make the whole document a seeded value of `kind`. Only ever the first
    /// edit of a script, when the document is `null`.
    Open { kind: Kind },
}

/// One edit: which replica issues it, where, and what it does.
#[derive(Clone, Debug)]
struct Edit {
    writer: char,
    path: Path,
    action: Action,
}

impl Edit {
    fn show(&self) -> String {
        format!("on {} at {} — {:?}", self.writer, self.path.show(), self.action)
    }
}

/// A whole script. `Deliver` marks the points where everything in flight
/// crosses.
#[derive(Clone, Debug, Default)]
struct EditScript {
    label: String,
    steps: Vec<Move>,
}

#[derive(Clone, Debug)]
enum Move {
    Edit(Edit),
    Deliver,
}

// ---------------------------------------------------------------------------
// 3. The two encoders
// ---------------------------------------------------------------------------

/// The kind at each level of a path, the root first.
///
/// The document tells us: because every class is transparent, an array *is*
/// an `Array` and an object *is* an `Object`.
fn kinds_along(doc: &Value, path: &Path) -> Vec<Kind> {
    let mut out = Vec::new();
    let mut at = doc;
    out.push(Kind::of(at).unwrap_or_else(|| panic!("the document at $ is {at}")));
    for step in &path.steps {
        at = match step {
            Step::Index(pos) => &at[*pos],
            Step::Key(key) => &at[key.as_str()],
        };
        out.push(Kind::of(at).unwrap_or_else(|| panic!("the document at a hop is {at}")));
    }
    out
}

/// `Variant(kind, Field(<the field it is represented by>, inner))`: the one
/// wrapper every value of every kind is reached through on the interpreted
/// path, and the wrapper that has no counterpart at all on the generated one.
fn interp_wrap(meta: &Meta, kind: Kind, inner: InstanceOp) -> InstanceOp {
    InstanceOp::variant(
        meta.slot(kind),
        InstanceOp::field(meta.field_slot(kind), inner),
    )
}

/// The interpreted operation that makes a value of `kind` *and writes into
/// it*.
///
/// Never a bare `New`: a `JsonKind` union has no operation that mints a
/// variant without writing to it, so an interpreted object minted and left
/// empty would have no generated counterpart. Every seed bottoms out in a
/// character, an increment or an enable.
fn interp_seed(meta: &Meta, kind: Kind) -> InstanceOp {
    let inner = match kind {
        Kind::String => InstanceOp::Leaf(LeafOp::InsertChar { pos: 0, ch: 's' }),
        Kind::Number => InstanceOp::Leaf(LeafOp::Inc(Scalar::Int(1))),
        Kind::Boolean => InstanceOp::Leaf(LeafOp::Enable),
        Kind::Array => InstanceOp::insert(0, interp_seed(meta, Kind::String)),
        Kind::Object => InstanceOp::entry(Scalar::text("seed"), interp_seed(meta, Kind::String)),
    };
    interp_wrap(meta, kind, inner)
}

/// One edit as an [`InstanceOp`].
fn interp_op(meta: &Meta, doc: &Value, edit: &Edit) -> InstanceOp {
    if let Action::Open { kind } = edit.action {
        return interp_seed(meta, kind);
    }
    let kinds = kinds_along(doc, &edit.path);
    let here = *kinds.last().expect("the root is always there");
    let mut op = interp_wrap(meta, here, interp_action(meta, &edit.action));
    for (index, step) in edit.path.steps.iter().enumerate().rev() {
        let parent = kinds[index];
        op = match step {
            Step::Index(pos) => InstanceOp::at(*pos, op),
            Step::Key(key) => InstanceOp::entry(Scalar::text(key.clone()), op),
        };
        op = interp_wrap(meta, parent, op);
    }
    op
}

/// The body of an edit, inside the wrapper of the kind it addresses.
fn interp_action(meta: &Meta, action: &Action) -> InstanceOp {
    match action {
        Action::InsertChar { pos, ch } => InstanceOp::Leaf(LeafOp::InsertChar {
            pos: *pos,
            ch: *ch,
        }),
        Action::DeleteChar { pos } => InstanceOp::Leaf(LeafOp::DeleteChar { pos: *pos }),
        Action::Inc(by) => InstanceOp::Leaf(LeafOp::Inc(Scalar::Int(*by))),
        Action::Flag { on: true } => InstanceOp::Leaf(LeafOp::Enable),
        Action::Flag { on: false } => InstanceOp::Leaf(LeafOp::Disable),
        Action::InsertItem { pos, kind } => InstanceOp::insert(*pos, interp_seed(meta, *kind)),
        Action::DeleteItem { pos } => InstanceOp::delete(*pos),
        Action::PutKey { key, kind } => {
            InstanceOp::entry(Scalar::text(key.clone()), interp_seed(meta, *kind))
        }
        Action::RemoveKey { key } => InstanceOp::remove(Scalar::text(key.clone())),
        Action::ClearKeys => InstanceOp::clear(),
        Action::Open { .. } => unreachable!("handled by `interp_op`"),
    }
}

/// One externally tagged enum value.
fn tagged(variant: &str, payload: Value) -> Value {
    let mut map = Map::new();
    map.insert(variant.to_string(), payload);
    Value::Object(map)
}

/// The generated counterpart of [`interp_wrap`]: the `union!` variant, and
/// **nothing else**. This is what a transparent class is: there is no record
/// here, so there is no field step, and the whole difference between the two
/// encoders is this function against that one.
fn typed_wrap(kind: Kind, inner: Value) -> Value {
    tagged(kind.name(), inner)
}

fn typed_seed(kind: Kind) -> Value {
    let inner = match kind {
        Kind::String => json!({"Insert": {"content": "s", "pos": 0}}),
        Kind::Number => json!({"Inc": 1.0}),
        Kind::Boolean => json!("Enable"),
        Kind::Array => json!({"Insert": {"pos": 0, "op": typed_seed(Kind::String)}}),
        Kind::Object => json!({"Update": ["seed", typed_seed(Kind::String)]}),
    };
    typed_wrap(kind, inner)
}

/// One edit as this crate's typed operation.
fn typed_op(doc: &Value, edit: &Edit) -> Json {
    let value = tagged("JsonKind", typed_json(doc, edit));
    serde_json::from_value(value.clone()).unwrap_or_else(|error| {
        panic!(
            "the typed encoder built an operation `Json` cannot take: {error}\n{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        )
    })
}

fn typed_json(doc: &Value, edit: &Edit) -> Value {
    if let Action::Open { kind } = edit.action {
        return typed_seed(kind);
    }
    let kinds = kinds_along(doc, &edit.path);
    let here = *kinds.last().expect("the root is always there");
    let mut op = typed_wrap(here, typed_action(&edit.action));
    for (index, step) in edit.path.steps.iter().enumerate().rev() {
        let parent = kinds[index];
        op = match step {
            Step::Index(pos) => json!({"Update": {"pos": pos, "op": op}}),
            Step::Key(key) => json!({"Update": [key, op]}),
        };
        op = typed_wrap(parent, op);
    }
    op
}

fn typed_action(action: &Action) -> Value {
    match action {
        Action::InsertChar { pos, ch } => json!({"Insert": {"content": ch, "pos": pos}}),
        Action::DeleteChar { pos } => json!({"Delete": {"pos": pos}}),
        Action::Inc(by) => json!({"Inc": *by as f64}),
        Action::Flag { on: true } => json!("Enable"),
        Action::Flag { on: false } => json!("Disable"),
        Action::InsertItem { pos, kind } => json!({"Insert": {"pos": pos, "op": typed_seed(*kind)}}),
        Action::DeleteItem { pos } => json!({"Delete": {"pos": pos}}),
        Action::PutKey { key, kind } => json!({"Update": [key, typed_seed(*kind)]}),
        Action::RemoveKey { key } => json!({"Remove": key}),
        Action::ClearKeys => json!("Clear"),
        Action::Open { .. } => unreachable!("handled by `typed_json`"),
    }
}

// ---------------------------------------------------------------------------
// 4. The projection of the generated read-out onto the canonical form
// ---------------------------------------------------------------------------

/// `JsonValue` in the canonical form of `02 Validation Plan` §2.
///
/// Three rewrites and no fourth: a `union!` wrapper becomes what it holds
/// (there is no `eClass` to unwrap it into, because a transparent class has
/// no record for one to name), a `Vec<char>` becomes a string, and a
/// `Conflict` is re-sorted from `union!`'s variant rank into §2's class name
/// and then canonical bytes.
fn project(value: &JsonValue) -> Value {
    let raw = serde_json::to_value(value).expect("the generated read-out serializes");
    project_kind(&raw["json"]).unwrap_or(Value::Null)
}

/// One `JsonKindValue`.
fn project_kind(value: &Value) -> Option<Value> {
    match value {
        Value::String(word) if word == "Unset" => None,
        Value::Object(map) if map.len() == 1 => {
            let (key, payload) = map.iter().next().expect("just checked");
            match key.as_str() {
                "Value" => project_child(payload).map(|(_, value)| value),
                "Conflict" => {
                    let mut held: Vec<(String, Vec<u8>, Value)> = payload
                        .as_array()
                        .unwrap_or_else(|| panic!("a `Conflict` holds an array"))
                        .iter()
                        .filter_map(project_child)
                        .map(|(name, value)| {
                            let bytes = serde_json::to_vec(&value).unwrap_or_default();
                            (name, bytes, value)
                        })
                        .collect();
                    match held.len() {
                        0 => None,
                        1 => Some(held.pop().expect("just counted").2),
                        _ => {
                            held.sort_by(|l, r| l.0.cmp(&r.0).then_with(|| l.1.cmp(&r.1)));
                            let mut out = Map::new();
                            out.insert(
                                CONFLICT.to_string(),
                                Value::Array(held.into_iter().map(|(_, _, v)| v).collect()),
                            );
                            Some(Value::Object(out))
                        }
                    }
                }
                other => panic!("`{other}` is not a `union!` state"),
            }
        }
        other => panic!("`{other}` is not a `JsonKindValue`"),
    }
}

/// One `JsonKindChildValue`: the variant name, and what it renders as.
fn project_child(value: &Value) -> Option<(String, Value)> {
    let Value::Object(map) = value else {
        panic!("a `union!` child is its variant object");
    };
    let (variant, payload) = map.iter().next().expect("a variant carries one payload");
    let rendered = match variant.as_str() {
        "Array" => Value::Array(
            payload
                .as_array()
                .unwrap_or_else(|| panic!("an `Array` reads as an array"))
                .iter()
                .filter_map(project_kind)
                .collect(),
        ),
        "Object" => {
            let entries: BTreeMap<String, Value> = payload
                .as_object()
                .unwrap_or_else(|| panic!("an `Object` reads as a map"))
                .iter()
                .filter_map(|(key, child)| project_kind(child).map(|value| (key.clone(), value)))
                .collect();
            Value::Object(entries.into_iter().collect())
        }
        "String" => Value::String(
            payload
                .as_array()
                .unwrap_or_else(|| panic!("a `String` reads as an array of characters"))
                .iter()
                .filter_map(Value::as_str)
                .collect(),
        ),
        "Number" | "Boolean" => payload.clone(),
        other => panic!("`{other}` is not a `JsonKind` variant"),
    };
    Some((variant.clone(), rendered))
}

/// The first place two canonical documents differ, as a path and the two
/// values, so a failure is readable without a diff tool.
fn difference(left: &Value, right: &Value, at: &str) -> Option<String> {
    match (left, right) {
        (Value::Object(l), Value::Object(r)) => {
            let mut keys: Vec<&String> = l.keys().chain(r.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let here = format!("{at}/{key}");
                match (l.get(key), r.get(key)) {
                    (Some(l), Some(r)) => {
                        if let Some(found) = difference(l, r, &here) {
                            return Some(found);
                        }
                    }
                    (Some(l), None) => {
                        return Some(format!("{here}: interpreted {l}, generated has no key"));
                    }
                    (None, Some(r)) => {
                        return Some(format!("{here}: interpreted has no key, generated {r}"));
                    }
                    (None, None) => {}
                }
            }
            None
        }
        (Value::Array(l), Value::Array(r)) => {
            if l.len() != r.len() {
                return Some(format!(
                    "{at}: interpreted holds {} items, generated {}\n  interpreted: {left}\n  generated:   {right}",
                    l.len(),
                    r.len()
                ));
            }
            for (index, (l, r)) in l.iter().zip(r).enumerate() {
                if let Some(found) = difference(l, r, &format!("{at}[{index}]")) {
                    return Some(found);
                }
            }
            None
        }
        (l, r) if l == r => None,
        (l, r) => Some(format!("{at}: interpreted {l}, generated {r}")),
    }
}

// ---------------------------------------------------------------------------
// 5. The harness
// ---------------------------------------------------------------------------

/// Two `twins` pairs, one per path, driven by one script.
struct Harness {
    meta: Meta,
    ia: InterpReplica,
    ib: InterpReplica,
    ga: GenReplica,
    gb: GenReplica,
    pending_a: Vec<(EventMessage<InstanceOp>, EventMessage<Json>)>,
    pending_b: Vec<(EventMessage<InstanceOp>, EventMessage<Json>)>,
    ops: usize,
    refused: usize,
    comparisons: usize,
}

fn json_descriptor() -> Value {
    serde_json::from_str(JSON_DESCRIPTOR).expect("the fixture is JSON")
}

impl Harness {
    fn new() -> Harness {
        let meta = Meta::new(&json_descriptor());
        let (ia, ib) = twins(&meta.sem, "Json");
        let (ga, gb) = twins_log::<JsonLog>();
        Harness {
            meta,
            ia,
            ib,
            ga,
            gb,
            pending_a: Vec::new(),
            pending_b: Vec::new(),
            ops: 0,
            refused: 0,
            comparisons: 0,
        }
    }

    fn interp_doc(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ia } else { &self.ib };
        replica.query(Read::<Value>::new())
    }

    fn gen_doc(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ga } else { &self.gb };
        project(&replica.query(Read::<JsonValue>::new()))
    }

    /// The four read-outs, compared. No pruning on either side.
    fn compare(&mut self) -> Result<(), String> {
        for writer in ['a', 'b'] {
            self.comparisons += 1;
            let interp = self.interp_doc(writer);
            let generated = self.gen_doc(writer);
            if interp != generated {
                let where_ = difference(&interp, &generated, "")
                    .unwrap_or_else(|| "the documents differ but no key does".to_string());
                return Err(format!(
                    "replica {writer}: {where_}\n  interpreted: {}\n  generated:   {}",
                    serde_json::to_string(&interp).unwrap_or_default(),
                    serde_json::to_string(&generated).unwrap_or_default(),
                ));
            }
        }
        Ok(())
    }

    /// One edit, encoded twice against the writer's own view and handed to
    /// the two replicas of that writer.
    fn carry(&mut self, edit: &Edit) -> Result<bool, String> {
        let doc = self.interp_doc(edit.writer);
        let interp = interp_op(&self.meta, &doc, edit);
        let generated = typed_op(&doc, edit);
        self.ops += 1;
        let (interp_event, gen_event) = if edit.writer == 'a' {
            (self.ia.send(interp), self.ga.send(generated))
        } else {
            (self.ib.send(interp), self.gb.send(generated))
        };
        match (interp_event, gen_event) {
            (Some(interp_event), Some(gen_event)) => {
                let pending = if edit.writer == 'a' {
                    &mut self.pending_a
                } else {
                    &mut self.pending_b
                };
                pending.push((interp_event, gen_event));
                Ok(true)
            }
            (None, None) => {
                // Both intakes refused it, which is itself an equality worth
                // having.
                self.refused += 1;
                Ok(false)
            }
            (interp_event, _) => Err(format!(
                "the two intakes disagree on {}: interpreted {}, generated {}",
                edit.show(),
                if interp_event.is_some() {
                    "accepted"
                } else {
                    "refused"
                },
                if interp_event.is_some() {
                    "refused"
                } else {
                    "accepted"
                },
            )),
        }
    }

    fn apply(&mut self, edit: &Edit) -> Result<(), String> {
        self.carry(edit)?;
        self.compare()
    }

    fn cross(&mut self) {
        for (interp_event, gen_event) in std::mem::take(&mut self.pending_a) {
            self.ib.receive(interp_event);
            self.gb.receive(gen_event);
        }
        for (interp_event, gen_event) in std::mem::take(&mut self.pending_b) {
            self.ia.receive(interp_event);
            self.ga.receive(gen_event);
        }
    }

    fn deliver(&mut self) -> Result<(), String> {
        for (interp_event, gen_event) in std::mem::take(&mut self.pending_a) {
            self.ib.receive(interp_event);
            self.gb.receive(gen_event);
            self.compare()?;
        }
        for (interp_event, gen_event) in std::mem::take(&mut self.pending_b) {
            self.ia.receive(interp_event);
            self.ga.receive(gen_event);
            self.compare()?;
        }
        Ok(())
    }

    fn run(&mut self, script: &EditScript) -> Result<(), String> {
        for (index, step) in script.steps.iter().enumerate() {
            match step {
                Move::Edit(edit) => self.apply(edit).map_err(|reason| {
                    format!("{}: edit {index} — {}\n{reason}", script.label, edit.show())
                })?,
                Move::Deliver => self.deliver().map_err(|reason| {
                    format!("{}: delivery after edit {index}\n{reason}", script.label)
                })?,
            }
        }
        self.deliver()
            .map_err(|reason| format!("{}: final delivery\n{reason}", script.label))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Seeded generation over the document itself
// ---------------------------------------------------------------------------

/// splitmix64, written out so a seed means the same script on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        assert!(bound > 0);
        (self.next() % bound as u64) as usize
    }
}

/// One thing a writer looking at `doc` could do.
#[derive(Clone, Debug)]
struct Candidate {
    path: Path,
    action_kind: Slot,
}

#[derive(Clone, Debug)]
enum Slot {
    Chars(usize),
    Number,
    Flag,
    Array(usize),
    Object(Vec<String>),
}

/// Everything the writer could do, in a deterministic order.
fn candidates(doc: &Value, path: &Path, depth: usize, out: &mut Vec<Candidate>) {
    const DEPTH_CAP: usize = 4;
    let here = |action_kind| Candidate {
        path: path.clone(),
        action_kind,
    };
    match doc {
        Value::String(text) => out.push(here(Slot::Chars(text.chars().count()))),
        Value::Number(_) => out.push(here(Slot::Number)),
        Value::Bool(_) => out.push(here(Slot::Flag)),
        Value::Array(items) => {
            out.push(here(Slot::Array(items.len())));
            if depth < DEPTH_CAP {
                for (index, item) in items.iter().enumerate() {
                    candidates(item, &path.child(Step::Index(index)), depth + 1, out);
                }
            }
        }
        Value::Object(map) if !map.contains_key(CONFLICT) => {
            out.push(here(Slot::Object(map.keys().cloned().collect())));
            if depth < DEPTH_CAP {
                for (key, child) in map {
                    candidates(child, &path.child(Step::Key(key.clone())), depth + 1, out);
                }
            }
        }
        // A conflict is compared but never descended into: both paths keep
        // both members and an edit aimed at one would only test the refusal.
        _ => {}
    }
}

/// How many values a document holds, which is what caps a script's growth.
fn value_count(doc: &Value) -> usize {
    match doc {
        Value::Array(items) => 1 + items.iter().map(value_count).sum::<usize>(),
        Value::Object(map) => 1 + map.values().map(value_count).sum::<usize>(),
        Value::Null => 0,
        _ => 1,
    }
}

/// One edit, drawn from what the writer can see.
fn propose(doc: &Value, rng: &mut Rng, writer: char) -> Option<Edit> {
    const ALPHABET: [char; 6] = ['a', 'b', 'c', 'd', 'e', 'f'];
    const KEYS: [&str; 5] = ["one", "two", "three", "four", "five"];
    const VALUE_CAP: usize = 18;

    if doc.is_null() {
        return Some(Edit {
            writer,
            path: Path::default(),
            action: Action::Open {
                kind: Kind::ALL[rng.below(Kind::ALL.len())],
            },
        });
    }
    let mut found = Vec::new();
    candidates(doc, &Path::default(), 0, &mut found);
    if found.is_empty() {
        return None;
    }
    let room = value_count(doc) < VALUE_CAP;
    let candidate = found[rng.below(found.len())].clone();
    let action = match &candidate.action_kind {
        Slot::Chars(len) => {
            if *len > 0 && rng.below(4) == 0 {
                Action::DeleteChar {
                    pos: rng.below(*len),
                }
            } else {
                Action::InsertChar {
                    pos: rng.below(len + 1),
                    ch: ALPHABET[rng.below(ALPHABET.len())],
                }
            }
        }
        Slot::Number => Action::Inc(rng.below(5) as i64),
        Slot::Flag => Action::Flag {
            on: rng.below(2) == 0,
        },
        Slot::Array(len) => {
            if *len > 0 && (!room || rng.below(4) == 0) {
                Action::DeleteItem {
                    pos: rng.below(*len),
                }
            } else if room {
                Action::InsertItem {
                    pos: rng.below(len + 1),
                    kind: Kind::ALL[rng.below(Kind::ALL.len())],
                }
            } else {
                return None;
            }
        }
        Slot::Object(keys) => {
            if !keys.is_empty() && (!room || rng.below(4) == 0) {
                Action::RemoveKey {
                    key: keys[rng.below(keys.len())].clone(),
                }
            } else if room {
                Action::PutKey {
                    key: KEYS[rng.below(KEYS.len())].to_string(),
                    kind: Kind::ALL[rng.below(Kind::ALL.len())],
                }
            } else {
                return None;
            }
        }
    };
    Some(Edit {
        writer,
        path: candidate.path,
        action,
    })
}

/// Ten sequential scripts and twenty concurrent ones, over `json.ecore`.
///
/// The generator runs against a shadow pair driven by the same edits, so it
/// proposes against what a writer would actually be looking at. The shadow is
/// carried and never compared: a difference is the finding `ip28` exists to
/// report, and a generator that halted on the first one would only ever
/// report the first one.
fn seeded_script(seed: u64, concurrent: bool) -> EditScript {
    let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x1234_5678);
    let mut steps: Vec<Move> = Vec::new();
    let mut shadow = Harness::new();
    let mut broken = false;

    if concurrent {
        // The document has to exist before two writers can disagree inside
        // it, so the opening edit is `a`'s alone and is delivered.
        let doc = shadow.interp_doc('a');
        if let Some(edit) = propose(&doc, &mut rng, 'a') {
            broken = shadow.carry(&edit).is_err();
            steps.push(Move::Edit(edit));
        }
        shadow.cross();
        steps.push(Move::Deliver);

        for _ in 0..4 {
            if broken {
                break;
            }
            let edits = 2 + rng.below(3);
            for _ in 0..edits {
                for writer in ['a', 'b'] {
                    if broken {
                        break;
                    }
                    let doc = shadow.interp_doc(writer);
                    if let Some(edit) = propose(&doc, &mut rng, writer) {
                        broken = shadow.carry(&edit).is_err();
                        steps.push(Move::Edit(edit));
                    }
                }
            }
            shadow.cross();
            steps.push(Move::Deliver);
        }
    } else {
        for index in 0..24 {
            if broken {
                break;
            }
            let writer = if index % 3 == 0 { 'b' } else { 'a' };
            let doc = shadow.interp_doc(writer);
            if let Some(edit) = propose(&doc, &mut rng, writer) {
                broken = shadow.carry(&edit).is_err();
                steps.push(Move::Edit(edit));
                steps.push(Move::Deliver);
                shadow.cross();
            }
        }
    }
    EditScript {
        label: format!(
            "{} seed {seed}",
            if concurrent { "concurrent" } else { "sequential" }
        ),
        steps,
    }
}

fn scripts() -> Vec<EditScript> {
    let mut out: Vec<EditScript> = (0..10).map(|seed| seeded_script(seed, false)).collect();
    out.extend((0..20).map(|seed| seeded_script(seed, true)));
    out
}

// ---------------------------------------------------------------------------
// 7. The tests
// ---------------------------------------------------------------------------

/// The descriptor the interpreted arm runs is byte-for-byte the one this
/// crate was generated from. Without this, everything below compares two
/// metamodels rather than two paths.
#[test]
fn the_two_arms_hold_the_same_metamodel() {
    let checked_in: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/metamodel.json"))
            .expect("this crate's own descriptor is readable"),
    )
    .expect("it is JSON");
    assert_eq!(
        checked_in,
        json_descriptor(),
        "`moirai-interp`'s fixture and this crate's `metamodel.json` have parted company"
    );
    let sem = Meta::new(&json_descriptor()).sem;
    assert_eq!(&*sem.package, "json");
    assert_eq!(sem.roots.len(), 1, "`Json` is the one declared root");
}

/// The table really does carry the two forms, and the encoders really do use
/// them: a keyed shape on `Object.entry`, and a transparent field on each of
/// the five concrete classes.
#[test]
fn the_metamodel_reaches_the_two_forms_bt_never_did() {
    let meta = Meta::new(&json_descriptor());
    for kind in Kind::ALL {
        // Panics unless the table's own `transparent` slot is the one this
        // file names.
        let _ = meta.field_slot(kind);
    }
    let object = meta.slot(Kind::Object);
    let rule = meta
        .sem
        .rule(object, feature_slot(&meta.sem, object, "entry"))
        .expect("`Object` declares `entry`");
    assert!(
        matches!(
            rule,
            moirai_semantics::MergeRule::Containment {
                shape: moirai_semantics::Shape::Keyed { .. },
                ..
            }
        ),
        "`Object.entry` is keyed, and it is the only keyed feature in the corpus: {rule:?}"
    );
}

/// A document built by hand, one value of every kind, read out identically on
/// both paths and equal to the JSON it describes.
#[test]
fn ip28_a_document_of_every_kind_reads_the_same_on_both_paths() {
    let mut harness = Harness::new();
    let edits = vec![
        Edit {
            writer: 'a',
            path: Path::default(),
            action: Action::Open { kind: Kind::Object },
        },
        Edit {
            writer: 'a',
            path: Path::default(),
            action: Action::PutKey {
                key: "n".to_string(),
                kind: Kind::Number,
            },
        },
        Edit {
            writer: 'a',
            path: Path::default(),
            action: Action::PutKey {
                key: "ok".to_string(),
                kind: Kind::Boolean,
            },
        },
        Edit {
            writer: 'a',
            path: Path::default(),
            action: Action::PutKey {
                key: "list".to_string(),
                kind: Kind::Array,
            },
        },
        Edit {
            writer: 'b',
            path: Path {
                steps: vec![Step::Key("n".to_string())],
            },
            action: Action::Inc(4),
        },
    ];
    for edit in &edits {
        harness.apply(edit).unwrap_or_else(|reason| panic!("{reason}"));
        harness.deliver().unwrap_or_else(|reason| panic!("{reason}"));
    }
    assert_eq!(
        harness.interp_doc('a'),
        json!({"seed": "s", "n": 5.0, "ok": true, "list": ["s"]}),
        "the model is the document"
    );
    assert_eq!(harness.interp_doc('a'), harness.gen_doc('a'));
    assert_eq!(harness.interp_doc('b'), harness.gen_doc('b'));
}

/// An update concurrent with the removal of the key it is under survives it
/// on both paths, and one causally below the removal does not. This is the
/// keyed container's whole semantics, asserted against the real `UWMapLog`
/// rather than against a copy of its rule.
#[test]
fn ip28_update_wins_over_a_concurrent_key_removal_on_both_paths() {
    let mut harness = Harness::new();
    let root = Path::default();
    let open = Edit {
        writer: 'a',
        path: root.clone(),
        action: Action::Open { kind: Kind::Object },
    };
    let put = Edit {
        writer: 'a',
        path: root.clone(),
        action: Action::PutKey {
            key: "k".to_string(),
            kind: Kind::Number,
        },
    };
    for edit in [&open, &put] {
        harness.apply(edit).unwrap_or_else(|reason| panic!("{reason}"));
        harness.deliver().unwrap_or_else(|reason| panic!("{reason}"));
    }

    let remove = Edit {
        writer: 'a',
        path: root.clone(),
        action: Action::RemoveKey {
            key: "k".to_string(),
        },
    };
    let bump = Edit {
        writer: 'b',
        path: Path {
            steps: vec![Step::Key("k".to_string())],
        },
        action: Action::Inc(10),
    };
    harness.carry(&remove).unwrap_or_else(|r| panic!("{r}"));
    harness.carry(&bump).unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|reason| panic!("{reason}"));

    assert_eq!(
        harness.interp_doc('a'),
        harness.gen_doc('a'),
        "the two paths settle the concurrency the same way"
    );
    assert_eq!(harness.interp_doc('a'), harness.interp_doc('b'));
    assert_eq!(
        harness.interp_doc('a')["k"],
        json!(10.0),
        "the increment concurrent with the removal survives it; the seeded 1 does not"
    );
}

/// **ip28** — thirty seeded scripts, ten sequential and twenty with a
/// concurrent half, compared after every operation and after every crossing,
/// with no exclusion of any kind on either side.
#[test]
fn ip28_thirty_scripts_over_json_ecore_agree_everywhere() {
    let scripts = scripts();
    let mut edits = 0usize;
    let mut comparisons = 0usize;
    let mut refused = 0usize;
    let mut failures = Vec::new();
    for script in &scripts {
        edits += script
            .steps
            .iter()
            .filter(|step| matches!(step, Move::Edit(_)))
            .count();
        let mut harness = Harness::new();
        if let Err(reason) = harness.run(script) {
            failures.push(reason);
        }
        comparisons += harness.comparisons;
        refused += harness.refused;
    }
    // The census, printed under `--nocapture` so the number in the vault is
    // a number this run produced.
    eprintln!(
        "ip28: {} scripts, {edits} edits, {comparisons} comparisons, {refused} refused by both intakes",
        scripts.len()
    );
    assert!(
        failures.is_empty(),
        "{} of {} scripts diverged:\n{}",
        failures.len(),
        scripts.len(),
        failures.join("\n\n")
    );
    assert_eq!(scripts.len(), 30, "ten sequential and twenty concurrent");
    assert!(
        edits >= 300,
        "the scripts have to carry real work: {edits} edits"
    );
}

/// The oracle fails when the two paths genuinely differ, which is criterion
/// I-A2 for this driver: the typed encoder is mutated to write the seeded
/// character into position 0 always, and the comparison catches it.
#[test]
fn ip28_the_oracle_notices_when_the_two_encoders_disagree() {
    let mut harness = Harness::new();
    let open = Edit {
        writer: 'a',
        path: Path::default(),
        action: Action::Open { kind: Kind::String },
    };
    harness.apply(&open).unwrap_or_else(|reason| panic!("{reason}"));
    harness.deliver().unwrap();

    // The mutation: the generated arm is handed a character the interpreted
    // arm never sees.
    let stray: Json = serde_json::from_value(tagged(
        "JsonKind",
        typed_wrap(Kind::String, json!({"Insert": {"content": "!", "pos": 0}})),
    ))
    .expect("the shape is right; the content is the lie");
    let event = harness.ga.send(stray).expect("the generated log takes it");
    harness.gb.receive(event);

    let reason = harness
        .compare()
        .expect_err("the two documents now differ and the oracle has to say so");
    assert!(reason.contains("interpreted"), "{reason}");
}

// ---------------------------------------------------------------------------
// 8. The conflict matrix over `json.ecore`
// ---------------------------------------------------------------------------
//
// `moirai_interp::matrix` holds the whole matrix and assigns this crate the
// keyed map, `Object.entry`, the only keyed feature in the corpus. The cells
// run on this file's own encoders and its raw comparison, which carries no
// exclusion of any kind.
//
// The model is opened as an array of three: a string, the object under test
// at index 1, seeded with one `seed` entry, and a number at index 2 that every
// cell acknowledges on. So the heartbeat is an increment of a sibling value
// and never an edit of the map a cell contends, which is also why the map is
// one level down rather than at the root.

use moirai_interp::matrix::{self, Arm, Cell, Construction, pattern as p};

fn json_at(steps: Vec<Step>, action: Action) -> Edit {
    Edit {
        writer: 'a',
        path: Path { steps },
        action,
    }
}

/// The object under test.
fn map_path() -> Vec<Step> {
    vec![Step::Index(1)]
}

fn key_path(key: &str) -> Vec<Step> {
    vec![Step::Index(1), Step::Key(key.to_string())]
}

fn json_beat() -> Edit {
    json_at(vec![Step::Index(2)], Action::Inc(1))
}

fn json_cells() -> Vec<Cell<Edit>> {
    let opened = |extra: Vec<Edit>| {
        let mut out = vec![
            json_at(vec![], Action::Open { kind: Kind::Array }),
            json_at(vec![], Action::InsertItem { pos: 1, kind: Kind::Object }),
            json_at(vec![], Action::InsertItem { pos: 2, kind: Kind::Number }),
        ];
        out.extend(extra);
        out
    };
    let put = |key: &str, kind: Kind| {
        json_at(map_path(), Action::PutKey { key: key.to_string(), kind })
    };
    let holding_k = || opened(vec![put("k", Kind::Number)]);
    let inc_k = |by: i64| vec![json_at(key_path("k"), Action::Inc(by))];
    let remove_k = || vec![json_at(map_path(), Action::RemoveKey { key: "k".to_string() })];
    let row = Construction::KeyedMap;
    vec![
        // Update-wins: the increment concurrent with the removal survives it,
        // the seeded 1 below the removal does not.
        Cell::new(row, p::UPDATE_REMOVE_KEY, holding_k(), vec![inc_k(10), remove_k()], json_beat())
            .expect("/1/k", json!(10.0))
            .expect("/1/seed", json!("s")),
        Cell::new(row, p::UPDATE_UPDATE_KEY, holding_k(), vec![inc_k(2), inc_k(3)], json_beat())
            .expect("/1/k", json!(6.0)),
        Cell::new(
            row,
            p::TWO_KINDS_ONE_KEY,
            opened(vec![]),
            vec![vec![put("k", Kind::Number)], vec![put("k", Kind::String)]],
            json_beat(),
        )
        .expect("/1/k", json!({CONFLICT: [1.0, "s"]})),
        // A removed key is not dropped on either path: `UWMap::Remove` resets
        // the entry rather than tombstoning it, and a reset `Number` is still a
        // `Number`, so both read `0.0` where the key was. `moirai-interp`'s
        // `a_key_removed_with_nothing_concurrent_keeps_the_emptied_entry` says
        // the same of a string.
        Cell::new(row, p::REMOVE_REMOVE_KEY, holding_k(), vec![remove_k(), remove_k()], json_beat())
            .expect("/1/k", json!(0.0))
            .expect("/1/seed", json!("s")),
        Cell::new(
            row,
            p::UPDATE_CLEAR,
            holding_k(),
            vec![inc_k(10), vec![json_at(map_path(), Action::ClearKeys)]],
            json_beat(),
        )
        .expect("/1/k", json!(10.0))
        .expect("/1/seed", json!("")),
        Cell::new(
            row,
            p::TWO_KEYS,
            opened(vec![]),
            vec![vec![put("k", Kind::Number)], vec![put("j", Kind::String)]],
            json_beat(),
        )
        .expect("/1/k", json!(1.0))
        .expect("/1/j", json!("s")),
        // Three writers on one key: both increments are concurrent with the
        // removal, so both survive it and the seeded 1 does not.
        Cell::new(row, p::THREE_WAY_KEY, holding_k(), vec![inc_k(2), remove_k(), inc_k(3)], json_beat())
            .expect("/1/k", json!(5.0)),
    ]
}

/// **The conflict matrix** over `json.ecore`: every cell the registry assigns
/// to this crate, each under every schedule.
#[test]
fn conflict_matrix_over_json_ecore() {
    let meta = Meta::new(&json_descriptor());
    moirai_interp::testing::install_fixture(&meta.sem, "Json");
    let interp_encode = |edit: &Edit, doc: &Value| interp_op(&meta, doc, edit);
    let gen_encode = |edit: &Edit, doc: &Value| typed_op(doc, edit);
    let interp_read = |replica: &InterpReplica| replica.query(Read::<Value>::new());
    let gen_read = |replica: &GenReplica| project(&replica.query(Read::<JsonValue>::new()));
    let interp = Arm {
        name: "interpreted",
        encode: &interp_encode,
        read: &interp_read,
    };
    let generated = Arm {
        name: "generated",
        encode: &gen_encode,
        read: &gen_read,
    };
    let cells = json_cells();
    matrix::run_matrix(matrix::JSON, &meta.sem, &cells, &interp, &generated)
        .unwrap_or_else(|reason| panic!("{reason}"));
}
