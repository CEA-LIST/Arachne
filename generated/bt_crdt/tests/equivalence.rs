//! The equivalence oracle: `ip13`, `ip14`'s shape and `ip15` of
//! [`02 Validation Plan`], step 5 of the implementation plan.
//!
//! # What is being claimed
//!
//! Criterion I-A1: for the same model edits applied in the same causal
//! order, a replica running the interpreted `ModelLog` over `bt.ecore`'s
//! table and a replica running this generated crate reach read-outs that are
//! equal under the canonical projection of the validation plan's section 2.
//! Criterion I-A2: the harness below fails when the two paths genuinely
//! differ, which `ip15` shows by binding `TreeNode.ID` to a multi-value
//! register on the interpreted side alone and demanding an inequality.
//!
//! # How the two paths are driven
//!
//! Four replicas: `a` and `b` of the interpreted `ModelLog`, `a` and `b` of
//! the generated `BehaviortreeLog`. One [`Edit`] is encoded twice, once into
//! a [`ModelOp`] and once into this crate's typed operation, and the two
//! encodings are handed to the two `a` replicas — or the two `b` replicas —
//! in the same order, with delivery under the test's control and identical
//! on both paths. The read-outs are compared **after every single
//! operation**, not only at the end, so a failure names the edit that broke
//! it rather than the script that contained it.
//!
//! Replica names are `a` and `b` on both paths and `EventId::cmp` orders by
//! replica name first (`event/id.rs:91-98`), so the two paths break their
//! concurrency ties the same way. The sequence numbers are lined up as well:
//! the interpreted log opens on a `ModelOp::Install` from `a`, so the
//! generated pair is opened with one `Root::New`, which writes nothing and
//! leaves the log default, purely so that edit *n* is event *n+1* on both
//! paths.
//!
//! # The typed encoder
//!
//! The generated operation is built as JSON and then deserialized into
//! `Behaviortree`, which is what the implementation plan asks for ("typed
//! `bt_crdt` operation JSON by the naming convention the generator uses").
//! Nothing about `bt.ecore` is hard-coded: the `<Super>Super` hops come from
//! the class table's `supers`, the `union!` variant chain from its subclass
//! relation, and the field names from `heck`, which is the crate the
//! generator itself names its fields with. A wrong shape is a
//! `serde_json::from_value` error naming the enum it could not build, not a
//! silent pass.
//!
//! # What the projection excludes, and why
//!
//! - **`DataFlowPort.entry`**, and every other non-containment reference.
//!   The interpreted path carries a reference as a string (design §8): a
//!   multi-value register for a single one, an add-wins set for a many.
//!   The generated path carries none of that in the record at all — a
//!   non-containment reference is a `typed_graph!` arc in a second log
//!   (`references.rs`), and `BehaviortreeValue::refs` is `serde(skip)`.
//!   There is no value on the generated side to compare against, so the
//!   projection drops the key from the interpreted side and the scripts
//!   never write one. This is the one feature of `bt.ecore` the oracle does
//!   not cover.
//! - **`Status`**, which no feature of `bt.ecore` reaches, so it appears on
//!   neither side. Stated by the validation plan §2 and true here.
//! - **Every value that is the default of its feature's rule**, dropped from
//!   both sides by the same function ([`without_defaults`]) before they are
//!   compared. This is the rule §2 was missing. `record!`'s `new` builds one
//!   field per feature eagerly, so a generated log that exists renders its
//!   whole shape — `Root.behaviortrees` as `[]`, a never-written
//!   `BehaviorTree.ID` as `""` — and a generated log renders that shape from
//!   the moment the log is constructed, before any operation at all. The
//!   interpreted path has no object anywhere until an operation mints one and
//!   reads `null` for the whole model until then. A never-written required
//!   attribute is semantically absent whichever path renders it, so the
//!   projection spells it absent on both, and a root every one of whose
//!   features is at its default is spelled `null` on both. Neither path is
//!   wrong and the difference is not observable in the canonical form.
//!
//!   What this costs is stated rather than hidden: the oracle cannot tell an
//!   object all of whose features are default from no object at all. It is
//!   not free to widen — an *optional* is exempt, because an optional's
//!   default is absence and absence already carries no key, so a value
//!   written and then emptied (`TreeNode.name` set and then unset, which both
//!   paths leave present and empty) stays present on both sides and is
//!   compared. [`an_emptied_optional_is_not_dropped`] is the test that keeps
//!   that honest.
//! - Nothing else. In particular the other structural difference — the
//!   generated path materialising a single-valued containment whose target
//!   has no subclasses (`Root.main`, `BehaviorTree.blackboard`,
//!   `SubTree.tree`) while the interpreted path mints an object only when
//!   something is written into it — is handled where it belongs, in the
//!   script: creating an object also mints every such mandatory child of it,
//!   in the same operation, so both paths hold the same objects.

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

type InterpReplica = Replica<ModelLog, Tcsb<ModelOp>>;
type GenReplica = Replica<BehaviortreeLog, Tcsb<Behaviortree>>;

/// The key the canonical form carries a class name under.
const ECLASS: &str = "eClass";
/// The key the canonical form carries a conflict set under.
const CONFLICT: &str = "__conflict";

// ---------------------------------------------------------------------------
// 1. The metamodel, read the way both paths read it
// ---------------------------------------------------------------------------

/// A [`MetamodelSemantics`] plus the two derived relations the encoders need:
/// who inherits from whom, and which class the generator would have wrapped
/// in a `union!`.
struct Meta {
    sem: Arc<MetamodelSemantics>,
    /// Direct subclasses, by declaring class slot.
    subs: BTreeMap<u16, Vec<ClassSlot>>,
}

impl Meta {
    fn new(sem: Arc<MetamodelSemantics>) -> Self {
        let mut subs: BTreeMap<u16, Vec<ClassSlot>> = BTreeMap::new();
        for class in &sem.classes {
            for sup in &class.supers {
                subs.entry(sup.0).or_default().push(class.slot);
            }
        }
        Meta { sem, subs }
    }

    fn class(&self, slot: ClassSlot) -> &ClassSemantics {
        &self.sem.classes[slot.index()]
    }

    fn name(&self, slot: ClassSlot) -> &str {
        &self.class(slot).name
    }

    fn slot(&self, name: &str) -> ClassSlot {
        self.sem
            .classes
            .iter()
            .find(|class| &*class.name == name)
            .unwrap_or_else(|| panic!("no class `{name}` in the table"))
            .slot
    }

    fn subs(&self, slot: ClassSlot) -> &[ClassSlot] {
        self.subs.get(&slot.0).map_or(&[], Vec::as_slice)
    }

    /// The class the model root is declared as. Every descriptor this test
    /// drives names exactly one.
    fn root_class(&self) -> ClassSlot {
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
    fn is_union(&self, slot: ClassSlot) -> bool {
        !self.class(slot).instantiable || !self.subs(slot).is_empty()
    }

    /// The visible slot of one feature on one class: what an
    /// `InstanceOp::Field` carries.
    fn visible_slot(&self, class: ClassSlot, feature: &str) -> FeatureSlot {
        let holder = self.class(class);
        let index = holder
            .visible
            .iter()
            .position(|(name, _, _)| &**name == feature)
            .unwrap_or_else(|| panic!("`{}` cannot see `{feature}`", holder.name));
        FeatureSlot(index as u16)
    }

    /// The class that declares one visible feature, and its rule.
    fn declared(&self, class: ClassSlot, feature: &str) -> (ClassSlot, MergeRule) {
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

    fn rule(&self, class: ClassSlot, feature: &str) -> MergeRule {
        self.declared(class, feature).1
    }

    /// The chain of classes from `from` up to `to`, `to` included and `from`
    /// excluded: one `<Super>Super` hop per entry on the generated path, and
    /// nothing at all on the interpreted one.
    fn super_path(&self, from: ClassSlot, to: ClassSlot) -> Vec<ClassSlot> {
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
    fn union_descent(&self, target: ClassSlot, concrete: ClassSlot) -> Vec<String> {
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
    fn mandatory(&self, class: ClassSlot) -> Vec<(String, ClassSlot)> {
        self.class(class)
            .visible
            .iter()
            .filter_map(|(name, owner, slot)| {
                let rule = self.sem.rule(*owner, *slot)?;
                match rule {
                    MergeRule::Containment { shape, target }
                        if shape.effective() == Shape::Single && !self.is_union(*target) =>
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
    fn mint_chain(&self, class: ClassSlot) -> Vec<(String, ClassSlot)> {
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
fn tagged(variant: impl Into<String>, payload: Value) -> Value {
    let mut map = Map::new();
    map.insert(variant.into(), payload);
    Value::Object(map)
}

fn field_of(feature: &str) -> String {
    feature.to_snake_case()
}

fn variant_of(feature: &str) -> String {
    field_of(feature).to_upper_camel_case()
}

/// The `<Super>Super` field the generator emits for an inherited class
/// (`classifier/mod.rs:62-70`), as its `record!` variant.
fn super_variant(class: &str) -> String {
    format!("{}_super", class.to_snake_case()).to_upper_camel_case()
}

// ---------------------------------------------------------------------------
// 2. The edit script
// ---------------------------------------------------------------------------

/// One step of a path from the model root: which feature, at which position
/// when the feature is ordered, and the concrete class of the object landed
/// on.
#[derive(Clone, Debug, PartialEq)]
struct Hop {
    feature: String,
    at: Option<usize>,
    class: String,
}

/// Where an edit happens: the root class, then the hops down to the object
/// the edit addresses.
#[derive(Clone, Debug, PartialEq, Default)]
struct Path {
    hops: Vec<Hop>,
}

impl Path {
    fn child(&self, hop: Hop) -> Path {
        let mut hops = self.hops.clone();
        hops.push(hop);
        Path { hops }
    }

    fn depth(&self) -> usize {
        self.hops.len()
    }

    fn show(&self) -> String {
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
enum Action {
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
enum TextOp {
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
struct Edit {
    /// Script-local object id: `0` is the root, and every `Create` mints the
    /// next one for the object it makes.
    id: u32,
    /// Which replica issues it.
    writer: char,
    path: Path,
    action: Action,
}

impl Edit {
    fn show(&self) -> String {
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
struct EditScript {
    label: String,
    steps: Vec<Step>,
}

#[derive(Clone, Debug)]
enum Step {
    Edit(Edit),
    /// Deliver everything both writers are holding, `a`'s events first.
    Deliver,
}

// ---------------------------------------------------------------------------
// 3. The two encoders
// ---------------------------------------------------------------------------

/// The class each hop of a path lands on, the root class first.
fn classes_along(meta: &Meta, path: &Path) -> Vec<ClassSlot> {
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
fn interp_op(meta: &Meta, edit: &Edit) -> ModelOp {
    let classes = classes_along(meta, &edit.path);
    let leaf_class = *classes.last().expect("the root is always there");
    let mut op = interp_action(meta, leaf_class, &edit.action);
    for (index, hop) in edit.path.hops.iter().enumerate().rev() {
        let parent = classes[index];
        let mut step = InstanceOp::variant(meta.slot(&hop.class), op);
        step = match (hop.at, meta.rule(parent, &hop.feature)) {
            (Some(pos), _) => InstanceOp::at(pos, step),
            (None, MergeRule::Containment { shape, .. }) if shape.effective() == Shape::Optional => {
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
fn interp_mint(meta: &Meta, class: ClassSlot) -> InstanceOp {
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

fn interp_action(meta: &Meta, class: ClassSlot, action: &Action) -> InstanceOp {
    match action {
        Action::Create { feature, pos, class: made } => {
            let made = meta.slot(made);
            let inner = InstanceOp::variant(made, interp_mint(meta, made));
            let shaped = match meta.rule(class, feature) {
                MergeRule::Containment { shape, .. } => shape.effective(),
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
                MergeRule::Attribute { shape, leaf } => (shape.effective(), leaf),
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
fn typed_op(meta: &Meta, edit: &Edit) -> Behaviortree {
    let value = typed_json(meta, edit);
    serde_json::from_value(value.clone()).unwrap_or_else(|error| {
        panic!(
            "the typed encoder built an operation `Behaviortree` cannot take: {error}\n{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        )
    })
}

fn typed_json(meta: &Meta, edit: &Edit) -> Value {
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
            (None, MergeRule::Containment { shape, .. }) if shape.effective() == Shape::Optional => {
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
fn feature_wrap(meta: &Meta, class: ClassSlot, feature: &str, payload: Value) -> Value {
    let (owner, _) = meta.declared(class, feature);
    let mut value = tagged(variant_of(feature), payload);
    for hop in meta.super_path(class, owner).iter().rev() {
        value = tagged(super_variant(meta.name(*hop)), value);
    }
    value
}

/// Wrap an operation on the record of `concrete` in the `union!` variants
/// that carry it from the containment's declared `target` down to it.
fn union_wrap(meta: &Meta, target: ClassSlot, concrete: ClassSlot, payload: Value) -> Value {
    let mut value = payload;
    for variant in meta.union_descent(target, concrete).iter().rev() {
        value = tagged(variant.clone(), value);
    }
    value
}

/// The typed payload that mints an object of `class` with its mandatory
/// children: `record!`'s `New`, wrapped in one feature per mandatory hop.
fn typed_mint(meta: &Meta, class: ClassSlot) -> Value {
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

fn typed_action(meta: &Meta, class: ClassSlot, action: &Action) -> Value {
    match action {
        Action::Create { feature, pos, class: made } => {
            let made = meta.slot(made);
            let (shape, target) = match meta.rule(class, feature) {
                MergeRule::Containment { shape, target } => (shape.effective(), target),
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
                MergeRule::Attribute { shape, .. } => shape.effective(),
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
fn project(meta: &Meta, value: &BehaviortreeValue) -> Value {
    let raw = serde_json::to_value(value).expect("the generated read-out serializes");
    let root = raw
        .get(field_of(meta.name(meta.root_class())))
        .unwrap_or_else(|| panic!("the package value carries the root under its field"));
    without_defaults(meta, project_object(meta, meta.root_class(), root))
}

fn project_object(meta: &Meta, class: ClassSlot, value: &Value) -> Value {
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
                let projected = match (shape.effective(), leaf) {
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
                match shape.effective() {
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
fn project_slot(meta: &Meta, target: ClassSlot, value: &Value) -> Option<Value> {
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

fn collect_union(meta: &Meta, target: ClassSlot, value: &Value, out: &mut Vec<(String, Value)>) {
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

fn collect_child(meta: &Meta, target: ClassSlot, value: &Value, out: &mut Vec<(String, Value)>) {
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
fn locate<'a>(
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
fn chars(value: &Value) -> Value {
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
fn without_defaults(meta: &Meta, value: Value) -> Value {
    let value = drop_defaults(meta, value);
    if only_a_class(&value) {
        Value::Null
    } else {
        value
    }
}

/// An object carrying nothing but its class name.
fn only_a_class(value: &Value) -> bool {
    matches!(value, Value::Object(map) if map.len() == 1 && map.contains_key(ECLASS))
}

fn drop_defaults(meta: &Meta, value: Value) -> Value {
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
fn is_default(rule: MergeRule, value: &Value) -> bool {
    let empty_collection = |value: &Value| value.as_array().is_some_and(|items| items.is_empty());
    match rule {
        // Both are dropped from the interpreted side before this runs and
        // never appear on the generated side at all.
        MergeRule::Reference { .. } | MergeRule::Unsupported { .. } => false,
        MergeRule::Attribute { shape, leaf } => match shape.effective() {
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
            },
            Shape::Sequence | Shape::Set { .. } | Shape::Bag => empty_collection(value),
            Shape::OrderedSet => unreachable!("`effective` degrades an ordered set to a sequence"),
        },
        MergeRule::Containment { shape, .. } => match shape.effective() {
            Shape::Optional => false,
            Shape::Single => only_a_class(value),
            Shape::Sequence | Shape::Set { .. } | Shape::Bag => empty_collection(value),
            Shape::OrderedSet => unreachable!("`effective` degrades an ordered set to a sequence"),
        },
    }
}

/// The interpreted read-out with every non-containment reference dropped and
/// every default-valued key with it: the two edits this oracle makes to the
/// canonical form the interpreter already produces.
fn canon(meta: &Meta, value: Value) -> Value {
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
fn strip_references(meta: &Meta, value: Value) -> Value {
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
struct Harness {
    /// The table the generated arm was compiled from, which is also the one
    /// the projection and the script generator read.
    gen_meta: Meta,
    /// The table the interpreted arm runs. The same one, except under
    /// `ip15`.
    interp_meta: Meta,
    ia: InterpReplica,
    ib: InterpReplica,
    ga: GenReplica,
    gb: GenReplica,
    /// Events not yet delivered, by the replica that wrote them.
    pending_a: Vec<(EventMessage<ModelOp>, EventMessage<Behaviortree>)>,
    pending_b: Vec<(EventMessage<ModelOp>, EventMessage<Behaviortree>)>,
    /// How many operations each edit cost, and how many were refused.
    ops: usize,
    refused: usize,
}

impl Harness {
    fn new(gen_descriptor: &Value, interp_descriptor: &Value) -> Harness {
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

    fn interp_doc(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ia } else { &self.ib };
        canon(&self.interp_meta, replica.query(Read::<Value>::new()))
    }

    /// What a writer sitting at one replica actually sees: the interpreted
    /// read-out with references gone and nothing else pruned. The script
    /// generator proposes against this and never against [`interp_doc`],
    /// whose whole job is to erase the difference between an object at its
    /// defaults and no object at all.
    fn interp_view(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ia } else { &self.ib };
        strip_references(&self.interp_meta, replica.query(Read::<Value>::new()))
    }

    fn gen_doc(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ga } else { &self.gb };
        project(
            &self.gen_meta,
            &replica.query(Read::<BehaviortreeValue>::new()),
        )
    }

    /// Both replicas of both paths, compared. `Ok` when the four read-outs
    /// are two equal pairs.
    fn compare(&self) -> Result<(), String> {
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
    fn apply(&mut self, edit: &Edit) -> Result<(), String> {
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
    fn carry(&mut self, edit: &Edit) -> Result<bool, String> {
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
            (Some(interp_event), Some(gen_event)) => {
                let pending = if edit.writer == 'a' {
                    &mut self.pending_a
                } else {
                    &mut self.pending_b
                };
                pending.push((interp_event, gen_event));
                taken = true;
            }
            (None, None) => {
                // Both intakes refused it, which is itself an equality worth
                // having: `ModelLog::is_enabled` and the generated
                // `is_enabled` agree.
                self.refused += 1;
            }
            (interp_event, _) => {
                return Err(format!(
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
                ));
            }
        }
        Ok(taken)
    }

    /// Everything both writers hold, delivered to the other, `a` first, with
    /// the read-outs compared after every crossing.
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

    /// The same crossing without comparing, for the generator's shadow.
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

    /// One whole script. `Ok(())` when every read-out after every operation
    /// was equal; the error names the edit that broke it.
    fn run(&mut self, script: &EditScript) -> Result<(), String> {
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

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// One thing a writer could do to the model it is looking at.
#[derive(Clone, Debug)]
struct Candidate {
    path: Path,
    class: String,
    feature: String,
    kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
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
fn candidates(meta: &Meta, doc: &Value, depth_cap: usize, room: bool) -> Vec<Candidate> {
    let mut out = Vec::new();
    if doc.is_null() {
        return out;
    }
    collect_candidates(meta, doc, &Path::default(), depth_cap, room, &mut out);
    out
}

fn collect_candidates(
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
                match shape.effective() {
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
            MergeRule::Containment { shape, target } => match shape.effective() {
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
fn object_count(doc: &Value) -> usize {
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
fn propose(meta: &Meta, doc: &Value, rng: &mut Rng, writer: char, id: &mut u32) -> Option<Edit> {
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
fn open_the_model(meta: &Meta) -> Vec<Edit> {
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
fn seeded_script(meta: &Meta, seed: u64, concurrent: bool) -> EditScript {
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

fn bt_descriptor() -> Value {
    serde_json::from_str(BT_DESCRIPTOR).expect("the fixture is JSON")
}

/// `bt.metamodel.json` with `TreeNode.ID` bound to a multi-value register
/// instead of a text: the mutation `ip15` demands the oracle catch.
fn register_id_descriptor() -> Value {
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

// ---------------------------------------------------------------------------
// 7. The tests
// ---------------------------------------------------------------------------

/// The descriptor the interpreted path runs is byte-for-byte the one this
/// crate was generated from. If it ever is not, everything below compares two
/// metamodels rather than two paths.
#[test]
fn the_two_arms_hold_the_same_metamodel() {
    let checked_in: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/metamodel.json"))
            .expect("the generated crate ships its descriptor"),
    )
    .expect("the descriptor is JSON");
    assert_eq!(
        checked_in,
        bt_descriptor(),
        "`moirai-interp`'s fixture and this crate's `metamodel.json` have drifted"
    );
}

/// `ip13`: thirty seeded edit scripts over `bt.ecore`, ten sequential and
/// twenty with two writers diverging and merging, every one equal under the
/// canonical projection after every operation.
///
/// **This test fails, and the failure is the finding.** Every one of the
/// thirty scripts differs, and every difference is the one inequality
/// [`a_sequence_child_with_nothing_written_is_invisible_on_the_generated_path`]
/// pins and [`the_thirty_scripts_find_exactly_one_kind_of_difference`] shows
/// to be the only one: an object created into an ordered containment and not
/// yet written into is on the interpreted read-out and absent from the
/// generated one, because `UWMapLog`'s read drops a child whose value equals
/// the default and `moirai-interp`'s sequence read does not. I-A1 does not
/// hold at this tip and it is one rule in one function away from holding.
/// The fix is in Moirai, not here, so this test says so rather than being
/// widened until it passes.
#[test]
fn ip13_thirty_seeded_scripts_over_bt_ecore_agree() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let mut failures = Vec::new();
    let mut edits = 0;
    let mut ops = 0;
    let mut refused = 0;
    for seed in 0..30u64 {
        let concurrent = seed >= 10;
        let script = seeded_script(&meta, seed, concurrent);
        edits += script
            .steps
            .iter()
            .filter(|step| matches!(step, Step::Edit(_)))
            .count();
        let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
        if let Err(reason) = harness.run(&script) {
            failures.push(reason);
        }
        ops += harness.ops;
        refused += harness.refused;
    }
    assert!(
        edits > 30 * 10,
        "thirty scripts came to only {edits} edits; the generator stopped early"
    );
    assert!(
        failures.is_empty(),
        "{} of thirty scripts differ. Every one of them is the empty sequence \
         child: an object created into an ordered containment and not yet \
         written into, which `moirai-interp`'s `eval::read_node` renders and \
         `UWMapLog`'s read drops. See \
         `a_sequence_child_with_nothing_written_is_invisible_on_the_generated_path` \
         for the reproducer and \
         `the_thirty_scripts_find_exactly_one_kind_of_difference` for the proof \
         that nothing else is behind it.\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
    println!(
        "ip13: 30 scripts, {edits} edits, {ops} operations, {refused} refused by both intakes"
    );
}

/// The story the validation plan tells under I-A1, written out rather than
/// drawn: two people concurrently rename the same `Sequence` and each add a
/// child to its `children`.
#[test]
fn ip13_the_validation_plans_own_story() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let script = rename_and_add_script(&meta);
    let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
    harness.run(&script).expect("the two paths agree");

    // And what they agree on is the merge, not an empty document.
    let doc = harness.interp_doc('a');
    let sequence = &doc["main"]["child"];
    assert_eq!(sequence[ECLASS], json!("Sequence"));
    assert_eq!(
        sequence["name"].as_str().expect("a name").chars().count(),
        2,
        "both renames survived, character by character: {}",
        sequence["name"]
    );
    assert_eq!(
        sequence["children"]
            .as_array()
            .expect("children read as an array")
            .len(),
        2,
        "both children survived"
    );
}

fn rename_and_add_script(meta: &Meta) -> EditScript {
    let mut steps: Vec<Step> = open_the_model(meta).into_iter().map(Step::Edit).collect();
    let main = Path::default().child(Hop {
        feature: "main".to_string(),
        at: None,
        class: "BehaviorTree".to_string(),
    });
    let sequence = main.child(Hop {
        feature: "child".to_string(),
        at: None,
        class: "Sequence".to_string(),
    });
    steps.push(Step::Edit(Edit {
        id: 2,
        writer: 'a',
        path: main.clone(),
        action: Action::Create {
            feature: "child".to_string(),
            pos: None,
            class: "Sequence".to_string(),
        },
    }));
    steps.push(Step::Deliver);
    // Both writers rename the same `Sequence` and add a child to it, each
    // seeing only its own edit.
    for (writer, ch, class) in [('a', 'x', "Fallback"), ('b', 'y', "Inverter")] {
        steps.push(Step::Edit(Edit {
            id: 2,
            writer,
            path: sequence.clone(),
            action: Action::Text {
                feature: "name".to_string(),
                op: TextOp::Insert {
                    pos: 0,
                    ch,
                    after: ch.to_string(),
                },
            },
        }));
        steps.push(Step::Edit(Edit {
            id: 3,
            writer,
            path: sequence.clone(),
            action: Action::Create {
                feature: "children".to_string(),
                pos: Some(0),
                class: class.to_string(),
            },
        }));
    }
    steps.push(Step::Deliver);
    EditScript {
        label: "the I-A1 story".to_string(),
        steps,
    }
}

/// `ip15`, the control: the same script, run twice. Once with both paths
/// holding `bt.ecore`'s own table, where it must be equal; once with
/// `TreeNode.ID` bound to a multi-value register on the interpreted side
/// alone, where the harness **must** report inequality.
///
/// Without the second half, I-A1 rests on an oracle that has never been seen
/// to fail.
#[test]
fn ip15_the_oracle_reports_a_difference_when_there_is_one() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let script = concurrent_id_script(&meta);

    let mut faithful = Harness::new(&bt_descriptor(), &bt_descriptor());
    faithful
        .run(&script)
        .expect("with the same table on both sides the two paths agree");

    let mut mutated = Harness::new(&bt_descriptor(), &register_id_descriptor());
    let verdict = mutated.run(&script);
    let reason = verdict.expect_err(
        "`TreeNode.ID` bound to a multi-value register on the interpreted side alone \
         is a genuine difference; an oracle that reports equality here is worthless",
    );
    assert!(
        reason.contains("ID"),
        "the failure should name the feature that was rebound: {reason}"
    );
    println!("ip15: the oracle reported the mutation —\n{reason}");
}

/// Two writers appending to the same `TreeNode.ID` while divergent: a text
/// leaf merges them character by character, a multi-value register keeps
/// both and reads out a conflict.
fn concurrent_id_script(meta: &Meta) -> EditScript {
    let mut steps: Vec<Step> = open_the_model(meta).into_iter().map(Step::Edit).collect();
    let main = Path::default().child(Hop {
        feature: "main".to_string(),
        at: None,
        class: "BehaviorTree".to_string(),
    });
    let sequence = main.child(Hop {
        feature: "child".to_string(),
        at: None,
        class: "Sequence".to_string(),
    });
    steps.push(Step::Edit(Edit {
        id: 2,
        writer: 'a',
        path: main,
        action: Action::Create {
            feature: "child".to_string(),
            pos: None,
            class: "Sequence".to_string(),
        },
    }));
    steps.push(Step::Edit(Edit {
        id: 2,
        writer: 'a',
        path: sequence.clone(),
        action: Action::Text {
            feature: "ID".to_string(),
            op: TextOp::Insert {
                pos: 0,
                ch: 's',
                after: "s".to_string(),
            },
        },
    }));
    steps.push(Step::Deliver);
    for (writer, ch) in [('a', 'a'), ('b', 'b')] {
        steps.push(Step::Edit(Edit {
            id: 2,
            writer,
            path: sequence.clone(),
            action: Action::Text {
                feature: "ID".to_string(),
                op: TextOp::Insert {
                    pos: 1,
                    ch,
                    after: format!("s{ch}"),
                },
            },
        }));
    }
    steps.push(Step::Deliver);
    EditScript {
        label: "two writers on one `TreeNode.ID`".to_string(),
        steps,
    }
}

/// The encoders, checked against the shapes the generator actually emitted,
/// so that a wrong hop is a failure here and not a puzzling inequality later.
#[test]
fn the_typed_encoder_walks_the_super_hops_and_the_union_variants() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let edit = Edit {
        id: 1,
        writer: 'a',
        path: Path::default()
            .child(Hop {
                feature: "main".to_string(),
                at: None,
                class: "BehaviorTree".to_string(),
            })
            .child(Hop {
                feature: "child".to_string(),
                at: None,
                class: "Sequence".to_string(),
            }),
        action: Action::Text {
            feature: "ID".to_string(),
            op: TextOp::Insert {
                pos: 0,
                ch: 'q',
                after: "q".to_string(),
            },
        },
    };
    assert_eq!(
        typed_json(&meta, &edit),
        json!({
            "Root": { "Main": { "Child": { "ControlNode": { "Sequence": {
                "ControlNodeSuper": { "TreeNodeSuper": {
                    "Id": { "Insert": { "content": "q", "pos": 0 } }
                } }
            } } } } }
        }),
        "`Sequence` sees `ID` through `control_node_super` and then `tree_node_super`"
    );
    // And the interpreted encoding of the same edit has no hop at all.
    assert_eq!(
        interp_op(&meta, &edit),
        ModelOp::Instance(InstanceOp::variant(
            meta.slot("Root"),
            InstanceOp::field(
                meta.visible_slot(meta.slot("Root"), "main"),
                InstanceOp::variant(
                    meta.slot("BehaviorTree"),
                    InstanceOp::field(
                        meta.visible_slot(meta.slot("BehaviorTree"), "child"),
                        InstanceOp::variant(
                            meta.slot("Sequence"),
                            InstanceOp::field(
                                meta.visible_slot(meta.slot("Sequence"), "ID"),
                                InstanceOp::Leaf(LeafOp::InsertChar { pos: 0, ch: 'q' })
                            )
                        )
                    )
                )
            )
        ))
    );
    // The mint of a `SubTree` carries the two objects the generated path
    // materialises with it.
    assert_eq!(
        typed_mint(&meta, meta.slot("SubTree")),
        json!({"Tree": {"Blackboard": "New"}}),
        "a `SubTree` brings its `tree`, and that `BehaviorTree` its `blackboard`"
    );
}

/// The exclusion, stated as a test: `DataFlowPort.entry` is on the
/// interpreted read-out and nowhere on the generated one, and the projection
/// is what removes it.
///
/// The claim is about the projection, so the script is carried and not
/// compared. It cannot be compared: an `OutFlowPort` has no feature but
/// `entry`, so once the projection has taken `entry` out there is nothing
/// left in it, and an object with nothing left in it is one the generated
/// path cannot render at all — which is
/// [`a_sequence_child_with_nothing_written_is_invisible_on_the_generated_path`],
/// the one inequality this oracle has found. That test owns the finding;
/// this one owns the exclusion. The assertion at the end pins the two
/// together rather than letting either hide the other.
#[test]
fn the_only_exclusion_is_the_non_containment_reference() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let references: Vec<String> = meta
        .sem
        .classes
        .iter()
        .flat_map(|class| {
            class.declared.iter().filter_map(move |feature| {
                matches!(feature.merge, MergeRule::Reference { .. })
                    .then(|| format!("{}.{}", class.name, feature.name))
            })
        })
        .collect();
    assert_eq!(
        references,
        vec!["DataFlowPort.entry".to_string()],
        "`bt.ecore` has exactly one non-containment reference and this is it"
    );

    let mut steps: Vec<Step> = open_the_model(&meta).into_iter().map(Step::Edit).collect();
    let main = Path::default().child(Hop {
        feature: "main".to_string(),
        at: None,
        class: "BehaviorTree".to_string(),
    });
    steps.push(Step::Edit(Edit {
        id: 2,
        writer: 'a',
        path: main.clone(),
        action: Action::Create {
            feature: "child".to_string(),
            pos: None,
            class: "OpenDoor".to_string(),
        },
    }));
    let door = main.child(Hop {
        feature: "child".to_string(),
        at: None,
        class: "OpenDoor".to_string(),
    });
    steps.push(Step::Edit(Edit {
        id: 3,
        writer: 'a',
        path: door,
        action: Action::Create {
            feature: "outflowports".to_string(),
            pos: Some(0),
            class: "OutFlowPort".to_string(),
        },
    }));
    let script = EditScript {
        label: "a flow port, whose only feature is a reference".to_string(),
        steps,
    };
    let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
    for step in &script.steps {
        match step {
            Step::Edit(edit) => {
                assert_eq!(
                    harness.carry(edit),
                    Ok(true),
                    "both intakes take {}",
                    edit.show()
                );
            }
            Step::Deliver => harness.cross(),
        }
    }

    let raw: Value = harness.ia.query(Read::<Value>::new());
    let port = &raw["main"]["child"]["outflowports"][0];
    assert_eq!(port[ECLASS], json!("OutFlowPort"));
    assert!(
        port.get("entry").is_some(),
        "the interpreted read-out carries `entry`: {port}"
    );
    let stripped = harness.interp_doc('a');
    assert_eq!(
        stripped["main"]["child"]["outflowports"][0],
        json!({"eClass": "OutFlowPort"}),
        "and the projection is what takes it out"
    );
    assert_eq!(
        harness.gen_doc('a'),
        Value::Null,
        "and with `entry` gone an `OutFlowPort` holds nothing, which is the \
         one thing the generated path cannot render"
    );
}

/// The guard on the projection's one exemption: a value written and then
/// emptied is **not** dropped, and compares equal on both paths.
///
/// [`without_defaults`] drops a key whose value is the default of its
/// feature's rule, and the whole rule turns on optionals being exempt. An
/// optional's default is absence, and an absent optional carries no key at
/// all, so a key that *is* there under an optional was put there by an
/// operation and has to survive the projection even when what it holds is
/// the empty string. Without this test the exemption is a comment; with it,
/// widening the rule to cover optionals fails here.
#[test]
fn an_emptied_optional_is_not_dropped() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    assert!(
        matches!(
            meta.rule(meta.slot("Sequence"), "name"),
            MergeRule::Attribute { shape, leaf: LeafRule::Text } if shape.effective() == Shape::Optional
        ),
        "`TreeNode.name` is the optional text this test is about"
    );

    let main = Path::default().child(Hop {
        feature: "main".to_string(),
        at: None,
        class: "BehaviorTree".to_string(),
    });
    let node = main.child(Hop {
        feature: "child".to_string(),
        at: None,
        class: "Sequence".to_string(),
    });
    let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
    for edit in open_the_model(&meta) {
        harness.apply(&edit).expect("the model opens");
    }
    harness
        .apply(&Edit {
            id: 2,
            writer: 'a',
            path: main,
            action: Action::Create {
                feature: "child".to_string(),
                pos: None,
                class: "Sequence".to_string(),
            },
        })
        .expect("a `Sequence` goes under `child`");

    // Nothing has been written into `name`, so neither path carries the key.
    assert!(
        harness.interp_doc('a')["main"]["child"].get("name").is_none(),
        "an untouched optional carries no key: {}",
        harness.interp_doc('a')
    );

    harness
        .apply(&Edit {
            id: 2,
            writer: 'a',
            path: node.clone(),
            action: Action::Text {
                feature: "name".to_string(),
                op: TextOp::Insert { pos: 0, ch: 'z', after: "z".to_string() },
            },
        })
        .expect("both paths take the character");
    assert_eq!(
        harness.interp_doc('a')["main"]["child"]["name"],
        json!("z"),
        "written, it is there on both paths"
    );

    harness
        .apply(&Edit {
            id: 2,
            writer: 'a',
            path: node,
            action: Action::Text {
                feature: "name".to_string(),
                op: TextOp::Delete { pos: 0, after: String::new() },
            },
        })
        .expect("both paths take the deletion");

    let interpreted = harness.interp_doc('a');
    let generated = harness.gen_doc('a');
    assert_eq!(interpreted, generated, "and emptied, the two paths agree");
    assert_eq!(
        interpreted["main"]["child"]["name"],
        json!(""),
        "emptied is present-and-empty and not absent: {interpreted}"
    );
    assert_eq!(
        interpreted["main"]["child"][ECLASS],
        json!("Sequence"),
        "and the object it hangs off is not collapsed either"
    );
}

/// **The one inequality this oracle has found, and it is a real one.**
///
/// An object created into an ordered containment and not yet written into is
/// on the interpreted read-out and is *not* on the generated one. It is not
/// a projection artefact: the projection is applied to both sides by the same
/// function and drops keys, never elements, and the two logs hold the same
/// object at the same position — only the read-outs differ.
///
/// # Where it comes from
///
/// The generated path's ordered containment is
/// `NestedListLog<L>` (`moirai-crdt/src/list/nested_list.rs`), an ordering
/// half over `EventGraph<List<EventId>>` and a mapping half that is a
/// `UWMapLog`. `UWMapLog`'s read (`moirai-crdt/src/map/uw_map.rs:199-210`)
/// keeps a child only when its value differs from `Value::default()`, and it
/// has to: `UWMap::Remove` is not a tombstone, it calls
/// `redundant_by_parent` on the child and leaves it in the map, so *reading
/// as the default is how the generated path spells removed*. A child that
/// was created and never written into reads as the default too, and the
/// generated path cannot tell the two apart.
///
/// `moirai-interp`'s `eval::read_node` (`moirai-interp/src/eval.rs:77-88`)
/// copied `NestedListLog`'s ordering half and not `UWMapLog`'s filter: its
/// sequence arm drops a *hole*, an id in the ordering with no child behind
/// it, and nothing else. Its own comment shows the pattern was known for
/// slots — "`union.rs`'s `Value` branch reads whatever is there, default or
/// not; only its `Conflicts` branch drops the empty ones. Copied" — and the
/// sequence arm is where it was not.
///
/// # Which path is wrong
///
/// For I-A1, the interpreted one: it set out to carry the generated path's
/// merge semantics and this is one rule of them it does not carry. The fix
/// is in `eval::read_node`'s `Shaped::Sequence` arm, which must drop a child
/// whose read-out is what `read_absent` gives for the same rule, exactly as
/// `UWMapLog` compares against `Value::default()`. Moirai is not this
/// commit's to change, so the oracle reports it and `ip13` fails on it.
///
/// Semantically the generated path is the lossy one — it conflates *removed*
/// with *empty*, and there is no read-out of a `bt_crdt` model in which an
/// `OutFlowPort`, whose only feature is a reference, can ever be seen at all
/// — but that is a finding about the generated path and not a licence for
/// the interpreted one to differ from it.
///
/// # It heals the moment anything is written
///
/// The second half of this test is the important half: one character into
/// the new object's `key` and the two paths agree again, with the object at
/// the same index. So the divergence is confined to the window between an
/// object's creation and its first write, and it does not compound: the
/// positions the script addresses are the log's, and both logs hold every
/// element.
#[test]
fn a_sequence_child_with_nothing_written_is_invisible_on_the_generated_path() {
    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let blackboard = Path::default()
        .child(Hop { feature: "main".to_string(), at: None, class: "BehaviorTree".to_string() })
        .child(Hop { feature: "blackboard".to_string(), at: None, class: "Blackboard".to_string() });
    let entry = blackboard.child(Hop {
        feature: "entries".to_string(),
        at: Some(0),
        class: "BlackboardEntry".to_string(),
    });

    let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
    for edit in open_the_model(&meta) {
        harness.apply(&edit).expect("the model opens");
    }
    let create = Edit {
        id: 2,
        writer: 'a',
        path: blackboard,
        action: Action::Create {
            feature: "entries".to_string(),
            pos: Some(0),
            class: "BlackboardEntry".to_string(),
        },
    };
    assert_eq!(
        harness.carry(&create),
        Ok(true),
        "both intakes take the create; this is not a refusal"
    );

    let interpreted = harness.interp_doc('a');
    let generated = harness.gen_doc('a');
    assert_eq!(
        interpreted,
        json!({
            "eClass": "Root",
            "main": {
                "eClass": "BehaviorTree",
                "blackboard": {"eClass": "Blackboard", "entries": [{"eClass": "BlackboardEntry"}]}
            }
        }),
        "the interpreted path holds the object it was told to make"
    );
    assert_eq!(
        generated,
        Value::Null,
        "the generated path holds it too and cannot render it, so the whole \
         model projects as still-unwritten"
    );
    assert_ne!(interpreted, generated, "and that is an inequality, not a nicety");

    // One character, and they agree again.
    harness
        .apply(&Edit {
            id: 2,
            writer: 'a',
            path: entry,
            action: Action::Text {
                feature: "key".to_string(),
                op: TextOp::Insert { pos: 0, ch: 'k', after: "k".to_string() },
            },
        })
        .expect("one write into the new object and the two paths agree again");
    assert_eq!(
        harness.interp_doc('a')["main"]["blackboard"]["entries"],
        json!([{"eClass": "BlackboardEntry", "key": "k"}]),
        "at the same index, on both paths"
    );
}

/// And it is the *only* one. Thirty scripts, every read-out after every
/// operation, and every difference between the two paths is the empty
/// sequence child of
/// [`a_sequence_child_with_nothing_written_is_invisible_on_the_generated_path`]
/// or something that collapses upward from it.
///
/// The check is mechanical rather than by eye: both canonical documents are
/// re-pruned by a rule this test owns and the projection does not — drop a
/// sequence element that carries nothing but its class, then re-run
/// [`without_defaults`], to a fixed point, since dropping the element can
/// empty the array that held it and so empty the object that held *that*.
/// What is left over after that is a difference the finding does not
/// explain, and there must be none.
///
/// This test is what makes `ip13`'s failure actionable: it says the whole of
/// I-A1 rests on one rule in one function, and that nothing else is hiding
/// behind it.
#[test]
fn the_thirty_scripts_find_exactly_one_kind_of_difference() {
    fn without_empty_children(value: Value) -> Value {
        match value {
            Value::Array(items) => Value::Array(
                items
                    .into_iter()
                    .map(without_empty_children)
                    .filter(|item| !only_a_class(item))
                    .collect(),
            ),
            Value::Object(map) => Value::Object(
                map.into_iter()
                    .map(|(key, item)| (key, without_empty_children(item)))
                    .collect(),
            ),
            other => other,
        }
    }
    fn explained(meta: &Meta, mut value: Value) -> Value {
        for _ in 0..16 {
            let next = without_defaults(meta, without_empty_children(value.clone()));
            if next == value {
                break;
            }
            value = next;
        }
        value
    }

    let meta = Meta::new(Arc::new(
        from_descriptor(&bt_descriptor()).expect("the descriptor parses"),
    ));
    let mut sequential = 0;
    let mut concurrent = 0;
    let mut comparisons = 0;
    let mut differing = 0;
    let mut unexplained = Vec::new();
    for seed in 0..30u64 {
        let script = seeded_script(&meta, seed, seed >= 10);
        let here = script
            .steps
            .iter()
            .filter(|step| matches!(step, Step::Edit(_)))
            .count();
        if seed >= 10 {
            concurrent += here;
        } else {
            sequential += here;
        }
        let mut harness = Harness::new(&bt_descriptor(), &bt_descriptor());
        let check = |harness: &Harness,
                     unexplained: &mut Vec<String>,
                     comparisons: &mut usize,
                     differing: &mut usize,
                     at: String| {
            for writer in ['a', 'b'] {
                *comparisons += 1;
                let interpreted = harness.interp_doc(writer);
                let generated = harness.gen_doc(writer);
                if interpreted == generated {
                    continue;
                }
                *differing += 1;
                let left = explained(&harness.interp_meta, interpreted);
                let right = explained(&harness.gen_meta, generated);
                if left != right {
                    unexplained.push(format!(
                        "{} seed {seed} {at} replica {writer}: {}",
                        script.label,
                        difference(&left, &right, "").unwrap_or_default()
                    ));
                }
            }
        };
        for (index, step) in script.steps.iter().enumerate() {
            match step {
                Step::Edit(edit) => {
                    harness
                        .carry(edit)
                        .unwrap_or_else(|reason| panic!("{}: {reason}", script.label));
                    check(&harness, &mut unexplained, &mut comparisons, &mut differing, format!("after edit {index}"));
                }
                Step::Deliver => {
                    harness.cross();
                    check(&harness, &mut unexplained, &mut comparisons, &mut differing, format!("after delivery {index}"));
                }
            }
        }
        harness.cross();
        check(&harness, &mut unexplained, &mut comparisons, &mut differing, "at the end".to_string());
    }
    let edits = sequential + concurrent;
    assert!(
        edits > 30 * 10,
        "thirty scripts came to only {edits} edits; the generator stopped early"
    );
    assert!(
        unexplained.is_empty(),
        "{} of {comparisons} comparisons differ for a reason the empty sequence \
         child does not explain, which would be a second inequality:\n\n{}",
        unexplained.len(),
        unexplained.join("\n")
    );
    println!(
        "30 scripts, {edits} edits ({sequential} over the ten sequential, \
         {concurrent} over the twenty concurrent), {comparisons} comparisons, \
         {differing} of them differing, and every one of those is the empty \
         sequence child"
    );
}
