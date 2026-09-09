//! The equivalence oracle over `examples/class_diagram.ecore`: `ip30`, the
//! fourth driver of criterion I-A1 and the whole of I-A18.
//!
//! # What is being claimed
//!
//! The same claim the other three drivers make, over the last of the leaf
//! vocabulary none of them reaches. `bt.ecore` is text and containment;
//! `json.ecore` adds a `Counter<f64>`, an `EWFlag`, a `uw-map` and five
//! transparent classes; `kitchen_sink.ecore` adds six counter widths, one
//! multi-value register and two bags. Between the three of them, no register
//! tie-break other than the multi-value one is ever driven, no flag other
//! than the enable-wins one, no set at all, and no enum. `class_diagram.ecore`
//! drives all eight of those on one root class, and this file is what says
//! the interpreted path merges them the way `record!` and the `moirai-crdt`
//! logs do.
//!
//! # The eight, and the feature of `Class` that reaches each
//!
//! | construction | feature | annotation |
//! |---|---|---|
//! | `LwwRegister` | `qualifiedName` | `lww-register` |
//! | `FairRegister` | `author` | `fair-register` |
//! | `PORegister` | `stereotype` | `po-register` |
//! | `TORegister` | `layer` | `to-register` |
//! | `DWFlag` | `isAbstract` | `dw-flag` |
//! | `AWSet` | `tags` | `aw-set`, with `unique="true" ordered="false"` |
//! | `RWSet` | `invariants` | `rw-set`, with `unique="true" ordered="false"` |
//! | `LeafRule::Enum` | `visibility` | none; an `EEnum`-typed attribute |
//!
//! `Class.name` is a ninth feature and a plain text attribute, which
//! `bt.ecore` already proves; it is here because a class diagram whose classes
//! have no names is not one, and because a script that only ever writes the
//! eight new constructions never exercises them beside anything else.
//!
//! # What this metamodel does *not* reach, stated so the claim is not read
//! wider than it is
//!
//! The six `SimpleCounter` arms of `moirai-interp`'s `LeafLog`. No `.ecore`
//! file reaches them and none can: `annotation.rs`'s `parse_datatype_override`
//! has no spelling that produces `Counter::Counter`, and `to_crdt.rs` maps
//! every numeric Ecore builtin to `Counter::default()`, which is
//! `Counter::ResettableCounter`. `ip30_no_ecore_file_can_ask_for_a_simple_counter`
//! below asserts the half of that this crate can see.
//!
//! # Two differences between the paths that are bridged in the projection,
//! and why each is a rendering and not a merge
//!
//! **A unique register cannot say "unwritten" on the generated path.**
//! `Register<V, P>::execute_query` (`unique_register.rs:72-84`) starts from
//! `V::default()` and folds the writes over it, so a `LwwRegister<String>`
//! nobody has written reads `""`, and so does one written back to `""`. The
//! interpreted arm is `Register<Scalar, P>` and `Scalar::default()` is
//! `Scalar::Null`, which reads as JSON `null`. The two describe the same
//! state, nothing has been written, and [`project_feature`] renders the
//! generated `""` as `null` so they can be compared. What the bridge costs is
//! named rather than hidden: a *write* of the empty string is invisible on the
//! generated path, and the script generator never proposes one. The same
//! applies to `TORegister`, whose read also starts at `V::default()`.
//!
//! **An enum literal is spelled twice.** The interpreted path renders
//! `Scalar::Enum(class, literal)` as the literal's name out of the descriptor
//! (`leaf.rs:118-131`); the generated path renders the Rust variant
//! `heck`'s `to_upper_camel_case` made of that name
//! (`classifier/mod.rs:717-753`). The two agree exactly when the `.ecore`
//! file's literals are already upper camel case, which is why
//! `class_diagram.ecore` writes `Public` and not `PUBLIC`, and
//! `ip30_the_two_paths_spell_an_enum_literal_the_same_way_only_because_the_file_does`
//! is where that is written down rather than assumed.
//!
//! # The two arms
//!
//! Interpreted: a pair of `moirai_interp::testing::Harness` replicas rooted
//! at `Class`, the node tree under a real `Replica` with no
//! `ModelOp::Install` in front of it, as `ip28` and `ip29` do.
//!
//! Generated: a `twins_log::<ClassdiagramLog>()` pair, driven by operations
//! built as JSON and deserialized into `Classdiagram`, so a wrong shape is a
//! `serde_json::from_value` error naming the enum and never a silent pass.
//!
//! # Why the root is `Class` and not all three
//!
//! The descriptor names three root classes, `Class`, `Feature` and
//! `Relation`, mirroring `moirai-crdt/src/model/class_diagram.rs`, where a
//! class diagram is a `UWGraphLog` over class nodes and relation edges and
//! neither contains the other. `Class` carries every one of the eight
//! constructions this driver exists for; `Feature` and `Relation` carry
//! rules already driven here or by an earlier oracle. Driving all three would
//! need three interpreted `Harness` pairs against one generated pair, which
//! would give the two arms different event sequence numbers and so different
//! concurrency tie-breaks, and that is a difference in the *harness* and not
//! in the paths.
//!
//! # Every script opens with `New`, and the edits contend
//!
//! `record!`'s `new` builds one field per feature, so `ClassLog` renders its
//! whole shape from construction, while the interpreted slot holds no object
//! until one is minted; `Class::New` on one arm and `InstanceOp::New` on the
//! other is the first edit of every script. A tie-break that is never
//! contended is not being tested at all, so each of the five rounds of a
//! concurrent script picks one feature and has *both* writers write it before
//! anything is delivered. [`ip30_thirty_scripts_over_class_diagram_ecore_agree_everywhere`]
//! prints how many of the thirty scripts contended each of the nine features.
//!
//! # `ip31`: three replicas, for the one rule two cannot reach
//!
//! Everything above drives two replicas, and one merge rule in the whole
//! vocabulary reads the member count. `FairPolicy::compare`
//! (`moirai-crdt/src/policy/mod.rs:63-88`) breaks a Lamport tie by a
//! round-robin over the sorted member list: `round_leader = val % n`, and the
//! write that survives is the one whose seat is `n - 1` places past the
//! leader. With `n` equal to two the leader alternates between the only two
//! seats there are, so the rule reduces to "the two writers take turns" and
//! its round-robin is never exercised as a round-robin. With `n` equal to
//! three the leader walks a three-cycle and a different one of the three
//! writers wins on successive Lamport values, which is what
//! [`ip31_the_fair_registers_round_robin_rotates_with_three_members`] shows
//! and what no earlier oracle could have shown.
//!
//! [`Trio`] is [`Harness`] over
//! `moirai_crdt::utils::membership::triplet_log` and
//! `moirai_interp::testing::triplet`: three replicas per path rather than
//! two, every delivery going to both of the other two, and the six read-outs
//! compared after each one. Its scripts contend three-way — each round has
//! *all three* writers write one feature before anything crosses — and the
//! first round of every concurrent script writes `Class.author`, the
//! metamodel's only fair register.
//!
//! `FairPolicy` is the only rule here whose outcome moves with `n`, and that
//! is walked rather than asserted by
//! [`ip31_the_fair_register_is_the_only_rule_that_reads_the_member_count`].
//! It is also why the other three oracles are left at two replicas:
//! `bt.ecore` and `json.ecore` produce no policy-ordered register at all and
//! `kitchen_sink.ecore` produces only multi-value ones, which keep every
//! concurrent write and so cannot depend on any ordering of the writers.
//!
//! # `ip32`: a joiner that adopts a snapshot
//!
//! Everything above replays operations into a replica that was present from
//! the start. `ip32` is the other way in: a replica bootstrapped alone, after
//! the fact, that takes over a donor's state wholesale through
//! `Replica::adopt` (`moirai-protocol/src/replica.rs:223-236`) and then goes
//! on working. See the note on [`Transfer`] for what each path puts on the
//! wire and for the metamodel shape that cannot go on it at all.

use std::sync::Arc;

use classdiagram_crdt::package::{Classdiagram, ClassdiagramLog, ClassdiagramValue};
use moirai_crdt::utils::membership::{triplet_log, twins_log};
use moirai_interp::testing::{class_slot, feature_slot, triplet, twins};
use moirai_interp::{InstanceOp, LeafOp, Scalar};
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::Tcsb;
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::{IsReplica, Replica};
use moirai_semantics::{
    ClassSlot, FeatureSlot, FlagWins, LeafRule, MergeRule, MetamodelSemantics, SetTie, Shape,
    TieBreak, from_descriptor,
};
use serde_json::{Map, Value, json};

type InterpReplica = Replica<moirai_interp::testing::Harness, Tcsb<InstanceOp>>;
type GenReplica = Replica<ClassdiagramLog, Tcsb<Classdiagram>>;

/// The key a conflict set is carried under, `02 Validation Plan` §2.
const CONFLICT: &str = "__conflict";
/// The key a class name is carried under.
const ECLASS: &str = "eClass";
/// The one root class this oracle drives.
const ROOT: &str = "Class";

// ---------------------------------------------------------------------------
// 1. The nine features of `Class`, and what each of them takes
// ---------------------------------------------------------------------------

/// The construction one feature of `Class` is, as far as an edit is concerned.
///
/// Written here and checked against the table in
/// [`the_table_says_what_this_file_says`], which is the point: this is the
/// second implementation, and a disagreement between the two is a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// `EventGraph<List<char>>`.
    Text,
    /// `VecLog<LwwRegister<String>>` or `VecLog<FairRegister<String>>`: one
    /// value, and no `Clear` operation at all.
    RegisterUnique,
    /// `VecLog<TORegister<String>>`: one value, the greatest ever written.
    RegisterTotal,
    /// `VecLog<PORegister<String>>`: a set of pairwise incomparable values.
    RegisterPartial,
    /// `VecLog<MVRegister<Visibility>>` over an enum literal.
    Enum,
    /// `VecLog<DWFlag>`.
    FlagDisable,
    /// `VecLog<AWSet<String>>`.
    SetAdd,
    /// `VecLog<RWSet<String>>`.
    SetRemove,
}

impl Kind {
    /// Whether the arm takes `LeafOp::Clear`. `LwwRegister` and
    /// `FairRegister` are `Register<V, P>`, whose only operation is `Write`
    /// (`unique_register.rs:26-29`), so a `Clear` is not a refusal on either
    /// path but an operation neither type can even spell.
    fn clearable(self) -> bool {
        !matches!(self, Kind::RegisterUnique | Kind::Text)
    }
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

/// `Class`'s features in declaration order, which is also visible-slot order:
/// `Class` has no supertype, so the two coincide.
const FEATURES: [Feature; 9] = [
    feature("name", Kind::Text),
    feature("qualifiedName", Kind::RegisterUnique),
    feature("author", Kind::RegisterUnique),
    feature("stereotype", Kind::RegisterPartial),
    feature("layer", Kind::RegisterTotal),
    feature("isAbstract", Kind::FlagDisable),
    feature("visibility", Kind::Enum),
    feature("tags", Kind::SetAdd),
    feature("invariants", Kind::SetRemove),
];

/// The Rust field name `record!` gives a feature: `paste!`'s `:camel` runs
/// over the snake-cased name the generator writes.
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
fn class_diagram_descriptor() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/metamodel.json"))
            .expect("this crate's own descriptor is readable"),
    )
    .expect("it is JSON")
}

struct Meta {
    sem: Arc<MetamodelSemantics>,
    root: ClassSlot,
    /// The slot of `Visibility` in the table's `enums`, and its literals in
    /// declaration order: what an enum write has to name.
    visibility: ClassSlot,
    literals: Vec<String>,
}

impl Meta {
    fn new() -> Meta {
        let sem = Arc::new(
            from_descriptor(&class_diagram_descriptor()).expect("the checked-in descriptor parses"),
        );
        let root = class_slot(&sem, ROOT);
        let slot = feature_slot(&sem, root, "visibility");
        let (visibility, literals) = match sem.rule(root, slot) {
            Some(MergeRule::Attribute {
                leaf: LeafRule::Enum { class, .. },
                ..
            }) => (
                *class,
                sem.enums[class.index()]
                    .literals
                    .iter()
                    .map(|literal| literal.to_string())
                    .collect(),
            ),
            other => panic!("`Class.visibility` is {other:?} and not an enum leaf"),
        };
        Meta {
            sem,
            root,
            visibility,
            literals,
        }
    }

    fn slot(&self, feature: &str) -> FeatureSlot {
        feature_slot(&self.sem, self.root, feature)
    }

    fn rule(&self, feature: &str) -> MergeRule {
        *self
            .sem
            .rule(self.root, self.slot(feature))
            .expect("every feature of `Class` carries a rule")
    }
}

// ---------------------------------------------------------------------------
// 3. The edit script
// ---------------------------------------------------------------------------

/// What one edit does to the leaf it addresses.
#[derive(Clone, Debug, PartialEq)]
enum Elem {
    InsertChar { pos: usize, ch: char },
    DeleteChar { pos: usize },
    /// A register write of a word.
    Write(&'static str),
    /// A register write of an enum literal, by its position in the enum's
    /// declaration order.
    WriteLiteral(usize),
    Enable,
    Disable,
    Add(&'static str),
    Remove(&'static str),
    Clear,
}

#[derive(Clone, Debug, PartialEq)]
enum Action {
    /// Mint the root object; the first edit of every script.
    New,
    /// A write to a leaf.
    Leaf(Elem),
}

/// One edit: which replica issues it, which feature of `Class` it addresses,
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
            (None, Action::New) => format!("{}: New Class", self.writer),
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
    /// The features both writers wrote inside one undelivered round: what the
    /// contention census counts.
    contended: Vec<&'static str>,
}

// ---------------------------------------------------------------------------
// 4. The two encoders
// ---------------------------------------------------------------------------

/// One element write as the interpreted path spells it.
fn interp_elem(meta: &Meta, elem: &Elem) -> LeafOp {
    match elem {
        Elem::InsertChar { pos, ch } => LeafOp::InsertChar { pos: *pos, ch: *ch },
        Elem::DeleteChar { pos } => LeafOp::DeleteChar { pos: *pos },
        Elem::Write(word) => LeafOp::Write(Scalar::text(*word)),
        Elem::WriteLiteral(index) => {
            LeafOp::Write(Scalar::Enum(meta.visibility.0, *index as u16))
        }
        Elem::Enable => LeafOp::Enable,
        Elem::Disable => LeafOp::Disable,
        Elem::Add(word) => LeafOp::Add(Scalar::text(*word)),
        Elem::Remove(word) => LeafOp::Remove(Scalar::text(*word)),
        Elem::Clear => LeafOp::Clear,
    }
}

/// One edit as an [`InstanceOp`] against the root `Class`.
fn interp_op(meta: &Meta, edit: &Edit) -> InstanceOp {
    let inner = match (&edit.feature, &edit.action) {
        (None, Action::New) => return InstanceOp::variant(meta.root, InstanceOp::New),
        (Some(name), Action::Leaf(elem)) => {
            InstanceOp::field(meta.slot(name), InstanceOp::Leaf(interp_elem(meta, elem)))
        }
        (feature, action) => panic!("{action:?} against {feature:?} is not an edit of this file"),
    };
    InstanceOp::variant(meta.root, inner)
}

/// One element write as the generated path spells it: the op of whichever
/// `moirai-crdt` log the field holds.
fn typed_elem(meta: &Meta, elem: &Elem) -> Value {
    match elem {
        Elem::InsertChar { pos, ch } => json!({"Insert": {"content": ch, "pos": pos}}),
        Elem::DeleteChar { pos } => json!({"Delete": {"pos": pos}}),
        Elem::Write(word) => tagged("Write", json!(word)),
        Elem::WriteLiteral(index) => tagged("Write", json!(meta.literals[*index])),
        Elem::Enable => json!("Enable"),
        Elem::Disable => json!("Disable"),
        Elem::Add(word) => tagged("Add", json!(word)),
        Elem::Remove(word) => tagged("Remove", json!(word)),
        Elem::Clear => json!("Clear"),
    }
}

/// One edit as this crate's typed operation, built as JSON by the naming
/// convention `record!` uses and then deserialized, so a wrong shape is an
/// error naming the enum and never a silent pass.
fn typed_op(meta: &Meta, edit: &Edit) -> Classdiagram {
    let value = typed_json(meta, edit);
    serde_json::from_value(value.clone()).unwrap_or_else(|error| {
        panic!(
            "the typed encoder built an operation `Classdiagram` cannot take: {error}\n{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        )
    })
}

fn typed_json(meta: &Meta, edit: &Edit) -> Value {
    let inner = match (&edit.feature, &edit.action) {
        (None, Action::New) => json!("New"),
        (Some(name), Action::Leaf(elem)) => tagged(variant_of(name), typed_elem(meta, elem)),
        (feature, action) => panic!("{action:?} against {feature:?} is not an edit of this file"),
    };
    tagged(ROOT, inner)
}

// ---------------------------------------------------------------------------
// 5. The projection of the generated read-out onto the canonical form
// ---------------------------------------------------------------------------

/// The generated `Read` value in the canonical form of `02 Validation Plan`
/// §2: an `eClass` key, `Vec<char>` joined, a many-valued register collapsed
/// the way `eval::read` collapses it, a set sorted, and a unique register at
/// its value type's default rendered as `null` for the reason the module note
/// gives.
///
/// Nothing here prunes: every one of the nine keys is present on both sides
/// after `New`, which is why `New` is the first edit of every script.
fn project(meta: &Meta, value: &ClassdiagramValue) -> Value {
    let raw = serde_json::to_value(value).expect("the generated read-out serializes");
    let class = raw
        .get("class")
        .expect("the package value carries `Class` under its field");
    let mut out = Map::new();
    out.insert(ECLASS.to_string(), Value::String(ROOT.to_string()));
    for Feature { name, kind } in FEATURES {
        let found = class
            .get(field_of(name))
            .unwrap_or_else(|| panic!("`ClassValue` has no field for `{name}`"));
        out.insert(name.to_string(), project_feature(meta, kind, found));
    }
    Value::Object(out)
}

fn project_feature(meta: &Meta, kind: Kind, value: &Value) -> Value {
    match kind {
        Kind::Text => chars(value),
        Kind::FlagDisable => value.clone(),
        // A unique or total-order register reads its value type's default
        // when nothing has been written, and `String::default()` is `""`.
        // The interpreted arm reads `Scalar::Null` there. See the module note.
        Kind::RegisterUnique | Kind::RegisterTotal => match value.as_str() {
            Some("") => Value::Null,
            Some(text) => Value::String(text.to_string()),
            None => panic!("a unique register over `EString` reads as a string: {value}"),
        },
        Kind::RegisterPartial => many_valued(value, None),
        // A conflict set of enum literals is ordered by `Scalar`'s own `Ord`
        // on the interpreted path, which compares the literal's *position*
        // in the enum's declaration order and not its name. The generated
        // arm reads a `HashSet<Visibility>`, which carries no order at all,
        // so the projection has to impose one and imposes the metamodel's.
        // Sorting the names instead put `Protected` before `Public`, which
        // is the one thing this projection got wrong on its first run.
        Kind::Enum => many_valued(value, Some(&meta.literals)),
        Kind::SetAdd | Kind::SetRemove => sorted(value),
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

/// A `HashSet<V>` as `leaf.rs`'s `many_valued` renders one: nothing at all is
/// `null`, one value is that value, and more than one is a conflict set
/// sorted the way `Scalar`'s own `Ord` sorts strings.
fn many_valued(value: &Value, order: Option<&[String]>) -> Value {
    let mut held = strings(value);
    match order {
        Some(literals) => held.sort_by_key(|value| {
            literals
                .iter()
                .position(|literal| literal == value)
                .unwrap_or_else(|| panic!("`{value}` is not a literal of this enum"))
        }),
        None => held.sort(),
    }
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

/// A `HashSet<String>` as `leaf.rs`'s `sorted_array` renders one.
fn sorted(value: &Value) -> Value {
    let mut held = strings(value);
    held.sort();
    Value::Array(held.into_iter().map(Value::String).collect())
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("a set reads as an array: {value}"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("a set of strings holds strings: {item}"))
                .to_string()
        })
        .collect()
}

/// A canonical document with every default-valued key dropped, and a root
/// left with nothing but its class name spelled `null`.
///
/// Called on **both** sides of the comparison by the same function, and it is
/// the only place either side is pruned, so the two sides cannot drift.
///
/// `record!`'s `new` constructs one field per feature, so a `ClassLog` renders
/// its whole shape from the moment it is built, while the interpreted
/// `ObjectNode` holds no object until an operation mints one and `eval::read`
/// spells a slot that holds none as `null`. Both describe the same state and
/// §2's canonical form had no way to say so. This is `bt_crdt`'s
/// `without_defaults` rule, reduced to the leaves this metamodel has.
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
        Kind::RegisterUnique
        | Kind::RegisterTotal
        | Kind::RegisterPartial
        | Kind::Enum => Value::Null,
        Kind::FlagDisable => json!(false),
        Kind::SetAdd | Kind::SetRemove => json!([]),
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
    pending_a: Vec<(EventMessage<InstanceOp>, EventMessage<Classdiagram>)>,
    pending_b: Vec<(EventMessage<InstanceOp>, EventMessage<Classdiagram>)>,
    ops: usize,
    refused: usize,
    comparisons: usize,
}

impl Harness {
    fn new() -> Harness {
        let meta = Meta::new();
        let (ia, ib) = twins(&meta.sem, ROOT);
        let (ga, gb) = twins_log::<ClassdiagramLog>();
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
        without_defaults(project(
            &self.meta,
            &replica.query(Read::<ClassdiagramValue>::new()),
        ))
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
        let generated = typed_op(&self.meta, edit);
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
/// The words a register is written with and a set is filled from. Small and
/// few, so that two writers really do write the same word concurrently and
/// really do add and remove the same element; none of them is `""`, which is
/// the value a generated unique register cannot tell from unwritten.
const WORDS: [&str; 4] = ["alpha", "beta", "delta", "gamma"];

/// One element write for a leaf of this kind, proposed against what the
/// writer can see of it.
fn propose_elem(meta: &Meta, kind: Kind, seen: &Value, rng: &mut Rng) -> Elem {
    if kind.clearable() && rng.below(9) == 0 {
        return Elem::Clear;
    }
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
        Kind::RegisterUnique | Kind::RegisterTotal | Kind::RegisterPartial => {
            Elem::Write(WORDS[rng.below(WORDS.len())])
        }
        Kind::Enum => Elem::WriteLiteral(rng.below(meta.literals.len())),
        Kind::FlagDisable => match rng.below(3) {
            0 => Elem::Disable,
            _ => Elem::Enable,
        },
        Kind::SetAdd | Kind::SetRemove => {
            let held: Vec<String> = seen
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            // A removal is proposed against what the writer can see about
            // half the time, and against the whole vocabulary the rest, so
            // that removing what is not there is exercised too: a set's
            // `Remove` is not a counter decrement and does not underflow.
            if !held.is_empty() && rng.below(3) == 0 {
                let word = &held[rng.below(held.len())];
                Elem::Remove(
                    WORDS
                        .iter()
                        .copied()
                        .find(|candidate| candidate == word)
                        .expect("a set only ever holds a word from the vocabulary"),
                )
            } else if rng.below(4) == 0 {
                Elem::Remove(WORDS[rng.below(WORDS.len())])
            } else {
                Elem::Add(WORDS[rng.below(WORDS.len())])
            }
        }
    }
}

/// One edit a writer looking at `seen` could make against a named feature.
fn propose_on(meta: &Meta, at: Feature, seen: &Value, rng: &mut Rng, writer: char) -> Edit {
    // A pruned read-out carries no key for a feature at its default, and a
    // model that has just been minted is pruned to `null` entirely; either
    // way the writer is looking at the default.
    let here = seen
        .get(at.name)
        .cloned()
        .unwrap_or_else(|| default_of(at.kind));
    Edit {
        writer,
        feature: Some(at.name),
        action: Action::Leaf(propose_elem(meta, at.kind, &here, rng)),
    }
}

/// One edit against a feature the generator picks.
fn propose(meta: &Meta, seen: &Value, rng: &mut Rng, writer: char) -> Edit {
    propose_on(meta, FEATURES[rng.below(FEATURES.len())], seen, rng, writer)
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
/// carried and never compared: a difference is the finding `ip30` exists to
/// report, and a generator that halted on the first one would only ever
/// report the first one.
///
/// **Every concurrent round contends.** A tie-break that is never written
/// concurrently is not being tested, so each round picks one feature and has
/// both writers write it before anything is delivered, and the feature it
/// picked is recorded in `contended`.
fn seeded_script(seed: u64, concurrent: bool) -> EditScript {
    let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x0BAD_C0DE);
    let mut steps: Vec<Move> = vec![Move::Edit(open()), Move::Deliver];
    let mut contended: Vec<&'static str> = Vec::new();
    let mut shadow = Harness::new();
    let meta = Meta::new();
    shadow.carry(&open()).expect("a fresh root takes `New`");
    shadow.cross();

    let label = format!(
        "{} seed {seed}",
        if concurrent { "concurrent" } else { "sequential" }
    );

    if concurrent {
        for _ in 0..5 {
            let focus = FEATURES[rng.below(FEATURES.len())];
            let mut both_landed = true;
            // The contended pair first: both writers write the focus feature
            // against the state they share, before anything crosses.
            for writer in ['a', 'b'] {
                let seen = shadow.interp_doc(writer);
                let edit = propose_on(&meta, focus, &seen, &mut rng, writer);
                match shadow.carry(&edit) {
                    Ok(landed) => both_landed &= landed,
                    Err(_) => {
                        return EditScript {
                            label,
                            steps,
                            contended,
                        };
                    }
                }
                steps.push(Move::Edit(edit));
            }
            if both_landed {
                contended.push(focus.name);
            }
            // Then one to three free edits from each writer, still undelivered.
            for _ in 0..1 + rng.below(3) {
                for writer in ['a', 'b'] {
                    let seen = shadow.interp_doc(writer);
                    let edit = propose(&meta, &seen, &mut rng, writer);
                    if shadow.carry(&edit).is_err() {
                        return EditScript {
                            label,
                            steps,
                            contended,
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
            let edit = propose(&meta, &seen, &mut rng, writer);
            if shadow.carry(&edit).is_err() {
                break;
            }
            steps.push(Move::Edit(edit));
            steps.push(Move::Deliver);
            shadow.cross();
        }
    }
    EditScript {
        label,
        steps,
        contended,
    }
}

fn scripts() -> Vec<EditScript> {
    let mut out: Vec<EditScript> = (0..10).map(|seed| seeded_script(seed, false)).collect();
    out.extend((0..20).map(|seed| seeded_script(seed, true)));
    out
}

// ---------------------------------------------------------------------------
// 8. The tests
// ---------------------------------------------------------------------------

/// The nine features this file names, and the construction it names for each,
/// are the ones the table derived from the same descriptor carries. Without
/// this, every encoder above is writing against a metamodel of its own
/// invention.
#[test]
fn the_table_says_what_this_file_says() {
    let meta = Meta::new();
    let class = &meta.sem.classes[meta.root.index()];
    assert_eq!(
        class.visible.len(),
        FEATURES.len(),
        "`Class` carries {} features and this file names {}",
        class.visible.len(),
        FEATURES.len()
    );
    let mut named: Vec<&str> = FEATURES.iter().map(|f| f.name).collect();
    let mut visible: Vec<&str> = class.visible.iter().map(|(name, _, _)| &**name).collect();
    named.sort_unstable();
    visible.sort_unstable();
    assert_eq!(named, visible, "`Class`'s features, as the table holds them");
    for Feature { name, kind } in FEATURES.iter() {
        let derived = match meta.rule(name) {
            MergeRule::Attribute { shape, leaf } => match (shape, leaf) {
                (Shape::Single, LeafRule::Text) => Kind::Text,
                (
                    Shape::Single,
                    LeafRule::Register {
                        tie: TieBreak::LastWriterWins | TieBreak::Fair,
                    },
                ) => Kind::RegisterUnique,
                (
                    Shape::Single,
                    LeafRule::Register {
                        tie: TieBreak::TotalOrder,
                    },
                ) => Kind::RegisterTotal,
                (
                    Shape::Single,
                    LeafRule::Register {
                        tie: TieBreak::PartialOrder,
                    },
                ) => Kind::RegisterPartial,
                (Shape::Single, LeafRule::Enum { .. }) => Kind::Enum,
                (
                    Shape::Single,
                    LeafRule::Flag {
                        wins: FlagWins::Disable,
                    },
                ) => Kind::FlagDisable,
                (
                    Shape::Set {
                        tie: SetTie::AddWins,
                    },
                    _,
                ) => Kind::SetAdd,
                (
                    Shape::Set {
                        tie: SetTie::RemoveWins,
                    },
                    _,
                ) => Kind::SetRemove,
                other => panic!("`{name}` is {other:?}, which this file does not name"),
            },
            other => panic!("`{name}` is {other:?}, which is not an attribute"),
        };
        assert_eq!(derived, *kind, "`{name}`");
    }
    assert_eq!(&*meta.sem.package, "classdiagram");
    assert_eq!(
        meta.sem.roots.len(),
        3,
        "`Class`, `Feature` and `Relation` are roots"
    );
}

/// The census this driver was written to establish, asserted rather than
/// described: which of `moirai-interp`'s leaf constructions
/// `class_diagram.ecore` reaches, and which of the ones the earlier three
/// oracles left open it closes. A metamodel edited to reach one more makes
/// this fail and the doc comment at the top of this file wrong at the same
/// time, which is the point.
#[test]
fn ip30_the_census_of_what_this_metamodel_reaches() {
    let meta = Meta::new();
    let mut ties: Vec<String> = Vec::new();
    let mut flags: Vec<String> = Vec::new();
    let mut shapes: Vec<String> = Vec::new();
    let mut enums: Vec<String> = Vec::new();
    let mut counters = 0usize;
    for class in &meta.sem.classes {
        for (index, _) in class.visible.iter().enumerate() {
            let Some(rule) = meta.sem.rule(class.slot, FeatureSlot(index as u16)) else {
                continue;
            };
            if let MergeRule::Attribute { shape, leaf } = rule {
                shapes.push(format!("{shape:?}"));
                match leaf {
                    LeafRule::Counter { .. } => counters += 1,
                    LeafRule::Register { tie } => ties.push(format!("{tie:?}")),
                    LeafRule::Enum { class, tie } => {
                        enums.push(meta.sem.enums[class.index()].name.to_string());
                        ties.push(format!("{tie:?}"));
                    }
                    LeafRule::Flag { wins } => flags.push(format!("{wins:?}")),
                    LeafRule::Text => {}
                }
            }
        }
    }
    for list in [&mut ties, &mut flags, &mut shapes, &mut enums] {
        list.sort();
        list.dedup();
    }
    assert_eq!(
        ties,
        vec!["Fair", "LastWriterWins", "MultiValue", "PartialOrder", "TotalOrder"],
        "all five register tie-breaks, which is what this metamodel is for"
    );
    assert_eq!(flags, vec!["Disable"], "the disable-wins flag, and no other");
    assert_eq!(
        shapes,
        vec!["Set { tie: AddWins }", "Set { tie: RemoveWins }", "Single"],
        "the first two `Shape::Set` features in the checked-in corpus"
    );
    assert_eq!(
        enums,
        vec!["PrimitiveType", "Visibility"],
        "two enum-typed attributes carry `LeafRule::Enum`"
    );
    assert_eq!(counters, 0, "no counter here; `kitchen_sink.ecore` has those");
    eprintln!("ip30 census: register ties {ties:?}, flags {flags:?}, shapes {shapes:?}, enums {enums:?}");
}

/// The claim the module note makes about the six `SimpleCounter` arms, as far
/// as this crate can see it: nothing in the descriptor of any checked-in
/// metamodel carries a non-resettable counter, because no annotation spelling
/// produces one. The other half of the claim lives in
/// `arachne-codegen/src/codegen/annotation.rs`, whose
/// `parse_datatype_override` has no arm returning `Counter::Counter`, and in
/// `arachne-codegen/src/codegen/datatype/to_crdt.rs`, which maps every numeric
/// Ecore builtin to `Counter::default()`.
#[test]
fn ip30_no_ecore_file_can_ask_for_a_simple_counter() {
    let meta = Meta::new();
    for class in &meta.sem.classes {
        for (index, _) in class.visible.iter().enumerate() {
            if let Some(MergeRule::Attribute {
                leaf: LeafRule::Counter { resettable, num },
                ..
            }) = meta.sem.rule(class.slot, FeatureSlot(index as u16))
            {
                assert!(
                    *resettable,
                    "a `SimpleCounter<{num:?}>` reached a descriptor, which `annotation.rs` \
                     has no spelling for"
                );
            }
        }
    }
}

/// A helper: one write to one leaf.
fn edit(writer: char, feature: &'static str, elem: Elem) -> Edit {
    Edit {
        writer,
        feature: Some(feature),
        action: Action::Leaf(elem),
    }
}

/// One write of every construction the metamodel carries, sequential and
/// delivered, read out identically on both paths.
#[test]
fn ip30_one_write_of_every_construction_reads_the_same_on_both_paths() {
    let mut harness = Harness::new();
    let edits = vec![
        open(),
        edit('a', "name", Elem::InsertChar { pos: 0, ch: 'x' }),
        edit('a', "qualifiedName", Elem::Write("alpha")),
        edit('b', "author", Elem::Write("beta")),
        edit('a', "stereotype", Elem::Write("delta")),
        edit('b', "layer", Elem::Write("gamma")),
        edit('a', "isAbstract", Elem::Enable),
        edit('b', "visibility", Elem::WriteLiteral(1)),
        edit('a', "tags", Elem::Add("alpha")),
        edit('b', "tags", Elem::Add("beta")),
        edit('a', "invariants", Elem::Add("gamma")),
    ];
    for one in &edits {
        harness
            .apply(one)
            .unwrap_or_else(|reason| panic!("{reason}"));
        harness
            .deliver()
            .unwrap_or_else(|reason| panic!("{reason}"));
    }
    let read = harness.interp_doc('a');
    assert_eq!(read, harness.gen_doc('a'));
    assert_eq!(read, harness.interp_doc('b'));
    assert_eq!(read, harness.gen_doc('b'));
    assert_eq!(read["name"], json!("x"));
    assert_eq!(read["qualifiedName"], json!("alpha"));
    assert_eq!(read["author"], json!("beta"));
    assert_eq!(read["stereotype"], json!("delta"));
    assert_eq!(read["layer"], json!("gamma"));
    assert_eq!(read["isAbstract"], json!(true));
    assert_eq!(read["visibility"], json!("Private"));
    assert_eq!(read["tags"], json!(["alpha", "beta"]));
    assert_eq!(read["invariants"], json!(["gamma"]));
}

/// Two concurrent writes to each of the five register tie-breaks settle the
/// same way on both paths, and the three whose outcome is decided by the
/// value rather than by the event are asserted by value.
///
/// `LwwRegister` and `FairRegister` settle by comparing the two events' tags,
/// which both arms carry unchanged through `Event::unfold`, so what is
/// asserted for those two is that the four read-outs agree and that exactly
/// one of the two words survives; asserting *which* would be asserting
/// `LwwPolicy`, which is `moirai-crdt`'s business and not this file's.
#[test]
fn ip30_every_contended_register_settles_the_same_way_on_both_paths() {
    let mut harness = Harness::new();
    harness.apply(&open()).unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    for feature in ["qualifiedName", "author", "layer", "stereotype"] {
        harness
            .carry(&edit('a', feature, Elem::Write("alpha")))
            .unwrap_or_else(|r| panic!("{r}"));
        harness
            .carry(&edit('b', feature, Elem::Write("gamma")))
            .unwrap_or_else(|r| panic!("{r}"));
    }
    // The enum, written concurrently at two different literals.
    harness
        .carry(&edit('a', "visibility", Elem::WriteLiteral(0)))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .carry(&edit('b', "visibility", Elem::WriteLiteral(2)))
        .unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    let read = harness.interp_doc('a');
    assert_eq!(read, harness.gen_doc('a'));
    assert_eq!(read, harness.interp_doc('b'));
    assert_eq!(read, harness.gen_doc('b'));

    // Last-writer-wins and fair: one of the two words, the same one on all
    // four read-outs.
    for feature in ["qualifiedName", "author"] {
        let held = read[feature].as_str().unwrap_or_else(|| {
            panic!("a unique register holds one value: {}", read[feature])
        });
        assert!(
            held == "alpha" || held == "gamma",
            "`{feature}` holds `{held}`, which neither writer wrote"
        );
    }
    // Total order: the greater of the two words, and `gamma` > `alpha`.
    assert_eq!(read["layer"], json!("gamma"), "a total-order register keeps the maximum");
    // Partial order over `String`, which is totally ordered, so the greater
    // one survives here too and the conflict set holds one value.
    assert_eq!(read["stereotype"], json!("gamma"));
    // Multi-value over an enum literal: both survive, sorted by name.
    assert_eq!(
        read["visibility"],
        json!({CONFLICT: ["Public", "Protected"]}),
        "two concurrent literals both survive, in the enum's declaration order"
    );

    // And a write causally below the conflict replaces it, on both paths.
    harness
        .apply(&edit('a', "visibility", Elem::WriteLiteral(3)))
        .unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));
    assert_eq!(harness.interp_doc('a')["visibility"], json!("Package"));
    assert_eq!(harness.gen_doc('b')["visibility"], json!("Package"));
}

/// A concurrent add and remove of the same element settles add-wins on `tags`
/// and remove-wins on `invariants`, identically on both paths; and a
/// concurrent enable and disable of the disable-wins flag settles false.
#[test]
fn ip30_contended_sets_and_the_disable_wins_flag_settle_the_same_way_on_both_paths() {
    let mut harness = Harness::new();
    harness.apply(&open()).unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    // Seed both sets with the element, delivered, so the removal below has
    // something causally beneath it to take out.
    harness
        .apply(&edit('a', "tags", Elem::Add("alpha")))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .apply(&edit('a', "invariants", Elem::Add("alpha")))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .apply(&edit('a', "isAbstract", Elem::Enable))
        .unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    // Now the contention: a removes, b adds the same element again, and a
    // disables the flag while b enables it.
    harness
        .carry(&edit('a', "tags", Elem::Remove("alpha")))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .carry(&edit('b', "tags", Elem::Add("alpha")))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .carry(&edit('a', "invariants", Elem::Remove("alpha")))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .carry(&edit('b', "invariants", Elem::Add("alpha")))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .carry(&edit('a', "isAbstract", Elem::Disable))
        .unwrap_or_else(|r| panic!("{r}"));
    harness
        .carry(&edit('b', "isAbstract", Elem::Enable))
        .unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    let read = harness.interp_doc('a');
    assert_eq!(read, harness.gen_doc('a'));
    assert_eq!(read, harness.interp_doc('b'));
    assert_eq!(read, harness.gen_doc('b'));
    assert_eq!(
        read["tags"],
        json!(["alpha"]),
        "an add concurrent with a remove wins in an add-wins set"
    );
    assert_eq!(
        read.get("invariants"),
        None,
        "a remove concurrent with an add wins in a remove-wins set, so the \
         set is empty and the key is pruned"
    );
    assert_eq!(
        read.get("isAbstract"),
        None,
        "a disable concurrent with an enable wins in a disable-wins flag, so \
         the flag reads false and the key is pruned"
    );
}

/// The two paths spell an enum literal the same way only because
/// `class_diagram.ecore` writes its literals in upper camel case.
///
/// The interpreted path renders `Scalar::Enum` as the literal's name from the
/// descriptor; the generated path renders the Rust variant, which is
/// `heck`'s `to_upper_camel_case` of that name. `Public` survives that
/// transformation unchanged and `PUBLIC` would not, which is a property of
/// this file and not of either path, and is written down here so that a
/// metamodel whose literals shout is known in advance to need a projection
/// step this oracle does not have.
#[test]
fn ip30_the_two_paths_spell_an_enum_literal_the_same_way_only_because_the_file_does() {
    let meta = Meta::new();
    for literal in &meta.literals {
        let camel: String = {
            let mut out = String::new();
            let mut upper = true;
            for ch in literal.chars() {
                if ch == '_' || ch == '-' {
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
        };
        assert_eq!(
            &camel, literal,
            "`{literal}` is not already the Rust variant the generator writes"
        );
    }
    assert_eq!(meta.literals, ["Public", "Private", "Protected", "Package"]);
}

/// **ip30** — thirty seeded scripts, ten sequential and twenty with a
/// concurrent half in which every round is contended, compared after every
/// operation and after every crossing, with no exclusion on either side's
/// read-out and nothing held back from the generator.
#[test]
fn ip30_thirty_scripts_over_class_diagram_ecore_agree_everywhere() {
    let scripts = scripts();
    let mut edits = 0usize;
    let mut comparisons = 0usize;
    let mut refused = 0usize;
    let mut failures = Vec::new();
    // For each feature, how many of the thirty scripts contended it at least
    // once: the number the brief asks for, printed rather than described.
    let mut contention: Vec<(&str, usize)> = FEATURES.iter().map(|f| (f.name, 0)).collect();
    for script in &scripts {
        edits += script
            .steps
            .iter()
            .filter(|step| matches!(step, Move::Edit(_)))
            .count();
        for (name, count) in contention.iter_mut() {
            if script.contended.contains(name) {
                *count += 1;
            }
        }
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
        "ip30: {} scripts, {edits} edits, {comparisons} comparisons, {refused} refused by both intakes",
        scripts.len()
    );
    eprintln!("ip30 contended scripts per feature: {contention:?}");
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
    for (name, count) in &contention {
        assert!(
            *count > 0,
            "`{name}` was never written concurrently by both replicas, so its \
             merge rule is not being tested"
        );
    }
}

/// The oracle fails when the two paths genuinely differ, which is criterion
/// I-A2 for this driver: the generated arm is handed one set element the
/// interpreted arm never sees, and the comparison catches it and names the
/// feature.
#[test]
fn ip30_the_oracle_notices_when_the_two_encoders_disagree() {
    let mut harness = Harness::new();
    harness.apply(&open()).unwrap_or_else(|r| panic!("{r}"));
    harness.deliver().unwrap_or_else(|r| panic!("{r}"));

    // The mutation: one `tags` element added on the generated arm alone.
    let stray: Classdiagram = serde_json::from_value(tagged(
        ROOT,
        tagged(variant_of("tags"), tagged("Add", json!("alpha"))),
    ))
    .expect("the shape is right; the asymmetry is the lie");
    let event = harness.ga.send(stray).expect("the generated log takes it");
    harness.gb.receive(event);

    let reason = harness
        .compare()
        .expect_err("the two read-outs now differ and the oracle has to say so");
    assert!(reason.contains("tags"), "{reason}");
    assert!(reason.contains("interpreted"), "{reason}");
}

// ---------------------------------------------------------------------------
// 9. Three replicas, for the one rule two cannot reach
// ---------------------------------------------------------------------------

/// The three writers, in the order `triplet_log` and `triplet` mint them,
/// which is also the order `FairPolicy::compare` sorts them into.
const WRITERS: [char; 3] = ['a', 'b', 'c'];

/// The position of `Class.author`, the metamodel's only fair register, in
/// [`FEATURES`]. Checked against the table in
/// [`ip31_the_fair_register_is_the_only_rule_that_reads_the_member_count`].
const FAIR: usize = 2;

/// Which of the three replicas a writer is.
fn seat(writer: char) -> usize {
    WRITERS
        .iter()
        .position(|held| *held == writer)
        .unwrap_or_else(|| panic!("`{writer}` is not one of the three writers"))
}

/// Two `triplet` sets, one per path, driven by one script.
///
/// The same harness as [`Harness`] with the pair replaced by a triple: an
/// edit is encoded twice and handed to its writer's two replicas, a delivery
/// goes to *both* of the other two, and the six read-outs are compared after
/// every operation and after every delivery.
struct Trio {
    meta: Meta,
    interp: Vec<InterpReplica>,
    generated: Vec<GenReplica>,
    /// What each replica has sent and not yet had crossed, by seat.
    pending: Vec<Vec<(EventMessage<InstanceOp>, EventMessage<Classdiagram>)>>,
    ops: usize,
    refused: usize,
    comparisons: usize,
}

impl Trio {
    fn new() -> Trio {
        let meta = Meta::new();
        let (ia, ib, ic) = triplet(&meta.sem, ROOT);
        let (ga, gb, gc) = triplet_log::<ClassdiagramLog>();
        Trio {
            meta,
            interp: vec![ia, ib, ic],
            generated: vec![ga, gb, gc],
            pending: vec![Vec::new(), Vec::new(), Vec::new()],
            ops: 0,
            refused: 0,
            comparisons: 0,
        }
    }

    fn interp_doc(&self, writer: char) -> Value {
        without_defaults(self.interp[seat(writer)].query(Read::<Value>::new()))
    }

    fn gen_doc(&self, writer: char) -> Value {
        without_defaults(project(
            &self.meta,
            &self.generated[seat(writer)].query(Read::<ClassdiagramValue>::new()),
        ))
    }

    /// The six read-outs, compared path against path at each of the three
    /// seats, each pruned by [`without_defaults`].
    fn compare(&mut self) -> Result<(), String> {
        for writer in WRITERS {
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
        let generated = typed_op(&self.meta, edit);
        self.ops += 1;
        let at = seat(edit.writer);
        let interp_event = self.interp[at].send(interp);
        let gen_event = self.generated[at].send(generated);
        match (interp_event, gen_event) {
            (Some(interp_event), Some(gen_event)) => {
                self.pending[at].push((interp_event, gen_event));
                Ok(true)
            }
            (None, None) => {
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

    /// Everything everyone is holding, to both of the other two, in seat
    /// order and with no comparison in between: what the script generator
    /// runs its shadow with.
    fn cross(&mut self) {
        for from in 0..WRITERS.len() {
            for (interp_event, gen_event) in std::mem::take(&mut self.pending[from]) {
                for to in 0..WRITERS.len() {
                    if to == from {
                        continue;
                    }
                    self.interp[to].receive(interp_event.clone());
                    self.generated[to].receive(gen_event.clone());
                }
            }
        }
    }

    /// The same crossing, comparing all six read-outs after each event has
    /// reached both of its recipients.
    fn deliver(&mut self) -> Result<(), String> {
        for from in 0..WRITERS.len() {
            for (interp_event, gen_event) in std::mem::take(&mut self.pending[from]) {
                for to in 0..WRITERS.len() {
                    if to == from {
                        continue;
                    }
                    self.interp[to].receive(interp_event.clone());
                    self.generated[to].receive(gen_event.clone());
                }
                self.compare()?;
            }
        }
        Ok(())
    }

    fn run(&mut self, script: &TrioScript) -> Result<(), String> {
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

/// One three-replica script, and the features every one of the three wrote
/// inside one undelivered round.
#[derive(Clone, Debug, Default)]
struct TrioScript {
    label: String,
    steps: Vec<Move>,
    /// What the three-way contention census counts: a feature all three
    /// writers wrote before anything crossed.
    contended: Vec<&'static str>,
}

/// Ten sequential three-replica scripts and twenty with a concurrent half.
///
/// Built the way [`seeded_script`] builds the two-replica ones, against a
/// shadow [`Trio`] driven by the same edits, with one difference that is the
/// point of the whole arm: the first round of every concurrent script is the
/// fair register, so a genuine three-way concurrent write lands on
/// `Class.author` in all twenty. The remaining four rounds pick a feature at
/// random, so the other eight are contended three-way too, which
/// [`ip31_thirty_three_replica_scripts_over_class_diagram_ecore_agree_everywhere`]
/// asserts rather than hopes for.
fn seeded_trio_script(seed: u64, concurrent: bool) -> TrioScript {
    let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x51ED_2701);
    let mut steps: Vec<Move> = vec![Move::Edit(open()), Move::Deliver];
    let mut contended: Vec<&'static str> = Vec::new();
    let mut shadow = Trio::new();
    let meta = Meta::new();
    shadow.carry(&open()).expect("a fresh root takes `New`");
    shadow.cross();

    let label = format!(
        "{} trio seed {seed}",
        if concurrent { "concurrent" } else { "sequential" }
    );

    if concurrent {
        for round in 0..5 {
            let focus = if round == 0 {
                FEATURES[FAIR]
            } else {
                FEATURES[rng.below(FEATURES.len())]
            };
            let mut all_landed = true;
            // The contended triple first: all three writers write the focus
            // feature against the state they share, before anything crosses.
            for writer in WRITERS {
                let seen = shadow.interp_doc(writer);
                let edit = propose_on(&meta, focus, &seen, &mut rng, writer);
                match shadow.carry(&edit) {
                    Ok(landed) => all_landed &= landed,
                    Err(_) => {
                        return TrioScript {
                            label,
                            steps,
                            contended,
                        };
                    }
                }
                steps.push(Move::Edit(edit));
            }
            if all_landed {
                contended.push(focus.name);
            }
            // Then one to three free edits from each writer, still
            // undelivered.
            for _ in 0..1 + rng.below(3) {
                for writer in WRITERS {
                    let seen = shadow.interp_doc(writer);
                    let edit = propose(&meta, &seen, &mut rng, writer);
                    if shadow.carry(&edit).is_err() {
                        return TrioScript {
                            label,
                            steps,
                            contended,
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
            let writer = WRITERS[index % WRITERS.len()];
            let seen = shadow.interp_doc(writer);
            let edit = propose(&meta, &seen, &mut rng, writer);
            if shadow.carry(&edit).is_err() {
                break;
            }
            steps.push(Move::Edit(edit));
            steps.push(Move::Deliver);
            shadow.cross();
        }
    }
    TrioScript {
        label,
        steps,
        contended,
    }
}

fn trio_scripts() -> Vec<TrioScript> {
    let mut out: Vec<TrioScript> = (0..10).map(|seed| seeded_trio_script(seed, false)).collect();
    out.extend((0..20).map(|seed| seeded_trio_script(seed, true)));
    out
}

// ---------------------------------------------------------------------------
// 10. The three-replica tests
// ---------------------------------------------------------------------------

/// `FairPolicy` is the only rule in this metamodel whose outcome moves with
/// the member count, and `Class.author` is the only feature that carries it.
///
/// This is the whole justification for a third replica, walked over the table
/// rather than asserted in prose. `LwwPolicy::compare` ties on
/// `origin_id`, which does not read `n`; a multi-value register keeps every
/// concurrent write and orders nothing; a total-order and a partial-order
/// register compare *values*; a set, a flag and a text sequence never consult
/// the member list at all. `FairPolicy::compare` is the one that takes
/// `a.id().resolver().into_vec()` and reduces a Lamport tie modulo its
/// length.
#[test]
fn ip31_the_fair_register_is_the_only_rule_that_reads_the_member_count() {
    let meta = Meta::new();
    assert_eq!(
        FEATURES[FAIR].name, "author",
        "`FAIR` indexes the feature the three-replica scripts contend first"
    );
    let mut fair: Vec<&str> = Vec::new();
    for class in &meta.sem.classes {
        for (index, (name, _, _)) in class.visible.iter().enumerate() {
            if let Some(MergeRule::Attribute {
                leaf: LeafRule::Register { tie: TieBreak::Fair } | LeafRule::Enum { tie: TieBreak::Fair, .. },
                ..
            }) = meta.sem.rule(class.slot, FeatureSlot(index as u16))
            {
                fair.push(name);
            }
        }
    }
    assert_eq!(
        fair,
        vec!["author"],
        "one fair register in the metamodel, and `FAIR` points at it"
    );
    assert!(
        matches!(
            meta.rule("author"),
            MergeRule::Attribute {
                leaf: LeafRule::Register {
                    tie: TieBreak::Fair
                },
                ..
            }
        ),
        "`Class.author` is a fair register: {:?}",
        meta.rule("author")
    );
}

/// One write of every construction, from three writers rather than two,
/// sequential and delivered, read out identically on all six.
#[test]
fn ip31_three_replicas_write_every_construction_and_agree() {
    let mut trio = Trio::new();
    let edits = vec![
        open(),
        edit('a', "name", Elem::InsertChar { pos: 0, ch: 'x' }),
        edit('b', "qualifiedName", Elem::Write("alpha")),
        edit('c', "author", Elem::Write("beta")),
        edit('a', "stereotype", Elem::Write("delta")),
        edit('b', "layer", Elem::Write("gamma")),
        edit('c', "isAbstract", Elem::Enable),
        edit('a', "visibility", Elem::WriteLiteral(1)),
        edit('b', "tags", Elem::Add("alpha")),
        edit('c', "tags", Elem::Add("beta")),
        edit('a', "invariants", Elem::Add("gamma")),
    ];
    for one in &edits {
        trio.apply(one).unwrap_or_else(|reason| panic!("{reason}"));
        trio.deliver().unwrap_or_else(|reason| panic!("{reason}"));
    }
    let read = trio.interp_doc('a');
    for writer in WRITERS {
        assert_eq!(read, trio.interp_doc(writer), "interpreted replica {writer}");
        assert_eq!(read, trio.gen_doc(writer), "generated replica {writer}");
    }
    assert_eq!(read["name"], json!("x"));
    assert_eq!(read["qualifiedName"], json!("alpha"));
    assert_eq!(read["author"], json!("beta"));
    assert_eq!(read["stereotype"], json!("delta"));
    assert_eq!(read["layer"], json!("gamma"));
    assert_eq!(read["isAbstract"], json!(true));
    assert_eq!(read["visibility"], json!("Private"));
    assert_eq!(read["tags"], json!(["alpha", "beta"]));
    assert_eq!(read["invariants"], json!(["gamma"]));
}

/// Three concurrent writes to each of the five register tie-breaks settle the
/// same way on both paths, and the three decided by value rather than by
/// event are asserted by value.
///
/// The three-way case is where a total order over tags stops being a
/// formality: with two writes the survivor is whichever of the two the policy
/// prefers, and any comparison at all produces one. With three, a policy
/// whose `compare` is not transitive can leave the three replicas holding
/// three different survivors, so the assertion that all six read-outs agree
/// is doing work here that it was not doing at two.
#[test]
fn ip31_three_concurrent_writes_to_every_register_settle_the_same_way_on_both_paths() {
    let mut trio = Trio::new();
    trio.apply(&open()).unwrap_or_else(|r| panic!("{r}"));
    trio.deliver().unwrap_or_else(|r| panic!("{r}"));

    let words = [('a', "alpha"), ('b', "beta"), ('c', "gamma")];
    for feature in ["qualifiedName", "author", "layer", "stereotype"] {
        for (writer, word) in words {
            trio.carry(&edit(writer, feature, Elem::Write(word)))
                .unwrap_or_else(|r| panic!("{r}"));
        }
    }
    // The enum, written concurrently at three different literals.
    for (index, writer) in WRITERS.iter().enumerate() {
        trio.carry(&edit(*writer, "visibility", Elem::WriteLiteral(index)))
            .unwrap_or_else(|r| panic!("{r}"));
    }
    trio.deliver().unwrap_or_else(|r| panic!("{r}"));

    let read = trio.interp_doc('a');
    for writer in WRITERS {
        assert_eq!(read, trio.interp_doc(writer), "interpreted replica {writer}");
        assert_eq!(read, trio.gen_doc(writer), "generated replica {writer}");
    }

    // Last-writer-wins and fair: one of the three words, the same one on all
    // six read-outs. *Which* is `LwwPolicy`'s and `FairPolicy`'s business.
    for feature in ["qualifiedName", "author"] {
        let held = read[feature]
            .as_str()
            .unwrap_or_else(|| panic!("a unique register holds one value: {}", read[feature]));
        assert!(
            words.iter().any(|(_, word)| *word == held),
            "`{feature}` holds `{held}`, which none of the three writers wrote"
        );
    }
    // Total order over `String`: the greatest of the three.
    assert_eq!(
        read["layer"],
        json!("gamma"),
        "a total-order register keeps the maximum of the three"
    );
    // Partial order over `String`, which is totally ordered, so the greatest
    // survives here too and the conflict set holds one value.
    assert_eq!(read["stereotype"], json!("gamma"));
    // Multi-value over an enum literal: all three survive, in the enum's
    // declaration order.
    assert_eq!(
        read["visibility"],
        json!({CONFLICT: ["Public", "Private", "Protected"]}),
        "three concurrent literals all survive"
    );

    // And a write causally below the three-way conflict replaces it, on both
    // paths and at all three seats.
    trio.apply(&edit('c', "visibility", Elem::WriteLiteral(3)))
        .unwrap_or_else(|r| panic!("{r}"));
    trio.deliver().unwrap_or_else(|r| panic!("{r}"));
    for writer in WRITERS {
        assert_eq!(trio.interp_doc(writer)["visibility"], json!("Package"));
        assert_eq!(trio.gen_doc(writer)["visibility"], json!("Package"));
    }
}

/// The reason a third replica was needed at all: with three members the fair
/// register's round-robin actually rotates, and each of the three writers
/// wins the tie on some Lamport value.
///
/// Not an assertion about `FairPolicy`'s schedule, which is `moirai-crdt`'s
/// business — it is an assertion about *this harness*. At two members
/// `round_leader = val % 2` picks between the only two seats there are, so a
/// two-replica oracle can pass while reading a degenerate case of the rule.
/// This says the three-replica harness reaches all three outcomes of the
/// round-robin, so the equality asserted beside it is not equality over one
/// repeated case.
///
/// # A symmetric schedule pins the leader, which is why the offsets are here
///
/// Measured on the first version of this test, and kept because it is the
/// non-obvious half. Under the schedule every other test in this file uses —
/// all three writers write, everything crosses, repeat — the Lamport clock
/// advances by exactly three per round, one per member, so `val % 3` never
/// changes and the same writer wins every round. Six rounds of it produced
/// `beta` six times. Rotation needs the clock to step by something that is
/// not a multiple of the member count, so the second half of this test runs
/// one fresh trio per offset, with `offset` extra delivered writes from `a`
/// ahead of the contended triple: the triple then lands at Lamport
/// `offset + 2` and the winner walks `beta`, `gamma`, `alpha` and round
/// again. Both halves check all six read-outs agree; only the second asserts
/// rotation.
#[test]
fn ip31_the_fair_registers_round_robin_rotates_with_three_members() {
    const WORDS: [(char, &str); 3] = [('a', "alpha"), ('b', "beta"), ('c', "gamma")];

    // The symmetric schedule, printed rather than asserted: what the clock
    // does here is `moirai-protocol`'s business and this file only records it.
    let mut trio = Trio::new();
    trio.apply(&open()).unwrap_or_else(|r| panic!("{r}"));
    trio.deliver().unwrap_or_else(|r| panic!("{r}"));
    let mut symmetric: Vec<&str> = Vec::new();
    for _ in 0..6 {
        for (writer, word) in WORDS {
            trio.carry(&edit(writer, "author", Elem::Write(word)))
                .unwrap_or_else(|r| panic!("{r}"));
        }
        trio.deliver().unwrap_or_else(|r| panic!("{r}"));
        symmetric.push(agreed_fair_winner(&trio, &WORDS));
    }
    eprintln!("ip31 fair register, symmetric schedule, six rounds: {symmetric:?}");

    // The asymmetric schedule: one fresh trio per offset.
    let mut winners: Vec<&str> = Vec::new();
    for offset in 0..6 {
        let mut trio = Trio::new();
        trio.apply(&open()).unwrap_or_else(|r| panic!("{r}"));
        trio.deliver().unwrap_or_else(|r| panic!("{r}"));
        for _ in 0..offset {
            trio.apply(&edit('a', "layer", Elem::Write("alpha")))
                .unwrap_or_else(|r| panic!("{r}"));
            trio.deliver().unwrap_or_else(|r| panic!("{r}"));
        }
        for (writer, word) in WORDS {
            trio.carry(&edit(writer, "author", Elem::Write(word)))
                .unwrap_or_else(|r| panic!("{r}"));
        }
        trio.deliver().unwrap_or_else(|r| panic!("{r}"));
        winners.push(agreed_fair_winner(&trio, &WORDS));
    }
    let mut distinct = winners.clone();
    distinct.sort_unstable();
    distinct.dedup();
    eprintln!("ip31 fair register, offsets 0 to 5: {winners:?}");
    assert_eq!(
        distinct.len(),
        3,
        "the round-robin reached {} of the three seats over the six offsets \
         ({winners:?}), so three members are still exercising a degenerate \
         case of the rule",
        distinct.len()
    );
}

/// The value every one of the six read-outs holds for `Class.author`, checked
/// to be the same on all of them and to be one of the three words written.
fn agreed_fair_winner(trio: &Trio, words: &[(char, &'static str); 3]) -> &'static str {
    let read = trio.interp_doc('a');
    for writer in WRITERS {
        assert_eq!(read, trio.interp_doc(writer), "interpreted replica {writer}");
        assert_eq!(read, trio.gen_doc(writer), "generated replica {writer}");
    }
    let held = read["author"]
        .as_str()
        .unwrap_or_else(|| panic!("a fair register holds one value: {}", read["author"]));
    words
        .iter()
        .find(|(_, word)| *word == held)
        .unwrap_or_else(|| panic!("`{held}` is none of the three words written"))
        .1
}

/// **ip31** — thirty seeded three-replica scripts, ten sequential and twenty
/// whose every round has all three writers write one feature before anything
/// crosses, compared at all three seats after every operation and after every
/// delivery, with no exclusion on either side's read-out.
#[test]
fn ip31_thirty_three_replica_scripts_over_class_diagram_ecore_agree_everywhere() {
    let scripts = trio_scripts();
    let mut edits = 0usize;
    let mut comparisons = 0usize;
    let mut refused = 0usize;
    let mut failures = Vec::new();
    // For each feature, how many of the thirty scripts had all three writers
    // write it inside one undelivered round.
    let mut contention: Vec<(&str, usize)> = FEATURES.iter().map(|f| (f.name, 0)).collect();
    for script in &scripts {
        edits += script
            .steps
            .iter()
            .filter(|step| matches!(step, Move::Edit(_)))
            .count();
        for (name, count) in contention.iter_mut() {
            if script.contended.contains(name) {
                *count += 1;
            }
        }
        let mut trio = Trio::new();
        if let Err(reason) = trio.run(script) {
            failures.push(reason);
        }
        comparisons += trio.comparisons;
        refused += trio.refused;
    }
    eprintln!(
        "ip31: {} three-replica scripts, {edits} edits, {comparisons} comparisons, \
         {refused} refused by both intakes",
        scripts.len()
    );
    eprintln!("ip31 three-way contended scripts per feature: {contention:?}");
    assert!(
        failures.is_empty(),
        "{} of {} three-replica scripts diverged:\n{}",
        failures.len(),
        scripts.len(),
        failures.join("\n\n")
    );
    assert_eq!(scripts.len(), 30, "ten sequential and twenty concurrent");
    assert!(
        edits >= 400,
        "the scripts have to carry real work: {edits} edits"
    );
    let fair = contention
        .iter()
        .find(|(name, _)| *name == FEATURES[FAIR].name)
        .expect("the fair register is one of the nine");
    assert_eq!(
        fair.1, 20,
        "every one of the twenty concurrent scripts opens with a three-way \
         write to the fair register, and {} did",
        fair.1
    );
    for (name, count) in &contention {
        assert!(
            *count > 0,
            "`{name}` was never written concurrently by all three replicas, so \
             its merge rule is not being tested three-way"
        );
    }
}
