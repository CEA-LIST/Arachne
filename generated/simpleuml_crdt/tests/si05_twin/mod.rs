//! SI.05's twin comparison of references, the part both of its drivers share
//! (`si05_reference_twin.rs` over `SimpleUML.ecore`, this crate, and
//! `si05_reference_twin_class_hierarchy.rs` over `class_hierarchy.ecore`,
//! generated into `fixtures/si05/class_hierarchy/` by Arachne d81cae8).
//!
//! The spec is `cea-cdrt-knowledge/spec/items/SI.05.md`, claim R3 and R4,
//! method section 5.3. In one paragraph: two replicas of the interpreted
//! `ModelLog`, running the descriptor with the new rule (K1), and two replicas
//! of the generated log are driven by one script of abstract edits, each edit
//! encoded once per path and sent to the same writer on both paths, delivery
//! identical on both. Each edit costs exactly one event on each path, and the
//! interpreted `Install` is answered by one generated opener, so every event
//! carries the same `(origin, sequence, Lamport)` tag on both paths and a tie
//! the generated graph breaks by `FairPolicy` is a tie of the same tags on the
//! interpreted side. After every edit and every delivery the reference
//! read-out of every replica of both paths is projected to creation identity
//! and compared.
//!
//! The projection. Every object is named by the label of the edit that
//! created it (`0` for the root). On the interpreted side a label is the
//! object's served identity (`__id` of the identified read, recorded on the
//! writer right after the create), and a reference value (K3) is mapped back
//! through it. On the generated side a label is the vertex the create added
//! to the reference graph (the one vertex of the created class's own kind,
//! `ListElement` of the creating event at the end of its path), and an arc
//! `(source, edge, target)` is mapped through it, its edge kind naming the
//! feature. Both sides become `{(source label, feature): {target labels}}`,
//! entries with no target dropped. An arc or a value that maps to no label is
//! a failure, never skipped.
//!
//! What is left out: the references that declare an `eOpposite` (Cam's
//! decision 3), which keep today's rule and which no script writes, and
//! nothing else ([`the_only_references_left_out`]).
//!
//! The generated intake decides what is sent: an edit is offered to the
//! generated writer first, and an edit it refuses (a second arc on a full
//! `[0, 1]` edge, an unlink below a minimum) is dropped from both paths and
//! counted. An edit it accepts must be accepted by the interpreted writer.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use heck::{ToSnakeCase, ToUpperCamelCase};
use moirai_crdt::utils::membership::twins_log;
use moirai_interp::eval::read_identified;
use moirai_interp::leaf::LeafOp;
use moirai_interp::testing::install;
use moirai_interp::{InstanceOp, ModelLog, ModelOp};
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::Tcsb;
use moirai_protocol::replica::{IsReplica, Replica};
use moirai_semantics::{
    ClassSemantics, ClassSlot, FeatureSlot, LeafRule, MergeRule, MetamodelSemantics, Shape,
    from_descriptor,
};
use serde_json::{Map, Value, json};

pub type InterpReplica = Replica<ModelLog, Tcsb<ModelOp>>;
pub type Label = u32;

/// The key an element's store identity is served under.
pub const ID: &str = "__id";

// ---------------------------------------------------------------------------
// 1. The metamodel
// ---------------------------------------------------------------------------

/// The table, read from today's descriptor (which names the same classes and
/// features as the new one, so every slot is the same), and the relations the
/// encoders need. Built from today's descriptor so that a build that cannot
/// read the new rule fails where the interpreted replicas are opened.
pub struct Meta {
    pub sem: Arc<MetamodelSemantics>,
    subs: BTreeMap<u16, Vec<ClassSlot>>,
    /// The in-scope references, read from the new descriptor's JSON:
    /// `(declaring class, feature)` whose `merge.kind` is `identityReference`.
    identity: BTreeSet<(String, String)>,
}

impl Meta {
    pub fn new(today: &Value, identity_descriptor: &Value) -> Meta {
        let sem = Arc::new(from_descriptor(today).expect("today's descriptor parses"));
        let mut subs: BTreeMap<u16, Vec<ClassSlot>> = BTreeMap::new();
        for class in &sem.classes {
            for sup in &class.supers {
                subs.entry(sup.0).or_default().push(class.slot);
            }
        }
        let mut identity = BTreeSet::new();
        for (class, body) in identity_descriptor["classes"].as_object().expect("classes") {
            for entry in body["references"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[])
            {
                if entry["merge"]["kind"] == json!("identityReference") {
                    identity.insert((
                        class.clone(),
                        entry["name"].as_str().expect("a name").to_string(),
                    ));
                }
            }
        }
        Meta {
            sem,
            subs,
            identity,
        }
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

    pub fn is_union(&self, slot: ClassSlot) -> bool {
        !self.class(slot).instantiable || !self.subs(slot).is_empty()
    }

    pub fn visible_slot(&self, class: ClassSlot, feature: &str) -> FeatureSlot {
        let holder = self.class(class);
        let index = holder
            .visible
            .iter()
            .position(|(name, _, _)| &**name == feature)
            .unwrap_or_else(|| panic!("`{}` cannot see `{feature}`", holder.name));
        FeatureSlot(index as u16)
    }

    pub fn declared(&self, class: ClassSlot, feature: &str) -> (ClassSlot, MergeRule) {
        let holder = self.class(class);
        let (_, owner, slot) = holder
            .visible
            .iter()
            .find(|(name, _, _)| &**name == feature)
            .unwrap_or_else(|| panic!("`{}` cannot see `{feature}`", holder.name));
        (
            *owner,
            *self
                .sem
                .rule(*owner, *slot)
                .expect("a visible feature has a rule"),
        )
    }

    pub fn rule(&self, class: ClassSlot, feature: &str) -> MergeRule {
        self.declared(class, feature).1
    }

    pub fn sees(&self, class: ClassSlot, feature: &str) -> bool {
        self.class(class)
            .visible
            .iter()
            .any(|(name, _, _)| &**name == feature)
    }

    /// The in-scope references `class` can see, by name, in name order.
    pub fn identity_references(&self, class: ClassSlot) -> Vec<String> {
        self.class(class)
            .visible
            .iter()
            .filter(|(name, owner, _)| {
                self.identity
                    .contains(&(self.name(*owner).to_string(), name.to_string()))
            })
            .map(|(name, _, _)| name.to_string())
            .collect()
    }

    /// Every in-scope reference, as `Declaring.feature`.
    pub fn identity_set(&self) -> BTreeSet<String> {
        self.identity
            .iter()
            .map(|(class, feature)| format!("{class}.{feature}"))
            .collect()
    }

    /// The target class of a reference or containment `class` sees.
    pub fn target(&self, class: ClassSlot, feature: &str) -> ClassSlot {
        match self.rule(class, feature) {
            MergeRule::Containment { target, .. } | MergeRule::Reference { target, .. } => target,
            other => panic!("`{feature}` has no target: {other:?}"),
        }
    }

    /// The sequence containments `class` sees, by name.
    pub fn sequences(&self, class: ClassSlot) -> Vec<String> {
        self.class(class)
            .visible
            .iter()
            .filter(|(name, _, _)| {
                matches!(
                    self.rule(class, name),
                    MergeRule::Containment {
                        shape: Shape::Sequence,
                        ..
                    }
                )
            })
            .map(|(name, _, _)| name.to_string())
            .collect()
    }

    /// Whether `class` sees a text attribute `name`.
    pub fn has_name(&self, class: ClassSlot) -> bool {
        self.sees(class, "name")
            && matches!(
                self.rule(class, "name"),
                MergeRule::Attribute {
                    leaf: LeafRule::Text,
                    ..
                }
            )
    }

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

    pub fn union_descent(&self, target: ClassSlot, concrete: ClassSlot) -> Vec<String> {
        if !self.is_union(target) {
            assert_eq!(
                target, concrete,
                "a containment onto a class with no `union!` holds that class only"
            );
            return Vec::new();
        }
        if target == concrete {
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
}

fn tagged(variant: impl Into<String>, payload: Value) -> Value {
    let mut map = Map::new();
    map.insert(variant.into(), payload);
    Value::Object(map)
}

const RUST_KEYWORDS: [&str; 52] = [
    "as", "async", "await", "abstract", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "try", "type",
    "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

fn variant_of(feature: &str) -> String {
    let name = feature.to_snake_case();
    let name = if RUST_KEYWORDS.contains(&name.as_str()) {
        format!("{name}_field")
    } else {
        name
    };
    name.to_upper_camel_case()
}

fn super_variant(class: &str) -> String {
    format!("{}_super", class.to_snake_case()).to_upper_camel_case()
}

fn feature_wrap(meta: &Meta, class: ClassSlot, feature: &str, payload: Value) -> Value {
    let (owner, _) = meta.declared(class, feature);
    let mut value = tagged(variant_of(feature), payload);
    for hop in meta.super_path(class, owner).iter().rev() {
        value = tagged(super_variant(meta.name(*hop)), value);
    }
    value
}

fn union_wrap(meta: &Meta, target: ClassSlot, concrete: ClassSlot, payload: Value) -> Value {
    let mut value = payload;
    for variant in meta.union_descent(target, concrete).iter().rev() {
        value = tagged(variant.clone(), value);
    }
    value
}

// ---------------------------------------------------------------------------
// 2. The generated side, per crate
// ---------------------------------------------------------------------------

/// What a twin driver needs from one generated crate. Every operation is
/// handed over as the JSON of the crate's own operation enum; the graph comes
/// back as the JSON of its vertices and edge kinds.
pub trait GenSide {
    type Msg: Clone;
    /// The root class the scripts mint, by name.
    const ROOT_CLASS: &'static str;
    /// A fresh pair of generated replicas `a` and `b`.
    fn fresh() -> Self;
    /// The generated operation that answers the interpreted `Install`: one
    /// event of `a`'s that no projection reads.
    fn opener() -> Value;
    /// The generated operation that mints the root object.
    fn mint_root() -> Value;
    /// `op`, an operation on the root object's record, wrapped as the crate's
    /// top-level operation.
    fn on_root(meta: &Meta, op: Value) -> Value;
    /// The arc variant for `(source class, feature, target class)`, when the
    /// generated graph has one.
    fn arc(source: &str, feature: &str, target: &str) -> Option<&'static str>;
    /// The feature an arc variant carries.
    fn feature_of(arc: &str) -> Option<&'static str>;
    fn send(&mut self, writer: char, op: Value) -> Result<Self::Msg, String>;
    fn receive(&mut self, at: char, message: Self::Msg);
    /// The tag of a generated event, `origin@seq L<lamport>`.
    fn tag(message: &Self::Msg) -> String;
    /// The reference graph of a replica: vertex JSON, and arcs as indexes
    /// into it with their edge kind JSON.
    fn graph(&self, at: char) -> (Vec<Value>, Vec<(usize, usize, Value)>);
}

// ---------------------------------------------------------------------------
// 3. Edits
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum Action {
    /// Mint the root object, label 0.
    Root,
    /// Mint `class` at `pos` of the sequence `feature` of `parent`, as label `label`.
    Create {
        parent: Label,
        feature: String,
        pos: usize,
        class: String,
        label: Label,
    },
    /// Take `target` out of the sequence it sits in.
    Delete { target: Label },
    /// One character in front of `target`'s `name`.
    Rename { target: Label, ch: char },
    /// K2 `Link` / generated `AddReference`.
    Link {
        source: Label,
        feature: String,
        target: Label,
    },
    /// K2 `Unlink` / generated `RemoveReference`.
    Unlink {
        source: Label,
        feature: String,
        target: Label,
    },
}

#[derive(Clone, Debug)]
pub struct Edit {
    pub writer: char,
    pub action: Action,
}

impl Edit {
    pub fn show(&self) -> String {
        format!("{} {:?}", self.writer, self.action)
    }
}

#[derive(Clone, Debug)]
pub enum Step {
    Edit(Edit),
    /// Everything both writers hold, delivered to the other.
    Deliver,
}

pub fn edit(writer: char, action: Action) -> Step {
    Step::Edit(Edit { writer, action })
}

pub fn create(
    writer: char,
    parent: Label,
    feature: &str,
    pos: usize,
    class: &str,
    label: Label,
) -> Step {
    edit(
        writer,
        Action::Create {
            parent,
            feature: feature.into(),
            pos,
            class: class.into(),
            label,
        },
    )
}

pub fn link(writer: char, source: Label, feature: &str, target: Label) -> Step {
    edit(
        writer,
        Action::Link {
            source,
            feature: feature.into(),
            target,
        },
    )
}

pub fn unlink(writer: char, source: Label, feature: &str, target: Label) -> Step {
    edit(
        writer,
        Action::Unlink {
            source,
            feature: feature.into(),
            target,
        },
    )
}

pub fn delete(writer: char, target: Label) -> Step {
    edit(writer, Action::Delete { target })
}

pub fn rename(writer: char, target: Label, ch: char) -> Step {
    edit(writer, Action::Rename { target, ch })
}

/// One step of a path from the root: the feature, the position, the class
/// of the object landed on.
#[derive(Clone, Debug)]
struct Hop {
    feature: String,
    at: usize,
    class: String,
}

/// The projection: `(source label, feature) -> target labels`.
pub type Projection = BTreeMap<(Label, String), BTreeSet<Label>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// `a`'s pending events cross first.
    AFirst,
    BFirst,
}

// ---------------------------------------------------------------------------
// 4. The harness
// ---------------------------------------------------------------------------

pub struct Twin<G: GenSide> {
    pub meta: Meta,
    ia: InterpReplica,
    ib: InterpReplica,
    gen_side: G,
    order: Order,
    /// Interpreted identity and generated vertex of every label.
    ids: BTreeMap<Label, String>,
    vertices: BTreeMap<Label, String>,
    classes: BTreeMap<Label, String>,
    pending_a: Vec<(EventMessage<ModelOp>, G::Msg)>,
    pending_b: Vec<(EventMessage<ModelOp>, G::Msg)>,
    pub edits: usize,
    pub refused: usize,
    pub comparisons: usize,
    pub links_read: usize,
    pub log: Vec<String>,
    /// The harness's own rehearsal: the interpreted pair runs today's
    /// descriptor and every `Link` or `Unlink` is answered on it by a stand-in
    /// event (one character into the root's `name`), so the tags stay aligned
    /// and only the generated half is read. See [`Twin::rehearsal`].
    rehearsal: bool,
}

fn locate(meta: &Meta, doc: &Value, id: &str) -> Option<Vec<Hop>> {
    fn go(meta: &Meta, value: &Value, id: &str, acc: &mut Vec<Hop>) -> bool {
        if value.get(ID).and_then(Value::as_str) == Some(id) {
            return true;
        }
        let Some(class) = value.get("eClass").and_then(Value::as_str) else {
            return false;
        };
        let slot = meta.slot(class);
        for feature in meta.sequences(slot) {
            let Some(items) = value.get(&feature).and_then(Value::as_array) else {
                continue;
            };
            for (at, item) in items.iter().enumerate() {
                let Some(child) = item.get("eClass").and_then(Value::as_str) else {
                    continue;
                };
                acc.push(Hop {
                    feature: feature.clone(),
                    at,
                    class: child.to_string(),
                });
                if go(meta, item, id, acc) {
                    return true;
                }
                acc.pop();
            }
        }
        false
    }
    let mut acc = Vec::new();
    go(meta, doc, id, &mut acc).then_some(acc)
}

/// Every object of a served document with its class.
fn objects(doc: &Value, out: &mut Vec<(String, String, Value)>) {
    match doc {
        Value::Object(map) => {
            if let (Some(Value::String(id)), Some(Value::String(class))) =
                (map.get(ID), map.get("eClass"))
            {
                out.push((id.clone(), class.clone(), doc.clone()));
            }
            map.values().for_each(|item| objects(item, out));
        }
        Value::Array(items) => items.iter().for_each(|item| objects(item, out)),
        _ => {}
    }
}

/// K3 for one value: a set of identity strings.
fn k3(value: Option<&Value>, at: &str) -> Result<BTreeSet<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(BTreeSet::new()),
        Some(Value::String(id)) => Ok([id.clone()].into_iter().collect()),
        Some(Value::Array(items)) => {
            let mut out = BTreeSet::new();
            for item in items {
                let id = item
                    .as_str()
                    .ok_or_else(|| format!("K3: {at} holds a non-string {item}"))?;
                if !out.insert(id.to_string()) {
                    return Err(format!("K3: {at} reads {id} twice"));
                }
            }
            Ok(out)
        }
        Some(other) => Err(format!(
            "K3: {at} reads as {other}, neither an identity nor a list"
        )),
    }
}

impl<G: GenSide> Twin<G> {
    /// Open both paths: the interpreted pair on `identity_descriptor`, the
    /// generated pair with its opener. Panics with the interpreted refusal
    /// when this build cannot read the new rule.
    pub fn new(today: &Value, identity_descriptor: &Value, order: Order) -> Twin<G> {
        Self::open(today, identity_descriptor, order, false)
    }

    /// The harness rehearsed against the generated runtime alone: the
    /// interpreted pair opens on today's descriptor and stands in for every
    /// reference operation, the generated half runs exactly as in a twin run,
    /// and the projections compared are the generated peers'. What a test
    /// holds the result to is a hand-computed answer, so this proves the
    /// encoders, the labels, the projection and the schedules before any
    /// interpreted implementation of the new rule exists.
    pub fn rehearsal(today: &Value, identity_descriptor: &Value, order: Order) -> Twin<G> {
        Self::open(today, identity_descriptor, order, true)
    }

    fn open(today: &Value, identity_descriptor: &Value, order: Order, rehearsal: bool) -> Twin<G> {
        let meta = Meta::new(today, identity_descriptor);
        let (mut ia, mut ib) = twins_log::<ModelLog>();
        let opened_on = if rehearsal {
            today
        } else {
            identity_descriptor
        };
        let event = ia
            .send(install("si05-twin", opened_on))
            .unwrap_or_else(|error| panic!("R3: the interpreted log does not open on the descriptor with the new rule: {error:?}"));
        ib.receive(event);
        let mut gen_side = G::fresh();
        let opener = gen_side
            .send('a', G::opener())
            .expect("the generated opener is taken");
        gen_side.receive('b', opener);
        Twin {
            meta,
            ia,
            ib,
            gen_side,
            order,
            ids: BTreeMap::new(),
            vertices: BTreeMap::new(),
            classes: BTreeMap::new(),
            pending_a: Vec::new(),
            pending_b: Vec::new(),
            edits: 0,
            refused: 0,
            comparisons: 0,
            links_read: 0,
            log: Vec::new(),
            rehearsal,
        }
    }

    fn interp(&self, at: char) -> &InterpReplica {
        if at == 'a' { &self.ia } else { &self.ib }
    }

    pub fn view(&self, at: char) -> Value {
        let log = self.interp(at).state();
        match log.semantics() {
            Some(sem) => read_identified(sem, log.root()),
            None => Value::Null,
        }
    }

    fn hops_of(&self, at: char, label: Label) -> Option<Vec<Hop>> {
        let id = self.ids.get(&label)?;
        locate(&self.meta, &self.view(at), id)
    }

    pub fn is_live(&self, at: char, label: Label) -> bool {
        self.hops_of(at, label).is_some()
    }

    pub fn class_of(&self, label: Label) -> Option<&str> {
        self.classes.get(&label).map(String::as_str)
    }

    pub fn labels(&self) -> Vec<Label> {
        self.ids.keys().copied().collect()
    }

    /// The interpreted operation for `inner`, which acts on the record of the
    /// object at the end of `hops`.
    fn interp_at(&self, hops: &[Hop], inner: InstanceOp) -> ModelOp {
        let root = self.meta.slot(G::ROOT_CLASS);
        let mut classes = vec![root];
        classes.extend(hops.iter().map(|hop| self.meta.slot(&hop.class)));
        let mut op = inner;
        for (index, hop) in hops.iter().enumerate().rev() {
            let parent = classes[index];
            op = InstanceOp::field(
                self.meta.visible_slot(parent, &hop.feature),
                InstanceOp::at(hop.at, InstanceOp::variant(self.meta.slot(&hop.class), op)),
            );
        }
        ModelOp::Instance(InstanceOp::variant(root, op))
    }

    /// The generated operation for `inner`, an operation on the record of the
    /// object at the end of `hops`.
    fn gen_at(&self, hops: &[Hop], inner: Value) -> Value {
        let root = self.meta.slot(G::ROOT_CLASS);
        let mut classes = vec![root];
        classes.extend(hops.iter().map(|hop| self.meta.slot(&hop.class)));
        let mut op = inner;
        for (index, hop) in hops.iter().enumerate().rev() {
            let parent = classes[index];
            let target = self.meta.target(parent, &hop.feature);
            let step = union_wrap(&self.meta, target, self.meta.slot(&hop.class), op);
            op = feature_wrap(
                &self.meta,
                parent,
                &hop.feature,
                json!({"Update": {"pos": hop.at, "op": step}}),
            );
        }
        G::on_root(&self.meta, op)
    }

    /// The rehearsal's stand-in for a reference operation: one character into
    /// the root's `name`, which no projection reads.
    fn stand_in(&self) -> ModelOp {
        let root = self.meta.slot(G::ROOT_CLASS);
        let optional = matches!(
            self.meta.rule(root, "name"),
            MergeRule::Attribute {
                shape: Shape::Optional,
                ..
            }
        );
        let leaf = InstanceOp::Leaf(LeafOp::InsertChar { pos: 0, ch: '~' });
        self.interp_at(
            &[],
            InstanceOp::field(
                self.meta.visible_slot(root, "name"),
                if optional {
                    InstanceOp::set(leaf)
                } else {
                    leaf
                },
            ),
        )
    }

    fn class_at_end(&self, hops: &[Hop]) -> ClassSlot {
        hops.last().map_or_else(
            || self.meta.slot(G::ROOT_CLASS),
            |hop| self.meta.slot(&hop.class),
        )
    }

    fn vertex_set(&self, at: char) -> BTreeSet<String> {
        self.gen_side
            .graph(at)
            .0
            .iter()
            .map(Value::to_string)
            .collect()
    }

    /// The two encodings of one edit, against the writer's own view.
    fn encode(&self, edit: &Edit) -> Result<(Result<ModelOp, String>, Value), String> {
        let at = edit.writer;
        let missing = |label: &Label| format!("{} does not see label {label}", edit.show());
        Ok(match &edit.action {
            Action::Root => {
                let root = self.meta.slot(G::ROOT_CLASS);
                (
                    Ok(ModelOp::Instance(InstanceOp::variant(
                        root,
                        InstanceOp::New,
                    ))),
                    G::mint_root(),
                )
            }
            Action::Create {
                parent,
                feature,
                pos,
                class,
                ..
            } => {
                let hops = self.hops_of(at, *parent).ok_or_else(|| missing(parent))?;
                let holder = self.class_at_end(&hops);
                let made = self.meta.slot(class);
                let interp = InstanceOp::field(
                    self.meta.visible_slot(holder, feature),
                    InstanceOp::insert(*pos, InstanceOp::variant(made, InstanceOp::New)),
                );
                let target = self.meta.target(holder, feature);
                let generated = feature_wrap(
                    &self.meta,
                    holder,
                    feature,
                    json!({"Insert": {"pos": pos, "op": union_wrap(&self.meta, target, made, json!("New"))}}),
                );
                (
                    Ok(self.interp_at(&hops, interp)),
                    self.gen_at(&hops, generated),
                )
            }
            Action::Delete { target } => {
                let hops = self.hops_of(at, *target).ok_or_else(|| missing(target))?;
                let (last, parents) = hops.split_last().ok_or("the root is not deleted")?;
                let holder = self.class_at_end(parents);
                let interp = InstanceOp::field(
                    self.meta.visible_slot(holder, &last.feature),
                    InstanceOp::delete(last.at),
                );
                let generated = feature_wrap(
                    &self.meta,
                    holder,
                    &last.feature,
                    json!({"Delete": {"pos": last.at}}),
                );
                (
                    Ok(self.interp_at(parents, interp)),
                    self.gen_at(parents, generated),
                )
            }
            Action::Rename { target, ch } => {
                let hops = self.hops_of(at, *target).ok_or_else(|| missing(target))?;
                let holder = self.class_at_end(&hops);
                let optional = matches!(
                    self.meta.rule(holder, "name"),
                    MergeRule::Attribute {
                        shape: Shape::Optional,
                        ..
                    }
                );
                let leaf = InstanceOp::Leaf(LeafOp::InsertChar { pos: 0, ch: *ch });
                let interp = InstanceOp::field(
                    self.meta.visible_slot(holder, "name"),
                    if optional {
                        InstanceOp::set(leaf)
                    } else {
                        leaf
                    },
                );
                let insert = json!({"Insert": {"content": ch, "pos": 0}});
                let generated = feature_wrap(
                    &self.meta,
                    holder,
                    "name",
                    if optional {
                        json!({"Set": insert})
                    } else {
                        insert
                    },
                );
                (
                    Ok(self.interp_at(&hops, interp)),
                    self.gen_at(&hops, generated),
                )
            }
            Action::Link {
                source,
                feature,
                target,
            }
            | Action::Unlink {
                source,
                feature,
                target,
            } => {
                let verb = if matches!(edit.action, Action::Link { .. }) {
                    "Link"
                } else {
                    "Unlink"
                };
                let hops = self.hops_of(at, *source).ok_or_else(|| missing(source))?;
                if self.hops_of(at, *target).is_none() && verb == "Link" {
                    return Err(missing(target));
                }
                let holder = self.class_at_end(&hops);
                let id = self.ids.get(target).ok_or_else(|| missing(target))?;
                let placeholder = self.interp_at(
                    &hops,
                    InstanceOp::field(
                        self.meta.visible_slot(holder, feature),
                        InstanceOp::Leaf(LeafOp::Clear),
                    ),
                );
                let mut wire = serde_json::to_value(&placeholder).expect("an operation serializes");
                assert!(
                    replace_placeholder(&mut wire, &json!({ verb: id })),
                    "the placeholder leaf is in {wire}"
                );
                let interp = serde_json::from_value::<ModelOp>(wire.clone()).map_err(|error| {
                    format!("K2: this build does not read {wire} as a ModelOp: {error}")
                });
                let source_class = self.classes.get(source).ok_or_else(|| missing(source))?;
                let target_class = self.classes.get(target).ok_or_else(|| missing(target))?;
                let arc = G::arc(source_class, feature, target_class).ok_or_else(|| {
                    format!("no generated arc for {source_class}.{feature} -> {target_class}")
                })?;
                let vertex = |label: &Label| -> Result<Value, String> {
                    let text = self
                        .vertices
                        .get(label)
                        .ok_or_else(|| format!("label {label} has no generated vertex"))?;
                    let parsed: Value = serde_json::from_str(text).expect("a vertex is JSON");
                    // `{"ClassId": <path>}`: the arc carries the path alone.
                    Ok(parsed
                        .as_object()
                        .and_then(|m| m.values().next())
                        .cloned()
                        .expect("a vertex is one tagged path"))
                };
                let body = json!({ arc: {"source": vertex(source)?, "target": vertex(target)?, "kind": null} });
                let generated = if verb == "Link" {
                    json!({"AddReference": body})
                } else {
                    json!({"RemoveReference": body})
                };
                if self.rehearsal {
                    (Ok(self.stand_in()), generated)
                } else {
                    (interp, generated)
                }
            }
        })
    }

    /// Offer one edit to both writers. `Ok(true)` when both took it,
    /// `Ok(false)` when the generated intake refused it (dropped from both).
    pub fn carry(&mut self, edit: &Edit) -> Result<bool, String> {
        let (interp, generated) = self.encode(edit)?;
        let at = edit.writer;
        let before = self.vertex_set(at);
        let gen_message = match self.gen_side.send(at, generated.clone()) {
            Ok(message) => message,
            Err(rejection) => {
                self.refused += 1;
                self.log.push(format!(
                    "refused by the generated intake: {} ({rejection})",
                    edit.show()
                ));
                return Ok(false);
            }
        };
        let interp = interp?;
        let replica = if at == 'a' {
            &mut self.ia
        } else {
            &mut self.ib
        };
        let interp_message = replica.send(interp.clone()).map_err(|error| {
            format!(
                "R3: the interpreted intake refused {} which the generated runtime accepted: {error:?}",
                edit.show()
            )
        })?;
        // The harness's own premise: one event per edit on each path, so the
        // two events carry one tag and a `FairPolicy` tie is the same tie.
        let interp_tag = {
            let event = interp_message.event();
            format!(
                "{}@{} L{}",
                event.id().origin_id(),
                event.id().seq(),
                event.lamport().val()
            )
        };
        if interp_tag != G::tag(&gen_message) {
            return Err(format!(
                "harness: {} is {interp_tag} on the interpreted path and {} on the generated one",
                edit.show(),
                G::tag(&gen_message)
            ));
        }
        self.edits += 1;
        self.log.push(edit.show());
        if at == 'a' {
            self.pending_a.push((interp_message, gen_message));
        } else {
            self.pending_b.push((interp_message, gen_message));
        }
        // Labels of what a create made.
        let made = match &edit.action {
            Action::Root => Some((0, G::ROOT_CLASS.to_string(), Vec::new())),
            Action::Create {
                parent,
                feature,
                pos,
                class,
                label,
            } => {
                let mut hops = self
                    .hops_of(at, *parent)
                    .expect("the parent was just addressed");
                hops.push(Hop {
                    feature: feature.clone(),
                    at: *pos,
                    class: class.clone(),
                });
                Some((*label, class.clone(), hops))
            }
            _ => None,
        };
        if let Some((label, class, hops)) = made {
            let view = self.view(at);
            let mut node = &view;
            for hop in &hops {
                node = &node[&hop.feature][hop.at];
            }
            let id = node.get(ID).and_then(Value::as_str).ok_or_else(|| {
                format!(
                    "SI.01: the object {} made carries no identity in {view}",
                    edit.show()
                )
            })?;
            self.ids.insert(label, id.to_string());
            self.classes.insert(label, class.clone());
            let kind = format!("{class}Id");
            let added: Vec<String> = self
                .vertex_set(at)
                .difference(&before)
                .filter(|vertex| {
                    serde_json::from_str::<Value>(vertex)
                        .ok()
                        .and_then(|v| v.as_object().map(|m| m.contains_key(&kind)))
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            match added.len() {
                0 => {}
                1 => {
                    self.vertices.insert(label, added[0].clone());
                }
                n => return Err(format!("{} added {n} vertices of kind {kind}", edit.show())),
            }
        }
        self.compare(false)?;
        Ok(true)
    }

    /// The interpreted projection of one replica.
    pub fn interp_projection(&self, at: char) -> Result<Projection, String> {
        let doc = self.view(at);
        let by_id: BTreeMap<&str, Label> = self
            .ids
            .iter()
            .map(|(label, id)| (id.as_str(), *label))
            .collect();
        let mut all = Vec::new();
        objects(&doc, &mut all);
        let mut out = Projection::new();
        for (id, class, object) in all {
            let Some(&source) = by_id.get(id.as_str()) else {
                return Err(format!(
                    "replica {at} serves an object {id} no edit created"
                ));
            };
            for feature in self.meta.identity_references(self.meta.slot(&class)) {
                let values = k3(
                    object.get(&feature),
                    &format!("{class}.{feature} of label {source}"),
                )?;
                let mut targets = BTreeSet::new();
                for value in values {
                    let target = by_id.get(value.as_str()).ok_or_else(|| {
                        format!("K3: {class}.{feature} of label {source} reads {value}, the identity of no labelled object")
                    })?;
                    targets.insert(*target);
                }
                if !targets.is_empty() {
                    out.insert((source, feature), targets);
                }
            }
        }
        Ok(out)
    }

    /// The generated projection of one replica.
    pub fn gen_projection(&self, at: char) -> Result<Projection, String> {
        let (vertices, arcs) = self.gen_side.graph(at);
        let by_vertex: BTreeMap<&str, Label> = self
            .vertices
            .iter()
            .map(|(label, v)| (v.as_str(), *label))
            .collect();
        let mut out = Projection::new();
        for (from, to, kind) in arcs {
            let name = kind
                .as_object()
                .and_then(|m| m.keys().next())
                .cloned()
                .unwrap_or_default();
            let feature = G::feature_of(&name)
                .ok_or_else(|| format!("an arc of kind {kind} no script writes"))?;
            let source = by_vertex
                .get(vertices[from].to_string().as_str())
                .ok_or_else(|| format!("an arc from an unlabelled vertex {}", vertices[from]))?;
            let target = by_vertex
                .get(vertices[to].to_string().as_str())
                .ok_or_else(|| format!("an arc to an unlabelled vertex {}", vertices[to]))?;
            out.entry((*source, feature.to_string()))
                .or_default()
                .insert(*target);
        }
        Ok(out)
    }

    /// Both replicas of both paths. With `settled`, the peers of each path
    /// are held to each other as well.
    pub fn compare(&mut self, settled: bool) -> Result<(), String> {
        if self.rehearsal {
            let a = self.gen_projection('a')?;
            let b = self.gen_projection('b')?;
            self.comparisons += 1;
            if settled && a != b {
                return Err(format!(
                    "the generated peers disagree once settled: {a:?} / {b:?}"
                ));
            }
            return Ok(());
        }
        let mut seen = Vec::new();
        for at in ['a', 'b'] {
            let interp = self.interp_projection(at)?;
            let generated = self.gen_projection(at)?;
            self.comparisons += 1;
            if interp != generated {
                return Err(format!(
                    "R3: replica {at}: interpreted {interp:?}, generated {generated:?}\n  after: {}",
                    self.log.join("\n         ")
                ));
            }
            seen.push(interp);
        }
        if settled && seen[0] != seen[1] {
            return Err(format!(
                "R2: replicas a and b disagree once settled: {:?} / {:?}",
                seen[0], seen[1]
            ));
        }
        self.links_read = self
            .links_read
            .max(seen[0].values().map(BTreeSet::len).sum());
        Ok(())
    }

    /// The links a writer sees: the interpreted projection, or in a
    /// rehearsal, where the interpreted pair holds none, the generated one.
    pub fn links_seen(&self, at: char) -> Option<Projection> {
        if self.rehearsal {
            self.gen_projection(at).ok()
        } else {
            self.interp_projection(at).ok()
        }
    }

    pub fn projection(&self, at: char) -> Projection {
        self.interp_projection(at).expect("a projection")
    }

    /// Everything pending crosses, in the harness's order, compared after
    /// every event.
    pub fn deliver(&mut self) -> Result<(), String> {
        let order = if self.order == Order::AFirst {
            ['a', 'b']
        } else {
            ['b', 'a']
        };
        for from in order {
            let pending = if from == 'a' {
                std::mem::take(&mut self.pending_a)
            } else {
                std::mem::take(&mut self.pending_b)
            };
            for (interp, generated) in pending {
                if from == 'a' {
                    self.ib.receive(interp);
                    self.gen_side.receive('b', generated);
                } else {
                    self.ia.receive(interp);
                    self.gen_side.receive('a', generated);
                }
                self.compare(false)?;
            }
        }
        self.log.push("-- delivered --".into());
        self.compare(true)
    }

    pub fn run(&mut self, steps: &[Step]) -> Result<(), String> {
        for (index, step) in steps.iter().enumerate() {
            match step {
                Step::Edit(edit) => {
                    self.carry(edit)
                        .map_err(|why| format!("step {index} {}: {why}", edit.show()))?;
                }
                Step::Deliver => self
                    .deliver()
                    .map_err(|why| format!("delivery at step {index}: {why}"))?,
            }
        }
        self.deliver()
    }
}

/// Swap the first `{"Leaf": "Clear"}` under `value` for `{"Leaf": leaf}`.
fn replace_placeholder(value: &mut Value, leaf: &Value) -> bool {
    match value {
        Value::Object(map) => {
            if map.len() == 1 && map.get("Leaf") == Some(&json!("Clear")) {
                map.insert("Leaf".to_string(), leaf.clone());
                return true;
            }
            map.values_mut().any(|item| replace_placeholder(item, leaf))
        }
        Value::Array(items) => items.iter_mut().any(|item| replace_placeholder(item, leaf)),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// 5. Seeded scripts
// ---------------------------------------------------------------------------

/// splitmix64, so a seed means the same script on every machine.
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
}

/// One edit a writer could make against its own view, or `None`.
fn propose<G: GenSide>(
    twin: &Twin<G>,
    rng: &mut Rng,
    writer: char,
    next: &mut Label,
    creatable: &[(&str, &str, &str)],
) -> Option<Edit> {
    let view = twin.view(writer);
    let live: Vec<Label> = twin
        .labels()
        .into_iter()
        .filter(|label| twin.is_live(writer, *label))
        .collect();
    if live.is_empty() {
        return None;
    }
    let roll = rng.below(100);
    let action = if roll < 35 || live.len() < 4 {
        // Create: one of the (parent class, feature, class) the driver names.
        let options: Vec<(Label, &str, &str)> = live
            .iter()
            .flat_map(|label| {
                let class = twin.class_of(*label).unwrap_or("").to_string();
                creatable
                    .iter()
                    .filter(move |(parent, _, _)| *parent == class)
                    .map(move |(_, feature, made)| (*label, *feature, *made))
            })
            .collect();
        if options.is_empty() || live.len() > 14 {
            return None;
        }
        let (parent, feature, class) = options[rng.below(options.len())];
        let hops = locate(&twin.meta, &view, twin.ids.get(&parent)?)?;
        let mut node = &view;
        for hop in &hops {
            node = &node[&hop.feature][hop.at];
        }
        let len = node
            .get(feature)
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        *next += 1;
        Action::Create {
            parent,
            feature: feature.to_string(),
            pos: rng.below(len + 1),
            class: class.to_string(),
            label: *next,
        }
    } else if roll < 65 {
        // Link: a source, one of its in-scope references, a target the generated graph has an arc for.
        let mut options = Vec::new();
        for source in &live {
            let class = twin.class_of(*source)?;
            for feature in twin.meta.identity_references(twin.meta.slot(class)) {
                for target in &live {
                    let target_class = twin.class_of(*target)?;
                    if G::arc(class, &feature, target_class).is_some() {
                        options.push((*source, feature.clone(), *target));
                    }
                }
            }
        }
        if options.is_empty() {
            return None;
        }
        let (source, feature, target) = options[rng.below(options.len())].clone();
        Action::Link {
            source,
            feature,
            target,
        }
    } else if roll < 75 {
        // Unlink: one value a reference reads in the writer's view.
        let projection = twin.links_seen(writer)?;
        let options: Vec<(Label, String, Label)> = projection
            .iter()
            .flat_map(|((source, feature), targets)| {
                targets.iter().map(move |t| (*source, feature.clone(), *t))
            })
            .collect();
        if options.is_empty() {
            return None;
        }
        let (source, feature, target) = options[rng.below(options.len())].clone();
        Action::Unlink {
            source,
            feature,
            target,
        }
    } else if roll < 88 {
        let named: Vec<Label> = live
            .iter()
            .copied()
            .filter(|label| {
                twin.class_of(*label)
                    .is_some_and(|class| twin.meta.has_name(twin.meta.slot(class)))
            })
            .collect();
        if named.is_empty() {
            return None;
        }
        Action::Rename {
            target: named[rng.below(named.len())],
            ch: ['a', 'b', 'c', 'd'][rng.below(4)],
        }
    } else {
        let deletable: Vec<Label> = live.iter().copied().filter(|label| *label != 0).collect();
        if deletable.is_empty() {
            return None;
        }
        Action::Delete {
            target: deletable[rng.below(deletable.len())],
        }
    };
    Some(Edit { writer, action })
}

/// One seeded script, run as it is generated. `concurrent` scripts run three
/// rounds in which both writers make two to four edits against their own view
/// before a delivery; sequential ones deliver after every edit. Answers the
/// harness for its counters.
pub fn seeded<G: GenSide>(
    today: &Value,
    identity_descriptor: &Value,
    seed: u64,
    concurrent: bool,
    opening: &[Step],
    creatable: &[(&str, &str, &str)],
    rehearsal: bool,
) -> Result<Twin<G>, String> {
    let order = if seed % 2 == 0 {
        Order::AFirst
    } else {
        Order::BFirst
    };
    let mut twin = if rehearsal {
        Twin::<G>::rehearsal(today, identity_descriptor, order)
    } else {
        Twin::<G>::new(today, identity_descriptor, order)
    };
    for step in opening {
        match step {
            Step::Edit(edit) => {
                twin.carry(edit)?;
            }
            Step::Deliver => twin.deliver()?,
        }
    }
    twin.deliver()?;
    let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x5105_5105);
    let mut next: Label = 1000;
    let label = format!(
        "{} seed {seed}",
        if concurrent {
            "concurrent"
        } else {
            "sequential"
        }
    );
    if concurrent {
        for _ in 0..3 {
            let edits = 2 + rng.below(3);
            for _ in 0..edits {
                for writer in ['a', 'b'] {
                    if let Some(edit) = propose(&twin, &mut rng, writer, &mut next, creatable) {
                        twin.carry(&edit).map_err(|why| format!("{label}: {why}"))?;
                    }
                }
            }
            twin.deliver().map_err(|why| format!("{label}: {why}"))?;
        }
    } else {
        for index in 0..24 {
            let writer = if index % 3 == 0 { 'b' } else { 'a' };
            if let Some(edit) = propose(&twin, &mut rng, writer, &mut next, creatable) {
                twin.carry(&edit).map_err(|why| format!("{label}: {why}"))?;
                twin.deliver().map_err(|why| format!("{label}: {why}"))?;
            }
        }
    }
    Ok(twin)
}
