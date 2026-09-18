//! The equivalence oracle over `ecore_builtins.ecore`: the two paths driven
//! through Ecore's own classes.
//!
//! # What is being claimed
//!
//! The claim `generated/bt_crdt/tests/equivalence.rs` makes for `bt.ecore` and
//! `generated/json_crdt/tests/equivalence.rs` for `json.ecore`, over the one
//! metamodel in the corpus whose classes extend Ecore's: a `Part` is an
//! annotated element because `Element` extends `EModelElement`, a `Port` is
//! one because `ENamedElement` does, and an annotation is one itself, so
//! annotations nest. Three constructions are reached here that neither of the
//! other two reaches:
//!
//! - **an ordered containment of annotations**, `EModelElement.eAnnotations`,
//!   which is recursive and therefore boxed on the generated path;
//! - **`EAnnotation.details`**, a keyed map whose value is a *register over an
//!   optional value* — `UWMapLog<String, VecLog<MVRegister<Option<String>>>>`
//!   on the generated path and `LeafLog::RegisterOptional` on the interpreted
//!   one. It is the one construction that has to tell a key put with no value
//!   from a key nobody wrote, because a keyed collection drops an entry that
//!   reads as its default;
//! - **three features that carry no construction at all**: `EAnnotation`'s
//!   `contents` and `references`, typed by `EObject`, and its transient
//!   back-pointer `eModelElement`. The generated path emits nothing for them
//!   and warns; the descriptor records them as `unsupported` with a reason,
//!   and the interpreted path refuses an operation that addresses one.
//!
//! # The two arms
//!
//! Interpreted: a pair of `moirai_interp::testing::Harness` replicas, the node
//! tree under a real `Replica` with no `Install` in the way, exactly as the
//! `json.ecore` oracle drives it. Neither arm is given an opening operation of
//! its own — the root object is minted by the first `Variant(Model, …)` that
//! reaches it and the generated record is born — so event *n* is event *n* on
//! both arms and the two eg-walkers break their concurrency ties on the same
//! sequence numbers.
//!
//! Generated: a `twins_log::<AnnotatedLog>()` pair, driven by operations built
//! with this crate's own types. The other two oracles build theirs as JSON and
//! deserialize, because their encoders derive a variant name from a feature
//! name and a wrong derivation has to fail loudly rather than silently. This
//! metamodel is fixed and small enough to name every variant, so the compiler
//! does that job instead.
//!
//! # The canonical projection
//!
//! `02 Validation Plan` §2: `<Super>Super` hops flattened, `Vec<char>` joined,
//! an unset optional absent, sequences in read order, keys sorted, and every
//! object carrying the class name the *descriptor* gives it — which for
//! Ecore's own classes is `ecore::EAnnotation` and not the generated Rust
//! `EcoreEAnnotation`. Both sides take that name from the same table, so the
//! two spellings meet in exactly one place: `super_field`, which turns a
//! descriptor key back into the Rust field the generator wrote for it.
//!
//! # What the oracle excludes
//!
//! Three things, all of them `bt_crdt`'s oracle's and none of them new here.
//!
//! - **`Port.annotated`**, design §8's non-containment reference: a string on
//!   the interpreted path and a `typed_graph!` arc on the generated one, so
//!   there is nothing to compare. [`project`] drops it by rule and [`canon`]
//!   drops it from the interpreted side. `Part.subject` and
//!   `EAnnotation.references` need no exclusion: they carry no rule at all, so
//!   neither read-out has a key for them.
//! - **Every value that is the default of its feature's rule**, dropped from
//!   both sides by [`without_defaults`]. `record!`'s `new` builds one field
//!   per feature eagerly, so a generated log renders its whole shape from the
//!   moment it is constructed, while the interpreted tree holds no object
//!   until an operation mints one. An optional is exempt, because an
//!   optional's default is absence and an absent optional carries no key, so a
//!   `source` written and then emptied stays present on both sides and is
//!   compared.
//! - **An object created into an ordered containment and never written into**,
//!   criterion I-A1's one named exception (code note 31): `NestedListLog` sits
//!   on a `UWMapLog`, which keeps a child only while it differs from its
//!   default, so a freshly created annotation is indistinguishable on the
//!   generated read-out from a removed one. [`except_unwritten`] removes it
//!   from both sides, and
//!   [`an_annotation_with_nothing_written_is_invisible_on_the_generated_path`]
//!   keeps the finding visible rather than letting the workaround erase it.

use std::collections::BTreeMap;
use std::sync::Arc;

use annotated::classifiers::{
    EcoreEAnnotation, EcoreEModelElement, EcoreENamedElement, Element, Model, Part, Port,
};
use annotated::package::{Annotated, AnnotatedLog, AnnotatedValue};
use heck::ToSnakeCase;
use moirai_crdt::{
    list::{eg_walker::List, nested_list::NestedList},
    map::uw_map::UWMap,
    option::Optional,
    register::mv_register::MVRegister,
    utils::membership::twins_log,
};
use moirai_interp::testing::{class_slot, feature_slot, twins};
use moirai_interp::{InstanceOp, LeafOp, Scalar};
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::Tcsb;
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::{IsReplica, Replica};
use moirai_semantics::{
    ClassSlot, LeafRule, MergeRule, MetamodelSemantics, Shape, from_descriptor,
};
use serde_json::{Map, Value, json};

type InterpReplica = Replica<moirai_interp::testing::Harness, Tcsb<InstanceOp>>;
type GenReplica = Replica<AnnotatedLog, Tcsb<Annotated>>;

/// The key the canonical form carries a class name under.
const ECLASS: &str = "eClass";

/// The descriptor keys of the four classes this oracle names.
const MODEL: &str = "Model";
const PART: &str = "Part";
const PORT: &str = "Port";
const ANNOTATION: &str = "ecore::EAnnotation";

// ---------------------------------------------------------------------------
// 1. The table
// ---------------------------------------------------------------------------

/// This crate's own descriptor, which is the one the interpreted arm runs.
fn descriptor() -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/metamodel.json"))
        .expect("this crate's own descriptor is readable");
    serde_json::from_str(&text).expect("it is JSON")
}

struct Meta {
    sem: Arc<MetamodelSemantics>,
}

impl Meta {
    fn new() -> Meta {
        Meta {
            sem: Arc::new(from_descriptor(&descriptor()).expect("the descriptor is a table")),
        }
    }

    fn class(&self, name: &str) -> ClassSlot {
        class_slot(&self.sem, name)
    }

    fn name(&self, class: ClassSlot) -> &str {
        &self.sem.classes[class.index()].name
    }

    /// The **visible** slot of one feature on one class, which is what an
    /// `InstanceOp::Field` carries.
    fn slot(&self, class: &str, feature: &str) -> moirai_semantics::FeatureSlot {
        feature_slot(&self.sem, self.class(class), feature)
    }
}

// ---------------------------------------------------------------------------
// 2. What an edit is
// ---------------------------------------------------------------------------

/// Which element an edit addresses, from the root down: a part, then
/// optionally one of its ports, then a chain of annotations. Every step is a
/// position in an ordered containment, resolved against the writer's own view
/// at the moment the edit was generated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Place {
    part: Option<usize>,
    port: Option<usize>,
    annotations: Vec<usize>,
}

impl Place {
    fn part(index: usize) -> Place {
        Place {
            part: Some(index),
            ..Place::default()
        }
    }

    fn port(part: usize, port: usize) -> Place {
        Place {
            part: Some(part),
            port: Some(port),
            annotations: Vec::new(),
        }
    }

    fn annotation(mut self, index: usize) -> Place {
        self.annotations.push(index);
        self
    }

    /// The descriptor key of the class of the element this addresses.
    fn class(&self) -> &'static str {
        if !self.annotations.is_empty() {
            ANNOTATION
        } else if self.port.is_some() {
            PORT
        } else if self.part.is_some() {
            PART
        } else {
            MODEL
        }
    }

    /// The optional text every element but the model has, by feature name.
    fn text(&self) -> &'static str {
        match self.class() {
            ANNOTATION => "source",
            PORT => "name",
            PART => "elementId",
            other => panic!("`{other}` has no optional text"),
        }
    }

    fn show(&self) -> String {
        let mut out = String::new();
        if let Some(part) = self.part {
            out.push_str(&format!("/parts[{part}]"));
        }
        if let Some(port) = self.port {
            out.push_str(&format!("/ports[{port}]"));
        }
        for index in &self.annotations {
            out.push_str(&format!("/eAnnotations[{index}]"));
        }
        if out.is_empty() {
            "/".to_string()
        } else {
            out
        }
    }
}

#[derive(Clone, Debug)]
enum Action {
    /// Create an object *and write into it*, in one operation.
    ///
    /// Never a bare creation: `NestedListLog`'s children sit in a `UWMapLog`,
    /// which keeps a child only while it differs from its default, so an
    /// object created and left empty is not on the generated read-out at all
    /// — and a later operation addressing a position past it would then mean
    /// two different children on the two paths. The divergence is real and is
    /// criterion I-A1's named exception; what a script must not do is *build*
    /// on it. So every creation seeds the new object's optional text, exactly
    /// as the `json.ecore` oracle's seeds bottom out in a character.
    /// [`Action::AddBareAnnotation`] is the one that does not, and no script
    /// ever proposes it.
    AddPart { pos: usize, seed: char },
    DeletePart { pos: usize },
    AddPort { pos: usize, seed: char },
    DeletePort { pos: usize },
    AddAnnotation { pos: usize, seed: char },
    AddBareAnnotation { pos: usize },
    DeleteAnnotation { pos: usize },
    InsertChar { pos: usize, ch: char },
    DeleteChar { pos: usize },
    UnsetText,
    PutDetail { key: String, value: Option<String> },
    RemoveDetail { key: String },
    ClearDetail { key: String },
    ClearDetails,
}

#[derive(Clone, Debug)]
struct Edit {
    writer: char,
    at: Place,
    action: Action,
}

impl Edit {
    fn show(&self) -> String {
        format!("{} {} {:?}", self.writer, self.at.show(), self.action)
    }
}

/// An edit by `a`, spelled short.
fn at(place: Place, action: Action) -> Edit {
    Edit {
        writer: 'a',
        at: place,
        action,
    }
}

#[derive(Clone, Debug)]
enum Move {
    Edit(Edit),
    Deliver,
}

struct EditScript {
    label: String,
    steps: Vec<Move>,
}

// ---------------------------------------------------------------------------
// 3. The interpreted encoder
// ---------------------------------------------------------------------------

/// One edit as an `InstanceOp`.
///
/// Inheritance is flat here: `eAnnotations` is one visible slot on `Part`,
/// on `Port` and on the annotation itself, whichever class declares it, which
/// is the whole difference from the typed encoder below.
fn interp_op(meta: &Meta, edit: &Edit) -> InstanceOp {
    let class = edit.at.class();
    let inner = interp_action(meta, class, &edit.action);

    // The chain from the root down, each step naming the class that owns the
    // collection, the collection, the position, and the class of the child.
    let mut steps: Vec<(&str, &str, usize, &str)> = Vec::new();
    if let Some(part) = edit.at.part {
        steps.push((MODEL, "parts", part, PART));
    }
    if let Some(port) = edit.at.port {
        steps.push((PART, "ports", port, PORT));
    }
    let owner_of_annotations = if edit.at.port.is_some() { PORT } else { PART };
    for (depth, index) in edit.at.annotations.iter().enumerate() {
        let owner = if depth == 0 {
            owner_of_annotations
        } else {
            ANNOTATION
        };
        steps.push((owner, "eAnnotations", *index, ANNOTATION));
    }

    let mut op = inner;
    for (owner, feature, index, child) in steps.iter().rev() {
        op = InstanceOp::variant(meta.class(child), op);
        op = InstanceOp::field(meta.slot(owner, feature), InstanceOp::at(*index, op));
    }
    InstanceOp::variant(meta.class(MODEL), op)
}

/// The body of an edit, inside the addressed object's own `Variant`.
fn interp_action(meta: &Meta, class: &str, action: &Action) -> InstanceOp {
    let seq = |feature: &str, op: InstanceOp| InstanceOp::field(meta.slot(class, feature), op);
    match action {
        Action::AddPart { pos, seed } => seq(
            "parts",
            InstanceOp::insert(
                *pos,
                InstanceOp::variant(meta.class(PART), interp_seed(meta, PART, *seed)),
            ),
        ),
        Action::DeletePart { pos } => seq("parts", InstanceOp::delete(*pos)),
        Action::AddPort { pos, seed } => seq(
            "ports",
            InstanceOp::insert(
                *pos,
                InstanceOp::variant(meta.class(PORT), interp_seed(meta, PORT, *seed)),
            ),
        ),
        Action::DeletePort { pos } => seq("ports", InstanceOp::delete(*pos)),
        Action::AddAnnotation { pos, seed } => seq(
            "eAnnotations",
            InstanceOp::insert(
                *pos,
                InstanceOp::variant(meta.class(ANNOTATION), interp_seed(meta, ANNOTATION, *seed)),
            ),
        ),
        Action::AddBareAnnotation { pos } => seq(
            "eAnnotations",
            InstanceOp::insert(
                *pos,
                InstanceOp::variant(meta.class(ANNOTATION), InstanceOp::New),
            ),
        ),
        Action::DeleteAnnotation { pos } => seq("eAnnotations", InstanceOp::delete(*pos)),
        Action::InsertChar { pos, ch } => seq(
            text_of(class),
            InstanceOp::set(InstanceOp::Leaf(LeafOp::InsertChar { pos: *pos, ch: *ch })),
        ),
        Action::DeleteChar { pos } => seq(
            text_of(class),
            InstanceOp::set(InstanceOp::Leaf(LeafOp::DeleteChar { pos: *pos })),
        ),
        Action::UnsetText => seq(text_of(class), InstanceOp::unset()),
        Action::PutDetail { key, value } => seq(
            "details",
            InstanceOp::entry(
                Scalar::text(key.clone()),
                InstanceOp::Leaf(LeafOp::Write(scalar(value.as_deref()))),
            ),
        ),
        Action::RemoveDetail { key } => {
            seq("details", InstanceOp::remove(Scalar::text(key.clone())))
        }
        Action::ClearDetail { key } => seq(
            "details",
            InstanceOp::entry(Scalar::text(key.clone()), InstanceOp::Leaf(LeafOp::Clear)),
        ),
        Action::ClearDetails => seq("details", InstanceOp::clear()),
    }
}

/// The payload that mints an object of `class` and writes its optional text
/// in the same operation.
fn interp_seed(meta: &Meta, class: &str, seed: char) -> InstanceOp {
    InstanceOp::field(
        meta.slot(class, text_of(class)),
        InstanceOp::set(InstanceOp::Leaf(LeafOp::InsertChar { pos: 0, ch: seed })),
    )
}

/// The optional text of one class, by the descriptor's own feature name.
fn text_of(class: &str) -> &'static str {
    match class {
        ANNOTATION => "source",
        PORT => "name",
        PART => "elementId",
        other => panic!("`{other}` has no optional text"),
    }
}

/// A detail value as the interpreted vocabulary spells it: `Scalar::Null` is
/// this vocabulary's absent value, which is what the generated `None` is.
fn scalar(value: Option<&str>) -> Scalar {
    match value {
        Some(value) => Scalar::text(value),
        None => Scalar::Null,
    }
}

// ---------------------------------------------------------------------------
// 4. The generated encoder
// ---------------------------------------------------------------------------

/// An operation on whichever record the chain has reached.
enum Carried {
    Model(Model),
    Part(Part),
    Port(Port),
    Annotation(EcoreEAnnotation),
}

/// An operation on `EModelElement`'s own features, lifted to the record of the
/// class that inherits it: one `<Super>Super` hop per level of the chain the
/// descriptor's `supers` records, written out.
fn part_element(op: EcoreEModelElement) -> Part {
    Part::ElementSuper(Element::EModelElementSuper(op))
}

fn port_element(op: EcoreEModelElement) -> Port {
    Port::ENamedElementSuper(EcoreENamedElement::EModelElementSuper(op))
}

fn annotation_element(op: EcoreEModelElement) -> EcoreEAnnotation {
    EcoreEAnnotation::EModelElementSuper(op)
}

fn typed_op(edit: &Edit) -> Annotated {
    let mut carried = typed_action(&edit.at, &edit.action);

    if !edit.at.annotations.is_empty() {
        let Carried::Annotation(mut op) = carried else {
            panic!("an annotation place carries an annotation operation");
        };
        // The chain, innermost first: every index but the first hangs off
        // another annotation.
        for index in edit.at.annotations[1..].iter().rev() {
            op = annotation_element(EcoreEModelElement::EAnnotations(NestedList::Update {
                pos: *index,
                op: op.into(),
            }));
        }
        let outermost = EcoreEModelElement::EAnnotations(NestedList::Update {
            pos: edit.at.annotations[0],
            op: op.into(),
        });
        carried = match edit.at.port {
            Some(_) => Carried::Port(port_element(outermost)),
            None => Carried::Part(part_element(outermost)),
        };
    }

    if let Some(port) = edit.at.port {
        let Carried::Port(op) = carried else {
            panic!("a port place carries a port operation");
        };
        carried = Carried::Part(Part::Ports(NestedList::Update { pos: port, op }));
    }

    if let Some(part) = edit.at.part {
        let Carried::Part(op) = carried else {
            panic!("a part place carries a part operation");
        };
        carried = Carried::Model(Model::Parts(NestedList::Update { pos: part, op }));
    }

    match carried {
        Carried::Model(op) => Annotated::Model(op),
        _ => panic!("the chain did not reach the model"),
    }
}

fn typed_action(place: &Place, action: &Action) -> Carried {
    match place.class() {
        MODEL => Carried::Model(match action {
            Action::AddPart { pos, seed } => Model::Parts(NestedList::Insert {
                pos: *pos,
                op: Part::ElementSuper(Element::ElementId(Optional::Set(List::Insert {
                    content: *seed,
                    pos: 0,
                }))),
            }),
            Action::DeletePart { pos } => Model::Parts(NestedList::Delete { pos: *pos }),
            other => panic!("the model takes no {other:?}"),
        }),
        PART => Carried::Part(match action {
            Action::AddPort { pos, seed } => Part::Ports(NestedList::Insert {
                pos: *pos,
                op: Port::ENamedElementSuper(EcoreENamedElement::Name(Optional::Set(
                    List::Insert {
                        content: *seed,
                        pos: 0,
                    },
                ))),
            }),
            Action::DeletePort { pos } => Part::Ports(NestedList::Delete { pos: *pos }),
            Action::AddAnnotation { .. }
            | Action::AddBareAnnotation { .. }
            | Action::DeleteAnnotation { .. } => part_element(typed_annotations(action)),
            Action::InsertChar { .. } | Action::DeleteChar { .. } | Action::UnsetText => {
                Part::ElementSuper(Element::ElementId(typed_text(action)))
            }
            other => panic!("a part takes no {other:?}"),
        }),
        PORT => Carried::Port(match action {
            Action::AddAnnotation { .. }
            | Action::AddBareAnnotation { .. }
            | Action::DeleteAnnotation { .. } => port_element(typed_annotations(action)),
            Action::InsertChar { .. } | Action::DeleteChar { .. } | Action::UnsetText => {
                Port::ENamedElementSuper(EcoreENamedElement::Name(typed_text(action)))
            }
            other => panic!("a port takes no {other:?}"),
        }),
        ANNOTATION => Carried::Annotation(match action {
            Action::AddAnnotation { .. }
            | Action::AddBareAnnotation { .. }
            | Action::DeleteAnnotation { .. } => annotation_element(typed_annotations(action)),
            Action::InsertChar { .. } | Action::DeleteChar { .. } | Action::UnsetText => {
                EcoreEAnnotation::Source(typed_text(action))
            }
            Action::PutDetail { key, value } => EcoreEAnnotation::Details(UWMap::Update(
                key.clone(),
                MVRegister::Write(value.clone()),
            )),
            Action::RemoveDetail { key } => EcoreEAnnotation::Details(UWMap::Remove(key.clone())),
            Action::ClearDetail { key } => {
                EcoreEAnnotation::Details(UWMap::Update(key.clone(), MVRegister::Clear))
            }
            Action::ClearDetails => EcoreEAnnotation::Details(UWMap::Clear),
            other => panic!("an annotation takes no {other:?}"),
        }),
        other => panic!("no class `{other}`"),
    }
}

fn typed_annotations(action: &Action) -> EcoreEModelElement {
    match action {
        Action::AddAnnotation { pos, seed } => {
            EcoreEModelElement::EAnnotations(NestedList::Insert {
                pos: *pos,
                op: EcoreEAnnotation::Source(Optional::Set(List::Insert {
                    content: *seed,
                    pos: 0,
                }))
                .into(),
            })
        }
        Action::AddBareAnnotation { pos } => {
            EcoreEModelElement::EAnnotations(NestedList::Insert {
                pos: *pos,
                op: EcoreEAnnotation::New.into(),
            })
        }
        Action::DeleteAnnotation { pos } => {
            EcoreEModelElement::EAnnotations(NestedList::Delete { pos: *pos })
        }
        other => panic!("{other:?} is not an annotation-list operation"),
    }
}

fn typed_text(action: &Action) -> Optional<List<char>> {
    match action {
        Action::InsertChar { pos, ch } => Optional::Set(List::Insert {
            content: *ch,
            pos: *pos,
        }),
        Action::DeleteChar { pos } => Optional::Set(List::Delete { pos: *pos }),
        Action::UnsetText => Optional::Unset,
        other => panic!("{other:?} is not a text operation"),
    }
}

// ---------------------------------------------------------------------------
// 5. The canonical projection
// ---------------------------------------------------------------------------

/// The generated read-out in the canonical form of `02 Validation Plan` §2,
/// with every value that is the default of its feature's rule dropped.
fn project(meta: &Meta, value: &AnnotatedValue) -> Value {
    let raw = serde_json::to_value(&value.model).expect("the generated read-out serializes");
    without_defaults(meta, project_object(meta, meta.class(MODEL), &raw))
}

fn project_object(meta: &Meta, class: ClassSlot, value: &Value) -> Value {
    let mut out = Map::new();
    out.insert(ECLASS.to_string(), json!(meta.name(class)));
    for (name, owner, slot) in &meta.sem.classes[class.index()].visible {
        let Some(rule) = meta.sem.rule(*owner, *slot) else {
            continue;
        };
        match rule {
            // Design §8, and the two `EObject` forms and the transient
            // back-pointer: nothing is emitted on either path, so there is
            // nothing to compare and `canon` drops the other side's key too.
            MergeRule::Reference { .. } | MergeRule::Unsupported { .. } => continue,
            MergeRule::Attribute {
                shape: Shape::Optional,
                leaf: LeafRule::Text,
            } => {
                let found = locate(meta, class, *owner, name, value);
                if !found.is_null() {
                    out.insert(name.to_string(), json!(chars(found)));
                }
            }
            MergeRule::Attribute {
                shape: Shape::Keyed { .. },
                leaf: LeafRule::OptionalRegister { .. },
            } => {
                let found = locate(meta, class, *owner, name, value);
                out.insert(name.to_string(), details(found));
            }
            MergeRule::Containment {
                shape: Shape::Sequence,
                target,
            } => {
                let found = locate(meta, class, *owner, name, value);
                let items = found
                    .as_array()
                    .unwrap_or_else(|| panic!("`{name}` reads as an array"))
                    .iter()
                    .map(|item| project_object(meta, *target, item))
                    .collect();
                out.insert(name.to_string(), Value::Array(items));
            }
            other => panic!(
                "no projection for {other:?} on `{}.{name}`; `ecore_builtins.ecore` has \
                 none and this oracle claims none",
                meta.name(class)
            ),
        }
    }
    Value::Object(out)
}

/// The generated field holding `feature`, reached from `class` through the
/// `<Super>Super` hops down to the class that declares it.
fn locate<'a>(
    meta: &Meta,
    class: ClassSlot,
    owner: ClassSlot,
    feature: &str,
    value: &'a Value,
) -> &'a Value {
    let mut here = value;
    let mut current = class;
    while current != owner {
        let supers = &meta.sem.classes[current.index()].supers;
        // Every class of this metamodel has at most one supertype, and the
        // one that leads to the owner is the one whose closure holds it.
        let next = *supers
            .iter()
            .find(|candidate| reaches(meta, **candidate, owner))
            .unwrap_or_else(|| {
                panic!(
                    "`{}` does not inherit `{feature}` from `{}`",
                    meta.name(class),
                    meta.name(owner)
                )
            });
        here = here
            .get(super_field(meta.name(next)))
            .unwrap_or_else(|| panic!("no `{}` field", super_field(meta.name(next))));
        current = next;
    }
    here.get(feature.to_snake_case())
        .unwrap_or_else(|| panic!("no `{feature}` field on `{}`", meta.name(current)))
}

fn reaches(meta: &Meta, from: ClassSlot, target: ClassSlot) -> bool {
    from == target
        || meta.sem.classes[from.index()]
            .supers
            .iter()
            .any(|next| reaches(meta, *next, target))
}

/// The Rust field the generator writes for an inherited class
/// (`classifier/mod.rs:62-70`): the class's *Ecore* name in snake case, with
/// `_super`.
///
/// The one place the descriptor's spelling and the generated path's meet. The
/// descriptor keys Ecore's own classes `ecore::EModelElement`, because its
/// keys are strings and a metamodel may declare an `EModelElement` of its
/// own; a Rust identifier cannot hold a `::`, so `ident.rs` gives them a
/// reserved `Ecore` prefix instead and renames a clashing class of the
/// metamodel. Neither renaming reaches a field name, which the generator
/// derives from the bare Ecore name on both sides of the prefix.
fn super_field(class: &str) -> String {
    let bare = class.rsplit("::").next().unwrap_or(class);
    format!("{}_super", bare.to_snake_case())
}

/// A `Vec<char>` read-out, joined.
fn chars(value: &Value) -> String {
    value
        .as_array()
        .unwrap_or_else(|| panic!("a text reads as an array of characters, got {value}"))
        .iter()
        .map(|ch| ch.as_str().unwrap_or_default().to_string())
        .collect()
}

/// A `HashMap<String, HashSet<Option<String>>>` read-out, canonical: every key
/// the map still holds, and under it every value written there in order, with
/// `null` for the value that is not one. That is what
/// `LeafLog::RegisterOptional` reads as, and it is the form that tells a key
/// put with no value from a key nobody wrote.
fn details(value: &Value) -> Value {
    let mut out = Map::new();
    for (key, values) in value
        .as_object()
        .unwrap_or_else(|| panic!("`details` reads as an object, got {value}"))
    {
        let mut held: Vec<Value> = values
            .as_array()
            .unwrap_or_else(|| panic!("a detail reads as a set, got {values}"))
            .clone();
        held.sort_by(|left, right| match (left, right) {
            (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
            (Value::Null, _) => std::cmp::Ordering::Less,
            (_, Value::Null) => std::cmp::Ordering::Greater,
            (left, right) => left.as_str().unwrap_or("").cmp(right.as_str().unwrap_or("")),
        });
        out.insert(key.clone(), Value::Array(held));
    }
    Value::Object(out)
}

/// The interpreted read-out with every non-containment reference dropped and
/// every default-valued key with it: the two edits this oracle makes to the
/// canonical form the interpreter already produces.
fn canon(meta: &Meta, value: &Value) -> Value {
    without_defaults(meta, strip_references(meta, value))
}

/// The interpreted read-out with `Port.annotated` dropped and nothing else
/// touched: sparse exactly as `eval::read` wrote it, which is the view the
/// script generator proposes against. [`without_defaults`] spells a model
/// whose features are all still default as `null`, which is right for the
/// comparison and useless for generation.
fn strip_references(meta: &Meta, value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| strip_references(meta, item))
                .collect(),
        ),
        Value::Object(map) => {
            let class = map
                .get(ECLASS)
                .and_then(Value::as_str)
                .map(|name| meta.class(name));
            let mut out = Map::new();
            for (key, child) in map {
                if let Some(class) = class
                    && let Some((_, owner, slot)) = meta.sem.classes[class.index()]
                        .visible
                        .iter()
                        .find(|(name, _, _)| &**name == key)
                    && matches!(
                        meta.sem.rule(*owner, *slot),
                        Some(MergeRule::Reference { .. })
                    )
                {
                    continue;
                }
                out.insert(key.clone(), strip_references(meta, child));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// A canonical model with every value that is the default of its feature's
/// rule dropped, and a root left with nothing but its class spelled `null`.
///
/// `record!`'s `new` builds one field per feature eagerly, so a generated log
/// renders its whole shape — `Model.parts` at `[]`, an annotation's `details`
/// at `{}` — from the moment it is constructed, before any operation at all;
/// the interpreted tree holds no object until one mints it and reads the whole
/// model as `null` until then. Both describe the same state and the canonical
/// form of `02 Validation Plan` §2 had no way to say so. An **optional** is
/// exempt: its default is absence, absence already carries no key, so a key
/// present under one was put there by an operation.
fn without_defaults(meta: &Meta, value: Value) -> Value {
    let value = drop_defaults(meta, value);
    if only_a_class(&value) { Value::Null } else { value }
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
            let class = map
                .get(ECLASS)
                .and_then(Value::as_str)
                .map(|name| meta.class(name));
            let mut out = Map::new();
            for (key, item) in map {
                let item = drop_defaults(meta, item);
                if let Some(class) = class
                    && key != ECLASS
                    && rule_of(meta, class, &key).is_some_and(|rule| is_default(&rule, &item))
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

fn rule_of(meta: &Meta, class: ClassSlot, feature: &str) -> Option<MergeRule> {
    let (_, owner, slot) = meta.sem.classes[class.index()]
        .visible
        .iter()
        .find(|(name, _, _)| &**name == feature)?;
    meta.sem.rule(*owner, *slot).copied()
}

/// Whether a canonical value, its own defaults already dropped, is the default
/// of the rule the feature carrying it is bound to.
fn is_default(rule: &MergeRule, value: &Value) -> bool {
    match rule {
        MergeRule::Reference { .. } | MergeRule::Unsupported { .. } => false,
        MergeRule::Attribute { shape, .. } => match shape {
            // Present means written. See the note on the exemption above.
            Shape::Optional => false,
            Shape::Keyed { .. } => value.as_object().is_some_and(Map::is_empty),
            other => panic!("`ecore_builtins.ecore` reaches no {other:?} attribute"),
        },
        MergeRule::Containment { shape, .. } => match shape {
            Shape::Sequence => value.as_array().is_some_and(Vec::is_empty),
            other => panic!("`ecore_builtins.ecore` reaches no {other:?} containment"),
        },
    }
}

/// A canonical model with criterion I-A1's one named exception removed: an
/// object created into an ordered containment and never written into.
///
/// `NestedListLog` keeps its children in a `UWMapLog`, which keeps a child
/// only while it differs from its default, and it must: `UWMap::Remove` is not
/// a tombstone, so reading as the default is exactly how the generated path
/// spells *removed*. A freshly created annotation, part or port is therefore
/// indistinguishable on the generated read-out from a removed one, while the
/// interpreted path mints it and shows it. Applied to **both** sides by this
/// one function, on top of the projection and never inside it, so the two
/// sides cannot drift.
fn except_unwritten(meta: &Meta, mut value: Value) -> Value {
    for _ in 0..16 {
        let next = without_defaults(meta, drop_empty_children(value.clone()));
        if next == value {
            break;
        }
        value = next;
    }
    value
}

fn drop_empty_children(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(drop_empty_children)
                .filter(|item| !only_a_class(item))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, item)| (key, drop_empty_children(item)))
                .collect(),
        ),
        other => other,
    }
}

/// The first place two canonical models differ, as a path and the two values.
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
                    "{at}: interpreted holds {} items, generated {}",
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

struct Harness {
    meta: Meta,
    ia: InterpReplica,
    ib: InterpReplica,
    ga: GenReplica,
    gb: GenReplica,
    pending_a: Vec<(EventMessage<InstanceOp>, EventMessage<Annotated>)>,
    pending_b: Vec<(EventMessage<InstanceOp>, EventMessage<Annotated>)>,
    ops: usize,
    refused: usize,
    comparisons: usize,
}

impl Harness {
    fn new() -> Harness {
        let meta = Meta::new();
        let (ia, ib) = twins(&meta.sem, MODEL);
        let (ga, gb) = twins_log::<AnnotatedLog>();
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

    /// Where every script starts. Both arms read `null` here: the generated
    /// record is born and the interpreted tree is not, and
    /// [`without_defaults`] is what says the two are one state.
    fn opened() -> Harness {
        Harness::new()
    }

    /// The interpreted read-out as a writer sees it: sparse, unpruned, and
    /// what the script generator proposes against.
    fn interp_doc(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ia } else { &self.ib };
        strip_references(&self.meta, &replica.query(&Read::<Value>::new()))
    }

    fn interp_canon(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ia } else { &self.ib };
        except_unwritten(
            &self.meta,
            canon(&self.meta, &replica.query(&Read::<Value>::new())),
        )
    }

    fn gen_doc(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ga } else { &self.gb };
        except_unwritten(
            &self.meta,
            project(&self.meta, &replica.query(&Read::<AnnotatedValue>::new())),
        )
    }

    fn compare(&mut self) -> Result<(), String> {
        for writer in ['a', 'b'] {
            self.comparisons += 1;
            let interp = self.interp_canon(writer);
            let generated = self.gen_doc(writer);
            if interp != generated {
                let where_ = difference(&interp, &generated, "")
                    .unwrap_or_else(|| "the models differ but no key does".to_string());
                return Err(format!(
                    "replica {writer}: {where_}\n  interpreted: {interp}\n  generated:   {generated}"
                ));
            }
        }
        Ok(())
    }

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
            (Ok(interp_event), Ok(gen_event)) => {
                let pending = if edit.writer == 'a' {
                    &mut self.pending_a
                } else {
                    &mut self.pending_b
                };
                pending.push((interp_event, gen_event));
                Ok(true)
            }
            (Err(_), Err(_)) => {
                self.refused += 1;
                Ok(false)
            }
            (interp_event, _) => Err(format!(
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
            .map_err(|reason| format!("{}: final delivery\n{reason}", script.label))
    }
}

// ---------------------------------------------------------------------------
// 7. Seeded scripts
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

/// Every element a writer looking at `doc` can address, with the place that
/// names it, in a deterministic order.
fn places(doc: &Value) -> Vec<Place> {
    const DEPTH_CAP: usize = 2;
    let mut out = vec![Place::default()];
    let parts = doc.get("parts").and_then(Value::as_array);
    for (index, part) in parts.into_iter().flatten().enumerate() {
        let place = Place::part(index);
        annotations_of(part, &place, DEPTH_CAP, &mut out);
        out.push(place.clone());
        for (port_index, port) in part
            .get("ports")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let port_place = Place::port(index, port_index);
            annotations_of(port, &port_place, DEPTH_CAP, &mut out);
            out.push(port_place);
        }
    }
    out
}

fn annotations_of(element: &Value, place: &Place, depth: usize, out: &mut Vec<Place>) {
    if depth == 0 {
        return;
    }
    for (index, annotation) in element
        .get("eAnnotations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let here = place.clone().annotation(index);
        annotations_of(annotation, &here, depth - 1, out);
        out.push(here);
    }
}

/// The element a place names, in the document.
fn element<'a>(doc: &'a Value, place: &Place) -> Option<&'a Value> {
    let mut here = doc;
    if let Some(part) = place.part {
        here = here.get("parts")?.as_array()?.get(part)?;
    }
    if let Some(port) = place.port {
        here = here.get("ports")?.as_array()?.get(port)?;
    }
    for index in &place.annotations {
        here = here.get("eAnnotations")?.as_array()?.get(*index)?;
    }
    Some(here)
}

fn len_of(element: &Value, feature: &str) -> usize {
    element
        .get(feature)
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

/// One edit a writer looking at `doc` could make.
///
/// `creating` is false for the second writer of a concurrent round. Two
/// writers inserting into *one* ordered containment at once is the finding
/// `json_crdt`'s `ip28` already drives and pins — the nested list's own
/// resolution puts two concurrent inserts in opposite orders on the two paths
/// — and on this metamodel it also reaches `moirai-protocol`'s `CachedLog`
/// defect, which panics rather than diverges. Both are Moirai's and neither
/// is Ecore's, so the scripts here stay off them and
/// [`ip36_two_writers_inserting_into_one_sequence_at_once_is_moirais_own_finding`]
/// drives them on their own.
fn propose(doc: &Value, rng: &mut Rng, writer: char, creating: bool) -> Option<Edit> {
    const ALPHABET: [char; 4] = ['a', 'b', 'c', 'd'];
    const KEYS: [&str; 3] = ["one", "two", "three"];
    const PART_CAP: usize = 3;
    const ANNOTATION_CAP: usize = 3;

    let found = places(doc);
    let place = found[rng.below(found.len())].clone();
    let element = element(doc, &place)?;
    let mut choices: Vec<Action> = Vec::new();

    match place.class() {
        MODEL => {
            let parts = len_of(element, "parts");
            if creating && parts < PART_CAP {
                choices.push(Action::AddPart {
                    pos: rng.below(parts + 1),
                    seed: ALPHABET[rng.below(ALPHABET.len())],
                });
            }
            if creating && parts > 0 && rng.below(4) == 0 {
                choices.push(Action::DeletePart {
                    pos: rng.below(parts),
                });
            }
        }
        _ => {
            let annotations = len_of(element, "eAnnotations");
            if creating && annotations < ANNOTATION_CAP {
                choices.push(Action::AddAnnotation {
                    pos: rng.below(annotations + 1),
                    seed: ALPHABET[rng.below(ALPHABET.len())],
                });
            }
            if creating && annotations > 0 && rng.below(4) == 0 {
                choices.push(Action::DeleteAnnotation {
                    pos: rng.below(annotations),
                });
            }
            let text = element
                .get(place.text())
                .and_then(Value::as_str)
                .unwrap_or_default();
            let len = text.chars().count();
            choices.push(Action::InsertChar {
                pos: rng.below(len + 1),
                ch: ALPHABET[rng.below(ALPHABET.len())],
            });
            if len > 0 {
                choices.push(Action::DeleteChar {
                    pos: rng.below(len),
                });
            }
            if element.get(place.text()).is_some() && rng.below(8) == 0 {
                choices.push(Action::UnsetText);
            }
            if place.class() == PART {
                let ports = len_of(element, "ports");
                if creating && ports < PART_CAP {
                    choices.push(Action::AddPort {
                        pos: rng.below(ports + 1),
                        seed: ALPHABET[rng.below(ALPHABET.len())],
                    });
                }
                if creating && ports > 0 && rng.below(4) == 0 {
                    choices.push(Action::DeletePort {
                        pos: rng.below(ports),
                    });
                }
            }
            if place.class() == ANNOTATION {
                let key = KEYS[rng.below(KEYS.len())].to_string();
                let held: Vec<String> = element
                    .get("details")
                    .and_then(Value::as_object)
                    .map(|map| map.keys().cloned().collect())
                    .unwrap_or_default();
                choices.push(Action::PutDetail {
                    key: key.clone(),
                    // One put in four writes no value at all, which is the
                    // state this construction exists for.
                    value: if rng.below(4) == 0 {
                        None
                    } else {
                        Some(ALPHABET[rng.below(ALPHABET.len())].to_string())
                    },
                });
                if !held.is_empty() {
                    let held_key = held[rng.below(held.len())].clone();
                    choices.push(Action::RemoveDetail {
                        key: held_key.clone(),
                    });
                    choices.push(Action::ClearDetail { key: held_key });
                    if rng.below(8) == 0 {
                        choices.push(Action::ClearDetails);
                    }
                }
            }
        }
    }

    if choices.is_empty() {
        return None;
    }
    let action = choices[rng.below(choices.len())].clone();
    Some(Edit {
        writer,
        at: place,
        action,
    })
}

/// Ten sequential scripts and twenty concurrent ones over
/// `ecore_builtins.ecore`, proposed against a shadow pair driven by the same
/// edits so that a writer only ever proposes against what it can see.
fn seeded_script(seed: u64, concurrent: bool) -> EditScript {
    let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x5EED_0000);
    let mut steps: Vec<Move> = Vec::new();
    let mut shadow = Harness::opened();
    let mut broken = false;

    if concurrent {
        // Two writers cannot disagree inside a part that only one of them has,
        // so the opening edits are `a`'s alone and are delivered.
        for _ in 0..2 {
            let doc = shadow.interp_doc('a');
            if let Some(edit) = propose(&doc, &mut rng, 'a', true) {
                broken = shadow.carry(&edit).is_err();
                steps.push(Move::Edit(edit));
            }
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
                    // Only `a` creates while the round is in flight.
                    if let Some(edit) = propose(&doc, &mut rng, writer, writer == 'a') {
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
            if let Some(edit) = propose(&doc, &mut rng, writer, true) {
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
// 8. The tests
// ---------------------------------------------------------------------------

/// The descriptor the interpreted arm runs is this crate's own, and it carries
/// Ecore's classes under the keys the emitter chose. Without this, everything
/// below compares two metamodels rather than two paths.
#[test]
fn the_two_arms_hold_the_same_metamodel() {
    let meta = Meta::new();
    assert_eq!(&*meta.sem.package, "annotated");
    assert_eq!(meta.sem.roots, vec![meta.class(MODEL)]);
    let mut names: Vec<&str> = meta
        .sem
        .classes
        .iter()
        .map(|class| &*class.name)
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "Element",
            "Model",
            "Part",
            "Port",
            "ecore::EAnnotation",
            "ecore::EModelElement",
            "ecore::ENamedElement",
            "ecore::EObject",
            "ecore::EStringToStringMapEntry",
        ]
    );
}

/// The three rules this metamodel reaches and the other two oracles do not,
/// read out of the table the interpreted arm runs.
#[test]
fn the_metamodel_reaches_the_forms_the_other_oracles_do_not() {
    let meta = Meta::new();
    let annotation = meta.class(ANNOTATION);
    let rule = |feature: &str| -> MergeRule {
        let (_, owner, slot) = meta.sem.classes[annotation.index()]
            .visible
            .iter()
            .find(|(name, _, _)| &**name == feature)
            .unwrap_or_else(|| panic!("`{ANNOTATION}` sees no `{feature}`"));
        *meta.sem.rule(*owner, *slot).expect("a rule")
    };
    assert!(matches!(
        rule("details"),
        MergeRule::Attribute {
            shape: Shape::Keyed { .. },
            leaf: LeafRule::OptionalRegister { class: None }
        }
    ));
    assert!(matches!(
        rule("eAnnotations"),
        MergeRule::Containment {
            shape: Shape::Sequence,
            ..
        }
    ));
    for feature in ["contents", "references", "eModelElement"] {
        assert!(
            matches!(rule(feature), MergeRule::Unsupported { .. }),
            "`{feature}` carries no construction on either path"
        );
    }
}

/// An operation addressing a feature neither path represents is refused, by
/// name and with the reason. The generated path has no operation to offer for
/// one at all, which is the other half of the same fact.
#[test]
fn an_eobject_feature_has_no_operation_on_either_path() {
    let meta = Meta::new();
    let annotation = meta.class(ANNOTATION);
    let (mut a, _b) = twins(&meta.sem, ANNOTATION);
    for feature in ["contents", "references"] {
        let slot = feature_slot(&meta.sem, annotation, feature);
        let refusal = a
            .send(InstanceOp::variant(
                annotation,
                InstanceOp::field(slot, InstanceOp::New),
            ))
            .expect_err("the interpreted path refuses it");
        let sentence = refusal.to_string();
        assert!(sentence.contains(feature), "{sentence}");
        assert!(sentence.contains("any class"), "{sentence}");
    }
}

/// The exclusion, shown rather than hidden: an annotation created into
/// `eAnnotations` and never written into is on the interpreted read-out and
/// not on the generated one, and one character into its `source` brings the
/// two back together at the same index.
///
/// Asserted on the *unexcluded* projections, so that the finding of code
/// note 31 stays visible in the suite instead of being erased by the pruning
/// that works around it.
#[test]
fn an_annotation_with_nothing_written_is_invisible_on_the_generated_path() {
    let mut harness = Harness::opened();
    for edit in [
        at(Place::default(), Action::AddPart { pos: 0, seed: 'p' }),
        at(Place::part(0), Action::AddAnnotation { pos: 0, seed: 'x' }),
        at(Place::part(0), Action::AddBareAnnotation { pos: 1 }),
    ] {
        harness
            .carry(&edit)
            .unwrap_or_else(|reason| panic!("{reason}"));
    }
    harness.cross();

    let interp = canon(
        &harness.meta,
        &harness.ia.query(&Read::<Value>::new()),
    );
    let generated = project(
        &harness.meta,
        &harness.ga.query(&Read::<AnnotatedValue>::new()),
    );
    let annotations = |doc: &Value| doc["parts"][0]["eAnnotations"].clone();
    assert_eq!(
        annotations(&interp),
        json!([{ECLASS: ANNOTATION, "source": "x"}, {ECLASS: ANNOTATION}]),
        "the interpreted path shows the annotation it minted"
    );
    assert_eq!(
        annotations(&generated),
        json!([{ECLASS: ANNOTATION, "source": "x"}]),
        "and the generated path cannot render it"
    );
    // One character in, and the two agree at the same index with nothing
    // excluded at all.
    harness
        .apply(&at(
            Place::part(0).annotation(1),
            Action::InsertChar { pos: 0, ch: 'y' },
        ))
        .unwrap_or_else(|reason| panic!("{reason}"));
    harness.deliver().unwrap_or_else(|reason| panic!("{reason}"));
    let interp = canon(
        &harness.meta,
        &harness.ia.query(&Read::<Value>::new()),
    );
    let generated = project(
        &harness.meta,
        &harness.ga.query(&Read::<AnnotatedValue>::new()),
    );
    assert_eq!(annotations(&interp), annotations(&generated));
    assert_eq!(
        annotations(&interp),
        json!([
            {ECLASS: ANNOTATION, "source": "x"},
            {ECLASS: ANNOTATION, "source": "y"}
        ])
    );
}

/// **The named scenarios.** Each is a script written by hand rather than
/// seeded, so that what it covers is legible: the annotations of an element,
/// `source`, the four ways two writers can meet on `details`, a key with no
/// value, and an element carrying several annotations.
#[test]
fn ip33_the_named_annotation_scenarios_agree_on_both_paths() {
    let part = || Place::part(0);
    let annotation = |index: usize| Place::part(0).annotation(index);
    let write = |place: Place, text: &str| -> Vec<Move> {
        text.chars()
            .enumerate()
            .map(|(pos, ch)| {
                Move::Edit(at(place.clone(), Action::InsertChar { pos, ch }))
            })
            .collect()
    };
    let put = |place: Place, writer: char, key: &str, value: Option<&str>| {
        Move::Edit(Edit {
            writer,
            at: place,
            action: Action::PutDetail {
                key: key.to_string(),
                value: value.map(str::to_string),
            },
        })
    };

    let mut scripts: Vec<EditScript> = Vec::new();
    let opening = || {
        vec![
            Move::Edit(at(Place::default(), Action::AddPart { pos: 0, seed: 'p' })),
            Move::Edit(at(part(), Action::AddAnnotation { pos: 0, seed: 's' })),
            Move::Deliver,
        ]
    };

    // An element gains annotations, and the two writers add them at once.
    let mut steps = opening();
    steps.push(Move::Edit(Edit {
        writer: 'a',
        at: part(),
        action: Action::AddAnnotation { pos: 1, seed: 'a' },
    }));
    steps.push(Move::Edit(Edit {
        writer: 'b',
        at: part(),
        action: Action::AddAnnotation { pos: 1, seed: 'b' },
    }));
    steps.push(Move::Deliver);
    steps.extend(write(annotation(0), "one"));
    steps.extend(write(annotation(1), "two"));
    steps.push(Move::Deliver);
    scripts.push(EditScript {
        label: "several annotations on one element".to_string(),
        steps,
    });

    // `source` written by both, concurrently, then deleted from.
    let mut steps = opening();
    steps.push(Move::Edit(Edit {
        writer: 'a',
        at: annotation(0),
        action: Action::InsertChar { pos: 0, ch: 's' },
    }));
    steps.push(Move::Edit(Edit {
        writer: 'b',
        at: annotation(0),
        action: Action::InsertChar { pos: 0, ch: 'y' },
    }));
    steps.push(Move::Deliver);
    steps.push(Move::Edit(at(annotation(0), Action::DeleteChar { pos: 0 })));
    steps.push(Move::Deliver);
    steps.push(Move::Edit(at(annotation(0), Action::UnsetText)));
    steps.push(Move::Deliver);
    scripts.push(EditScript {
        label: "a source written by both and then unset".to_string(),
        steps,
    });

    // `details`: the same key twice at once, two different keys at once, a
    // remove against a concurrent put, and a key with no value.
    let mut steps = opening();
    steps.push(put(annotation(0), 'a', "k", Some("left")));
    steps.push(put(annotation(0), 'b', "k", Some("right")));
    steps.push(Move::Deliver);
    steps.push(put(annotation(0), 'a', "one", Some("x")));
    steps.push(put(annotation(0), 'b', "two", Some("y")));
    steps.push(Move::Deliver);
    steps.push(put(annotation(0), 'a', "contested", Some("seed")));
    steps.push(Move::Deliver);
    steps.push(Move::Edit(Edit {
        writer: 'a',
        at: annotation(0),
        action: Action::RemoveDetail {
            key: "contested".to_string(),
        },
    }));
    steps.push(put(annotation(0), 'b', "contested", Some("kept")));
    steps.push(Move::Deliver);
    steps.push(put(annotation(0), 'a', "bare", None));
    steps.push(Move::Deliver);
    steps.push(put(annotation(0), 'a', "half", Some("v")));
    steps.push(put(annotation(0), 'b', "half", None));
    steps.push(Move::Deliver);
    steps.push(Move::Edit(at(
        annotation(0),
        Action::ClearDetail {
            key: "one".to_string(),
        },
    )));
    steps.push(Move::Deliver);
    steps.push(Move::Edit(at(annotation(0), Action::ClearDetails)));
    steps.push(Move::Deliver);
    scripts.push(EditScript {
        label: "the details map, every way two writers can meet on it".to_string(),
        steps,
    });

    // A port is an annotated element too, and so is an annotation.
    let mut steps = opening();
    steps.push(Move::Edit(at(part(), Action::AddPort { pos: 0, seed: 'q' })));
    steps.push(Move::Deliver);
    let port = Place::port(0, 0);
    steps.push(Move::Edit(at(
        port.clone(),
        Action::AddAnnotation { pos: 0, seed: 'r' },
    )));
    steps.push(Move::Edit(at(
        annotation(0),
        Action::AddAnnotation { pos: 0, seed: 't' },
    )));
    steps.push(Move::Deliver);
    steps.extend(write(port.clone().annotation(0), "p"));
    steps.extend(write(annotation(0).annotation(0), "n"));
    steps.push(put(port.annotation(0), 'a', "k", None));
    steps.push(put(annotation(0).annotation(0), 'b', "k", Some("v")));
    steps.push(Move::Deliver);
    scripts.push(EditScript {
        label: "a port and an annotation carry annotations of their own".to_string(),
        steps,
    });

    let mut edits = 0usize;
    let mut comparisons = 0usize;
    for script in &scripts {
        let mut harness = Harness::opened();
        harness
            .run(script)
            .unwrap_or_else(|reason| panic!("{reason}"));
        edits += harness.ops;
        comparisons += harness.comparisons;
    }
    eprintln!(
        "ip33: {} scripts, {edits} edits, {comparisons} comparisons",
        scripts.len()
    );
    assert!(edits >= 35, "the scenarios shrank: {edits} edits");
}

/// **The seeded oracle.** Thirty scripts over `ecore_builtins.ecore`, the
/// canonical read-outs of both replicas of both paths compared after every
/// operation and after every delivery.
///
/// # The one difference the run finds, and whose it is
///
/// A script whose noisy run differs is re-run **silently** — the same edits in
/// the same order, nothing read until the very end — and has to agree then.
/// That is not a weakening of the claim, it is the claim sharpened: a
/// difference that survives a silent replay is a real disagreement between the
/// two paths and fails here, and a difference that only a reader creates is
/// `moirai-protocol`'s `CachedLog`, which `NestedListLog` holds its positions
/// list in (`nested_list.rs:58`). It replays one operation onto the
/// materialised value rather than recomputing whenever the incoming event's
/// version compares `Greater` to the *previous* event's, and `Version`'s
/// `partial_cmp` answers `Greater` for two events of one origin from the
/// origin's own sequence without looking at what else each of them has seen;
/// the cache is populated by a read, which is why the divergence appears only
/// when someone looked. `simpleuml_crdt`'s
/// `a_read_between_deliveries_changes_what_the_generated_nested_list_holds` is
/// the other reproducer, and this is a third, on a metamodel that shares no
/// class with it.
///
/// The run prints how many scripts fall out that way, so a number that moves
/// is visible; what is asserted is that the silent residual is empty and that
/// the finding is still reachable at all.
#[test]
fn ip34_thirty_scripts_over_ecore_builtins_find_no_difference() {
    let mut edits = 0usize;
    let mut refused = 0usize;
    let mut comparisons = 0usize;
    let mut real: Vec<String> = Vec::new();
    let mut only_when_read: Vec<String> = Vec::new();
    let scripts = scripts();
    for script in &scripts {
        let mut harness = Harness::opened();
        let noisy = harness.run(script);
        edits += harness.ops;
        refused += harness.refused;
        comparisons += harness.comparisons;
        let Err(reason) = noisy else {
            continue;
        };
        // The same script, with nothing read until the end.
        let mut silent = Harness::opened();
        for step in &script.steps {
            match step {
                Move::Edit(edit) => {
                    silent.carry(edit).unwrap_or_else(|reason| panic!("{reason}"));
                }
                Move::Deliver => silent.cross(),
            }
        }
        silent.cross();
        match silent.compare() {
            Ok(()) => only_when_read.push(script.label.clone()),
            Err(silently) => real.push(format!("{reason}\n  and silently: {silently}")),
        }
    }
    eprintln!(
        "ip34: {} scripts, {edits} edits ({refused} refused on both paths), {comparisons} \
         comparisons, {} scripts differ only when read between deliveries: {only_when_read:?}",
        scripts.len(),
        only_when_read.len()
    );
    assert!(edits > 300, "the scripts shrank: {edits} edits");
    assert!(
        real.is_empty(),
        "{} of {} scripts differ on a silent replay too:\n\n{}",
        real.len(),
        scripts.len(),
        real.join("\n\n")
    );
    assert!(
        !only_when_read.is_empty(),
        "no script reaches `CachedLog`'s defect any more; if it is fixed, say so here"
    );
}

/// The exclusion the script generator carries, driven on its own: two writers
/// inserting into **one** ordered containment at once.
///
/// Ignored rather than a gate, because what it finds is not this metamodel's
/// and not the descriptor's. `json_crdt`'s `ip28` already drives and pins it —
/// seven of its thirty scripts put two concurrently inserted list elements in
/// opposite orders on the two paths, which is `NestedListLog`'s own
/// resolution — and on this metamodel it also reaches `CachedLog`, which
/// panics rather than diverges when an incremental insert index lands past the
/// materialised length. `propose` therefore lets only one writer of a
/// concurrent round create, and this is where that decision is written down
/// and can be re-checked.
#[test]
#[ignore = "a reproducer for `ip28` and for `CachedLog`, both of them Moirai's; not a gate"]
fn ip36_two_writers_inserting_into_one_sequence_at_once_is_moirais_own_finding() {
    let mut harness = Harness::opened();
    harness
        .apply(&at(Place::default(), Action::AddPart { pos: 0, seed: 'p' }))
        .expect("the first part");
    harness.deliver().expect("delivered");

    for (writer, seed) in [('a', 'x'), ('b', 'y')] {
        harness
            .carry(&Edit {
                writer,
                at: Place::default(),
                action: Action::AddPart { pos: 1, seed },
            })
            .expect("both intakes take it");
    }
    harness.cross();

    let a = harness.gen_doc('a');
    let b = harness.gen_doc('b');
    let interpreted = harness.interp_canon('a');
    eprintln!("ip36: interpreted {interpreted}\n      generated a {a}\n      generated b {b}");
    assert_eq!(a, b, "the two generated replicas converge");
    assert_eq!(
        interpreted, a,
        "and this is where the two paths part company when they do"
    );
}

/// The one state the whole construction exists for, asserted rather than left
/// to a script: a key put with **no** value is read back on both paths, and a
/// key nobody wrote is not there.
#[test]
fn ip35_a_detail_key_with_no_value_is_read_back_on_both_paths() {
    let mut harness = Harness::opened();
    let annotation = Place::part(0).annotation(0);
    for edit in [
        at(Place::default(), Action::AddPart { pos: 0, seed: 'p' }),
        at(Place::part(0), Action::AddAnnotation { pos: 0, seed: 's' }),
        at(
            annotation.clone(),
            Action::PutDetail {
                key: "bare".to_string(),
                value: None,
            },
        ),
        at(
            annotation.clone(),
            Action::PutDetail {
                key: "full".to_string(),
                value: Some("v".to_string()),
            },
        ),
    ] {
        harness.apply(&edit).unwrap_or_else(|reason| panic!("{reason}"));
    }
    harness.deliver().unwrap_or_else(|reason| panic!("{reason}"));

    for writer in ['a', 'b'] {
        let doc = harness.interp_canon(writer);
        let details = &doc["parts"][0]["eAnnotations"][0]["details"];
        assert_eq!(
            details,
            &json!({"bare": [null], "full": ["v"]}),
            "replica {writer} lost the key with no value"
        );
        assert_eq!(&harness.gen_doc(writer)["parts"][0]["eAnnotations"][0]["details"], details);
    }

    // And once it is removed it is gone on both, which is what says the key
    // was kept by the value and not by the entry merely existing.
    harness
        .apply(&at(
            annotation,
            Action::RemoveDetail {
                key: "bare".to_string(),
            },
        ))
        .unwrap_or_else(|reason| panic!("{reason}"));
    harness.deliver().unwrap_or_else(|reason| panic!("{reason}"));
    for writer in ['a', 'b'] {
        assert_eq!(
            harness.interp_canon(writer)["parts"][0]["eAnnotations"][0]["details"],
            json!({"full": ["v"]}),
            "replica {writer}"
        );
    }
}

// ---------------------------------------------------------------------------
// 9. The conflict matrix over `ecore_builtins.ecore`
// ---------------------------------------------------------------------------
//
// `moirai_interp::matrix` assigns this crate one row, the register over an
// optional value, driven through `ecore::EAnnotation.details`. The cells run
// on this file's own encoders and its comparison.
//
// The model is opened with one part holding one annotation; the heartbeat is
// a character inserted into that annotation's `source`, a sibling of the map
// under test, so no acknowledgement ever touches the feature a cell contends.

use moirai_interp::matrix::{self, Arm, Cell, Construction, pattern as p};

fn cell_annotation() -> Place {
    Place::part(0).annotation(0)
}

fn cell_setup() -> Vec<Edit> {
    vec![
        at(Place::default(), Action::AddPart { pos: 0, seed: 'p' }),
        at(Place::part(0), Action::AddAnnotation { pos: 0, seed: 's' }),
    ]
}

fn cell_beat() -> Edit {
    at(
        cell_annotation(),
        Action::InsertChar { pos: 0, ch: 'h' },
    )
}

fn cell_put(value: Option<&str>) -> Vec<Edit> {
    vec![at(
        cell_annotation(),
        Action::PutDetail {
            key: "k".to_string(),
            value: value.map(str::to_string),
        },
    )]
}

fn cell_cells() -> Vec<Cell<Edit>> {
    let row = Construction::OptionalRegister;
    let seeded = || {
        let mut out = cell_setup();
        out.extend(cell_put(Some("seed")));
        out
    };
    vec![
        // Two values at one key: a multi-value register keeps both until the
        // next write, and `HashSet<Option<String>>` holds them both.
        Cell::new(
            row,
            p::WRITE_WRITE_DIFFERENT,
            seeded(),
            vec![cell_put(Some("left")), cell_put(Some("right"))],
            cell_beat(),
        )
        .expect(
            "/parts/0/eAnnotations/0/details/k",
            json!(["left", "right"]),
        ),
        Cell::new(
            row,
            p::WRITE_WRITE_SAME,
            seeded(),
            vec![cell_put(Some("same")), cell_put(Some("same"))],
            cell_beat(),
        )
        .expect("/parts/0/eAnnotations/0/details/k", json!(["same"])),
        // The column this construction has and a plain register has not: one
        // writer puts a value and the other puts no value at all, and the key
        // has to come back holding both.
        Cell::new(
            row,
            p::WRITE_WRITE_ABSENT,
            seeded(),
            vec![cell_put(Some("value")), cell_put(None)],
            cell_beat(),
        )
        .expect(
            "/parts/0/eAnnotations/0/details/k",
            json!([Value::Null, "value"]),
        ),
        // A clear of the register at one key, concurrent with a write to it:
        // the write is not below the clear, so it survives and the key stays.
        Cell::new(
            row,
            p::WRITE_CLEAR,
            seeded(),
            vec![
                cell_put(Some("kept")),
                vec![at(
                    cell_annotation(),
                    Action::ClearDetail {
                        key: "k".to_string(),
                    },
                )],
            ],
            cell_beat(),
        )
        .expect("/parts/0/eAnnotations/0/details/k", json!(["kept"])),
        Cell::new(
            row,
            p::THREE_WRITERS,
            seeded(),
            vec![
                cell_put(Some("one")),
                cell_put(None),
                cell_put(Some("three")),
            ],
            cell_beat(),
        )
        .expect(
            "/parts/0/eAnnotations/0/details/k",
            json!([Value::Null, "one", "three"]),
        ),
    ]
}

/// **The conflict matrix** over `ecore_builtins.ecore`: every cell the
/// registry assigns to this crate, each under every schedule.
#[test]
fn conflict_matrix_over_ecore_builtins() {
    let meta = Meta::new();
    moirai_interp::testing::install_fixture(&meta.sem, MODEL);
    let interp_encode = |edit: &Edit, _doc: &Value| interp_op(&meta, edit);
    let gen_encode = |edit: &Edit, _doc: &Value| typed_op(edit);
    let interp_read = |replica: &InterpReplica| {
        except_unwritten(&meta, canon(&meta, &replica.query(&Read::<Value>::new())))
    };
    let gen_read = |replica: &GenReplica| {
        except_unwritten(
            &meta,
            project(&meta, &replica.query(&Read::<AnnotatedValue>::new())),
        )
    };
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
    let cells = cell_cells();
    matrix::run_matrix(matrix::ANNOTATED, &meta.sem, &cells, &interp, &generated)
        .unwrap_or_else(|reason| panic!("{reason}"));
}

/// The oracle's own bookkeeping, so that a script generator that quietly
/// stopped proposing could not make the file pass by doing nothing.
#[test]
fn the_scripts_reach_every_action_this_oracle_can_write() {
    let mut seen: BTreeMap<&'static str, usize> = BTreeMap::new();
    for script in scripts() {
        for step in &script.steps {
            if let Move::Edit(edit) = step {
                let word = match &edit.action {
                    Action::AddPart { .. } => "AddPart",
                    Action::DeletePart { .. } => "DeletePart",
                    Action::AddPort { .. } => "AddPort",
                    Action::DeletePort { .. } => "DeletePort",
                    Action::AddAnnotation { .. } => "AddAnnotation",
                    Action::AddBareAnnotation { .. } => "AddBareAnnotation",
                    Action::DeleteAnnotation { .. } => "DeleteAnnotation",
                    Action::InsertChar { .. } => "InsertChar",
                    Action::DeleteChar { .. } => "DeleteChar",
                    Action::UnsetText => "UnsetText",
                    Action::PutDetail { value: Some(_), .. } => "PutDetail",
                    Action::PutDetail { value: None, .. } => "PutDetailWithNoValue",
                    Action::RemoveDetail { .. } => "RemoveDetail",
                    Action::ClearDetail { .. } => "ClearDetail",
                    Action::ClearDetails => "ClearDetails",
                };
                *seen.entry(word).or_default() += 1;
            }
        }
    }
    for wanted in [
        "AddPart",
        "AddPort",
        "AddAnnotation",
        "DeleteAnnotation",
        "InsertChar",
        "DeleteChar",
        "PutDetail",
        "PutDetailWithNoValue",
        "RemoveDetail",
    ] {
        assert!(
            seen.get(wanted).copied().unwrap_or_default() > 0,
            "no script ever proposes `{wanted}`: {seen:?}"
        );
    }
    eprintln!("ip34 action census: {seen:?}");
}
