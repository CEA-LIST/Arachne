//! The equivalence oracle over `examples/pet_metamodels/kitchen_sink.ecore`:
//! `ip29`, the third driver of criterion I-A1 and the whole of I-A17.
//!
//! # What is being claimed
//!
//! The same claim the other two drivers make, over the leaf vocabulary
//! neither of them reaches. `bt.ecore` is text and containment; `json.ecore`
//! adds a `Counter<f64>`, an `EWFlag`, a `uw-map` and five transparent
//! classes. Between them they never write a counter of any other width, never
//! write a register, and never touch a bag. `kitchen_sink.ecore` writes six
//! counter widths, one multi-value register and two bags, and this file is
//! what says the interpreted path merges them the way `record!` and the
//! `moirai-crdt` logs do.
//!
//! # What this metamodel does *not* reach, stated here so the claim is not
//! read wider than it is
//!
//! `kitchen_sink.ecore` has no `EEnum`, so `LeafRule::Enum` is not driven
//! here. It has no `datatype` annotation, so every register it produces is a
//! multi-value one and the other four tie-breaks are not driven here, every
//! counter it produces is resettable and the six `SimpleCounter` arms are not
//! driven here, and every flag it produces is enable-wins. Its two unordered
//! attributes, `bag` and `set`, both derive `Shape::Bag`: `set` leaves
//! `unique` unspecified and `semantics.rs:588` reads that as
//! `unique.unwrap_or(false)`, so nothing in the checked-in corpus reaches
//! `Shape::Set` at all and neither `AWSet` nor `RWSet` is driven here. It has
//! no containment, so there is no tree below the root and the four
//! containment shapes are left to the two drivers that do reach them.
//!
//! # Why the root is `Foo` and not all three
//!
//! The descriptor names three root classes, `Bar`, `Baz` and `Foo`, and the
//! generated package holds one log for each. `Foo` carries nineteen of the
//! metamodel's twenty-two features and every construction that is new here;
//! `Bar.health` is the same rule as `Foo.myInt` and `Baz` adds one inherited
//! text attribute that `bt.ecore`'s oracle already proves. Driving all three
//! would need three interpreted `Harness` pairs against one generated pair,
//! which would give the two arms different event sequence numbers and so
//! different concurrency tie-breaks, and that is a difference in the
//! *harness* and not in the paths. So one root is driven, and event *n* is
//! event *n* on both arms.
//!
//! # The two arms
//!
//! Interpreted: a pair of `moirai_interp::testing::Harness` replicas rooted
//! at `Foo`, the node tree under a real `Replica` with no `ModelOp::Install`
//! in front of it, as `ip28` does and for the same reason.
//!
//! Generated: a `twins_log::<TestLog>()` pair, driven by operations built as
//! JSON and deserialized into `Test`, so a wrong shape is a
//! `serde_json::from_value` error naming the enum and never a silent pass.
//!
//! # Why this is not a third copy of `bt_crdt/tests/support`
//!
//! That module's `Meta::root_class` asserts one declared root, its `Action`
//! is `Create`/`Delete`/`Text`/`Unset`, and its `project_object` panics on
//! any attribute that is not `LeafRule::Text` — by design, since `bt.ecore`
//! has none. Every one of those is wrong here. `json_crdt`'s oracle is
//! standalone for the same reason and this one follows it.
//!
//! # Every script opens with `New`
//!
//! `record!`'s `new` builds one field per feature, so `FooLog` renders its
//! whole shape from construction, while the interpreted slot holds no object
//! until one is minted. `Foo::New` on one arm and `InstanceOp::New` on the
//! other is the first edit of every script, after which both sides carry the
//! same nineteen keys at the same defaults and the read-outs are compared
//! **raw**, with no pruning pass on either side.

use std::sync::Arc;

use kitchen_crdt::package::{Test, TestLog, TestValue};
use moirai_crdt::utils::membership::twins_log;
use moirai_interp::testing::{class_slot, feature_slot, twins};
use moirai_interp::{InstanceOp, LeafOp, Scalar};
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::Tcsb;
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::{IsReplica, Replica};
use moirai_semantics::{
    ClassSlot, FeatureSlot, LeafRule, MergeRule, MetamodelSemantics, NumKind, Shape, from_descriptor,
};
use serde_json::{Map, Value, json};

type InterpReplica = Replica<moirai_interp::testing::Harness, Tcsb<InstanceOp>>;
type GenReplica = Replica<TestLog, Tcsb<Test>>;

/// The key a conflict set is carried under, `02 Validation Plan` §2.
const CONFLICT: &str = "__conflict";
/// The key a class name is carried under.
const ECLASS: &str = "eClass";
/// The one root class this oracle drives.
const ROOT: &str = "Foo";

// ---------------------------------------------------------------------------
// 1. The nineteen features of `Foo`, and what each of them takes
// ---------------------------------------------------------------------------

/// The construction one feature of `Foo` is, as far as an edit is concerned.
///
/// Written here and checked against the table in
/// [`the_table_says_what_this_file_says`], which is the point: this is the
/// second implementation, and a disagreement between the two is a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// `EventGraph<List<char>>`.
    Text,
    /// `VecLog<Counter<T>>` at an integer width.
    CounterInt,
    /// `VecLog<Counter<T>>` at a float width.
    CounterFloat,
    /// `VecLog<EWFlag>`.
    Flag,
    /// `VecLog<MVRegister<char>>`.
    Register,
    /// `NestedListLog<VecLog<Counter<i16>>>`.
    SeqCounterInt,
    /// `NestedListLog<VecLog<EWFlag>>`.
    SeqFlag,
    /// `AWBagLog<i16>`.
    Bag,
}

/// One feature: its Ecore name and its construction.
#[derive(Clone, Copy, Debug)]
struct Feature {
    name: &'static str,
    kind: Kind,
}

const fn feature(name: &'static str, kind: Kind) -> Feature {
    Feature { name, kind }
}

/// `Foo`'s features in declaration order, which is also visible-slot order:
/// `Foo` has no supertype, so the two coincide and the encoders use the
/// table's own index rather than this one.
const FEATURES: [Feature; 19] = [
    feature("myString", Kind::Text),
    feature("myInt", Kind::CounterInt),
    feature("myBoolean", Kind::Flag),
    feature("myChar", Kind::Register),
    feature("myLong", Kind::CounterInt),
    feature("myFloat", Kind::CounterFloat),
    feature("myDouble", Kind::CounterFloat),
    feature("myByte", Kind::CounterInt),
    feature("myShort", Kind::CounterInt),
    feature("bounds0inf", Kind::SeqCounterInt),
    feature("bounds1inf", Kind::SeqCounterInt),
    feature("bounds01", Kind::CounterInt),
    feature("bounds11", Kind::CounterInt),
    feature("boundsninf", Kind::SeqCounterInt),
    feature("boundsnm", Kind::SeqCounterInt),
    feature("simpleList", Kind::SeqFlag),
    feature("uniqueList", Kind::SeqCounterInt),
    feature("bag", Kind::Bag),
    feature("set", Kind::Bag),
];

/// `myByte` is a `Counter<u8>`, and `u8: SubAssign` panics on underflow in a
/// debug build on both paths alike. The script generator never proposes a
/// decrement on an unsigned width, so the oracle measures merge and not
/// arithmetic overflow, which is a property of `Counter<u8>` and not of
/// either path.
const UNSIGNED: &str = "myByte";

/// The Rust field name `record!` gives a feature: `paste!`'s `:camel` runs
/// over the snake-cased name the generator writes, so this is the generator's
/// own rule and not `heck`'s, whose digit handling differs on `bounds0inf`.
fn field_of(feature: &str) -> String {
    let mut out = String::new();
    for ch in feature.chars() {
        if ch.is_ascii_uppercase() && !out.is_empty() {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

/// The `record!` variant a field name becomes, which is `paste!`'s `:camel`:
/// the first letter of each underscore-separated run, capitalised.
fn variant_of(feature: &str) -> String {
    let field = field_of(feature);
    let mut out = String::new();
    let mut upper = true;
    for ch in field.chars() {
        if ch == '_' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// One externally tagged enum value, written out rather than through `json!`,
/// whose keys are literals.
fn tagged(variant: impl Into<String>, payload: Value) -> Value {
    let mut map = Map::new();
    map.insert(variant.into(), payload);
    Value::Object(map)
}

// ---------------------------------------------------------------------------
// 2. The table
// ---------------------------------------------------------------------------

/// The descriptor this crate was generated from, read from the crate itself
/// so that the interpreted arm cannot be running a different metamodel from
/// the one the generated arm was compiled from.
fn kitchen_descriptor() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/metamodel.json"))
            .expect("this crate's own descriptor is readable"),
    )
    .expect("it is JSON")
}

struct Meta {
    sem: Arc<MetamodelSemantics>,
    root: ClassSlot,
}

impl Meta {
    fn new() -> Meta {
        let sem = Arc::new(
            from_descriptor(&kitchen_descriptor()).expect("the checked-in descriptor parses"),
        );
        let root = class_slot(&sem, ROOT);
        Meta { sem, root }
    }

    fn slot(&self, feature: &str) -> FeatureSlot {
        feature_slot(&self.sem, self.root, feature)
    }

    fn rule(&self, feature: &str) -> MergeRule {
        *self
            .sem
            .rule(self.root, self.slot(feature))
            .expect("every feature of `Foo` carries a rule")
    }
}

// ---------------------------------------------------------------------------
// 3. The edit script
// ---------------------------------------------------------------------------

/// What one edit does to the leaf it addresses. `Elem` is what a sequence's
/// `Insert` and `Update` carry, and is the same vocabulary one level down.
#[derive(Clone, Debug, PartialEq)]
enum Elem {
    InsertChar { pos: usize, ch: char },
    DeleteChar { pos: usize },
    Inc(i64),
    Dec(i64),
    IncFloat(f64),
    DecFloat(f64),
    Reset,
    Enable,
    Disable,
    ClearFlag,
    Write(char),
    ClearRegister,
}

#[derive(Clone, Debug, PartialEq)]
enum Action {
    /// Mint the root object; the first edit of every script.
    New,
    /// A write to a single-valued leaf.
    Leaf(Elem),
    /// Put a new element into a sequence at `pos` and write it.
    SeqInsert { pos: usize, elem: Elem },
    /// Write the element already at `pos`.
    SeqUpdate { pos: usize, elem: Elem },
    /// Take the element at `pos` out.
    SeqDelete { pos: usize },
    /// One value into a bag.
    BagAdd(i64),
    /// One value out of a bag.
    BagRemove(i64),
    /// Everything out of a bag.
    BagClear,
}

/// One edit: which replica issues it, which feature of `Foo` it addresses,
/// and what it does. `New` addresses no feature.
#[derive(Clone, Debug)]
struct Edit {
    writer: char,
    feature: Option<&'static str>,
    action: Action,
}

impl Edit {
    fn show(&self) -> String {
        match (&self.feature, &self.action) {
            (None, Action::New) => format!("{}: New Foo", self.writer),
            (Some(name), action) => format!("{}: {name} — {action:?}", self.writer),
            (None, action) => format!("{}: {action:?}", self.writer),
        }
    }
}

#[derive(Clone, Debug)]
enum Move {
    Edit(Edit),
    /// Deliver everything both writers are holding, `a`'s events first.
    Deliver,
}

#[derive(Clone, Debug, Default)]
struct EditScript {
    label: String,
    steps: Vec<Move>,
}

// ---------------------------------------------------------------------------
// 4. The two encoders
// ---------------------------------------------------------------------------

/// One element write as the interpreted path spells it.
fn interp_elem(elem: &Elem) -> LeafOp {
    match elem {
        Elem::InsertChar { pos, ch } => LeafOp::InsertChar { pos: *pos, ch: *ch },
        Elem::DeleteChar { pos } => LeafOp::DeleteChar { pos: *pos },
        Elem::Inc(by) => LeafOp::Inc(Scalar::Int(*by)),
        Elem::Dec(by) => LeafOp::Dec(Scalar::Int(*by)),
        Elem::IncFloat(by) => LeafOp::Inc(Scalar::float(*by)),
        Elem::DecFloat(by) => LeafOp::Dec(Scalar::float(*by)),
        Elem::Reset => LeafOp::Reset,
        Elem::Enable => LeafOp::Enable,
        Elem::Disable => LeafOp::Disable,
        Elem::ClearFlag => LeafOp::Clear,
        Elem::Write(ch) => LeafOp::Write(Scalar::Char(*ch)),
        Elem::ClearRegister => LeafOp::Clear,
    }
}

/// One edit as an [`InstanceOp`] against the root `Foo`.
fn interp_op(meta: &Meta, edit: &Edit) -> InstanceOp {
    let inner = match (&edit.feature, &edit.action) {
        (None, Action::New) => return InstanceOp::variant(meta.root, InstanceOp::New),
        (Some(name), action) => {
            let slot = meta.slot(name);
            let step = match action {
                Action::New => unreachable!("`New` addresses no feature"),
                Action::Leaf(elem) => InstanceOp::Leaf(interp_elem(elem)),
                Action::SeqInsert { pos, elem } => {
                    InstanceOp::insert(*pos, InstanceOp::Leaf(interp_elem(elem)))
                }
                Action::SeqUpdate { pos, elem } => {
                    InstanceOp::at(*pos, InstanceOp::Leaf(interp_elem(elem)))
                }
                Action::SeqDelete { pos } => InstanceOp::delete(*pos),
                Action::BagAdd(value) => InstanceOp::Leaf(LeafOp::Add(Scalar::Int(*value))),
                Action::BagRemove(value) => InstanceOp::Leaf(LeafOp::Remove(Scalar::Int(*value))),
                Action::BagClear => InstanceOp::Leaf(LeafOp::Clear),
            };
            InstanceOp::field(slot, step)
        }
        (None, action) => panic!("{action:?} addresses a feature and none was given"),
    };
    InstanceOp::variant(meta.root, inner)
}

/// One element write as the generated path spells it: the op of whichever
/// `moirai-crdt` log the field holds.
fn typed_elem(elem: &Elem) -> Value {
    match elem {
        Elem::InsertChar { pos, ch } => json!({"Insert": {"content": ch, "pos": pos}}),
        Elem::DeleteChar { pos } => json!({"Delete": {"pos": pos}}),
        Elem::Inc(by) => tagged("Inc", json!(by)),
        Elem::Dec(by) => tagged("Dec", json!(by)),
        Elem::IncFloat(by) => tagged("Inc", json!(by)),
        Elem::DecFloat(by) => tagged("Dec", json!(by)),
        Elem::Reset => json!("Reset"),
        Elem::Enable => json!("Enable"),
        Elem::Disable => json!("Disable"),
        Elem::ClearFlag => json!("Clear"),
        Elem::Write(ch) => tagged("Write", json!(ch)),
        Elem::ClearRegister => json!("Clear"),
    }
}

/// One edit as this crate's typed operation, built as JSON by the naming
/// convention `record!` uses and then deserialized, so a wrong shape is an
/// error naming the enum and never a silent pass.
fn typed_op(edit: &Edit) -> Test {
    let value = typed_json(edit);
    serde_json::from_value(value.clone()).unwrap_or_else(|error| {
        panic!(
            "the typed encoder built an operation `Test` cannot take: {error}\n{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        )
    })
}

fn typed_json(edit: &Edit) -> Value {
    let inner = match (&edit.feature, &edit.action) {
        (None, Action::New) => json!("New"),
        (Some(name), action) => {
            let step = match action {
                Action::New => unreachable!("`New` addresses no feature"),
                Action::Leaf(elem) => typed_elem(elem),
                Action::SeqInsert { pos, elem } => {
                    json!({"Insert": {"pos": pos, "op": typed_elem(elem)}})
                }
                Action::SeqUpdate { pos, elem } => {
                    json!({"Update": {"pos": pos, "op": typed_elem(elem)}})
                }
                Action::SeqDelete { pos } => json!({"Delete": {"pos": pos}}),
                Action::BagAdd(value) => tagged("Add", json!(value)),
                Action::BagRemove(value) => tagged("Remove", json!(value)),
                Action::BagClear => json!("Clear"),
            };
            tagged(variant_of(name), step)
        }
        (None, action) => panic!("{action:?} addresses a feature and none was given"),
    };
    tagged("Foo", inner)
}

// ---------------------------------------------------------------------------
// 5. The projection of the generated read-out onto the canonical form
// ---------------------------------------------------------------------------

/// The generated `Read` value in the canonical form of `02 Validation Plan`
/// §2: an `eClass` key, `Vec<char>` joined, a many-valued register collapsed
/// the way `eval::read` collapses it, and a bag's `HashMap<i16, usize>`
/// spread into the sorted array with repeats that a bag reads as.
///
/// Nothing here prunes: every one of the nineteen keys is present on both
/// sides after `New`, which is why `New` is the first edit of every script.
fn project(value: &TestValue) -> Value {
    let raw = serde_json::to_value(value).expect("the generated read-out serializes");
    let foo = raw
        .get("foo")
        .expect("the package value carries `Foo` under its field");
    let mut out = Map::new();
    out.insert(ECLASS.to_string(), Value::String(ROOT.to_string()));
    for Feature { name, kind } in FEATURES {
        let found = foo
            .get(field_of(name))
            .unwrap_or_else(|| panic!("`FooValue` has no field for `{name}`"));
        out.insert(name.to_string(), project_feature(kind, found));
    }
    Value::Object(out)
}

fn project_feature(kind: Kind, value: &Value) -> Value {
    match kind {
        Kind::Text => chars(value),
        Kind::CounterInt | Kind::CounterFloat | Kind::Flag => value.clone(),
        Kind::Register => register(value),
        Kind::SeqCounterInt | Kind::SeqFlag => value.clone(),
        Kind::Bag => bag(value),
    }
}

/// A `Vec<char>` as a string.
fn chars(value: &Value) -> Value {
    let text: String = value
        .as_array()
        .unwrap_or_else(|| panic!("a text attribute reads as an array of characters: {value}"))
        .iter()
        .map(|ch| {
            ch.as_str()
                .unwrap_or_else(|| panic!("a character reads as a string"))
                .to_string()
        })
        .collect();
    Value::String(text)
}

/// A `HashSet<char>` as `leaf.rs`'s `many_valued` renders one: nothing at all
/// is `null`, one value is that value, and more than one is a conflict set
/// sorted the way `Scalar`'s own `Ord` sorts characters.
fn register(value: &Value) -> Value {
    let mut held: Vec<String> = value
        .as_array()
        .unwrap_or_else(|| panic!("an `MVRegister` reads as an array: {value}"))
        .iter()
        .map(|ch| {
            ch.as_str()
                .unwrap_or_else(|| panic!("a character reads as a string"))
                .to_string()
        })
        .collect();
    held.sort();
    match held.len() {
        0 => Value::Null,
        1 => Value::String(held.remove(0)),
        _ => {
            let mut object = Map::new();
            object.insert(
                CONFLICT.to_string(),
                Value::Array(held.into_iter().map(Value::String).collect()),
            );
            Value::Object(object)
        }
    }
}

/// A `HashMap<i16, usize>` as `leaf.rs`'s bag arm renders one: every value
/// repeated as many times as its count, in ascending numeric order.
fn bag(value: &Value) -> Value {
    let mut spread: Vec<i64> = Vec::new();
    for (key, count) in value
        .as_object()
        .unwrap_or_else(|| panic!("a bag reads as a map of value to count: {value}"))
    {
        let key: i64 = key.parse().expect("a bag of `EShort` is keyed by numbers");
        let count = count.as_u64().expect("a count is a number");
        for _ in 0..count {
            spread.push(key);
        }
    }
    spread.sort_unstable();
    Value::Array(spread.into_iter().map(Value::from).collect())
}

/// A canonical document with every default-valued key dropped, and a root
/// left with nothing but its class name spelled `null`.
///
/// Called on **both** sides of the comparison by the same function, and it is
/// the only place either side is pruned, so the two sides cannot drift.
///
/// # Why the rule exists
///
/// `record!`'s `new` (`moirai-macros/src/record.rs:57-63`) constructs one
/// field per feature, so a `FooLog` renders its whole shape from the moment
/// it is built: nineteen keys at their defaults before any operation has been
/// applied. `Foo::New` does not change that — its effect is a sink expansion
/// and nothing else (`record.rs:97-101`), so the generated path has no
/// representation at all of "this object now exists". The interpreted
/// `ObjectNode` holds no object until an operation mints one, and
/// `eval::read` spells a slot that holds none as `null`. Both describe the
/// same state, nothing has been written, and §2's canonical form had no way
/// to say so. This is `bt_crdt`'s `without_defaults` rule, reduced to the
/// leaves this metamodel actually has.
///
/// # What it cannot hide
///
/// A key present on one side and pruned on the other is reported as a
/// missing key, because pruning only ever removes a value equal to the
/// default and the other side's value is then not equal to it. What the rule
/// does drop on both sides at once is a counter incremented and reset back to
/// zero, or a string written and emptied: both paths hold the operations,
/// both read the default, and the rule says they agree, which they do.
fn without_defaults(value: Value) -> Value {
    let Value::Object(mut map) = value else {
        return value;
    };
    for Feature { name, kind } in FEATURES {
        if map.get(name) == Some(&default_of(kind)) {
            map.remove(name);
        }
    }
    if map.len() == 1 && map.contains_key(ECLASS) {
        return Value::Null;
    }
    Value::Object(map)
}

/// What a feature nobody has written reads as, per construction.
fn default_of(kind: Kind) -> Value {
    match kind {
        Kind::Text => Value::String(String::new()),
        Kind::CounterInt => json!(0),
        Kind::CounterFloat => json!(0.0),
        Kind::Flag => json!(false),
        Kind::Register => Value::Null,
        Kind::SeqCounterInt | Kind::SeqFlag | Kind::Bag => json!([]),
    }
}

/// The first key at which two canonical documents differ, for the message.
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
// 6. The harness
// ---------------------------------------------------------------------------

/// Two `twins` pairs, one per path, driven by one script.
struct Harness {
    meta: Meta,
    ia: InterpReplica,
    ib: InterpReplica,
    ga: GenReplica,
    gb: GenReplica,
    pending_a: Vec<(EventMessage<InstanceOp>, EventMessage<Test>)>,
    pending_b: Vec<(EventMessage<InstanceOp>, EventMessage<Test>)>,
    ops: usize,
    refused: usize,
    comparisons: usize,
}

impl Harness {
    fn new() -> Harness {
        let meta = Meta::new();
        let (ia, ib) = twins(&meta.sem, ROOT);
        let (ga, gb) = twins_log::<TestLog>();
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

    /// The interpreted read-out, pruned by the same function.
    fn interp_doc(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ia } else { &self.ib };
        without_defaults(replica.query(Read::<Value>::new()))
    }

    /// The generated read-out, canonicalised and then pruned.
    fn gen_doc(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ga } else { &self.gb };
        without_defaults(project(&replica.query(Read::<TestValue>::new())))
    }

    /// The four read-outs, compared, each pruned by [`without_defaults`].
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

    /// One edit, encoded twice and handed to the two replicas of its writer.
    fn carry(&mut self, edit: &Edit) -> Result<bool, String> {
        let interp = interp_op(&self.meta, edit);
        let generated = typed_op(edit);
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
// 7. Seeded generation over the read-out itself
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

const ALPHABET: [char; 6] = ['a', 'b', 'c', 'd', 'e', 'f'];
/// The values a bag is filled from. Small and few, so that the same value
/// really is added twice and removed once across one script and the count a
/// bag keeps is exercised rather than only its membership.
const BAG_VALUES: [i64; 4] = [-2, 0, 3, 7];

/// One element write for a leaf of this kind, proposed against what the
/// writer can see of it.
fn propose_elem(kind: Kind, seen: &Value, rng: &mut Rng) -> Elem {
    match kind {
        Kind::Text => {
            let len = seen.as_str().map_or(0, |text| text.chars().count());
            if len > 0 && rng.below(4) == 0 {
                Elem::DeleteChar {
                    pos: rng.below(len),
                }
            } else {
                Elem::InsertChar {
                    pos: rng.below(len + 1),
                    ch: ALPHABET[rng.below(ALPHABET.len())],
                }
            }
        }
        Kind::CounterInt => match rng.below(8) {
            0 => Elem::Reset,
            1..=2 => Elem::Dec(1 + rng.below(3) as i64),
            _ => Elem::Inc(1 + rng.below(4) as i64),
        },
        Kind::CounterFloat => match rng.below(8) {
            0 => Elem::Reset,
            1..=2 => Elem::DecFloat(0.5 * (1 + rng.below(4)) as f64),
            _ => Elem::IncFloat(0.5 * (1 + rng.below(6)) as f64),
        },
        Kind::Flag | Kind::SeqFlag => match rng.below(8) {
            0 => Elem::ClearFlag,
            1..=4 => Elem::Enable,
            _ => Elem::Disable,
        },
        Kind::Register => {
            if rng.below(8) == 0 {
                Elem::ClearRegister
            } else {
                Elem::Write(ALPHABET[rng.below(ALPHABET.len())])
            }
        }
        Kind::SeqCounterInt => match rng.below(8) {
            0 => Elem::Reset,
            1..=2 => Elem::Dec(1 + rng.below(3) as i64),
            _ => Elem::Inc(1 + rng.below(4) as i64),
        },
        Kind::Bag => unreachable!("a bag takes no element write"),
    }
}

/// One edit a writer looking at `seen` could make.
fn propose(seen: &Value, rng: &mut Rng, writer: char, guard: bool) -> Edit {
    let Feature { name, kind } = FEATURES[rng.below(FEATURES.len())];
    // A pruned read-out carries no key for a feature at its default, and a
    // model that has just been minted is pruned to `null` entirely; either
    // way the writer is looking at the default, which is what it proposes
    // against.
    let here = seen.get(name).cloned().unwrap_or_else(|| default_of(kind));
    let here = &here;
    let action = match kind {
        Kind::Bag => {
            // A removal is proposed only for a value the writer can see.
            // `AWBag::Remove` is `Counter<usize>::Dec(1)`
            // (`aw_bag.rs:84`), so removing what is not there underflows a
            // `usize` in a debug build — identically on both paths, since
            // the interpreted bag is composed out of the same two logs
            // (`leaf.rs:336`). That is a hazard in `moirai-crdt` and not a
            // difference between the paths, and an oracle that tripped it
            // would be measuring the panic and not the merge.
            let held: Vec<i64> = here
                .as_array()
                .map(|items| items.iter().filter_map(Value::as_i64).collect())
                .unwrap_or_default();
            match rng.below(8) {
                0 => Action::BagClear,
                1..=3 if !held.is_empty() => Action::BagRemove(held[rng.below(held.len())]),
                _ => Action::BagAdd(BAG_VALUES[rng.below(BAG_VALUES.len())]),
            }
        }
        Kind::SeqCounterInt | Kind::SeqFlag => {
            let len = here.as_array().map_or(0, Vec::len);
            let mut elem = propose_elem(kind, &Value::Null, rng);
            // Under `guard`, an element write that could leave the element
            // reading its own type's default is replaced by one that cannot.
            // That is the one construction `ip29` found the two paths
            // disagreeing on, and it is pinned by
            // `ip29_a_sequence_element_at_its_own_default_is_dropped_by_the_generated_read_out`
            // rather than hidden here: this guard is what lets the other
            // eighteen features be measured over thirty scripts instead of
            // every script stopping at the first flag turned off.
            if guard {
                elem = match elem {
                    Elem::Disable | Elem::ClearFlag => Elem::Enable,
                    Elem::Reset | Elem::Dec(_) => Elem::Inc(1 + rng.below(4) as i64),
                    other => other,
                };
            }
            if len == 0 {
                Action::SeqInsert { pos: 0, elem }
            } else {
                match rng.below(6) {
                    0 => Action::SeqDelete {
                        pos: rng.below(len),
                    },
                    1..=3 => Action::SeqUpdate {
                        pos: rng.below(len),
                        elem,
                    },
                    _ => Action::SeqInsert {
                        pos: rng.below(len + 1),
                        elem,
                    },
                }
            }
        }
        _ => {
            let mut elem = propose_elem(kind, here, rng);
            // `Counter<u8>` underflows on both paths alike; see `UNSIGNED`.
            if name == UNSIGNED {
                elem = match elem {
                    Elem::Dec(by) => Elem::Inc(by),
                    other => other,
                };
            }
            Action::Leaf(elem)
        }
    };
    Edit {
        writer,
        feature: Some(name),
        action,
    }
}

/// The opening edit of every script: `a` mints the root and it is delivered
/// before anything else happens, because `record!`'s `New` is enabled only
/// while the record is at its default.
fn open() -> Edit {
    Edit {
        writer: 'a',
        feature: None,
        action: Action::New,
    }
}

/// Ten sequential scripts and twenty with a concurrent half.
///
/// The generator runs against a shadow pair driven by the same edits, so it
/// proposes against what a writer would actually be looking at. The shadow is
/// carried and never compared: a difference is the finding `ip29` exists to
/// report, and a generator that halted on the first one would only ever
/// report the first one.
fn seeded_script(seed: u64, concurrent: bool, guard: bool) -> EditScript {
    let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x0BAD_C0DE);
    let mut steps: Vec<Move> = vec![Move::Edit(open()), Move::Deliver];
    let mut shadow = Harness::new();
    shadow.carry(&open()).expect("a fresh root takes `New`");
    shadow.cross();

    if concurrent {
        for _ in 0..5 {
            let edits = 2 + rng.below(3);
            for _ in 0..edits {
                for writer in ['a', 'b'] {
                    let seen = shadow.interp_doc(writer);
                    let edit = propose(&seen, &mut rng, writer, guard);
                    if shadow.carry(&edit).is_err() {
                        return EditScript {
                            label: format!("concurrent seed {seed}"),
                            steps,
                        };
                    }
                    steps.push(Move::Edit(edit));
                }
            }
            shadow.cross();
            steps.push(Move::Deliver);
        }
    } else {
        for index in 0..24 {
            let writer = if index % 3 == 0 { 'b' } else { 'a' };
            let seen = shadow.interp_doc(writer);
            let edit = propose(&seen, &mut rng, writer, guard);
            if shadow.carry(&edit).is_err() {
                break;
            }
            steps.push(Move::Edit(edit));
            steps.push(Move::Deliver);
            shadow.cross();
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

fn scripts(guard: bool) -> Vec<EditScript> {
    let mut out: Vec<EditScript> = (0..10)
        .map(|seed| seeded_script(seed, false, guard))
        .collect();
    out.extend((0..20).map(|seed| seeded_script(seed, true, guard)));
    out
}

// ---------------------------------------------------------------------------
// 8. The tests
// ---------------------------------------------------------------------------

/// The nineteen features this file names, and the construction it names for
/// each, are the ones the table derived from the same descriptor carries.
/// Without this, every encoder below is writing against a metamodel of its
/// own invention.
#[test]
fn the_table_says_what_this_file_says() {
    let meta = Meta::new();
    let class = &meta.sem.classes[meta.root.index()];
    assert_eq!(
        class.visible.len(),
        FEATURES.len(),
        "`Foo` carries {} features and this file names {}",
        class.visible.len(),
        FEATURES.len()
    );
    let mut named: Vec<&str> = FEATURES.iter().map(|f| f.name).collect();
    let mut visible: Vec<&str> = class.visible.iter().map(|(name, _, _)| &**name).collect();
    named.sort_unstable();
    visible.sort_unstable();
    assert_eq!(named, visible, "`Foo`'s features, as the table holds them");
    for Feature { name, kind } in FEATURES.iter() {
        let derived = match meta.rule(name) {
            MergeRule::Attribute { shape, leaf } => match (shape, leaf) {
                (Shape::Single, LeafRule::Text) => Kind::Text,
                (
                    Shape::Single,
                    LeafRule::Counter {
                        num: NumKind::F32 | NumKind::F64,
                        ..
                    },
                ) => Kind::CounterFloat,
                (Shape::Single, LeafRule::Counter { .. }) => Kind::CounterInt,
                (Shape::Single, LeafRule::Flag { .. }) => Kind::Flag,
                (Shape::Single, LeafRule::Register { .. }) => Kind::Register,
                (Shape::Sequence, LeafRule::Counter { .. }) => Kind::SeqCounterInt,
                (Shape::Sequence, LeafRule::Flag { .. }) => Kind::SeqFlag,
                (Shape::Bag, _) => Kind::Bag,
                other => panic!("`{name}` is {other:?}, which this file does not name"),
            },
            other => panic!("`{name}` is {other:?}, which is not an attribute"),
        };
        assert_eq!(derived, *kind, "`{name}`");
    }
    assert_eq!(&*meta.sem.package, "test");
    assert_eq!(meta.sem.roots.len(), 3, "`Bar`, `Baz` and `Foo` are roots");
}

/// The census this driver was written to establish, asserted rather than
/// described: which of `moirai-interp`'s leaf arms `kitchen_sink.ecore`
/// reaches and which it does not. A metamodel edited to reach one more makes
/// this fail and the doc comment at the top of this file wrong at the same
/// time, which is the point.
#[test]
fn ip29_the_census_of_what_this_metamodel_reaches() {
    let meta = Meta::new();
    let mut widths: Vec<String> = Vec::new();
    let mut ties: Vec<String> = Vec::new();
    let mut shapes: Vec<String> = Vec::new();
    let mut resettable_only = true;
    let mut flags_ew_only = true;
    for class in &meta.sem.classes {
        for (index, _) in class.visible.iter().enumerate() {
            let Some(rule) = meta.sem.rule(class.slot, FeatureSlot(index as u16)) else {
                continue;
            };
            if let MergeRule::Attribute { shape, leaf } = rule {
                shapes.push(format!("{shape:?}"));
                match leaf {
                    LeafRule::Counter { num, resettable } => {
                        widths.push(format!("{num:?}"));
                        resettable_only &= *resettable;
                    }
                    LeafRule::Register { tie } => ties.push(format!("{tie:?}")),
                    LeafRule::Flag { wins } => {
                        flags_ew_only &= matches!(wins, moirai_semantics::FlagWins::Enable);
                    }
                    LeafRule::Text => {}
                    LeafRule::Enum { .. } => panic!("this metamodel has no enum"),
                }
            }
        }
    }
    widths.sort();
    widths.dedup();
    ties.sort();
    ties.dedup();
    shapes.sort();
    shapes.dedup();
    assert_eq!(
        widths,
        vec!["F32", "F64", "I16", "I32", "I64", "U8"],
        "all six counter widths, which is the whole of what this metamodel adds"
    );
    assert!(resettable_only, "every counter here is resettable");
    assert!(flags_ew_only, "every flag here is enable-wins");
    assert_eq!(
        ties,
        vec!["MultiValue"],
        "one register tie-break out of five; the other four need a `datatype` annotation"
    );
    assert_eq!(
        shapes,
        vec!["Bag", "Sequence", "Single"],
        "`set` derives `Bag` too, because `unique` unspecified reads as false"
    );
    assert!(
        meta.sem.enums.is_empty(),
        "no enum, so `LeafRule::Enum` is not driven here"
    );
    eprintln!(
        "ip29 census: widths {widths:?}, register ties {ties:?}, shapes {shapes:?}, enums 0"
    );
}

/// One write of every construction the metamodel carries, sequential and
/// delivered, read out identically on both paths.
#[test]
fn ip29_one_write_of_every_construction_reads_the_same_on_both_paths() {
    let mut harness = Harness::new();
    let edits = vec![
        open(),
        leaf('a', "myString", Elem::InsertChar { pos: 0, ch: 'x' }),
        leaf('a', "myInt", Elem::Inc(4)),
        leaf('b', "myLong", Elem::Dec(2)),
        leaf('a', "myShort", Elem::Inc(7)),
        leaf('b', "myByte", Elem::Inc(3)),
        leaf('a', "myFloat", Elem::IncFloat(1.5)),
        leaf('b', "myDouble", Elem::IncFloat(2.25)),
        leaf('a', "myBoolean", Elem::Enable),
        leaf('b', "myChar", Elem::Write('q')),
        leaf('a', "bounds01", Elem::Inc(1)),
        Edit {
            writer: 'b',
            feature: Some("bounds0inf"),
            action: Action::SeqInsert {
                pos: 0,
                elem: Elem::Inc(5),
            },
        },
        Edit {
            writer: 'a',
            feature: Some("simpleList"),
            action: Action::SeqInsert {
                pos: 0,
                elem: Elem::Enable,
            },
        },
        Edit {
            writer: 'b',
            feature: Some("bag"),
            action: Action::BagAdd(3),
        },
        Edit {
            writer: 'a',
            feature: Some("bag"),
            action: Action::BagAdd(3),
        },
        Edit {
            writer: 'b',
            feature: Some("set"),
            action: Action::BagAdd(-2),
        },
    ];
    for edit in &edits {
        harness
            .apply(edit)
            .unwrap_or_else(|reason| panic!("{reason}"));
        harness
            .deliver()
            .unwrap_or_else(|reason| panic!("{reason}"));
    }
    let read = harness.interp_doc('a');
    assert_eq!(read, harness.gen_doc('a'));
    assert_eq!(read, harness.interp_doc('b'));
    assert_eq!(read, harness.gen_doc('b'));
    assert_eq!(read["myString"], json!("x"));
    assert_eq!(read["myInt"], json!(4));
    assert_eq!(read["myLong"], json!(-2));
    assert_eq!(read["myShort"], json!(7));
    assert_eq!(read["myByte"], json!(3));
    assert_eq!(read["myFloat"], json!(1.5));
    assert_eq!(read["myDouble"], json!(2.25));
    assert_eq!(read["myBoolean"], json!(true));
    assert_eq!(read["myChar"], json!("q"));
    assert_eq!(read["bounds0inf"], json!([5]));
    assert_eq!(read["simpleList"], json!([true]));
    assert_eq!(read["bag"], json!([3, 3]), "a bag keeps the count");
    assert_eq!(read["set"], json!([-2]));
}

/// A helper for the hand-written cases: one write to one single-valued leaf.
fn leaf(writer: char, feature: &'static str, elem: Elem) -> Edit {
    Edit {
        writer,
        feature: Some(feature),
        action: Action::Leaf(elem),
    }
}

/// Two concurrent writes to the multi-value register keep both on both paths,
/// and a later write that is causally below them replaces both. This is the
/// only register tie-break `kitchen_sink.ecore` reaches, and a tie-break that
/// is never contended is not being tested at all.
#[test]
fn ip29_a_contended_multi_value_register_settles_the_same_way_on_both_paths() {
    let mut harness = Harness::new();
    harness.apply(&open()).unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    harness
        .carry(&leaf('a', "myChar", Elem::Write('m')))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .carry(&leaf('b', "myChar", Elem::Write('z')))
        .unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    let read = harness.interp_doc('a');
    assert_eq!(read, harness.gen_doc('a'));
    assert_eq!(read, harness.interp_doc('b'));
    assert_eq!(read, harness.gen_doc('b'));
    assert_eq!(
        read["myChar"],
        json!({CONFLICT: ["m", "z"]}),
        "both concurrent writes survive, sorted"
    );

    harness
        .apply(&leaf('a', "myChar", Elem::Write('k')))
        .unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));
    assert_eq!(harness.interp_doc('a')["myChar"], json!("k"));
    assert_eq!(harness.gen_doc('b')["myChar"], json!("k"));
}

/// A concurrent increment and reset of the same resettable counter, and a
/// concurrent add and remove of the same bag value, settle the same way on
/// both paths. These two are the whole reason `kitchen_sink.ecore` is worth a
/// driver, and neither is reachable from `bt.ecore` or `json.ecore`.
#[test]
fn ip29_a_contended_counter_and_a_contended_bag_settle_the_same_way_on_both_paths() {
    let mut harness = Harness::new();
    harness.apply(&open()).unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    harness
        .apply(&leaf('a', "myInt", Elem::Inc(10)))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .apply(&edit('a', "bag", Action::BagAdd(7)))
        .unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    harness
        .carry(&leaf('a', "myInt", Elem::Reset))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .carry(&leaf('b', "myInt", Elem::Inc(3)))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .carry(&edit('a', "bag", Action::BagRemove(7)))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .carry(&edit('b', "bag", Action::BagAdd(7)))
        .unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    let read = harness.interp_doc('a');
    assert_eq!(read, harness.gen_doc('a'), "the reset settles the same way");
    assert_eq!(read, harness.interp_doc('b'));
    assert_eq!(read, harness.gen_doc('b'));
}

fn edit(writer: char, feature: &'static str, action: Action) -> Edit {
    Edit {
        writer,
        feature: Some(feature),
        action,
    }
}

/// The body of the thirty-script run, shared by the guarded and unguarded
/// forms below so that the two differ in exactly one argument.
fn run_thirty(guard: bool, label: &str) {
    let scripts = scripts(guard);
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
    // The census, printed under `--nocapture` so the number in the vault is a
    // number this run produced.
    eprintln!(
        "{label}: {} scripts, {edits} edits, {comparisons} comparisons, {refused} refused by both intakes",
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

/// **ip29** — thirty seeded scripts, ten sequential and twenty with a
/// concurrent half, compared after every operation and after every crossing,
/// with no exclusion on either side's read-out and one construction held back
/// from the *generator*: a write to an element of an ordered attribute that
/// leaves that element reading its own type's default. That construction is
/// the divergence `ip29` found, and it is asserted in full by
/// `ip29_a_sequence_element_at_its_own_default_is_dropped_by_the_generated_read_out`
/// below. Every other construction of the metamodel is proposed freely.
#[test]
fn ip29_thirty_scripts_over_kitchen_sink_ecore_agree_everywhere() {
    run_thirty(true, "ip29");
}

/// The same thirty scripts with nothing held back. **Red**, and ignored for
/// that reason rather than deleted or narrowed: it is the record of the
/// divergence, and it goes green the day
/// `NestedListLog::execute_query` stops filtering its children through
/// `UWMapLog`'s "differs from its default" rule. Run it with
/// `cargo test --test equivalence -- --ignored --nocapture`.
#[test]
#[ignore = "records the NestedListLog default-element divergence; see ip29_a_sequence_element_at_its_own_default_is_dropped_by_the_generated_read_out"]
fn ip29_thirty_unguarded_scripts_over_kitchen_sink_ecore() {
    run_thirty(false, "ip29 unguarded");
}

/// The divergence, pinned.
///
/// `simpleList` is `NestedListLog<VecLog<EWFlag>>` on the generated path and
/// `Shaped::Sequence(LeafSite::Scalar(Flag))` on the interpreted one. One
/// element is inserted and disabled, which leaves it reading `false`, the
/// default of `EWFlag`. Both intakes accept the operation and both orderings
/// hold one element. The read-outs do not agree: the interpreted path renders
/// `[false]` and the generated path renders `[]`.
///
/// **Which path is right.** The interpreted one.
/// `NestedListLog::execute_query` (`nested_list.rs:239-268`) walks its
/// `positions` and looks each id up in `self.children.execute_query(..)`,
/// and `children` is a `UWMapLog`, whose read renders a child only when its
/// value differs from that child's default (`uw_map.rs:199-210`). That rule
/// is right for a keyed map, where reading as the default *is* how a removal
/// is spelled, and wrong for an ordered list, where the ordering says whether
/// an element is there and `NestedList::Delete` is what takes it out. The
/// generated log is inconsistent with itself about it:
/// `NestedListLog::is_enabled` (`nested_list.rs:214-231`) bounds-checks
/// against `positions.len()`, which counts the hidden element, so a writer
/// who reads `[]` and inserts at position 0 lands *before* an element it
/// cannot see.
///
/// Not fixed here, and deliberately: a fix to `moirai-crdt` and its own
/// oracle in one pass is how a wrong fix gets green.
#[test]
fn ip29_a_sequence_element_at_its_own_default_is_dropped_by_the_generated_read_out() {
    let mut harness = Harness::new();
    harness.apply(&open()).unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    let insert = Edit {
        writer: 'a',
        feature: Some("simpleList"),
        action: Action::SeqInsert {
            pos: 0,
            elem: Elem::Disable,
        },
    };
    assert!(
        harness.carry(&insert).expect("both intakes agree on it"),
        "both intakes accept an insert that disables the new flag"
    );
    harness.cross();

    let interp = harness.interp_doc('a');
    let generated = harness.gen_doc('a');
    assert_eq!(interp["simpleList"], json!([false]), "the interpreted path");
    assert_eq!(generated["simpleList"], Value::Null, "the generated path");
    assert_ne!(interp, generated, "this is the divergence, not agreement");
    assert_eq!(interp, harness.interp_doc('b'), "each path is self-consistent");
    assert_eq!(generated, harness.gen_doc('b'));

    // The same on a counter element: `Reset` leaves it at 0, which is
    // `Counter<i16>`'s default, and the generated read-out loses it.
    let reset = Edit {
        writer: 'b',
        feature: Some("uniqueList"),
        action: Action::SeqInsert {
            pos: 0,
            elem: Elem::Reset,
        },
    };
    assert!(harness.carry(&reset).expect("both intakes agree on it"));
    harness.cross();
    assert_eq!(harness.interp_doc('a')["uniqueList"], json!([0]));
    assert_eq!(harness.gen_doc('a')["uniqueList"], Value::Null);

    // And the generated log counts what it does not show: an insert at
    // position 1 is enabled, though the read-out offers only position 0.
    let second = Edit {
        writer: 'a',
        feature: Some("simpleList"),
        action: Action::SeqInsert {
            pos: 1,
            elem: Elem::Enable,
        },
    };
    assert!(
        harness.carry(&second).expect("both intakes agree on it"),
        "the generated ordering holds the element its read-out hides"
    );
    harness.cross();
    assert_eq!(harness.interp_doc('a')["simpleList"], json!([false, true]));
    assert_eq!(harness.gen_doc('a')["simpleList"], json!([true]));

    // And it does not heal irreversibly, which is the half `I-A1`'s
    // exception does not cover: the element at position 1 is visible on both
    // paths above, and disabling it takes it back out of the generated
    // read-out while the interpreted one keeps it.
    let hide = Edit {
        writer: 'b',
        feature: Some("simpleList"),
        action: Action::SeqUpdate {
            pos: 1,
            elem: Elem::Disable,
        },
    };
    assert!(harness.carry(&hide).expect("both intakes agree on it"));
    harness.cross();
    assert_eq!(harness.interp_doc('a')["simpleList"], json!([false, false]));
    assert_eq!(
        harness.gen_doc('a')["simpleList"],
        Value::Null,
        "a visible element written back to its default goes away again"
    );
}

/// The oracle fails when the two paths genuinely differ, which is criterion
/// I-A2 for this driver: the generated arm is handed one increment the
/// interpreted arm never sees, and the comparison catches it and names the
/// feature.
#[test]
fn ip29_the_oracle_notices_when_the_two_encoders_disagree() {
    let mut harness = Harness::new();
    harness.apply(&open()).unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    // The mutation: a `myShort` increment on the generated arm alone.
    let stray: Test = serde_json::from_value(tagged(
        "Foo",
        tagged(variant_of("myShort"), tagged("Inc", json!(11))),
    ))
    .expect("the shape is right; the asymmetry is the lie");
    let event = harness.ga.send(stray).expect("the generated log takes it");
    harness.gb.receive(event);

    let reason = harness
        .compare()
        .expect_err("the two read-outs now differ and the oracle has to say so");
    assert!(reason.contains("myShort"), "{reason}");
    assert!(reason.contains("interpreted"), "{reason}");
}

// ---------------------------------------------------------------------------
// The conflict matrix over `kitchen_sink.ecore`
// ---------------------------------------------------------------------------
//
// `moirai_interp::matrix` holds the whole matrix and assigns this crate the
// six resettable counter widths, the enable-wins flag, the multi-value
// register, the bag and the sequence of attribute values. What is here is the
// cell table for those rows, driven through this file's own encoders and its
// own projection, unchanged. Every cell opens with `New` and acknowledges with
// one character into `Foo.myString`, which no cell here contends.
//
// The sequence cells keep every element away from its own default at every
// point of every schedule, because an element at its default is the named
// divergence `ip29_a_sequence_element_at_its_own_default_is_dropped_by_the_generated_read_out`
// pins, and this file's projection carries no exception for it: seeding the
// elements at 1 and 2 and only ever incrementing them is what keeps each cell
// about the ordering and not about that divergence.

use moirai_interp::matrix::{self, Arm, Cell, Construction, pattern as p};
use moirai_semantics::TieBreak;

fn on(feature: &'static str, elem: Elem) -> Edit {
    leaf('a', feature, elem)
}

fn act(feature: &'static str, action: Action) -> Edit {
    edit('a', feature, action)
}

fn beat() -> Edit {
    on("myString", Elem::InsertChar { pos: 0, ch: 'h' })
}

fn opened(extra: Vec<Edit>) -> Vec<Edit> {
    let mut out = vec![open()];
    out.extend(extra);
    out
}

fn kitchen_cells() -> Vec<Cell<Edit>> {
    use serde_json::Value::Null;
    let mut cells = Vec::new();

    // The six resettable counter widths.
    for (feature, num, float) in [
        ("myByte", NumKind::U8, false),
        ("myShort", NumKind::I16, false),
        ("myInt", NumKind::I32, false),
        ("myLong", NumKind::I64, false),
        ("myFloat", NumKind::F32, true),
        ("myDouble", NumKind::F64, true),
    ] {
        let row = Construction::Counter(num);
        let pointer: &'static str = Box::leak(format!("/{feature}").into_boxed_str());
        let inc = move |by: i64| {
            on(feature, if float { Elem::IncFloat(by as f64) } else { Elem::Inc(by) })
        };
        let dec = move |by: i64| {
            on(feature, if float { Elem::DecFloat(by as f64) } else { Elem::Dec(by) })
        };
        let number = move |value: i64| if float { json!(value as f64) } else { json!(value) };
        let seeded = || opened(vec![inc(10)]);
        cells.push(
            Cell::new(row, p::INC_INC, opened(vec![]), vec![vec![inc(2)], vec![inc(3)]], beat())
                .expect(pointer, number(5)),
        );
        cells.push(
            Cell::new(row, p::INC_DEC, seeded(), vec![vec![inc(2)], vec![dec(3)]], beat())
                .expect(pointer, number(9)),
        );
        cells.push(
            Cell::new(row, p::INC_RESET, seeded(), vec![vec![inc(2)], vec![on(feature, Elem::Reset)]], beat())
                .expect(pointer, number(2)),
        );
        cells.push(
            Cell::new(
                row,
                p::RESET_RESET,
                seeded(),
                vec![vec![on(feature, Elem::Reset)], vec![on(feature, Elem::Reset)]],
                beat(),
            )
            .expect(pointer, Null),
        );
    }

    // The enable-wins flag: `Foo.myBoolean`.
    let row = Construction::EnableWinsFlag;
    let f = |elem: Elem| vec![on("myBoolean", elem)];
    let enabled = || opened(vec![on("myBoolean", Elem::Enable)]);
    cells.push(
        Cell::new(row, p::ENABLE_DISABLE, opened(vec![]), vec![f(Elem::Enable), f(Elem::Disable)], beat())
            .expect("/myBoolean", json!(true)),
    );
    cells.push(
        Cell::new(row, p::ENABLE_ENABLE, opened(vec![]), vec![f(Elem::Enable), f(Elem::Enable)], beat())
            .expect("/myBoolean", json!(true)),
    );
    cells.push(
        Cell::new(row, p::DISABLE_DISABLE, enabled(), vec![f(Elem::Disable), f(Elem::Disable)], beat())
            .expect("/myBoolean", Null),
    );
    cells.push(
        Cell::new(row, p::ENABLE_CLEAR, enabled(), vec![f(Elem::Enable), f(Elem::ClearFlag)], beat())
            .expect("/myBoolean", json!(true)),
    );
    cells.push(
        Cell::new(row, p::DISABLE_CLEAR, enabled(), vec![f(Elem::Disable), f(Elem::ClearFlag)], beat())
            .expect("/myBoolean", Null),
    );

    // The multi-value register over `EChar`: `Foo.myChar`.
    let row = Construction::Register(TieBreak::MultiValue);
    let wr = |ch| vec![on("myChar", Elem::Write(ch))];
    cells.push(
        Cell::new(row, p::WRITE_WRITE_DIFFERENT, opened(vec![]), vec![wr('z'), wr('m')], beat())
            .expect("/myChar", json!({CONFLICT: ["m", "z"]})),
    );
    cells.push(
        Cell::new(row, p::WRITE_WRITE_SAME, opened(vec![]), vec![wr('q'), wr('q')], beat())
            .expect("/myChar", json!("q")),
    );
    cells.push(
        Cell::new(
            row,
            p::WRITE_CLEAR,
            opened(vec![on("myChar", Elem::Write('k'))]),
            vec![wr('m'), vec![on("myChar", Elem::ClearRegister)]],
            beat(),
        )
        .expect("/myChar", json!("m")),
    );
    cells.push(
        Cell::new(row, p::THREE_WRITERS, opened(vec![]), vec![wr('z'), wr('m'), wr('q')], beat())
            .expect("/myChar", json!({CONFLICT: ["m", "q", "z"]})),
    );

    // The bag: `Foo.bag`, over `EShort`.
    let row = Construction::Bag;
    let b = |action: Action| vec![act("bag", action)];
    cells.push(
        Cell::new(
            row,
            p::ADD_REMOVE_PRESENT,
            opened(vec![act("bag", Action::BagAdd(7))]),
            vec![b(Action::BagRemove(7)), b(Action::BagAdd(7))],
            beat(),
        )
        .expect("/bag", json!([7])),
    );
    cells.push(
        Cell::new(row, p::ADD_ADD_SAME, opened(vec![]), vec![b(Action::BagAdd(7)), b(Action::BagAdd(7))], beat())
            .expect("/bag", json!([7, 7])),
    );
    cells.push(
        Cell::new(
            row,
            p::ADD_REMOVE_DIFFERENT,
            opened(vec![act("bag", Action::BagAdd(5))]),
            vec![b(Action::BagAdd(7)), b(Action::BagRemove(5))],
            beat(),
        )
        .expect("/bag", json!([7])),
    );
    cells.push(
        Cell::new(
            row,
            p::REMOVE_REMOVE_SAME,
            opened(vec![act("bag", Action::BagAdd(7)), act("bag", Action::BagAdd(7))]),
            vec![b(Action::BagRemove(7)), b(Action::BagRemove(7))],
            beat(),
        )
        .expect("/bag", Null),
    );
    cells.push(
        Cell::new(
            row,
            p::ADD_CLEAR,
            opened(vec![act("bag", Action::BagAdd(5))]),
            vec![b(Action::BagAdd(7)), b(Action::BagClear)],
            beat(),
        )
        .expect("/bag", json!([7])),
    );

    // The sequence of attribute values: `Foo.bounds0inf`, seeded [1, 2].
    let row = Construction::SequenceOfValues;
    let seeded = || {
        opened(vec![
            act("bounds0inf", Action::SeqInsert { pos: 0, elem: Elem::Inc(1) }),
            act("bounds0inf", Action::SeqInsert { pos: 1, elem: Elem::Inc(2) }),
        ])
    };
    let insert = |pos, by| vec![act("bounds0inf", Action::SeqInsert { pos, elem: Elem::Inc(by) })];
    let update = |pos, by| vec![act("bounds0inf", Action::SeqUpdate { pos, elem: Elem::Inc(by) })];
    let delete = |pos| vec![act("bounds0inf", Action::SeqDelete { pos })];
    cells.push(Cell::new(row, p::INSERT_INSERT_SAME_POS, seeded(), vec![insert(1, 5), insert(1, 6)], beat()));
    cells.push(
        Cell::new(row, p::INSERT_DELETE, seeded(), vec![insert(1, 5), delete(0)], beat())
            .expect("/bounds0inf", json!([5, 2])),
    );
    cells.push(Cell::new(row, p::DELETE_UPDATE_SAME, seeded(), vec![delete(0), update(0, 3)], beat()));
    cells.push(
        Cell::new(row, p::DELETE_DELETE_SAME, seeded(), vec![delete(0), delete(0)], beat())
            .expect("/bounds0inf", json!([2])),
    );
    cells.push(
        Cell::new(row, p::UPDATE_UPDATE_SAME, seeded(), vec![update(0, 3), update(0, 4)], beat())
            .expect("/bounds0inf", json!([8, 2])),
    );
    cells.push(Cell::new(
        row,
        p::THREE_INSERTS_SAME_POS,
        seeded(),
        vec![insert(1, 5), insert(1, 6), insert(1, 7)],
        beat(),
    ));
    cells
}

/// **The conflict matrix** over `kitchen_sink.ecore`: every cell the registry
/// assigns to this crate, each under every schedule.
#[test]
fn conflict_matrix_over_kitchen_sink_ecore() {
    let meta = Meta::new();
    moirai_interp::testing::install_fixture(&meta.sem, ROOT);
    let interp_encode = |edit: &Edit, _: &Value| interp_op(&meta, edit);
    let gen_encode = |edit: &Edit, _: &Value| typed_op(edit);
    let interp_read = |replica: &InterpReplica| without_defaults(replica.query(Read::<Value>::new()));
    let gen_read =
        |replica: &GenReplica| without_defaults(project(&replica.query(Read::<TestValue>::new())));
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
    let cells = kitchen_cells();
    matrix::run_matrix(matrix::KITCHEN, &meta.sem, &cells, &interp, &generated)
        .unwrap_or_else(|reason| panic!("{reason}"));
}

/// **A finding of the conflict matrix, pinned and not fixed.** Two replicas
/// that each hold one copy of a bag value and concurrently remove it take the
/// value's `Counter<usize>` below zero, and both paths panic on it identically:
/// on the first read of the bag, and again, inside `receive`, the moment the
/// two removes stabilize.
///
/// A bag is `UWMapLog<V, VecLog<Counter<usize>>>` on both paths, an `Add` being
/// `Inc(1)` and a `Remove` `Dec(1)` (`moirai-crdt/src/bag/aw_bag.rs`,
/// `moirai-interp/src/leaf.rs`'s `bag_op`). Each remove is enabled locally,
/// because each writer sees one copy, and a remote operation is never refused.
/// Once both have crossed, the unstable fold in `Counter::execute_query`
/// (`resettable_counter.rs:81`) computes `1 - 1 - 1` in `usize`, and once they
/// stabilize `CounterStable::apply` (`counter/stable.rs`) does the same
/// subtraction into the stable state, both `attempt to subtract with overflow`
/// under a debug build. The matrix's own `remove ∥ remove of the same value`
/// cell seeds two copies so that it tests the merge and not this; this test is
/// the one-copy case.
///
/// It is not an equivalence failure: the two paths reach the same state and
/// fail at the same line. It is a defect of the bag both paths share, in
/// `moirai-crdt`, and it is recorded in the vault beside the matrix. When it is
/// fixed this test turns red and should be flipped to assert what the fixed bag
/// reads.
#[test]
fn matrix_finding_a_concurrent_double_remove_of_one_bag_copy_underflows_on_both_paths() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    /// Both paths, one copy of 7 delivered, then a remove from each writer
    /// crossed, with nothing read yet.
    fn crossed() -> Harness {
        let meta = Meta::new();
        let mut harness = Harness::new();
        harness.apply(&open()).unwrap_or_else(|r| panic!("{r}"));
        harness.deliver().unwrap_or_else(|r| panic!("{r}"));
        harness
            .apply(&edit('a', "bag", Action::BagAdd(7)))
            .unwrap_or_else(|r| panic!("{r}"));
        harness.deliver().unwrap_or_else(|r| panic!("{r}"));
        let from_a = edit('a', "bag", Action::BagRemove(7));
        let from_b = edit('b', "bag", Action::BagRemove(7));
        let interp_a = harness.ia.send(interp_op(&meta, &from_a)).expect("a sees one copy");
        let interp_b = harness.ib.send(interp_op(&meta, &from_b)).expect("b sees one copy");
        let gen_a = harness.ga.send(typed_op(&from_a)).expect("a sees one copy");
        let gen_b = harness.gb.send(typed_op(&from_b)).expect("b sees one copy");
        // Delivery of the two removes does not panic on either path.
        harness.ia.receive(interp_b);
        harness.ib.receive(interp_a);
        harness.ga.receive(gen_b);
        harness.gb.receive(gen_a);
        harness
    }

    fn message<T: std::fmt::Debug>(outcome: &std::thread::Result<T>) -> String {
        match outcome {
            Ok(held) => format!("did not panic: {held:?}"),
            Err(payload) => payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|text| text.to_string()))
                .unwrap_or_default(),
        }
    }

    // Reading panics, on both paths, with the same message.
    let harness = crossed();
    let interp = catch_unwind(AssertUnwindSafe(|| harness.interp_doc('a')));
    let generated = catch_unwind(AssertUnwindSafe(|| harness.gen_doc('a')));
    assert_eq!(message(&interp), "attempt to subtract with overflow", "interpreted read");
    assert_eq!(message(&generated), "attempt to subtract with overflow", "generated read");

    // Stabilizing panics inside `receive`, on both paths: `b` acknowledges on
    // `myString`, and `a` receiving it stabilizes both removes.
    let mut harness = crossed();
    let meta = Meta::new();
    let ack = leaf('b', "myString", Elem::InsertChar { pos: 0, ch: 'h' });
    let interp_ack = harness.ib.send(interp_op(&meta, &ack)).expect("b acknowledges");
    let gen_ack = harness.gb.send(typed_op(&ack)).expect("b acknowledges");
    let mut ia = harness.ia;
    let mut ga = harness.ga;
    let interp = catch_unwind(AssertUnwindSafe(move || ia.receive(interp_ack)));
    let generated = catch_unwind(AssertUnwindSafe(move || ga.receive(gen_ack)));
    assert_eq!(message(&interp), "attempt to subtract with overflow", "interpreted stabilization");
    assert_eq!(message(&generated), "attempt to subtract with overflow", "generated stabilization");
}
