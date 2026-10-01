//! `arachne import-sirius`: a Sirius Web / SysON project export as the node operations that
//! build it.
//!
//! The export is `{"json", "ns", "content": [{"id", "eClass", "data"}]}`, a serialisation of
//! instances of the very metamodel the descriptor describes, so the walk is mechanical: an
//! object at a containment position becomes an operation ending in `New`, and every key of its
//! `data` is resolved to a visible feature slot and written by the rule the table
//! (`moirai_semantics::MetamodelSemantics`) binds that feature to. Classes and features are
//! resolved through that table and never through a map written here, which is what ip8's
//! transcoder learned (`experiments/ip8-sysml-usecase`): a second table is a second source of
//! truth.
//!
//! What this adds to ip8's walk is the references. Sirius writes a reference as the target's
//! Sirius `id`, a resource-local UUID that means nothing to a node; the editor resolves a
//! reference by the target's id attribute (`isId`, `elementId` in SysML). So a value that is the
//! Sirius `id` of an object of the document is rewritten to that object's id attribute, and
//! every other value (a standard-library URI, a dangling id) is written as the source wrote it
//! and reported, never dropped.

use std::collections::HashMap;

use moirai_interp::{InstanceOp, LeafOp, ModelOp, Scalar};
use moirai_semantics::{
    ClassSlot, FeatureSlot, LeafRule, MergeRule, MetamodelSemantics, NumKind, Shape,
    from_descriptor,
};
use serde_json::{Map, Value};

/// A reference value whose target is not an object of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    /// The id attribute of the object holding the reference (its Sirius `id` when it has none).
    pub holder: String,
    /// The reference feature.
    pub feature: String,
    /// The value, as the source wrote it and as it is written.
    pub value: String,
}

/// What an import produced.
#[derive(Debug)]
pub struct Import {
    /// The operations, in the order they must be applied: the root first, an object before
    /// anything written into it, and a sibling at position `p` before the one at `p + 1`.
    pub ops: Vec<ModelOp>,
    /// Reference values rewritten to the id attribute of an object of the document.
    pub resolved: usize,
    /// Reference values written verbatim, in document order.
    pub unresolved: Vec<Unresolved>,
}

/// The project as operations against the table `descriptor` describes; `Err` names every
/// element the table cannot carry.
pub fn import(descriptor: &Value, project: &Value) -> Result<Import, Vec<String>> {
    let sem = from_descriptor(descriptor)
        .map_err(|err| vec![format!("the descriptor does not parse: {err}")])?;
    let table = Table::new(sem, descriptor);
    let mut walk = Walk {
        table: &table,
        ids: HashMap::new(),
        ops: Vec::new(),
        resolved: 0,
        unresolved: Vec::new(),
        failures: Vec::new(),
        root: ClassSlot(0),
    };
    walk.document(project);
    if walk.failures.is_empty() {
        Ok(Import {
            ops: walk.ops,
            resolved: walk.resolved,
            unresolved: walk.unresolved,
        })
    } else {
        Err(walk.failures)
    }
}

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

/// The parsed table, with the one fact it does not keep: which attribute is a class's id.
struct Table {
    sem: MetamodelSemantics,
    /// Per class slot, the name of the first `isId` attribute along its lineage.
    id_attribute: Vec<Option<String>>,
}

impl Table {
    fn new(sem: MetamodelSemantics, descriptor: &Value) -> Table {
        let id_attribute = sem
            .classes
            .iter()
            .map(|class| {
                lineage(&sem, class.slot).into_iter().find_map(|slot| {
                    let name = &*sem.classes[slot.index()].name;
                    descriptor["classes"][name]["attributes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|attribute| attribute["isId"] == Value::Bool(true))
                        .and_then(|attribute| attribute["name"].as_str())
                        .map(str::to_string)
                })
            })
            .collect();
        Table { sem, id_attribute }
    }

    fn name(&self, class: ClassSlot) -> &str {
        &self.sem.classes[class.index()].name
    }

    fn instantiable(&self, class: ClassSlot) -> bool {
        self.sem.classes[class.index()].instantiable
    }

    /// Whether an instance of `class` may sit where `target` is declared.
    fn conforms(&self, class: ClassSlot, target: ClassSlot) -> bool {
        self.sem.classes[target.index()].concrete.contains(&class)
    }

    /// The class an `eClass` string names, tried as the table spells it
    /// (`ecore::EAnnotation`), bare after the namespace prefix (`sysml:PartUsage`), and, only
    /// when exactly one class matches, as `<any package>::<bare>`.
    fn class_named(&self, eclass: &str) -> Option<ClassSlot> {
        let bare = eclass.rsplit(':').next().unwrap_or(eclass);
        let classes = &self.sem.classes;
        let exact = classes.iter().find(|class| &*class.name == eclass);
        let plain = || classes.iter().find(|class| &*class.name == bare);
        let qualified = || {
            let suffix = format!("::{bare}");
            let mut hits = classes.iter().filter(|class| class.name.ends_with(&suffix));
            match (hits.next(), hits.next()) {
                (one, None) => one,
                _ => None,
            }
        };
        exact
            .or_else(plain)
            .or_else(qualified)
            .map(|class| class.slot)
    }

    /// A visible feature of `class` by name: the slot a `Field` carries and its rule.
    fn feature(&self, class: ClassSlot, name: &str) -> Option<(FeatureSlot, Option<MergeRule>)> {
        let visible = &self.sem.classes[class.index()].visible;
        let index = visible
            .iter()
            .position(|(feature, _, _)| &**feature == name)?;
        let (_, owner, declared) = visible[index];
        let slot = u16::try_from(index).ok().map(FeatureSlot)?;
        Some((slot, self.sem.rule(owner, declared).copied()))
    }

    /// The id attribute's value on an object of `class`, when it has one.
    fn id_of(&self, class: ClassSlot, data: &Map<String, Value>) -> Option<String> {
        let name = self.id_attribute[class.index()].as_deref()?;
        data.get(name)
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
    }
}

/// `class` and every class above it, nearest first.
fn lineage(sem: &MetamodelSemantics, class: ClassSlot) -> Vec<ClassSlot> {
    let mut out = vec![class];
    let mut at = 0;
    while at < out.len() {
        for up in &sem.classes[out[at].index()].supers {
            if !out.contains(up) {
                out.push(*up);
            }
        }
        at += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// The source format
// ---------------------------------------------------------------------------

/// An object's `data` and its Sirius `id`. SysON writes an `ecore::EAnnotation` as a bare
/// `{"source": ..}` with no `id`, `eClass` or `data`; then the object is its own data and its
/// class is the one its containment declares.
fn source_parts(obj: &Value) -> Option<(&Map<String, Value>, Option<&str>)> {
    let map = obj.as_object()?;
    let id = map.get("id").and_then(Value::as_str);
    match map.get("data") {
        Some(Value::Object(data)) => Some((data, id)),
        _ => Some((map, id)),
    }
}

/// The values of a source feature: a JSON array as it stands, nothing for `null`, one value
/// for anything else. A string is never split: a library reference holds a space.
fn values_of(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(items) => items.iter().collect(),
        Value::Null => Vec::new(),
        other => vec![other],
    }
}

fn as_flag(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn as_int(value: &Value) -> Option<i64> {
    match value {
        Value::Number(_) => value.as_i64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn as_float(value: &Value) -> Option<f64> {
    match value {
        Value::Number(_) => value.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// A JSON scalar as a register's value; `null` is not a value an import writes.
fn scalar_from_json(value: &Value) -> Option<Scalar> {
    match value {
        Value::Bool(flag) => Some(Scalar::Bool(*flag)),
        Value::Number(number) => number
            .as_i64()
            .map(Scalar::Int)
            .or_else(|| number.as_f64().map(Scalar::float)),
        Value::String(text) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(one), None) => Some(Scalar::Char(one)),
                _ => Some(Scalar::text(text.as_str())),
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// One step of the path an operation carries from the model root, one arm per containment
/// shape the table can declare.
#[derive(Clone, Copy)]
enum Step {
    SeqAt {
        slot: FeatureSlot,
        pos: usize,
        class: ClassSlot,
    },
    OptSet {
        slot: FeatureSlot,
        class: ClassSlot,
    },
    Single {
        slot: FeatureSlot,
        class: ClassSlot,
    },
}

struct Walk<'t> {
    table: &'t Table,
    /// Sirius `id` to the object's id attribute and class, for every object of the document.
    ids: HashMap<String, (String, ClassSlot)>,
    ops: Vec<ModelOp>,
    resolved: usize,
    unresolved: Vec<Unresolved>,
    failures: Vec<String>,
    root: ClassSlot,
}

impl Walk<'_> {
    fn fail(&mut self, at: &str, what: impl Into<String>) {
        self.failures.push(format!("{at}: {}", what.into()));
    }

    /// The operation that does `inner` at the end of `path`.
    fn emit(&mut self, path: &[Step], inner: InstanceOp) {
        let mut op = inner;
        for step in path.iter().rev() {
            op = match *step {
                Step::SeqAt { slot, pos, class } => {
                    InstanceOp::field(slot, InstanceOp::at(pos, InstanceOp::variant(class, op)))
                }
                Step::OptSet { slot, class } => {
                    InstanceOp::field(slot, InstanceOp::set(InstanceOp::variant(class, op)))
                }
                Step::Single { slot, class } => {
                    InstanceOp::field(slot, InstanceOp::variant(class, op))
                }
            };
        }
        self.ops
            .push(ModelOp::Instance(InstanceOp::variant(self.root, op)));
    }

    fn document(&mut self, document: &Value) {
        let Some(content) = document.get("content").and_then(Value::as_array) else {
            self.fail("/", "the document has no `content` array");
            return;
        };
        let [root_obj] = content.as_slice() else {
            self.fail(
                "/content",
                format!(
                    "a model has one root element and the document has {}",
                    content.len()
                ),
            );
            return;
        };
        let at = "/content[0]";
        let Some(eclass) = root_obj.get("eClass").and_then(Value::as_str) else {
            self.fail(at, "the root has no `eClass`");
            return;
        };
        let Some(root) = self.table.class_named(eclass) else {
            self.fail(at, format!("no class `{eclass}` in the table"));
            return;
        };
        let sem = &self.table.sem;
        let reachable = sem
            .roots
            .iter()
            .any(|slot| self.table.conforms(root, *slot));
        if !self.table.instantiable(root) || !reachable {
            self.fail(
                at,
                format!("`{}` cannot sit at the model root", self.table.name(root)),
            );
            return;
        }
        self.root = root;
        self.collect_ids(root_obj, root, at);
        if !self.failures.is_empty() {
            return;
        }
        self.ops.push(ModelOp::Instance(InstanceOp::variant(
            root,
            InstanceOp::New,
        )));
        self.object(root_obj, root, &mut Vec::new(), at);
    }

    /// The first pass: every object's Sirius `id` against its id attribute, so that a
    /// reference resolves wherever in the document its target sits.
    fn collect_ids(&mut self, obj: &Value, class: ClassSlot, at: &str) {
        let Some((data, id)) = source_parts(obj) else {
            return;
        };
        if let Some(id) = id {
            match self.table.id_of(class, data) {
                Some(element_id) => {
                    if self
                        .ids
                        .insert(id.to_string(), (element_id, class))
                        .is_some()
                    {
                        self.fail(at, format!("the Sirius id {id} is on two objects"));
                    }
                }
                None => self.fail(
                    at,
                    format!(
                        "`{}` id {id} has no id attribute value to reference it by",
                        self.table.name(class)
                    ),
                ),
            }
        }
        for (name, value) in data {
            let Some((_, Some(MergeRule::Containment { target, .. }))) =
                self.table.feature(class, name)
            else {
                continue;
            };
            for (pos, child) in values_of(value).into_iter().enumerate() {
                let child_class = match child.get("eClass").and_then(Value::as_str) {
                    Some(eclass) => self.table.class_named(eclass),
                    None => Some(target),
                };
                // An unknown class is the second pass's to report, with its context.
                if let Some(child_class) = child_class {
                    self.collect_ids(child, child_class, &format!("{at}/{name}[{pos}]"));
                }
            }
        }
    }

    /// One element: its own features, each by the rule the table binds it to.
    fn object(&mut self, obj: &Value, class: ClassSlot, path: &mut Vec<Step>, at: &str) {
        let Some((data, sirius_id)) = source_parts(obj) else {
            self.fail(at, "not a JSON object");
            return;
        };
        let holder = self
            .table
            .id_of(class, data)
            .or_else(|| sirius_id.map(str::to_string))
            .unwrap_or_else(|| at.to_string());
        for (name, value) in data {
            let Some((slot, rule)) = self.table.feature(class, name) else {
                self.fail(
                    at,
                    format!(
                        "`{}` has no feature called `{name}`",
                        self.table.name(class)
                    ),
                );
                continue;
            };
            match rule {
                None => self.fail(
                    at,
                    format!("`{}`.`{name}` carries no rule", self.table.name(class)),
                ),
                Some(MergeRule::Unsupported { reason }) => self.fail(
                    at,
                    format!(
                        "`{}`.`{name}` is written in the source and the table refuses it: {}",
                        self.table.name(class),
                        reason.why()
                    ),
                ),
                Some(MergeRule::Containment { shape, target }) => {
                    self.containment(value, class, slot, shape, target, path, at, name);
                }
                Some(MergeRule::Attribute { shape, leaf }) => {
                    self.attribute(value, class, slot, shape, leaf, path, at, name);
                }
                Some(MergeRule::Reference { many, target }) => {
                    let site = Site {
                        class,
                        slot,
                        name,
                        at,
                    };
                    self.reference(value, &site, many, target, path, &holder);
                }
            }
        }
    }

    /// A containment: one creation per child, then the child itself, in source order.
    #[allow(clippy::too_many_arguments)]
    fn containment(
        &mut self,
        value: &Value,
        class: ClassSlot,
        slot: FeatureSlot,
        shape: Shape,
        target: ClassSlot,
        path: &mut Vec<Step>,
        at: &str,
        name: &str,
    ) {
        let children = values_of(value);
        if shape != Shape::Sequence && children.len() > 1 {
            self.fail(
                at,
                format!(
                    "`{}`.`{name}` holds one element and the source puts {} in it",
                    self.table.name(class),
                    children.len()
                ),
            );
            return;
        }
        for (pos, child) in children.into_iter().enumerate() {
            let here = format!("{at}/{name}[{pos}]");
            let child_class = match child.get("eClass").and_then(Value::as_str) {
                Some(eclass) => match self.table.class_named(eclass) {
                    Some(found) => found,
                    None => {
                        self.fail(&here, format!("no class `{eclass}` in the table"));
                        continue;
                    }
                },
                None => target,
            };
            if !self.table.instantiable(child_class) || !self.table.conforms(child_class, target) {
                self.fail(
                    &here,
                    format!(
                        "`{}` cannot be minted in `{}`.`{name}`, which holds `{}`",
                        self.table.name(child_class),
                        self.table.name(class),
                        self.table.name(target)
                    ),
                );
                continue;
            }
            let created = InstanceOp::variant(child_class, InstanceOp::New);
            let (inner, step) = match shape {
                Shape::Sequence => (
                    InstanceOp::insert(pos, created),
                    Step::SeqAt {
                        slot,
                        pos,
                        class: child_class,
                    },
                ),
                Shape::Optional => (
                    InstanceOp::set(created),
                    Step::OptSet {
                        slot,
                        class: child_class,
                    },
                ),
                Shape::Single => (
                    created,
                    Step::Single {
                        slot,
                        class: child_class,
                    },
                ),
                other => {
                    self.fail(
                        &here,
                        format!("no containment of shape {other:?} is carried"),
                    );
                    continue;
                }
            };
            self.emit(path, InstanceOp::field(slot, inner));
            path.push(step);
            self.object(child, child_class, path, &here);
            path.pop();
        }
    }

    /// An attribute, by the rule the table binds it to.
    #[allow(clippy::too_many_arguments)]
    fn attribute(
        &mut self,
        value: &Value,
        class: ClassSlot,
        slot: FeatureSlot,
        shape: Shape,
        leaf: LeafRule,
        path: &[Step],
        at: &str,
        name: &str,
    ) {
        // An optional attribute is reached through the optional's `Set`.
        let shaped = |op: LeafOp| -> InstanceOp {
            let inner = InstanceOp::Leaf(op);
            match shape {
                Shape::Optional => InstanceOp::field(slot, InstanceOp::set(inner)),
                _ => InstanceOp::field(slot, inner),
            }
        };
        let wrong = |want: &str| {
            format!(
                "`{}`.`{name}` is {want} and the source writes {value}",
                self.table.name(class)
            )
        };
        match (shape, leaf) {
            (Shape::Single | Shape::Optional, LeafRule::Text) => {
                let Some(text) = value.as_str() else {
                    let why = wrong("a text attribute");
                    self.fail(at, why);
                    return;
                };
                // An optional text set to "" reads back as "" and one left out as no key; a
                // zero-length delete mints the text and leaves it empty (ip8).
                if text.is_empty() && shape == Shape::Optional {
                    self.emit(path, shaped(LeafOp::DeleteRange { start: 0, len: 0 }));
                }
                for (pos, ch) in text.chars().enumerate() {
                    self.emit(path, shaped(LeafOp::InsertChar { pos, ch }));
                }
            }
            (Shape::OrderedSet, LeafRule::Text) => {
                for (pos, item) in values_of(value).into_iter().enumerate() {
                    let Some(text) = item.as_str() else {
                        let why = wrong("an ordered set of text");
                        self.fail(at, why);
                        return;
                    };
                    self.emit(
                        path,
                        shaped(LeafOp::Insert {
                            pos,
                            value: Scalar::text(text),
                        }),
                    );
                }
            }
            (Shape::Single | Shape::Optional, LeafRule::Flag { .. }) => match as_flag(value) {
                Some(on) => self.emit(
                    path,
                    shaped(if on { LeafOp::Enable } else { LeafOp::Disable }),
                ),
                None => {
                    let why = wrong("a flag");
                    self.fail(at, why);
                }
            },
            // A numeric attribute is a counter on the interpreted path: a literal is loaded
            // with `Inc`, and a register write is refused (ip8).
            (Shape::Single | Shape::Optional, LeafRule::Counter { num, .. }) => {
                let by = if matches!(num, NumKind::F32 | NumKind::F64) {
                    as_float(value).map(Scalar::float)
                } else {
                    as_int(value).map(Scalar::Int)
                };
                match by {
                    Some(by) => self.emit(path, shaped(LeafOp::Inc(by))),
                    None => {
                        let why = wrong("a counter");
                        self.fail(at, why);
                    }
                }
            }
            (
                Shape::Single | Shape::Optional,
                LeafRule::Enum {
                    class: enum_slot, ..
                },
            ) => {
                let entry = &self.table.sem.enums[enum_slot.index()];
                let index = value
                    .as_str()
                    .and_then(|literal| entry.literals.iter().position(|lit| &**lit == literal))
                    .and_then(|index| u16::try_from(index).ok());
                match index {
                    Some(index) => {
                        self.emit(
                            path,
                            shaped(LeafOp::Write(Scalar::Enum(enum_slot.0, index))),
                        );
                    }
                    None => {
                        let why = wrong(&format!("a literal of `{}`", entry.name));
                        self.fail(at, why);
                    }
                }
            }
            (Shape::Single | Shape::Optional, LeafRule::Register { .. }) => {
                match scalar_from_json(value) {
                    Some(scalar) => self.emit(path, shaped(LeafOp::Write(scalar))),
                    None => {
                        let why = wrong("a register");
                        self.fail(at, why);
                    }
                }
            }
            (shape, leaf) => {
                let why = format!(
                    "`{}`.`{name}` is an attribute of shape {shape:?} over {leaf:?}, which is not \
                     carried",
                    self.table.name(class)
                );
                self.fail(at, why);
            }
        }
    }

    /// A non-containment reference, carried as text: the target's id attribute when the
    /// target is an object of the document, the source's value otherwise.
    fn reference(
        &mut self,
        value: &Value,
        site: &Site<'_>,
        many: bool,
        target: ClassSlot,
        path: &[Step],
        holder: &str,
    ) {
        let values = values_of(value);
        if !many && values.len() > 1 {
            self.fail(
                site.at,
                format!(
                    "`{}`.`{}` is single-valued and the source gives it {} values",
                    self.table.name(site.class),
                    site.name,
                    values.len()
                ),
            );
            return;
        }
        for item in values {
            let Some(source) = item.as_str() else {
                self.fail(
                    site.at,
                    format!(
                        "`{}`.`{}` is a reference and the source writes {item}",
                        self.table.name(site.class),
                        site.name
                    ),
                );
                return;
            };
            let written = match self.ids.get(source) {
                Some((element_id, class)) => {
                    if !self.table.conforms(*class, target) {
                        let why = format!(
                            "`{}`.`{}` holds a `{}` and the source points it at a `{}`",
                            self.table.name(site.class),
                            site.name,
                            self.table.name(target),
                            self.table.name(*class)
                        );
                        self.fail(site.at, why);
                        return;
                    }
                    self.resolved += 1;
                    element_id.clone()
                }
                None => {
                    self.unresolved.push(Unresolved {
                        holder: holder.to_string(),
                        feature: site.name.to_string(),
                        value: source.to_string(),
                    });
                    source.to_string()
                }
            };
            let op = if many {
                LeafOp::Add(Scalar::text(written))
            } else {
                LeafOp::Write(Scalar::text(written))
            };
            self.emit(path, InstanceOp::field(site.slot, InstanceOp::Leaf(op)));
        }
    }
}

/// Where a feature is written, for the operation and for a failure sentence.
struct Site<'a> {
    class: ClassSlot,
    slot: FeatureSlot,
    name: &'a str,
    at: &'a str,
}
