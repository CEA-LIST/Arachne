//! The shared machinery of the equivalence oracle: the metamodel as both
//! paths read it, the two encoders, the canonical projection, the four-replica
//! harness and the seeded script generator.
//!
//! This module is `equivalence.rs` from line 100 to the tests, moved out
//! unchanged except for the visibility keywords, so that a second driver can
//! use it without a third encoder being written. The reason it exists is
//! I-E1 of [`02 Validation Plan`] §5, which measures the interpreted path
//! against the generated one per model edit over these same scripts: the
//! measurement driver lives in the vault (`experiments/ip1-interp-overhead/`)
//! and is copied into this directory to run, so it cannot carry the machinery
//! itself.
//!
//! `tests/support/` is not a test target of its own — cargo takes only the
//! top-level `.rs` files under `tests/` — so nothing here runs on its own and
//! nothing here is a test. What the oracle claims, and the whole of why it
//! claims it, is documented at the top of `equivalence.rs`.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use bt_crdt::package::{Behaviortree, BehaviortreeLog, BehaviortreeValue};
use heck::{ToSnakeCase, ToUpperCamelCase};
use moirai_crdt::utils::membership::twins_log;
use moirai_interp::testing::{BT_DESCRIPTOR, opened};
use moirai_interp::{InstanceOp, LeafOp, ModelLog, ModelOp, Scalar};
use moirai_protocol::broadcast::tcsb::Tcsb;
use moirai_protocol::crdt::query::Read;
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::replica::{IsReplica, Replica};
use moirai_semantics::{
    ClassSemantics, ClassSlot, FeatureSlot, LeafRule, MergeRule, MetamodelSemantics, Shape,
    from_descriptor,
};
use serde_json::{Map, Value, json};

pub type InterpReplica = Replica<ModelLog, Tcsb<ModelOp>>;
pub type GenReplica = Replica<BehaviortreeLog, Tcsb<Behaviortree>>;

/// The key the canonical form carries a class name under.
pub const ECLASS: &str = "eClass";
/// The key the canonical form carries a conflict set under.
pub const CONFLICT: &str = "__conflict";

// ---------------------------------------------------------------------------
// 1. The metamodel, read the way both paths read it
// ---------------------------------------------------------------------------

/// A [`MetamodelSemantics`] plus the two derived relations the encoders need:
/// who inherits from whom, and which class the generator would have wrapped
/// in a `union!`.
pub struct Meta {
    pub sem: Arc<MetamodelSemantics>,
    /// Direct subclasses, by declaring class slot.
    pub subs: BTreeMap<u16, Vec<ClassSlot>>,
}

impl Meta {
    pub fn new(sem: Arc<MetamodelSemantics>) -> Self {
        let mut subs: BTreeMap<u16, Vec<ClassSlot>> = BTreeMap::new();
        for class in &sem.classes {
            for sup in &class.supers {
                subs.entry(sup.0).or_default().push(class.slot);
            }
        }
        Meta { sem, subs }
    }

    pub fn class(&self, slot: ClassSlot) -> &ClassSemantics {
        &self.sem.classes[slot.index()]
    }

    pub fn name(&self, slot: ClassSlot) -> &str {
        &self.class(slot).name
    }

    pub fn slot(&self, name: &str) -> ClassSlot {
        self.sem
            .classes
            .iter()
            .find(|class| &*class.name == name)
            .unwrap_or_else(|| panic!("no class `{name}` in the table"))
            .slot
    }

    pub fn subs(&self, slot: ClassSlot) -> &[ClassSlot] {
        self.subs.get(&slot.0).map_or(&[], Vec::as_slice)
    }

    /// The class the model root is declared as. Every descriptor this test
    /// drives names exactly one.
    pub fn root_class(&self) -> ClassSlot {
        assert_eq!(
            self.sem.roots.len(),
            1,
            "the oracle drives one declared root class"
        );
        self.sem.roots[0]
    }

    /// Whether `classifier/mod.rs:36-45` would have emitted a `<Name>Kind`
    /// `union!` for this class: abstract, an interface, or having subclasses.
    /// A containment onto such a class reaches its object through a variant
    /// chain; a containment onto any other reaches the record directly.
    pub fn is_union(&self, slot: ClassSlot) -> bool {
        !self.class(slot).instantiable || !self.subs(slot).is_empty()
    }

    /// The visible slot of one feature on one class: what an
    /// `InstanceOp::Field` carries.
    pub fn visible_slot(&self, class: ClassSlot, feature: &str) -> FeatureSlot {
        let holder = self.class(class);
        let index = holder
            .visible
            .iter()
            .position(|(name, _, _)| &**name == feature)
            .unwrap_or_else(|| panic!("`{}` cannot see `{feature}`", holder.name));
        FeatureSlot(index as u16)
    }

    /// The class that declares one visible feature, and its rule.
    pub fn declared(&self, class: ClassSlot, feature: &str) -> (ClassSlot, MergeRule) {
        let holder = self.class(class);
        let (_, owner, slot) = holder
            .visible
            .iter()
            .find(|(name, _, _)| &**name == feature)
            .unwrap_or_else(|| panic!("`{}` cannot see `{feature}`", holder.name));
        let rule = *self
            .sem
            .rule(*owner, *slot)
            .expect("a visible feature has a rule");
        (*owner, rule)
    }

    pub fn rule(&self, class: ClassSlot, feature: &str) -> MergeRule {
        self.declared(class, feature).1
    }

    /// The chain of classes from `from` up to `to`, `to` included and `from`
    /// excluded: one `<Super>Super` hop per entry on the generated path, and
    /// nothing at all on the interpreted one.
    pub fn super_path(&self, from: ClassSlot, to: ClassSlot) -> Vec<ClassSlot> {
        fn walk(meta: &Meta, at: ClassSlot, to: ClassSlot, acc: &mut Vec<ClassSlot>) -> bool {
            if at == to {
                return true;
            }
            for sup in &meta.class(at).supers {
                acc.push(*sup);
                if walk(meta, *sup, to, acc) {
                    return true;
                }
                acc.pop();
            }
            false
        }
        let mut acc = Vec::new();
        assert!(
            walk(self, from, to, &mut acc),
            "`{}` does not inherit from `{}`",
            self.name(from),
            self.name(to)
        );
        acc
    }

    /// The `union!` variant names to descend, from the declared target of a
    /// containment down to the concrete class actually sitting there. Empty
    /// when the generator emitted the record directly.
    pub fn union_descent(&self, target: ClassSlot, concrete: ClassSlot) -> Vec<String> {
        if !self.is_union(target) {
            assert_eq!(
                target,
                concrete,
                "a containment onto a class with no `union!` holds that class only"
            );
            return Vec::new();
        }
        if target == concrete {
            // `classifier/mod.rs:592`: a concrete class with subclasses is
            // the first variant of its own union, named after itself.
            return vec![self.name(target).to_string()];
        }
        let step = self
            .subs(target)
            .iter()
            .copied()
            .find(|sub| self.class(*sub).concrete.contains(&concrete))
            .unwrap_or_else(|| {
                panic!(
                    "`{}` is not below `{}`",
                    self.name(concrete),
                    self.name(target)
                )
            });
        let mut chain = vec![self.name(step).to_string()];
        if self.is_union(step) {
            chain.extend(self.union_descent(step, concrete));
        } else {
            assert_eq!(step, concrete, "a record variant is its own class");
        }
        chain
    }

    /// The single-valued containments of `class` whose target the generator
    /// compiles as a plain record, which therefore exists on the generated
    /// path from the moment its parent does, and which the script has to
    /// mint on the interpreted path in the same operation.
    pub fn mandatory(&self, class: ClassSlot) -> Vec<(String, ClassSlot)> {
        self.class(class)
            .visible
            .iter()
            .filter_map(|(name, owner, slot)| {
                let rule = self.sem.rule(*owner, *slot)?;
                match rule {
                    MergeRule::Containment { shape, target }
                        if *shape == Shape::Single && !self.is_union(*target) =>
                    {
                        Some((name.to_string(), *target))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    /// The one chain of mandatory children a mint has to carry. A metamodel
    /// where a class has two of them needs two operations per create; this
    /// says so rather than dropping one.
    pub fn mint_chain(&self, class: ClassSlot) -> Vec<(String, ClassSlot)> {
        let mut chain = Vec::new();
        let mut at = class;
        let mut guard = 0;
        loop {
            let mandatory = self.mandatory(at);
            assert!(
                mandatory.len() < 2,
                "`{}` has {} mandatory single containments; this oracle mints one \
                 chain per create",
                self.name(at),
                mandatory.len()
            );
            let Some((feature, target)) = mandatory.into_iter().next() else {
                return chain;
            };
            chain.push((feature, target));
            at = target;
            guard += 1;
            assert!(guard < 16, "a cycle of mandatory single containments");
        }
    }
}

/// The Rust field name the generator gives a feature, and the enum variant
/// the `record!` macro derives from it.
/// One externally tagged enum value: the variant name, and what it carries.
/// Written out rather than through `json!`, whose keys are literals.
pub fn tagged(variant: impl Into<String>, payload: Value) -> Value {
    let mut map = Map::new();
    map.insert(variant.into(), payload);
    Value::Object(map)
}

pub fn field_of(feature: &str) -> String {
    feature.to_snake_case()
}

pub fn variant_of(feature: &str) -> String {
    field_of(feature).to_upper_camel_case()
}

/// The `<Super>Super` field the generator emits for an inherited class
/// (`classifier/mod.rs:62-70`), as its `record!` variant.
pub fn super_variant(class: &str) -> String {
    format!("{}_super", class.to_snake_case()).to_upper_camel_case()
}

// ---------------------------------------------------------------------------
// 2. The edit script
// ---------------------------------------------------------------------------

/// One step of a path from the model root: which feature, at which position
/// when the feature is ordered, and the concrete class of the object landed
/// on.
#[derive(Clone, Debug, PartialEq)]
pub struct Hop {
    pub feature: String,
    pub at: Option<usize>,
    pub class: String,
}

/// Where an edit happens: the root class, then the hops down to the object
/// the edit addresses.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Path {
    pub hops: Vec<Hop>,
}

impl Path {
    pub fn child(&self, hop: Hop) -> Path {
        let mut hops = self.hops.clone();
        hops.push(hop);
        Path { hops }
    }

    pub fn depth(&self) -> usize {
        self.hops.len()
    }

    pub fn show(&self) -> String {
        let mut out = String::from("/");
        for hop in &self.hops {
            out.push_str(&hop.feature);
            if let Some(at) = hop.at {
                out.push_str(&format!("[{at}]"));
            }
            out.push_str(&format!(":{}/", hop.class));
        }
        out
    }
}

/// What an edit does to the object its path names.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// `Create`: mint an object of `class` in `feature`, at `pos` when the
    /// feature is ordered, together with its mandatory children.
    Create {
        feature: String,
        pos: Option<usize>,
        class: String,
    },
    /// `Delete`: take the child at `pos` out of an ordered containment.
    Delete { feature: String, pos: usize },
    /// `InsertChar` and `DeleteChar`, and `SetText` as a run of them.
    Text { feature: String, op: TextOp },
    /// Unset an optional attribute.
    Unset { feature: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum TextOp {
    Insert {
        pos: usize,
        ch: char,
        /// The whole attribute as the writer sees it after this character
        /// goes in. Only a register binding reads it — a text leaf takes the
        /// character and the position, which is the point of `ip15`.
        after: String,
    },
    Delete {
        pos: usize,
        after: String,
    },
}

/// One edit, addressing an object by a script-local id and, resolved against
/// the writer's own view at the moment it was generated, by a path.
#[derive(Clone, Debug)]
pub struct Edit {
    /// Script-local object id: `0` is the root, and every `Create` mints the
    /// next one for the object it makes.
    pub id: u32,
    /// Which replica issues it.
    pub writer: char,
    pub path: Path,
    pub action: Action,
}

impl Edit {
    pub fn show(&self) -> String {
        let what = match &self.action {
            Action::Create { feature, pos, class } => format!(
                "Create {class} in {feature}{}",
                pos.map_or(String::new(), |p| format!("[{p}]"))
            ),
            Action::Delete { feature, pos } => format!("Delete {feature}[{pos}]"),
            Action::Text {
                feature,
                op: TextOp::Insert { pos, ch, .. },
            } => format!("InsertChar {feature}[{pos}] = {ch:?}"),
            Action::Text {
                feature,
                op: TextOp::Delete { pos, .. },
            } => format!("DeleteChar {feature}[{pos}]"),
            Action::Unset { feature } => format!("Unset {feature}"),
        };
        format!("#{} on {} at {} — {what}", self.id, self.writer, self.path.show())
    }
}

/// A whole script: what it is called, and the edits in the order they are
/// issued. `deliver` marks the points where every pending event crosses.
#[derive(Clone, Debug, Default)]
pub struct EditScript {
    pub label: String,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug)]
pub enum Step {
    Edit(Edit),
    /// Deliver everything both writers are holding, `a`'s events first.
    Deliver,
}

// ---------------------------------------------------------------------------
// 3. The two encoders
// ---------------------------------------------------------------------------

/// The class each hop of a path lands on, the root class first.
pub fn classes_along(meta: &Meta, path: &Path) -> Vec<ClassSlot> {
    let mut out = vec![meta.root_class()];
    for hop in &path.hops {
        out.push(meta.slot(&hop.class));
    }
    out
}

/// One edit as a [`ModelOp`]: the interpreted path's encoding.
///
/// Inheritance is flat here — an inherited feature is one visible slot on the
/// instance and no hop at all — which is the whole difference from the typed
/// encoder below.
pub fn interp_op(meta: &Meta, edit: &Edit) -> ModelOp {
    let classes = classes_along(meta, &edit.path);
    let leaf_class = *classes.last().expect("the root is always there");
    let mut op = interp_action(meta, leaf_class, &edit.action);
    for (index, hop) in edit.path.hops.iter().enumerate().rev() {
        let parent = classes[index];
        let mut step = InstanceOp::variant(meta.slot(&hop.class), op);
        step = match (hop.at, meta.rule(parent, &hop.feature)) {
            (Some(pos), _) => InstanceOp::at(pos, step),
            (None, MergeRule::Containment { shape, .. }) if shape == Shape::Optional => {
                InstanceOp::set(step)
            }
            (None, _) => step,
        };
        op = InstanceOp::field(meta.visible_slot(parent, &hop.feature), step);
    }
    ModelOp::Instance(InstanceOp::variant(meta.root_class(), op))
}

/// The interpreted payload that mints an object of `class` together with
/// every mandatory child it has.
pub fn interp_mint(meta: &Meta, class: ClassSlot) -> InstanceOp {
    let chain = meta.mint_chain(class);
    let mut op = InstanceOp::New;
    let mut owners: Vec<ClassSlot> = vec![class];
    owners.extend(chain.iter().map(|(_, target)| *target));
    for (index, (feature, target)) in chain.iter().enumerate().rev() {
        op = InstanceOp::field(
            meta.visible_slot(owners[index], feature),
            InstanceOp::variant(*target, op),
        );
    }
    op
}

pub fn interp_action(meta: &Meta, class: ClassSlot, action: &Action) -> InstanceOp {
    match action {
        Action::Create { feature, pos, class: made } => {
            let made = meta.slot(made);
            let inner = InstanceOp::variant(made, interp_mint(meta, made));
            let shaped = match meta.rule(class, feature) {
                MergeRule::Containment { shape, .. } => shape,
                other => panic!("`{feature}` is not a containment: {other:?}"),
            };
            let step = match (shaped, pos) {
                (Shape::Sequence, Some(pos)) => InstanceOp::insert(*pos, inner),
                (Shape::Optional, None) => InstanceOp::set(inner),
                (Shape::Single, None) => inner,
                (shape, pos) => panic!("{shape:?} does not take {pos:?}"),
            };
            InstanceOp::field(meta.visible_slot(class, feature), step)
        }
        Action::Delete { feature, pos } => {
            InstanceOp::field(meta.visible_slot(class, feature), InstanceOp::delete(*pos))
        }
        Action::Unset { feature } => {
            InstanceOp::field(meta.visible_slot(class, feature), InstanceOp::unset())
        }
        Action::Text { feature, op } => {
            let (shape, leaf) = match meta.rule(class, feature) {
                MergeRule::Attribute { shape, leaf } => (shape, leaf),
                other => panic!("`{feature}` is not an attribute: {other:?}"),
            };
            // The leaf the *table* names decides the operation, which is what
            // makes `ip15` a mutation of the semantics rather than of the
            // script: a text leaf takes the character, a register takes the
            // whole string the writer now sees.
            let leaf_op = match (leaf, op) {
                (LeafRule::Text, TextOp::Insert { pos, ch, .. }) => {
                    LeafOp::InsertChar { pos: *pos, ch: *ch }
                }
                (LeafRule::Text, TextOp::Delete { pos, .. }) => LeafOp::DeleteChar { pos: *pos },
                (
                    LeafRule::Register { .. },
                    TextOp::Insert { after, .. } | TextOp::Delete { after, .. },
                ) => LeafOp::Write(Scalar::text(after.clone())),
                (leaf, op) => panic!("{leaf:?} does not take {op:?}"),
            };
            let step = match shape {
                Shape::Optional => InstanceOp::set(InstanceOp::Leaf(leaf_op)),
                _ => InstanceOp::Leaf(leaf_op),
            };
            InstanceOp::field(meta.visible_slot(class, feature), step)
        }
    }
}

/// One edit as this crate's typed operation, built as JSON by the naming
/// convention the generator uses and then deserialized, so a wrong shape is
/// an error naming the enum and never a silent pass.
pub fn typed_op(meta: &Meta, edit: &Edit) -> Behaviortree {
    let value = typed_json(meta, edit);
    serde_json::from_value(value.clone()).unwrap_or_else(|error| {
        panic!(
            "the typed encoder built an operation `Behaviortree` cannot take: {error}\n{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        )
    })
}

pub fn typed_json(meta: &Meta, edit: &Edit) -> Value {
    let classes = classes_along(meta, &edit.path);
    let leaf_class = *classes.last().expect("the root is always there");
    let mut op = typed_action(meta, leaf_class, &edit.action);
    for (index, hop) in edit.path.hops.iter().enumerate().rev() {
        let parent = classes[index];
        let target = match meta.rule(parent, &hop.feature) {
            MergeRule::Containment { target, .. } => target,
            other => panic!("`{}` is not a containment: {other:?}", hop.feature),
        };
        let mut step = union_wrap(meta, target, meta.slot(&hop.class), op);
        step = match (hop.at, meta.rule(parent, &hop.feature)) {
            (Some(pos), _) => json!({ "Update": { "pos": pos, "op": step } }),
            (None, MergeRule::Containment { shape, .. }) if shape == Shape::Optional => {
                json!({ "Set": step })
            }
            (None, _) => step,
        };
        op = feature_wrap(meta, parent, &hop.feature, step);
    }
    tagged(meta.name(meta.root_class()).to_upper_camel_case(), op)
}

/// Wrap an operation on the record of `class` in the `<Super>Super` hops that
/// carry it from `class` down to the class that declares `feature`, then in
/// the feature's own variant.
///
/// `Sequence` sees `ID` through `control_node_super` and then
/// `tree_node_super`; the chain comes out of the table's `supers`, not out of
/// a list written here.
pub fn feature_wrap(meta: &Meta, class: ClassSlot, feature: &str, payload: Value) -> Value {
    let (owner, _) = meta.declared(class, feature);
    let mut value = tagged(variant_of(feature), payload);
    for hop in meta.super_path(class, owner).iter().rev() {
        value = tagged(super_variant(meta.name(*hop)), value);
    }
    value
}

/// Wrap an operation on the record of `concrete` in the `union!` variants
/// that carry it from the containment's declared `target` down to it.
pub fn union_wrap(meta: &Meta, target: ClassSlot, concrete: ClassSlot, payload: Value) -> Value {
    let mut value = payload;
    for variant in meta.union_descent(target, concrete).iter().rev() {
        value = tagged(variant.clone(), value);
    }
    value
}

/// The typed payload that mints an object of `class` with its mandatory
/// children: `record!`'s `New`, wrapped in one feature per mandatory hop.
pub fn typed_mint(meta: &Meta, class: ClassSlot) -> Value {
    let chain = meta.mint_chain(class);
    let mut owners: Vec<ClassSlot> = vec![class];
    owners.extend(chain.iter().map(|(_, target)| *target));
    let mut value = json!("New");
    for (index, (feature, target)) in chain.iter().enumerate().rev() {
        let holder = owners[index];
        value = feature_wrap(
            meta,
            holder,
            feature,
            union_wrap(meta, *target, *target, value),
        );
    }
    value
}

pub fn typed_action(meta: &Meta, class: ClassSlot, action: &Action) -> Value {
    match action {
        Action::Create { feature, pos, class: made } => {
            let made = meta.slot(made);
            let (shape, target) = match meta.rule(class, feature) {
                MergeRule::Containment { shape, target } => (shape, target),
                other => panic!("`{feature}` is not a containment: {other:?}"),
            };
            let inner = union_wrap(meta, target, made, typed_mint(meta, made));
            let step = match (shape, pos) {
                (Shape::Sequence, Some(pos)) => json!({ "Insert": { "pos": pos, "op": inner } }),
                (Shape::Optional, None) => json!({ "Set": inner }),
                (Shape::Single, None) => inner,
                (shape, pos) => panic!("{shape:?} does not take {pos:?}"),
            };
            feature_wrap(meta, class, feature, step)
        }
        Action::Delete { feature, pos } => {
            feature_wrap(meta, class, feature, json!({ "Delete": { "pos": pos } }))
        }
        Action::Unset { feature } => feature_wrap(meta, class, feature, json!("Unset")),
        Action::Text { feature, op } => {
            let shape = match meta.rule(class, feature) {
                MergeRule::Attribute { shape, .. } => shape,
                other => panic!("`{feature}` is not an attribute: {other:?}"),
            };
            let leaf = match op {
                TextOp::Insert { pos, ch, .. } => {
                    json!({ "Insert": { "content": ch, "pos": pos } })
                }
                TextOp::Delete { pos, .. } => json!({ "Delete": { "pos": pos } }),
            };
            let step = match shape {
                Shape::Optional => json!({ "Set": leaf }),
                _ => leaf,
            };
            feature_wrap(meta, class, feature, step)
        }
    }
}

// ---------------------------------------------------------------------------
// 4. The projection of the generated read-out onto the canonical form
// ---------------------------------------------------------------------------

/// The generated `Read` value in the canonical form of `02 Validation Plan`
/// §2: `_super` hops flattened, `union!` wrappers unwrapped into an `eClass`
/// key, `Vec<char>` joined, unset optionals absent, sequences in read order,
/// keys sorted, conflicts by class name and then by canonical bytes.
pub fn project(meta: &Meta, value: &BehaviortreeValue) -> Value {
    let raw = serde_json::to_value(value).expect("the generated read-out serializes");
    let root = raw
        .get(field_of(meta.name(meta.root_class())))
        .unwrap_or_else(|| panic!("the package value carries the root under its field"));
    without_defaults(meta, project_object(meta, meta.root_class(), root))
}

pub fn project_object(meta: &Meta, class: ClassSlot, value: &Value) -> Value {
    let mut out = Map::new();
    out.insert(ECLASS.to_string(), Value::String(meta.name(class).to_string()));
    for (name, owner, slot) in &meta.class(class).visible {
        let Some(rule) = meta.sem.rule(*owner, *slot) else {
            continue;
        };
        match rule {
            // Design §8: a reference is a string on the interpreted path and
            // a `typed_graph!` arc on the generated one, so there is nothing
            // here to compare and `canon` drops the other side's key too.
            MergeRule::Reference { .. } | MergeRule::Unsupported { .. } => continue,
            MergeRule::Attribute { shape, leaf } => {
                let found = locate(meta, class, *owner, name, value);
                let projected = match (*shape, leaf) {
                    (Shape::Single, LeafRule::Text) => Some(chars(found)),
                    (Shape::Optional, LeafRule::Text) => {
                        if found.is_null() {
                            None
                        } else {
                            Some(chars(found))
                        }
                    }
                    (shape, leaf) => panic!(
                        "no projection for {shape:?} of {leaf:?} on `{}.{name}`; \
                         `bt.ecore` has none and this oracle claims none",
                        meta.name(class)
                    ),
                };
                if let Some(projected) = projected {
                    out.insert(name.to_string(), projected);
                }
            }
            MergeRule::Containment { shape, target } => {
                let found = locate(meta, class, *owner, name, value);
                match *shape {
                    Shape::Single => {
                        if let Some(object) = project_slot(meta, *target, found) {
                            out.insert(name.to_string(), object);
                        }
                    }
                    Shape::Optional => {
                        if !found.is_null()
                            && let Some(object) = project_slot(meta, *target, found)
                        {
                            out.insert(name.to_string(), object);
                        }
                    }
                    Shape::Sequence => {
                        let items = found
                            .as_array()
                            .unwrap_or_else(|| panic!("`{name}` reads as an array"))
                            .iter()
                            .filter_map(|item| project_slot(meta, *target, item))
                            .collect();
                        out.insert(name.to_string(), Value::Array(items));
                    }
                    shape => panic!("a containment cannot be {shape:?}"),
                }
            }
        }
    }
    Value::Object(out)
}

/// One containment site: the record directly, or whatever the `union!`
/// wrapper holds, flattened across nested unions and re-ordered the way §2
/// asks for.
pub fn project_slot(meta: &Meta, target: ClassSlot, value: &Value) -> Option<Value> {
    if !meta.is_union(target) {
        return Some(project_object(meta, target, value));
    }
    let mut held = Vec::new();
    collect_union(meta, target, value, &mut held);
    match held.len() {
        0 => None,
        1 => Some(held.pop().expect("just counted").1),
        _ => {
            // `union!`'s own `Conflict` is sorted by variant rank
            // (`union.rs:96-100`); §2 asks for class name and then canonical
            // bytes, which is what the interpreted read-out already does.
            let mut keyed: Vec<(String, Vec<u8>, Value)> = held
                .into_iter()
                .map(|(name, value)| {
                    let bytes = serde_json::to_vec(&value).unwrap_or_default();
                    (name, bytes, value)
                })
                .collect();
            keyed.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
            let mut out = Map::new();
            out.insert(
                CONFLICT.to_string(),
                Value::Array(keyed.into_iter().map(|(_, _, value)| value).collect()),
            );
            Some(Value::Object(out))
        }
    }
}

pub fn collect_union(meta: &Meta, target: ClassSlot, value: &Value, out: &mut Vec<(String, Value)>) {
    match value {
        Value::String(word) if word == "Unset" => {}
        Value::Object(map) if map.len() == 1 => {
            let (key, payload) = map.iter().next().expect("just checked");
            match key.as_str() {
                "Value" => collect_child(meta, target, payload, out),
                "Conflict" => {
                    for child in payload
                        .as_array()
                        .unwrap_or_else(|| panic!("a `Conflict` holds an array"))
                    {
                        collect_child(meta, target, child, out);
                    }
                }
                other => panic!("`{other}` is not a `union!` state"),
            }
        }
        other => panic!("`{other}` is not a `union!` value"),
    }
}

pub fn collect_child(meta: &Meta, target: ClassSlot, value: &Value, out: &mut Vec<(String, Value)>) {
    let Value::Object(map) = value else {
        panic!("a `union!` child is its variant object");
    };
    let (variant, payload) = map.iter().next().expect("a variant carries one payload");
    let class = meta.slot(variant);
    if class != target && meta.is_union(class) {
        collect_union(meta, class, payload, out);
    } else {
        out.push((
            meta.name(class).to_string(),
            project_object(meta, class, payload),
        ));
    }
}

/// The JSON of one feature, reached from the record of `class` through the
/// `<Super>Super` fields that carry it to the class that declares it.
pub fn locate<'a>(
    meta: &Meta,
    class: ClassSlot,
    owner: ClassSlot,
    feature: &str,
    value: &'a Value,
) -> &'a Value {
    let mut at = value;
    for hop in meta.super_path(class, owner) {
        let field = format!("{}_super", meta.name(hop).to_snake_case());
        at = at.get(&field).unwrap_or_else(|| {
            panic!(
                "`{}` has no `{field}` on the way to `{feature}`",
                meta.name(class)
            )
        });
    }
    at.get(field_of(feature)).unwrap_or_else(|| {
        panic!(
            "`{}` has no field `{}` for `{feature}`",
            meta.name(owner),
            field_of(feature)
        )
    })
}

/// A `Vec<char>` as a string.
pub fn chars(value: &Value) -> Value {
    let text: String = value
        .as_array()
        .unwrap_or_else(|| panic!("a text attribute reads as an array of characters"))
        .iter()
        .map(|ch| {
            ch.as_str()
                .unwrap_or_else(|| panic!("a character reads as a string"))
                .to_string()
        })
        .collect();
    Value::String(text)
}

// ---------------------------------------------------------------------------
// 4b. Defaults, dropped from both sides by the same function
// ---------------------------------------------------------------------------

/// A canonical document with every default-valued key dropped, and a root
/// that is entirely default spelled `null`.
///
/// Called on both sides of the comparison, each with its own table, and it is
/// the *only* place either side is pruned, so the two sides cannot drift.
///
/// # Why the rule exists
///
/// `record!`'s `new` (`moirai-macros/src/record.rs:57-63`) constructs one
/// field per feature through `IsLog::new`, recursively, so a generated log
/// renders its entire shape from the moment it is constructed: a `Root` with
/// `behaviortrees` at `[]` and a `main` whose `ID` is `""` and whose
/// `blackboard` holds `entries: []`, all of it before any operation has been
/// applied. The interpreted `ObjectNode` (`moirai-interp/src/node.rs:514-518`)
/// holds no object until an operation mints one, and `eval::read` spells a
/// model whose root has not been written as `null`. Both describe the same
/// state — nothing has been written — and the canonical form of §2 had no way
/// to say so.
///
/// # The rule
///
/// A key whose value is the default of that feature's rule is dropped: `""`
/// for a text leaf, `null` for an unwritten register or enum, `0` for a
/// counter, `false` for a flag, `[]` for a sequence, set or bag, and, for a
/// single-valued containment, an object that carries nothing but its class
/// name once its own defaults have gone. Applied bottom-up, so the spine
/// `Root.main.blackboard` collapses in one pass, and at the top a root left
/// with nothing but its `eClass` becomes `null`.
///
/// # What is exempt, and why that matters
///
/// An **optional** — attribute or containment — is never dropped when it is
/// present. Its default is absence, and an absent optional already carries no
/// key at all, so anything present under one was put there by an operation.
/// This is what keeps the rule from swallowing a real value: `TreeNode.name`
/// written and then unset stays present-and-empty on *both* paths (the leaf
/// still holds the operations that emptied it, so neither path's log is at its
/// default), and the projection leaves it present on both.
pub fn without_defaults(meta: &Meta, value: Value) -> Value {
    let value = drop_defaults(meta, value);
    if only_a_class(&value) {
        Value::Null
    } else {
        value
    }
}

/// An object carrying nothing but its class name.
pub fn only_a_class(value: &Value) -> bool {
    matches!(value, Value::Object(map) if map.len() == 1 && map.contains_key(ECLASS))
}

pub fn drop_defaults(meta: &Meta, value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| drop_defaults(meta, item))
                .collect(),
        ),
        Value::Object(map) => {
            // A conflict set carries no `eClass` of its own, so its keys are
            // left alone and only its members are walked.
            let class = map
                .get(ECLASS)
                .and_then(Value::as_str)
                .map(|name| meta.slot(name));
            let mut out = Map::new();
            for (key, item) in map {
                let item = drop_defaults(meta, item);
                if let Some(class) = class
                    && key != ECLASS
                    && is_default(meta.rule(class, &key), &item)
                {
                    continue;
                }
                out.insert(key, item);
            }
            Value::Object(out)
        }
        other => other,
    }
}

/// Whether a canonical value, its own defaults already dropped, is the
/// default of the rule the feature carrying it is bound to.
pub fn is_default(rule: MergeRule, value: &Value) -> bool {
    let empty_collection = |value: &Value| value.as_array().is_some_and(|items| items.is_empty());
    let empty_map = |value: &Value| value.as_object().is_some_and(|entries| entries.is_empty());
    match rule {
        // Both are dropped from the interpreted side before this runs and
        // never appear on the generated side at all.
        MergeRule::Reference { .. } | MergeRule::Unsupported { .. } => false,
        MergeRule::Attribute { shape, leaf } => match shape {
            // Present means written. See the note on the exemption above.
            Shape::Optional => false,
            Shape::Single => match leaf {
                LeafRule::Text => value.as_str() == Some(""),
                LeafRule::Counter { .. } => value.as_f64() == Some(0.0),
                LeafRule::Flag { .. } => value.as_bool() == Some(false),
                // An unwritten register or enum reads as `null`:
                // `moirai-interp/src/leaf.rs`'s `many_valued` on no values,
                // and `Scalar`'s absent case for a single-valued one.
                LeafRule::Register { .. } | LeafRule::Enum { .. } => value.is_null(),
                // Only a keyed collection's value is one, so it is never a
                // `Shape::Single` leaf and `bt.ecore` has no keyed feature at
                // all. `generated/annotated`'s oracle is what drives it.
                LeafRule::OptionalRegister { .. } => unreachable!(
                    "a register over an optional value is a keyed collection's value only"
                ),
            },
            Shape::Sequence | Shape::Set { .. } | Shape::Bag => empty_collection(value),
            // A keyed collection reads as a JSON object, so its default is
            // the empty one. `bt.ecore` reaches none — `Blackboard.entries`
            // has its `uw-map` annotation commented out — so this arm is
            // here to compile and `json.ecore`'s own oracle is what exercises
            // it (`generated/json_crdt/tests/equivalence.rs`).
            Shape::Keyed { .. } => empty_map(value),
            Shape::OrderedSet => unreachable!("`effective` degrades an ordered set to a sequence"),
        },
        MergeRule::Containment { shape, .. } => match shape {
            Shape::Optional => false,
            Shape::Single => only_a_class(value),
            Shape::Sequence | Shape::Set { .. } | Shape::Bag => empty_collection(value),
            Shape::Keyed { .. } => empty_map(value),
            Shape::OrderedSet => unreachable!("`effective` degrades an ordered set to a sequence"),
        },
    }
}

/// The interpreted read-out with every non-containment reference dropped and
/// every default-valued key with it: the two edits this oracle makes to the
/// canonical form the interpreter already produces.
pub fn canon(meta: &Meta, value: Value) -> Value {
    without_defaults(meta, strip_references(meta, value))
}

/// The interpreted read-out with every non-containment reference dropped and
/// nothing else touched: sparse exactly as `eval::read` wrote it, so a key is
/// absent precisely when nothing has been written under it.
///
/// This is the view the script generator of section 6 proposes against, and
/// it has to be the unpruned one. [`without_defaults`] spells a model whose
/// features are all still at their defaults as `null`, which is right for the
/// comparison and useless for generation: a writer looking at `null` sees no
/// object, no feature and therefore no edit it could make, and the script
/// stops after the edit that opened the model.
pub fn strip_references(meta: &Meta, value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| strip_references(meta, item))
                .collect(),
        ),
        Value::Object(map) => {
            let class = map
                .get(ECLASS)
                .and_then(Value::as_str)
                .map(|name| meta.slot(name));
            let mut out = Map::new();
            for (key, item) in map {
                if let Some(class) = class
                    && key != ECLASS
                    && matches!(meta.rule(class, &key), MergeRule::Reference { .. })
                {
                    continue;
                }
                out.insert(key, strip_references(meta, item));
            }
            Value::Object(out)
        }
        other => other,
    }
}

/// The first place two canonical documents differ, as a path and the two
/// values, so a failure is readable without a diff tool.
pub fn difference(left: &Value, right: &Value, at: &str) -> Option<String> {
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
                        return Some(format!(
                            "{here}: interpreted has {l}, generated has no key"
                        ));
                    }
                    (None, Some(r)) => {
                        return Some(format!(
                            "{here}: interpreted has no key, generated has {r}"
                        ));
                    }
                    (None, None) => {}
                }
            }
            None
        }
        (Value::Array(l), Value::Array(r)) => {
            if l.len() != r.len() {
                return Some(format!(
                    "{at}: interpreted holds {} items, generated holds {}\n  interpreted: {left}\n  generated:   {right}",
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

/// Two `twins_log` pairs, one per path, driven by one script.
pub struct Harness {
    /// The table the generated arm was compiled from, which is also the one
    /// the projection and the script generator read.
    pub gen_meta: Meta,
    /// The table the interpreted arm runs. The same one, except under
    /// `ip15`.
    pub interp_meta: Meta,
    pub ia: InterpReplica,
    pub ib: InterpReplica,
    pub ga: GenReplica,
    pub gb: GenReplica,
    /// Events not yet delivered, by the replica that wrote them.
    pub pending_a: Vec<(EventMessage<ModelOp>, EventMessage<Behaviortree>)>,
    pub pending_b: Vec<(EventMessage<ModelOp>, EventMessage<Behaviortree>)>,
    /// How many operations each edit cost, and how many were refused.
    pub ops: usize,
    pub refused: usize,
}

impl Harness {
    pub fn new(gen_descriptor: &Value, interp_descriptor: &Value) -> Harness {
        let gen_meta = Meta::new(Arc::new(
            from_descriptor(gen_descriptor).expect("the checked-in descriptor parses"),
        ));
        let interp_meta = Meta::new(Arc::new(
            from_descriptor(interp_descriptor).expect("the interpreted descriptor parses"),
        ));
        let (ia, ib) = opened("equivalence", interp_descriptor);
        let (mut ga, mut gb) = twins_log::<BehaviortreeLog>();
        // The interpreted log opens on `Install` from `a`; this is the
        // generated arm's answer to it, so that edit *n* is event *n+1* on
        // both paths and the two eg-walkers break their ties on the same
        // sequence numbers. `Root::New` writes nothing: `record!`'s `New`
        // only reports the object's arrival.
        let root = tagged(
            gen_meta.name(gen_meta.root_class()).to_upper_camel_case(),
            json!("New"),
        );
        let open: Behaviortree =
            serde_json::from_value(root).expect("the root record takes a `New`");
        let event = ga.send(open).expect("a fresh generated log takes it");
        gb.receive(event);
        Harness {
            gen_meta,
            interp_meta,
            ia,
            ib,
            ga,
            gb,
            pending_a: Vec::new(),
            pending_b: Vec::new(),
            ops: 0,
            refused: 0,
        }
    }

    pub fn interp_doc(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ia } else { &self.ib };
        canon(&self.interp_meta, replica.query(&Read::<Value>::new()))
    }

    /// What a writer sitting at one replica actually sees: the interpreted
    /// read-out with references gone and nothing else pruned. The script
    /// generator proposes against this and never against [`interp_doc`],
    /// whose whole job is to erase the difference between an object at its
    /// defaults and no object at all.
    pub fn interp_view(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ia } else { &self.ib };
        strip_references(&self.interp_meta, replica.query(&Read::<Value>::new()))
    }

    pub fn gen_doc(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ga } else { &self.gb };
        project(
            &self.gen_meta,
            &replica.query(&Read::<BehaviortreeValue>::new()),
        )
    }

    /// Both replicas of both paths, compared. `Ok` when the four read-outs
    /// are two equal pairs.
    ///
    /// The canonical projections themselves, with nothing taken off either
    /// side. The criterion's one named exception used to be pruned here first,
    /// by `except_unwritten_sequence_children`, as a step on top of the
    /// projection; there is no exception left to prune, so the comparison is
    /// the projections as [`Harness::interp_doc`] and [`Harness::gen_doc`]
    /// give them.
    pub fn compare(&self) -> Result<(), String> {
        for writer in ['a', 'b'] {
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

    /// One edit, encoded twice and sent to the two replicas of its writer,
    /// with the read-outs compared afterwards. This is what a *test* uses.
    pub fn apply(&mut self, edit: &Edit) -> Result<(), String> {
        self.carry(edit)?;
        self.compare()
    }

    /// The same, without comparing: `Ok(true)` when both intakes took the
    /// edit, `Ok(false)` when both refused it, `Err` when they disagreed.
    ///
    /// The script generator of section 6 drives its shadow through this and
    /// never through [`apply`]. Generation must stop when an edit cannot be
    /// carried — an edit the shadow silently dropped is an edit `ip13` never
    /// runs — but it must *not* stop when the two read-outs merely differ,
    /// because a difference is the finding `ip13` exists to report and a
    /// generator that halts on the first one would only ever report the
    /// first one.
    pub fn carry(&mut self, edit: &Edit) -> Result<bool, String> {
        let interp = interp_op(&self.interp_meta, edit);
        let generated = typed_op(&self.gen_meta, edit);
        self.ops += 1;
        let mut taken = false;
        let (interp_event, gen_event) = if edit.writer == 'a' {
            (self.ia.send(interp), self.ga.send(generated))
        } else {
            (self.ib.send(interp), self.gb.send(generated))
        };
        match (interp_event, gen_event) {
            (Ok(interp_event), Ok(gen_event)) => {
                let pending = if edit.writer == 'a' {
                    &mut self.pending_a
                } else {
                    &mut self.pending_b
                };
                pending.push((interp_event, gen_event));
                taken = true;
            }
            (Err(_), Err(_)) => {
                // Both intakes refused it, which is itself an equality worth
                // having: `ModelLog::is_enabled` and the generated
                // `is_enabled` agree.
                self.refused += 1;
            }
            (interp_event, _) => {
                return Err(format!(
                    "the two intakes disagree on {}: interpreted {}, generated {}",
                    edit.show(),
                    if interp_event.is_ok() {
                        "accepted"
                    } else {
                        "refused"
                    },
                    if interp_event.is_ok() {
                        "refused"
                    } else {
                        "accepted"
                    },
                ));
            }
        }
        Ok(taken)
    }

    /// Everything both writers hold, delivered to the other, `a` first, with
    /// the read-outs compared after every crossing.
    pub fn deliver(&mut self) -> Result<(), String> {
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

    /// The same crossing without comparing, for the generator's shadow.
    pub fn cross(&mut self) {
        for (interp_event, gen_event) in std::mem::take(&mut self.pending_a) {
            self.ib.receive(interp_event);
            self.gb.receive(gen_event);
        }
        for (interp_event, gen_event) in std::mem::take(&mut self.pending_b) {
            self.ia.receive(interp_event);
            self.ga.receive(gen_event);
        }
    }

    /// One whole script. `Ok(())` when every read-out after every operation
    /// was equal; the error names the edit that broke it.
    pub fn run(&mut self, script: &EditScript) -> Result<(), String> {
        for (index, step) in script.steps.iter().enumerate() {
            match step {
                Step::Edit(edit) => self.apply(edit).map_err(|reason| {
                    format!(
                        "{}: edit {index} — {}\n{reason}",
                        script.label,
                        edit.show()
                    )
                })?,
                Step::Deliver => self
                    .deliver()
                    .map_err(|reason| format!("{}: delivery after edit {index}\n{reason}", script.label))?,
            }
        }
        // Nothing may be left in flight: a script that ends divergent is a
        // script that never tested the merge.
        self.deliver()
            .map_err(|reason| format!("{}: final delivery\n{reason}", script.label))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Seeded generation over `bt.ecore`'s real shape
// ---------------------------------------------------------------------------

/// splitmix64, written out so that a seed means the same script on every
/// machine and under every version of `rand`.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, bound: usize) -> usize {
        assert!(bound > 0);
        (self.next() % bound as u64) as usize
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// One thing a writer could do to the model it is looking at.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub path: Path,
    pub class: String,
    pub feature: String,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Mint into an empty single-valued containment.
    CreateSingle,
    /// Mint into an ordered containment holding `len` children.
    CreateAt(usize),
    /// Take one of `len` children out.
    DeleteAt(usize),
    /// Put a character into a text of this length.
    InsertChar(usize),
    /// Take one out of a text of this length.
    DeleteChar(usize),
    /// Unset an optional that is currently there.
    UnsetOptional,
}

/// Everything the writer looking at `doc` could do, in a deterministic order.
pub fn candidates(meta: &Meta, doc: &Value, depth_cap: usize, room: bool) -> Vec<Candidate> {
    let mut out = Vec::new();
    if doc.is_null() {
        return out;
    }
    collect_candidates(meta, doc, &Path::default(), depth_cap, room, &mut out);
    out
}

pub fn collect_candidates(
    meta: &Meta,
    object: &Value,
    path: &Path,
    depth_cap: usize,
    room: bool,
    out: &mut Vec<Candidate>,
) {
    let Some(class_name) = object.get(ECLASS).and_then(Value::as_str) else {
        return;
    };
    let class = meta.slot(class_name);
    for (name, owner, slot) in &meta.class(class).visible {
        let Some(rule) = meta.sem.rule(*owner, *slot) else {
            continue;
        };
        let here = |kind| Candidate {
            path: path.clone(),
            class: class_name.to_string(),
            feature: name.to_string(),
            kind,
        };
        match rule {
            MergeRule::Reference { .. } | MergeRule::Unsupported { .. } => {}
            MergeRule::Attribute { shape, leaf } => {
                if !matches!(leaf, LeafRule::Text) {
                    continue;
                }
                match *shape {
                    Shape::Single => {
                        let len = object
                            .get(&**name)
                            .and_then(Value::as_str)
                            .map_or(0, |text| text.chars().count());
                        out.push(here(Kind::InsertChar(len)));
                        if len > 0 {
                            out.push(here(Kind::DeleteChar(len)));
                        }
                    }
                    Shape::Optional => match object.get(&**name).and_then(Value::as_str) {
                        None => out.push(here(Kind::InsertChar(0))),
                        Some(text) => {
                            let len = text.chars().count();
                            out.push(here(Kind::InsertChar(len)));
                            if len > 0 {
                                out.push(here(Kind::DeleteChar(len)));
                            }
                            out.push(here(Kind::UnsetOptional));
                        }
                    },
                    _ => {}
                }
            }
            MergeRule::Containment { shape, target } => match *shape {
                Shape::Single | Shape::Optional => {
                    match object.get(&**name) {
                        None => {
                            if room && path.depth() < depth_cap {
                                out.push(here(Kind::CreateSingle));
                            }
                        }
                        Some(child) => {
                            // A conflict is compared but never descended
                            // into: both paths refuse a *local* second
                            // variant (`union.rs:130-145`,
                            // `node.rs:1086-1108`), so aiming an edit at one
                            // would only test the refusal.
                            if child.get(ECLASS).is_some() {
                                collect_candidates(
                                    meta,
                                    child,
                                    &path.child(Hop {
                                        feature: name.to_string(),
                                        at: None,
                                        class: child[ECLASS].as_str().unwrap_or("").to_string(),
                                    }),
                                    depth_cap,
                                    room,
                                    out,
                                );
                            }
                        }
                    }
                    let _ = target;
                }
                Shape::Sequence => {
                    let items = object.get(&**name).and_then(Value::as_array);
                    let len = items.map_or(0, Vec::len);
                    if room && path.depth() < depth_cap {
                        out.push(here(Kind::CreateAt(len)));
                    }
                    if len > 0 {
                        out.push(here(Kind::DeleteAt(len)));
                    }
                    for (index, child) in items.into_iter().flatten().enumerate() {
                        if child.get(ECLASS).is_some() {
                            collect_candidates(
                                meta,
                                child,
                                &path.child(Hop {
                                    feature: name.to_string(),
                                    at: Some(index),
                                    class: child[ECLASS].as_str().unwrap_or("").to_string(),
                                }),
                                depth_cap,
                                room,
                                out,
                            );
                        }
                    }
                }
                _ => {}
            },
        }
    }
}

/// How many objects a canonical document holds, which is what caps a script's
/// growth.
pub fn object_count(doc: &Value) -> usize {
    match doc {
        Value::Object(map) => {
            let here = usize::from(map.contains_key(ECLASS));
            here + map.values().map(object_count).sum::<usize>()
        }
        Value::Array(items) => items.iter().map(object_count).sum(),
        _ => 0,
    }
}

/// One edit, drawn from what the writer can see.
pub fn propose(meta: &Meta, doc: &Value, rng: &mut Rng, writer: char, id: &mut u32) -> Option<Edit> {
    const ALPHABET: [char; 6] = ['a', 'b', 'c', 'd', 'e', 'f'];
    const OBJECT_CAP: usize = 22;
    const DEPTH_CAP: usize = 5;

    let room = object_count(doc) < OBJECT_CAP;
    let candidates = candidates(meta, doc, DEPTH_CAP, room);
    if candidates.is_empty() {
        return None;
    }
    let candidate = candidates[rng.below(candidates.len())].clone();
    let class = meta.slot(&candidate.class);
    let text_at = |path: &Path, feature: &str| -> String {
        let mut at = doc;
        for hop in &path.hops {
            at = match hop.at {
                Some(index) => &at[&hop.feature][index],
                None => &at[&hop.feature],
            };
        }
        at.get(feature)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let action = match candidate.kind {
        Kind::CreateSingle | Kind::CreateAt(_) => {
            let MergeRule::Containment { target, .. } = meta.rule(class, &candidate.feature) else {
                unreachable!("a create candidate is a containment")
            };
            let allowed = &meta.class(target).concrete;
            if allowed.is_empty() {
                return None;
            }
            let made = allowed[rng.below(allowed.len())];
            Action::Create {
                feature: candidate.feature.clone(),
                pos: match candidate.kind {
                    Kind::CreateAt(len) => Some(rng.below(len + 1)),
                    _ => None,
                },
                class: meta.name(made).to_string(),
            }
        }
        Kind::DeleteAt(len) => Action::Delete {
            feature: candidate.feature.clone(),
            pos: rng.below(len),
        },
        Kind::InsertChar(len) => {
            let pos = rng.below(len + 1);
            let ch = *rng.pick(&ALPHABET);
            let mut after: Vec<char> = text_at(&candidate.path, &candidate.feature).chars().collect();
            after.insert(pos.min(after.len()), ch);
            Action::Text {
                feature: candidate.feature.clone(),
                op: TextOp::Insert {
                    pos,
                    ch,
                    after: after.into_iter().collect(),
                },
            }
        }
        Kind::DeleteChar(len) => {
            let pos = rng.below(len);
            let mut after: Vec<char> = text_at(&candidate.path, &candidate.feature).chars().collect();
            if pos < after.len() {
                after.remove(pos);
            }
            Action::Text {
                feature: candidate.feature.clone(),
                op: TextOp::Delete {
                    pos,
                    after: after.into_iter().collect(),
                },
            }
        }
        Kind::UnsetOptional => Action::Unset {
            feature: candidate.feature.clone(),
        },
    };
    if matches!(action, Action::Create { .. }) {
        *id += 1;
    }
    Some(Edit {
        id: *id,
        writer,
        path: candidate.path,
        action,
    })
}

/// The edit every script opens with: the root object and the mandatory
/// children the generated path materialises the moment its log exists.
///
/// Without it the very first comparison fails for a reason that is not a
/// defect — the generated `RootValue` always carries a `main`, and a `main`
/// always carries a `blackboard`, while the interpreted path has no object
/// anywhere until an operation makes one.
pub fn open_the_model(meta: &Meta) -> Vec<Edit> {
    let root = meta.root_class();
    meta.mandatory(root)
        .into_iter()
        .map(|(feature, target)| Edit {
            id: 1,
            writer: 'a',
            path: Path::default(),
            action: Action::Create {
                feature,
                pos: None,
                class: meta.name(target).to_string(),
            },
        })
        .collect()
}

/// Ten sequential scripts and twenty concurrent ones, over `bt.ecore`.
pub fn seeded_script(meta: &Meta, seed: u64, concurrent: bool) -> EditScript {
    let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x1234_5678);
    let mut steps: Vec<Step> = open_the_model(meta)
        .into_iter()
        .map(Step::Edit)
        .collect();
    steps.push(Step::Deliver);
    let mut id = 1;

    // The generator has to see what the writer sees, so it runs against a
    // shadow pair driven by the same edits. The shadow is the interpreted
    // path, whose read-out *is* the canonical form; the harness then asserts
    // the generated one equals it at every step, which is the claim.
    let mut shadow = Harness::new(&bt_descriptor(), &bt_descriptor());
    for step in &steps {
        match step {
            Step::Edit(edit) => {
                shadow.carry(edit).expect("the opening edit applies");
            }
            Step::Deliver => shadow.cross(),
        }
    }

    // An edit the shadow could not carry is *kept* and generation stops
    // there: dropping it is how an oracle quietly stops testing the thing it
    // was written for, and the run in `ip13` has to meet the same edit and
    // say so. The shadow is carried, never compared — `ip13` does the
    // comparing, on a fresh harness, and a generator that stopped at the
    // first disagreement would hide every one after it.
    let mut broken = false;
    if concurrent {
        for _ in 0..3 {
            if broken {
                break;
            }
            let edits = 2 + rng.below(3);
            for _ in 0..edits {
                for writer in ['a', 'b'] {
                    if broken {
                        break;
                    }
                    let doc = shadow.interp_view(writer);
                    if let Some(edit) = propose(meta, &doc, &mut rng, writer, &mut id) {
                        broken = shadow.carry(&edit).is_err();
                        steps.push(Step::Edit(edit));
                    }
                }
            }
            shadow.cross();
            steps.push(Step::Deliver);
        }
    } else {
        for index in 0..24 {
            if broken {
                break;
            }
            let writer = if index % 3 == 0 { 'b' } else { 'a' };
            let doc = shadow.interp_view(writer);
            if let Some(edit) = propose(meta, &doc, &mut rng, writer, &mut id) {
                broken = shadow.carry(&edit).is_err();
                steps.push(Step::Edit(edit));
                steps.push(Step::Deliver);
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

pub fn bt_descriptor() -> Value {
    serde_json::from_str(BT_DESCRIPTOR).expect("the fixture is JSON")
}

/// `bt.metamodel.json` with `TreeNode.ID` bound to a multi-value register
/// instead of a text: the mutation `ip15` demands the oracle catch.
pub fn register_id_descriptor() -> Value {
    let mut descriptor = bt_descriptor();
    let attributes = descriptor["classes"]["TreeNode"]["attributes"]
        .as_array_mut()
        .expect("`TreeNode` has attributes");
    let id = attributes
        .iter_mut()
        .find(|attribute| attribute["name"] == json!("ID"))
        .expect("`TreeNode` declares `ID`");
    id["merge"] = json!({
        "kind": "attribute",
        "shape": {"kind": "single"},
        "leaf": {"kind": "register", "tie": "mv"}
    });
    descriptor
}
