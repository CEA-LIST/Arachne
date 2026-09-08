//! Structural conformance, from the descriptor as data.
//!
//! The first of the model plane's two enforcement points. What a single
//! operation and the descriptor alone can show to be malformed is refused at
//! the node's intake, identically on every replica holding the descriptor of
//! one digest, because the verdict reads nothing else: no document, no
//! history, no other operation. Whatever needs the converged document — two
//! objects sharing an identifying value, a `required` feature absent, a
//! reference naming nothing — is the adaptation layer's to report, never the
//! node's to refuse, since a refusal on receive is what would let two replicas
//! disagree.
//!
//! The [`Schema`] is the descriptor parsed once: per class the supertype
//! closure flattened into one feature table, as the format asks of its
//! clients, and the *family* of concrete classes an instance slot typed by
//! that class may hold. [`check_structure`] walks one operation in the JSON
//! CRDT's own grammar — `Object.Update` keys and `Array.Insert` positions
//! from the root down to one leaf — and resolves every step against the
//! schema: a key is a feature of a class allowed at that step, a leaf is
//! written with the operations its `kind` is written with, a containment
//! holds an object or a list of objects as its `many` says.
//!
//! The class an object *is* never travels in an operation: the wire writes a
//! string one character per `String.Insert`, and `eClass` is a string. So a
//! step is typed by the path — under `BehaviorTree.child` there is a
//! `TreeNode`, whichever concrete subtype it turns out to be — and a
//! character written into `eClass` is checked as far as one character
//! allows: it must sit at its position in the name of some class allowed at
//! that slot. A name written left to right, which is the only way the
//! editor writes one, is refused at its first character that fits no allowed
//! class: `Class` under `BehaviorTree.child` at the `a`, since no `TreeNode`
//! has an `a` third; the abstract `TreeNode` itself at the `T`. A name whose
//! every character fits some allowed class at that index (`Sallback`) passes
//! here and is caught on the converged document by the editor's diagnostics.
//! An enum literal is the same story over the enum's literals. This is the
//! strongest verdict the operation supports, and it is stated rather than
//! rounded up to a check of the whole name.
//!
//! Written once, in `arachne-codegen`, and copied verbatim beside every
//! generated `network_node`, which installs it through the node's intake
//! guard. It names no generated type: the operation arrives as the
//! `serde_json::Value` the header guard already serializes it to.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

/// The instance encoding's class tag: every object carries its class name
/// under this key, and a slot is present iff the string is non-empty.
pub const CLASS_KEY: &str = "eClass";

/// The reserved root key of the model header, which is not the descriptor's
/// business: the node's header guard refuses a local write to it before the
/// structural check runs.
pub const HEADER_KEY: &str = "__model";

/// An attribute's `kind`, as the descriptor declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    String,
    Int,
    Float,
    Bool,
    Enum,
}

impl Kind {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "string" => Self::String,
            "int" => Self::Int,
            "float" => Self::Float,
            "bool" => Self::Bool,
            "enum" => Self::Enum,
            _ => return None,
        })
    }

    /// The operation kind a value of this kind is written with, which is the
    /// variant of the JSON CRDT's union the slot adopts.
    pub fn written_with(self) -> &'static str {
        match self {
            Self::String | Self::Enum => "String",
            Self::Int | Self::Float => "Number",
            Self::Bool => "Boolean",
        }
    }

    /// The kind with its article, for a sentence.
    fn described(self) -> &'static str {
        match self {
            Self::String => "a string",
            Self::Int => "an int",
            Self::Float => "a float",
            Self::Bool => "a bool",
            Self::Enum => "an enum",
        }
    }
}

/// What a feature holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    Attribute {
        kind: Kind,
        /// The enum's name and its literals, for an attribute of kind `enum`.
        enumeration: Option<(String, Vec<String>)>,
    },
    /// A contained object, or a list of them: the target class names the
    /// family allowed there.
    Containment { target: String },
    /// A reference, written as the target's identifying value: a string, or
    /// a list of strings.
    Reference { target: String },
}

/// One feature as an instance has it, with the class that declared it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    /// The declaring class, for the sentence `Owner.name`.
    pub owner: String,
    pub name: String,
    pub many: bool,
    pub shape: Shape,
}

impl Feature {
    fn qualified(&self) -> String {
        format!("`{}.{}`", self.owner, self.name)
    }
}

/// One class with its inheritance closure flattened.
#[derive(Debug, Clone, Default)]
pub struct Class {
    pub name: String,
    pub is_abstract: bool,
    /// Every feature an instance has, declared here or on a supertype; the
    /// nearest declaration wins a name.
    pub features: BTreeMap<String, Feature>,
    /// The concrete classes an instance slot typed by this class may hold:
    /// the class itself when concrete, and every concrete class that has it
    /// among its supertypes. Sorted.
    pub family: Vec<String>,
}

/// A descriptor parsed for checking: the classes, and the family a document
/// root may belong to.
#[derive(Debug, Clone)]
pub struct Schema {
    classes: BTreeMap<String, Class>,
    /// The concrete classes a document root may be: the root classes'
    /// families, or every concrete class when the descriptor names no root.
    root: Vec<String>,
}

impl Schema {
    /// Parse a descriptor's text. Fails on what a check could not be run
    /// against: no `classes` object, an attribute of a kind the format does
    /// not name, an enum attribute naming an enum the descriptor does not list.
    pub fn parse(text: &str) -> Result<Self, String> {
        let descriptor: Value =
            serde_json::from_str(text).map_err(|err| format!("not JSON: {err}"))?;
        Self::from_value(&descriptor)
    }

    /// [`parse`], from the descriptor already parsed.
    ///
    /// [`parse`]: Schema::parse
    pub fn from_value(descriptor: &Value) -> Result<Self, String> {
        let declared = descriptor
            .get("classes")
            .and_then(Value::as_object)
            .ok_or_else(|| "no `classes` object".to_string())?;
        let enums: BTreeMap<String, Vec<String>> = descriptor
            .get("enums")
            .and_then(Value::as_object)
            .map(|enums| {
                enums
                    .iter()
                    .map(|(name, literals)| {
                        let literals = literals
                            .as_array()
                            .map(|literals| {
                                literals
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_string)
                                    .collect()
                            })
                            .unwrap_or_default();
                        (name.clone(), literals)
                    })
                    .collect()
            })
            .unwrap_or_default();

        // The supertype closure of every class, itself included; an unknown
        // supertype contributes nothing, as it does for the editor.
        let mut closures: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for name in declared.keys() {
            let mut closure = BTreeSet::new();
            let mut pending = vec![name.as_str()];
            while let Some(current) = pending.pop() {
                if !closure.insert(current) {
                    continue;
                }
                if let Some(supers) = declared
                    .get(current)
                    .and_then(|class| class.get("superTypes"))
                    .and_then(Value::as_array)
                {
                    pending.extend(supers.iter().filter_map(Value::as_str));
                }
            }
            closures.insert(name, closure);
        }

        let mut classes: BTreeMap<String, Class> = BTreeMap::new();
        for (name, raw) in declared {
            let is_abstract = raw
                .get("abstract")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut class = Class {
                name: name.clone(),
                is_abstract,
                ..Class::default()
            };
            // Supertypes' declarations first, so a class's own overrides.
            let mut order: Vec<&str> = closures[name.as_str()]
                .iter()
                .copied()
                .filter(|super_name| *super_name != name)
                .collect();
            order.push(name);
            for owner in order {
                let Some(raw_owner) = declared.get(owner) else {
                    continue;
                };
                for feature in declared_features(owner, raw_owner, &enums)? {
                    class.features.insert(feature.name.clone(), feature);
                }
            }
            classes.insert(name.clone(), class);
        }
        for (name, closure) in &closures {
            if classes[*name].is_abstract {
                continue;
            }
            for member in closure {
                if let Some(class) = classes.get_mut(*member) {
                    class.family.push((*name).to_string());
                }
            }
        }

        let named_roots: Vec<&str> = descriptor
            .get("rootClasses")
            .and_then(Value::as_array)
            .map(|roots| roots.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let root: BTreeSet<String> = if named_roots.is_empty() {
            classes
                .values()
                .filter(|class| !class.is_abstract)
                .map(|class| class.name.clone())
                .collect()
        } else {
            named_roots
                .iter()
                .filter_map(|root| classes.get(*root))
                .flat_map(|class| class.family.iter().cloned())
                .collect()
        };
        Ok(Self {
            classes,
            root: root.into_iter().collect(),
        })
    }

    pub fn class(&self, name: &str) -> Option<&Class> {
        self.classes.get(name)
    }

    pub fn classes(&self) -> impl Iterator<Item = &Class> {
        self.classes.values()
    }

    /// The concrete classes a document root may be.
    pub fn root_family(&self) -> &[String] {
        &self.root
    }

    /// The family allowed under a containment: the target's, or nothing
    /// when the target is not a class the descriptor lists.
    fn family_of(&self, target: &str) -> &[String] {
        self.classes
            .get(target)
            .map(|class| class.family.as_slice())
            .unwrap_or(&[])
    }

    /// The declarations named `key` on the classes of `family`, in family
    /// order; an inherited feature is one declaration seen through several
    /// classes and checks the same each time.
    fn features_named<'a>(
        &'a self,
        family: &'a [String],
        key: &'a str,
    ) -> impl Iterator<Item = &'a Feature> + 'a {
        family
            .iter()
            .filter_map(move |name| self.classes.get(name)?.features.get(key))
    }

    /// An operation on an object slot that may hold an instance of any class
    /// of `family`; `slot` is the containment it sits under, or `None` for
    /// the document root.
    fn check_element(
        &self,
        op: &Value,
        family: &[String],
        slot: Option<&Feature>,
    ) -> Result<(), String> {
        let (kind, inner) = op_kind(op)?;
        if kind != "Object" {
            return Err(format!(
                "{} is an instance of {}, written with `Object` operations and not `{kind}`",
                slot_name(slot),
                family_text(family)
            ));
        }
        if inner == "Clear" {
            return Ok(());
        }
        if let Some(key) = inner.get("Remove").and_then(Value::as_str) {
            let known = key == CLASS_KEY
                || (slot.is_none() && key == HEADER_KEY)
                || self.features_named(family, key).next().is_some();
            return if known {
                Ok(())
            } else {
                Err(unknown_feature(key, family, slot))
            };
        }
        let update = inner
            .get("Update")
            .and_then(Value::as_array)
            .filter(|pair| pair.len() == 2)
            .ok_or_else(|| format!("malformed `Object` operation: {inner}"))?;
        let key = update[0].as_str().ok_or_else(|| {
            format!("malformed `Object` operation: the key is not a string: {inner}")
        })?;
        let child = &update[1];
        if key == CLASS_KEY {
            return check_class_tag(child, family, slot);
        }
        if key == HEADER_KEY && slot.is_none() {
            return Ok(());
        }
        let mut first_refusal = None;
        for feature in self.features_named(family, key) {
            match self.check_feature(feature, child) {
                Ok(()) => return Ok(()),
                Err(why) => {
                    first_refusal.get_or_insert(why);
                }
            }
        }
        Err(first_refusal.unwrap_or_else(|| unknown_feature(key, family, slot)))
    }

    /// The operation under a feature key.
    fn check_feature(&self, feature: &Feature, op: &Value) -> Result<(), String> {
        match &feature.shape {
            Shape::Attribute { kind, enumeration } => {
                let scalar = |op: &Value| check_scalar(feature, *kind, enumeration.as_ref(), op);
                if feature.many {
                    check_list(feature, op, kind.described(), scalar)
                } else {
                    scalar(op)
                }
            }
            Shape::Reference { target } => {
                let reference = |op: &Value| check_reference(feature, target, op);
                if feature.many {
                    check_list(feature, op, "reference", reference)
                } else {
                    reference(op)
                }
            }
            Shape::Containment { target } => {
                let family = self.family_of(target);
                let element = |op: &Value| self.check_element(op, family, Some(feature));
                if feature.many {
                    return check_list(feature, op, &format!("`{target}`"), element);
                }
                let (kind, _) = op_kind(op)?;
                if kind == "Array" {
                    return Err(format!(
                        "{} holds one `{target}`, not a list",
                        feature.qualified()
                    ));
                }
                element(op)
            }
        }
    }
}

/// The features `raw` declares on the class `owner`.
fn declared_features(
    owner: &str,
    raw: &Value,
    enums: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<Feature>, String> {
    let entries = |key: &str| {
        raw.get(key)
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
    };
    let text = |entry: &Value, key: &str| {
        entry
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("a feature of `{owner}` has no `{key}`: {entry}"))
    };
    let many = |entry: &Value| entry.get("many").and_then(Value::as_bool).unwrap_or(false);

    let mut features = Vec::new();
    for entry in entries("attributes") {
        let name = text(entry, "name")?;
        let kind_name = text(entry, "kind")?;
        let kind = Kind::parse(&kind_name).ok_or_else(|| {
            format!("`{owner}.{name}` has kind `{kind_name}`, which the format does not name")
        })?;
        let enumeration = if kind == Kind::Enum {
            let enum_name = text(entry, "enum")?;
            let literals = enums.get(&enum_name).ok_or_else(|| {
                format!("`{owner}.{name}` names the enum `{enum_name}`, which the descriptor does not list")
            })?;
            Some((enum_name, literals.clone()))
        } else {
            None
        };
        features.push(Feature {
            owner: owner.to_string(),
            name,
            many: many(entry),
            shape: Shape::Attribute { kind, enumeration },
        });
    }
    for entry in entries("containments") {
        features.push(Feature {
            owner: owner.to_string(),
            name: text(entry, "name")?,
            many: many(entry),
            shape: Shape::Containment {
                target: text(entry, "target")?,
            },
        });
    }
    for entry in entries("references") {
        features.push(Feature {
            owner: owner.to_string(),
            name: text(entry, "name")?,
            many: many(entry),
            shape: Shape::Reference {
                target: text(entry, "target")?,
            },
        });
    }
    Ok(features)
}

/// The structural verdict on one operation, `{"JsonKind": ...}` as the node
/// serializes it: `Ok` when every step of its path and its leaf agree with
/// the schema, else the sentence naming the class or the feature at fault.
pub fn check_structure(schema: &Schema, op: &Value) -> Result<(), String> {
    let kind = op
        .get("JsonKind")
        .ok_or_else(|| "not a `JsonKind` operation".to_string())?;
    schema.check_element(kind, schema.root_family(), None)
}

/// The operation's kind — its single key, one of the union's variants — and
/// what is under it.
fn op_kind(op: &Value) -> Result<(&str, &Value), String> {
    let malformed = || format!("malformed operation, not one of the union's variants: {op}");
    let fields = op.as_object().ok_or_else(malformed)?;
    if fields.len() != 1 {
        return Err(malformed());
    }
    let (kind, inner) = fields.iter().next().ok_or_else(malformed)?;
    match kind.as_str() {
        "Object" | "Array" | "String" | "Number" | "Boolean" => Ok((kind, inner)),
        _ => Err(malformed()),
    }
}

fn slot_name(slot: Option<&Feature>) -> String {
    slot.map_or_else(|| "the document root".to_string(), Feature::qualified)
}

fn family_text(family: &[String]) -> String {
    if family.is_empty() {
        "no concrete class".to_string()
    } else {
        family.join(", ")
    }
}

fn unknown_feature(key: &str, family: &[String], slot: Option<&Feature>) -> String {
    format!(
        "`{key}` is not a feature of any class allowed at {} ({})",
        slot_name(slot),
        family_text(family)
    )
}

/// A write into `eClass`: a string, and each inserted character at its
/// position in the name of some class allowed at the slot.
fn check_class_tag(op: &Value, family: &[String], slot: Option<&Feature>) -> Result<(), String> {
    let (kind, inner) = op_kind(op)?;
    if kind != "String" {
        return Err(format!(
            "`{CLASS_KEY}` at {} is a string, written with `String` operations and not `{kind}`",
            slot_name(slot)
        ));
    }
    if family.is_empty() {
        return Err(format!(
            "nothing can be instantiated at {}: no concrete class is allowed there",
            slot_name(slot)
        ));
    }
    for (position, ch) in inserted_chars(inner) {
        if !family
            .iter()
            .any(|name| name.chars().nth(position) == Some(ch))
        {
            return Err(format!(
                "`{CLASS_KEY}` at {} cannot have `{ch}` at position {position}: no class allowed there ({}) has it",
                slot_name(slot),
                family_text(family)
            ));
        }
    }
    Ok(())
}

/// A scalar attribute's leaf: written with the operations its kind is
/// written with, and an enum's characters at their positions in some literal.
fn check_scalar(
    feature: &Feature,
    kind: Kind,
    enumeration: Option<&(String, Vec<String>)>,
    op: &Value,
) -> Result<(), String> {
    let (written, inner) = op_kind(op)?;
    if written != kind.written_with() {
        return Err(format!(
            "{} is {}, written with `{}` operations and not `{written}`",
            feature.qualified(),
            kind.described(),
            kind.written_with()
        ));
    }
    if let Some((enum_name, literals)) = enumeration {
        for (position, ch) in inserted_chars(inner) {
            if !literals
                .iter()
                .any(|literal| literal.chars().nth(position) == Some(ch))
            {
                return Err(format!(
                    "{} is an enum `{enum_name}` ({}): no literal has `{ch}` at position {position}",
                    feature.qualified(),
                    literals.join(", ")
                ));
            }
        }
    }
    Ok(())
}

/// A reference's leaf: the target's identifying value, a string.
fn check_reference(feature: &Feature, target: &str, op: &Value) -> Result<(), String> {
    let (written, _) = op_kind(op)?;
    if written != "String" {
        return Err(format!(
            "{} is a reference to a `{target}`, written as its identifying string and not with `{written}`",
            feature.qualified()
        ));
    }
    Ok(())
}

/// A `many` feature: an `Array` operation, whose inserted or updated element
/// is checked as one value of the feature.
fn check_list(
    feature: &Feature,
    op: &Value,
    holds: &str,
    element: impl Fn(&Value) -> Result<(), String>,
) -> Result<(), String> {
    let (kind, inner) = op_kind(op)?;
    if kind != "Array" {
        return Err(format!(
            "{} holds many of {holds}, written as a list and not with `{kind}`",
            feature.qualified()
        ));
    }
    match inner.get("Insert").or_else(|| inner.get("Update")) {
        Some(entry) => {
            let op = entry
                .get("op")
                .ok_or_else(|| format!("malformed `Array` operation, no `op`: {inner}"))?;
            element(op)
        }
        None => Ok(()),
    }
}

/// The characters a `String` operation inserts, each with the position it
/// lands at: one for the wire's single-character `Insert`, none for a
/// deletion.
fn inserted_chars(inner: &Value) -> impl Iterator<Item = (usize, char)> + '_ {
    let insert = inner.get("Insert");
    let start = insert
        .and_then(|insert| insert.get("pos"))
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    insert
        .and_then(|insert| insert.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .enumerate()
        .map(move |(offset, ch)| (start + offset, ch))
}
