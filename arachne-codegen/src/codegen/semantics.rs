//! The merge policy of one structural feature, derived as data.
//!
//! # What this module is for
//!
//! The generator decides a feature's CRDT once, at generation, by three
//! matches: `datatype/to_crdt.rs` maps the Ecore builtin to a CRDT family and
//! a Rust width, `annotation.rs`'s `datatype_override` lets a
//! `urn:arachne:semantics` `datatype` replace that family from a closed set of
//! spellings, and `feature/attribute.rs` combines the result with the triple
//! `(bound_kind, unique.unwrap_or(false), ordered.unwrap_or(true))` to pick the
//! wrapper — `feature/containment.rs` doing the same for a containment over
//! its target's family log. The outcome is a Rust type, which is exactly the
//! form a running node cannot read.
//!
//! [`merge_rule`] answers the same question as *data*: the
//! [`moirai_semantics`] vocabulary, whose every variant is one of those
//! outcomes and not one more. `descriptor.rs` writes it into the
//! `formatVersion` 2 descriptor, and `moirai_semantics::from_descriptor` reads
//! it back on the interpreted path.
//!
//! # It is a second implementation on purpose
//!
//! Nothing here is called by `AttributeGenerator` or `ContainmentGenerator`,
//! and those two are not refactored to route through it. The test at the
//! bottom of this file — `ip2` of the validation plan — runs the real
//! generators over `bt.ecore`, `SimpleUML.ecore`, `json.ecore` and
//! `kitchen_sink.ecore`, and asserts that the rule this module derives names
//! the construction those generators emit, feature by feature. Two
//! implementations that agree are evidence; one implementation quoting itself
//! is not. That test is leg 2 of the paper stated as a unit test, so the
//! duplication is the point and not an oversight.
//!
//! # Provenance, and what each source means
//!
//! Criterion I-A4 is falsified by a facet with no source, so every rule comes
//! with a [`Provenance`] naming where each of the four facets came from.
//!
//! - [`FacetSource::Declared`] — the `.ecore` file wrote the thing that
//!   decided this facet.
//! - [`FacetSource::EcoreDefault`] — the file is silent and Ecore's own
//!   default supplies the value Arachne then uses.
//! - [`FacetSource::HouseDefault`] — the file is silent, or Ecore has nothing
//!   to say, and Arachne's own rule supplies the value. `unique.unwrap_or(false)`
//!   is the sharp case: EMF's default is `true`, so a silent `unique` on a
//!   multi-valued attribute is a house rule and never an Ecore default.
//! - [`FacetSource::Annotation`] — a `urn:arachne:semantics` `datatype`
//!   annotation chose it.
//! - [`FacetSource::NotApplicable`] — the facet carries no information for
//!   this kind of feature: `ordered` and `unique` on anything single-valued
//!   (Arachne's own parser already warns about this, `raw.rs:1017-1030`),
//!   `unique` on a containment, which EMF makes structural, and every facet of
//!   a reference or of a feature with no rule.
//!
//! Three readings this module commits to, each because the alternative would
//! claim more than the file says:
//!
//! - **`leaf` is `Declared` only for [`LeafRule::Text`].** `EString` maps to
//!   `GraphLog<List<char>>` with no parameter left over, so the declared
//!   `eType` decided all of it. Every other leaf carries a parameter Ecore
//!   cannot express — a counter's resettability, a flag's winning side, a
//!   register's tie-break — and Arachne fills it in, so the source is
//!   `HouseDefault`.
//! - **`ordered` on a multi-valued containment is always `HouseDefault`.**
//!   `containment.rs:188-195` compiles a `NestedListLog` for every
//!   multi-valued containment whatever `ordered` says, so the value acted on
//!   is Arachne's and not the file's. What the file wrote is still visible: it
//!   is the `facets` object of the descriptor entry.
//! - **`presence` is `EcoreDefault` when the bounds equal Ecore's default**,
//!   `0..1` for either feature kind, and `Declared` otherwise. The parsed
//!   [`Structural`] does not keep whether `lowerBound` was written, only what
//!   it resolved to, and reading equal-to-default as a default is the side
//!   that never over-claims a declaration.
//!
//! One facet has no slot: which of `AWSet` and `RWSet` an `aw-set` or
//! `rw-set` annotation chose. [`Provenance`] names four facets and the set
//! tie-break is not one of them; `unique` is `Declared` there, because a set
//! shape is only reachable with `unique="true"` written in the file, and the
//! annotation itself is carried verbatim under the entry's `annotation` key.
//!
//! # Slots in this crate address the Ecore context
//!
//! [`MergeRule`] names its targets by [`ClassSlot`], which on the interpreted
//! side is a dense position in the parsed descriptor's sorted class vector.
//! Arachne does not know those positions while it is deriving a rule, and does
//! not need them: the descriptor carries target *names* and
//! `from_descriptor` assigns the real slots when it reads them back. So a
//! `ClassSlot` produced here carries the classifier's index in the [`Ctx`],
//! and [`class_name`] is the only thing that reads it.

use ecore_rs::{
    ctx::Ctx,
    repr::{Class, Structural, annot::Val, builtin::Typ as BuiltinTyp, idx, structural},
};
use moirai_semantics::{
    ClassSlot, FacetSource, FlagWins, KeyKind, LeafRule, MergeRule, NumKind, Provenance, SetTie,
    Shape, TieBreak, UnsupportedReason,
};
use serde_json::{Value, json};

/// The annotation source a `datatype` override is written under.
const SEMANTICS_SOURCE: &str = "urn:arachne:semantics";
/// The detail key naming the override.
const DATATYPE_KEY: &str = "datatype";
/// The annotation source a `transparent` class is written under.
const REPRESENTATION_SOURCE: &str = "urn:arachne:representation";
/// The detail key naming the representation.
const KIND_KEY: &str = "kind";
/// The detail key naming the field a transparent class is represented by.
const FIELD_KEY: &str = "field";
/// The detail keys of a `uw-map` annotation.
const KEY_FEATURE_KEY: &str = "key-feature";
/// The detail key naming the entry feature the map's values come from.
const VALUE_FEATURE_KEY: &str = "value-feature";

/// How many values a feature holds, as `feature/bounds.rs` normalises it.
///
/// Restated here rather than imported so the rule and the emitter do not share
/// the step the equivalence test exists to check.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Presence {
    /// `0..1`.
    Optional,
    /// `1..1`.
    Single,
    /// Anything else, which `bounds.rs` widens to `0..*`.
    Many,
}

/// The merge rule and the provenance of its facets for one structural feature.
///
/// `class` is the class that *declares* `feature`, which matters only for the
/// `urn:arachne:representation` `transparent` annotation: a transparent class
/// is compiled as its one field and has no record of its own, so none of its
/// features carries a rule the interpreted path can run.
pub fn merge_rule(feature: &Structural, class: &Class, ctx: &Ctx) -> (MergeRule, Provenance) {
    // The forms decision D6 and the behavioural flags keep off the interpreted
    // path. A rule with no construction has no facets either, so all four
    // sources are `NotApplicable`.
    if let Some(reason) = unsupported_reason(feature, class) {
        return (
            MergeRule::Unsupported { reason },
            Provenance {
                ordered: FacetSource::NotApplicable,
                unique: FacetSource::NotApplicable,
                leaf: FacetSource::NotApplicable,
                presence: FacetSource::NotApplicable,
            },
        );
    }

    // A `uw-map` containment is a keyed collection and not a sequence, and
    // it is checked before the feature kind because the annotation replaces
    // the whole construction `containment.rs` would otherwise emit.
    if let Some(rule) = keyed_rule(feature, ctx) {
        return rule;
    }

    let presence = presence_of(feature);
    let presence_source = presence_source(feature);

    match feature.kind {
        // A non-containment reference is carried as a string on both paths and
        // `process_structural_features` emits no field for it at all, so it
        // has no facet to source.
        structural::Typ::EReference if !feature.containment => (
            MergeRule::Reference {
                many: presence == Presence::Many,
                target: target_slot(feature),
            },
            Provenance {
                ordered: FacetSource::NotApplicable,
                unique: FacetSource::NotApplicable,
                leaf: FacetSource::NotApplicable,
                presence: FacetSource::NotApplicable,
            },
        ),
        structural::Typ::EReference => (
            MergeRule::Containment {
                shape: match presence {
                    Presence::Single => Shape::Single,
                    Presence::Optional => Shape::Optional,
                    Presence::Many => Shape::Sequence,
                },
                target: target_slot(feature),
            },
            Provenance {
                // `containment.rs:188-195` compiles a sequence for every
                // multi-valued containment whatever the file says.
                ordered: match presence {
                    Presence::Many => FacetSource::HouseDefault,
                    _ => FacetSource::NotApplicable,
                },
                // EMF makes a containment's uniqueness structural, and the
                // generator never reads the facet.
                unique: FacetSource::NotApplicable,
                leaf: FacetSource::NotApplicable,
                presence: presence_source,
            },
        ),
        structural::Typ::EAttribute => {
            let (leaf, leaf_source) = leaf_rule(feature, ctx);
            let (shape, ordered, unique) = attribute_shape(feature, presence);
            (
                MergeRule::Attribute { shape, leaf },
                Provenance {
                    ordered,
                    unique,
                    leaf: leaf_source,
                    presence: presence_source,
                },
            )
        }
    }
}

/// The rule as the descriptor spells it: targets by name, since a descriptor
/// carries no slots.
///
/// The shape and every leaf but an enum's serialize straight out of
/// `moirai-semantics`, so the emitter and the parser cannot drift on a tag
/// name; only the two name-typed fields are written by hand.
pub fn merge_json(rule: &MergeRule, ctx: &Ctx) -> Value {
    match rule {
        MergeRule::Attribute { shape, leaf } => json!({
            "kind": "attribute",
            "shape": to_value(shape),
            "leaf": leaf_json(leaf, ctx),
        }),
        MergeRule::Containment { shape, target } => json!({
            "kind": "containment",
            "shape": to_value(shape),
            "target": class_name(*target, ctx),
        }),
        MergeRule::Reference { many, target } => json!({
            "kind": "reference",
            "many": many,
            "target": class_name(*target, ctx),
        }),
        MergeRule::Unsupported { reason } => json!({
            "kind": "unsupported",
            "reason": reason.as_str(),
        }),
    }
}

/// The provenance as the descriptor spells it: four facets, four sources.
pub fn provenance_json(provenance: &Provenance) -> Value {
    to_value(provenance)
}

/// The facets exactly as the `.ecore` file wrote them, `null` where it was
/// silent. What the rule did with them is [`merge_rule`]'s answer; this is the
/// declaration, kept so a reader can see the two apart.
pub fn facets_json(feature: &Structural) -> Value {
    json!({ "ordered": feature.ordered, "unique": feature.unique })
}

/// The `urn:arachne:semantics` `datatype` value as written, or `None`.
pub fn datatype_annotation(feature: &Structural) -> Option<&Val> {
    feature
        .annotations()
        .iter()
        .find(|annot| annot.source() == SEMANTICS_SOURCE)
        .and_then(|annot| annot.details().get(DATATYPE_KEY))
}

/// The feature a `urn:arachne:representation` `kind="transparent"` class is
/// represented by, or `None`.
///
/// `classifier/mod.rs:565-567` emits no record for such a class and
/// `:497-517` puts the field's own construction in its parent union's
/// variant, so `JsonKind::Array` carries a `NestedList<Box<JsonKind>>` and
/// there is no `Array` record anywhere. The descriptor writes this name under
/// the class's `transparent` key so a node reading the descriptor knows the
/// same thing.
pub fn transparent_field<'a>(class: &'a Class) -> Option<&'a str> {
    let annot = class
        .annotations()
        .iter()
        .find(|annot| annot.source() == REPRESENTATION_SOURCE)?;
    if annot.details().get(KIND_KEY).map(String::as_str) != Some("transparent") {
        return None;
    }
    annot.details().get(FIELD_KEY).map(String::as_str)
}

/// The classifier a slot produced by this module names.
pub fn class_name(slot: ClassSlot, ctx: &Ctx) -> &str {
    ctx.classes()
        .get(slot.index())
        .expect("a slot this module minted indexes the context it was minted from")
        .name()
}

/* ---------- derivation ---------- */

/// The rule a `urn:arachne:semantics` `datatype="uw-map"` containment
/// carries, or `None` when the feature is not one.
///
/// `containment.rs:110-168` compiles `UWMapLog<KeyTy, ValueLog>` for it: the
/// entry class is not represented at all, its key attribute becomes the map's
/// key type, and its value feature's own construction becomes the map's value
/// log. So the rule is the *value feature's* rule with the collection
/// replaced by [`Shape::Keyed`], and the entry class never appears in it.
///
/// `None` is also the answer for a `uw-map` the generator would itself refuse
/// — a single-valued one, a missing or non-attribute key feature, a
/// non-containment value reference — because a descriptor entry claiming a
/// construction the generator will not emit is worse than one that claims
/// nothing. Such a feature falls through to the ordinary containment rule,
/// and `arachne generate` fails on it as it did before.
fn keyed_rule(feature: &Structural, ctx: &Ctx) -> Option<(MergeRule, Provenance)> {
    if !datatype_annotation(feature)
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("uw-map"))
    {
        return None;
    }
    if feature.kind != structural::Typ::EReference
        || !feature.containment
        || presence_of(feature) != Presence::Many
    {
        return None;
    }
    let annot = feature
        .annotations()
        .iter()
        .find(|annot| annot.source() == SEMANTICS_SOURCE)?;
    let key_name = annot
        .details()
        .get(KEY_FEATURE_KEY)
        .map_or("key", String::as_str);
    let value_name = annot
        .details()
        .get(VALUE_FEATURE_KEY)
        .map_or("value", String::as_str);

    let entry = ctx.classes().get(*feature.typ?)?;
    let key_feature = entry.structural().iter().find(|f| f.name == key_name)?;
    let value_feature = entry.structural().iter().find(|f| f.name == value_name)?;
    if key_feature.kind != structural::Typ::EAttribute {
        return None;
    }
    if presence_of(value_feature) != Presence::Single {
        return None;
    }
    let key = key_kind(key_feature, ctx)?;
    let shape = Shape::Keyed { key };

    match value_feature.kind {
        structural::Typ::EAttribute => {
            let (leaf, leaf_source) = leaf_rule(value_feature, ctx);
            Some((
                MergeRule::Attribute { shape, leaf },
                Provenance {
                    // The annotation replaced the collection outright, so
                    // neither facet the file wrote decided anything.
                    ordered: FacetSource::Annotation,
                    unique: FacetSource::Annotation,
                    leaf: leaf_source,
                    presence: presence_source(feature),
                },
            ))
        }
        structural::Typ::EReference if value_feature.containment => Some((
            MergeRule::Containment {
                shape,
                target: target_slot(value_feature),
            },
            Provenance {
                ordered: FacetSource::Annotation,
                unique: FacetSource::Annotation,
                leaf: FacetSource::NotApplicable,
                presence: presence_source(feature),
            },
        )),
        structural::Typ::EReference => None,
    }
}

/// The key type `containment.rs:150-163` compiles for the key attribute.
fn key_kind(key_feature: &Structural, ctx: &Ctx) -> Option<KeyKind> {
    let typ = key_feature.typ?;
    let declared = ctx.classes().get(*typ)?;
    if declared.is_enum() {
        return Some(KeyKind::Enum { class: slot(typ) });
    }
    Some(match declared.name().parse::<BuiltinTyp>() {
        Ok(BuiltinTyp::EByte) => KeyKind::Num { num: NumKind::U8 },
        Ok(BuiltinTyp::EShort) => KeyKind::Num { num: NumKind::I16 },
        Ok(BuiltinTyp::EInt) => KeyKind::Num { num: NumKind::I32 },
        Ok(BuiltinTyp::ELong) => KeyKind::Num { num: NumKind::I64 },
        Ok(BuiltinTyp::EFloat) => KeyKind::Num { num: NumKind::F32 },
        Ok(BuiltinTyp::EDouble) => KeyKind::Num { num: NumKind::F64 },
        Ok(BuiltinTyp::EBoolean) => KeyKind::Bool,
        Ok(BuiltinTyp::EChar) => KeyKind::Char,
        // `descriptor.rs`'s `attribute_kind` describes `Object` and a custom
        // `EDataType` as a string, and this follows it.
        Ok(BuiltinTyp::EString) | Ok(BuiltinTyp::Object) | Err(()) => KeyKind::Str,
    })
}

/// Why this feature carries no rule the interpreted path can run, if it does
/// not.
///
/// Until 2026-09-08 this answered [`UnsupportedReason::Keyed`] for a `uw-map`
/// feature and [`UnsupportedReason::Transparent`] for every feature of a
/// class carrying `urn:arachne:representation` `kind="transparent"`, which is
/// every feature `json.ecore` has. Both now carry the rule the generator
/// compiles: a `uw-map` is [`keyed_rule`], and a transparent class's feature
/// is derived exactly as any other feature of any other class, with the
/// class's own `transparent` key in the descriptor saying that the class is
/// rendered as that feature. The three behavioural flags below are what is
/// left.
fn unsupported_reason(feature: &Structural, class: &Class) -> Option<UnsupportedReason> {
    let _ = class;
    if feature.derived == Some(true) {
        return Some(UnsupportedReason::Derived);
    }
    if feature.transient == Some(true) {
        return Some(UnsupportedReason::Transient);
    }
    if feature.volatile == Some(true) {
        return Some(UnsupportedReason::Volatile);
    }
    None
}

/// How many values the feature holds, by the same widening `bounds.rs` does.
fn presence_of(feature: &Structural) -> Presence {
    match (feature.bounds.lbound, feature.bounds.ubound) {
        (0, Some(1)) | (0, Some(0)) => Presence::Optional,
        (1, Some(1)) => Presence::Single,
        _ => Presence::Many,
    }
}

/// Ecore's default bounds, `0..1`, for either feature kind
/// (`structural.rs`'s `parse_bounds`).
///
/// Until upstream's parser fixes, arachne read a silent `lowerBound` on an
/// attribute as `1`, so this read `1..1` as the attribute default. Ecore
/// defaults `ETypedElement.lowerBound` to `0` for every typed element, which
/// is what the parser now does, and reading `1..1` as a default here would
/// call a written `lowerBound="1"` an Ecore default and a silent attribute a
/// declaration — the descriptor's provenance the wrong way round on both.
fn presence_source(feature: &Structural) -> FacetSource {
    let default = match feature.kind {
        structural::Typ::EAttribute | structural::Typ::EReference => (0usize, Some(1usize)),
    };
    if (feature.bounds.lbound, feature.bounds.ubound) == default {
        FacetSource::EcoreDefault
    } else {
        FacetSource::Declared
    }
}

/// The classifier a reference or containment points at.
fn target_slot(feature: &Structural) -> ClassSlot {
    slot(
        feature
            .typ
            .expect("the descriptor omits a feature with no resolved target"),
    )
}

/// An Ecore classifier index as a slot. See the module note on slots.
fn slot(index: idx::Class) -> ClassSlot {
    ClassSlot(
        u16::try_from(*index)
            .expect("a metamodel with more classifiers than a slot addresses is out of scope"),
    )
}

/// The innermost CRDT of an attribute, and where it came from.
///
/// This is `to_crdt.rs`'s builtin match, then `annotation.rs`'s thirteen
/// outcomes on top of it, as data.
fn leaf_rule(feature: &Structural, ctx: &Ctx) -> (LeafRule, FacetSource) {
    let typ = feature
        .typ
        .expect("the descriptor omits an attribute with no resolved type");
    let declared = ctx
        .classes()
        .get(*typ)
        .expect("a resolved attribute type is a classifier of this context");

    // The declared type's own leaf, before any annotation.
    let (from_type, from_type_source) = if declared.is_enum() {
        (
            LeafRule::Enum {
                class: slot(typ),
                tie: TieBreak::MultiValue,
            },
            // Which of the five registers is Arachne's choice, not Ecore's.
            FacetSource::HouseDefault,
        )
    } else {
        match declared.name().parse::<BuiltinTyp>() {
            Ok(BuiltinTyp::EByte) => (counter(NumKind::U8), FacetSource::HouseDefault),
            Ok(BuiltinTyp::EShort) => (counter(NumKind::I16), FacetSource::HouseDefault),
            Ok(BuiltinTyp::EInt) => (counter(NumKind::I32), FacetSource::HouseDefault),
            Ok(BuiltinTyp::ELong) => (counter(NumKind::I64), FacetSource::HouseDefault),
            Ok(BuiltinTyp::EFloat) => (counter(NumKind::F32), FacetSource::HouseDefault),
            Ok(BuiltinTyp::EDouble) => (counter(NumKind::F64), FacetSource::HouseDefault),
            Ok(BuiltinTyp::EBoolean) => (
                LeafRule::Flag {
                    wins: FlagWins::Enable,
                },
                FacetSource::HouseDefault,
            ),
            Ok(BuiltinTyp::EChar) => (
                LeafRule::Register {
                    tie: TieBreak::MultiValue,
                },
                FacetSource::HouseDefault,
            ),
            // The declared type decides the whole leaf and leaves Arachne no
            // choice to make, which is what `Declared` means here.
            Ok(BuiltinTyp::EString) => (LeafRule::Text, FacetSource::Declared),
            // `Object` and custom `EDataType`s: `descriptor.rs`'s
            // `attribute_kind` describes them as `string`, and this follows it
            // so `arachne describe` keeps working on a metamodel the generator
            // itself would refuse. Nothing in the file said text.
            Ok(BuiltinTyp::Object) | Err(()) => (LeafRule::Text, FacetSource::HouseDefault),
        }
    };

    // `annotation.rs`'s `parse_datatype_override`, restated. The `aw-set` and
    // `rw-set` spellings name a collection rather than a leaf and are read by
    // `attribute_shape`; `list` is accepted on an attribute only.
    let Some(spelling) = datatype_annotation(feature) else {
        return (from_type, from_type_source);
    };
    let overridden = match spelling.trim().to_ascii_lowercase().as_str() {
        "resettable-counter" => Some(match from_type {
            // A counter's width follows the declared type; the annotation only
            // says that it resets. `i32` stands in where the declared type has
            // no width, which is a construction the generated path cannot
            // compile either.
            LeafRule::Counter { num, .. } => counter(num),
            _ => counter(NumKind::I32),
        }),
        "ew-flag" => Some(LeafRule::Flag {
            wins: FlagWins::Enable,
        }),
        "dw-flag" => Some(LeafRule::Flag {
            wins: FlagWins::Disable,
        }),
        "mv-register" => Some(register(from_type, TieBreak::MultiValue)),
        "lww-register" => Some(register(from_type, TieBreak::LastWriterWins)),
        "fair-register" => Some(register(from_type, TieBreak::Fair)),
        "po-register" | "partial-order-register" => {
            Some(register(from_type, TieBreak::PartialOrder))
        }
        "to-register" | "total-order-register" => Some(register(from_type, TieBreak::TotalOrder)),
        "list" => Some(LeafRule::Text),
        _ => None,
    };
    match overridden {
        Some(leaf) => (leaf, FacetSource::Annotation),
        None => (from_type, from_type_source),
    }
}

/// `Counter<T>` of the given width; the generator's default is the resettable
/// one (`crdt.rs`'s `Counter::default`), and no spelling reaches the other.
const fn counter(num: NumKind) -> LeafRule {
    LeafRule::Counter {
        num,
        resettable: true,
    }
}

/// A register over the attribute's own Rust type.
///
/// An enum-typed attribute stays an enum leaf under a register annotation,
/// with the annotation's tie-break: the generator writes `TORegister<RelationType>`
/// for `class_diagram.ecore`'s `Relation.typ`, which orders literals by
/// declaration position, and a plain `register` leaf would lose the enum
/// class, so the interpreted path could only write the literal as a string and
/// would order it by name.
const fn register(from_type: LeafRule, tie: TieBreak) -> LeafRule {
    match from_type {
        LeafRule::Enum { class, .. } => LeafRule::Enum { class, tie },
        _ => LeafRule::Register { tie },
    }
}

/// The wrapper `attribute.rs:155-218` picks, and the sources of the two facets
/// that pick it.
fn attribute_shape(feature: &Structural, presence: Presence) -> (Shape, FacetSource, FacetSource) {
    let Presence::Many = presence else {
        // `raw.rs:1017-1030` already warns that `ordered` and `unique` on a
        // single-valued feature say nothing; the rule agrees.
        return (
            match presence {
                Presence::Optional => Shape::Optional,
                _ => Shape::Single,
            },
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        );
    };

    let unique = feature.unique.unwrap_or(false);
    let ordered = feature.ordered.unwrap_or(true);
    let shape = match (unique, ordered) {
        (false, true) => Shape::Sequence,
        // No construction exists; the generator warns and compiles a list.
        (true, true) => Shape::OrderedSet,
        (false, false) => Shape::Bag,
        (true, false) => Shape::Set {
            tie: set_tie(feature),
        },
    };
    let ordered_source = if feature.ordered.is_some() {
        FacetSource::Declared
    } else {
        // Ecore's own default is `true` and Arachne takes it unchanged.
        FacetSource::EcoreDefault
    };
    let unique_source = if feature.unique.is_some() {
        FacetSource::Declared
    } else {
        // EMF's default is `true`; `unique.unwrap_or(false)` is Arachne's own
        // rule and the one divergence this whole facet exists to make visible.
        FacetSource::HouseDefault
    };
    (shape, ordered_source, unique_source)
}

/// Which side of a concurrent add and remove wins, from the `aw-set` and
/// `rw-set` spellings; add-wins when the file names neither.
fn set_tie(feature: &Structural) -> SetTie {
    match datatype_annotation(feature).map(|value| value.trim().to_ascii_lowercase()) {
        Some(spelling) if spelling == "rw-set" => SetTie::RemoveWins,
        _ => SetTie::AddWins,
    }
}

/* ---------- rendering ---------- */

/// A leaf, with an enum's class by name.
fn leaf_json(leaf: &LeafRule, ctx: &Ctx) -> Value {
    match leaf {
        LeafRule::Enum { class, tie } => json!({
            "kind": "enum",
            "class": class_name(*class, ctx),
            "tie": to_value(tie),
        }),
        other => to_value(other),
    }
}

/// `serde_json::to_value` on a type of the shared vocabulary, which cannot
/// fail: every one of them is a plain enum or a struct of plain enums.
fn to_value<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("the merge vocabulary serializes to JSON")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use ecore_rs::repr::{Class, Structural, builtin::Typ as BuiltinTyp, structural};
    use heck::ToUpperCamelCase;
    use moirai_semantics::{
        FacetSource, FlagWins, KeyKind, LeafRule, MergeRule, NumKind, SetTie, Shape, TieBreak,
    };

    use super::{datatype_annotation, merge_rule};
    use crate::EcoreParser;
    use crate::codegen::{
        cycles::analyze_cycles,
        feature::{attribute::AttributeGenerator, containment::ContainmentGenerator},
        generate::Generate,
    };

    fn example(file: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples")
            .join(file)
    }

    /// The three metamodels criterion I-A3 names, plus `kitchen_sink.ecore`,
    /// which is the only checked-in file that reaches a counter, a register, a
    /// bag and the widened bounds, plus `class_diagram.ecore`, which is the
    /// only one that reaches the four register tie-breaks other than the
    /// multi-value one, the disable-wins flag, both sets and an enum-typed
    /// attribute.
    const METAMODELS: [&str; 5] = [
        "behavior_tree.ecore",
        "SimpleUML.ecore",
        "json.ecore",
        "pet_metamodels/kitchen_sink.ecore",
        "class_diagram.ecore",
    ];

    /// The Rust type the generator writes for a declared Ecore type, stated
    /// here rather than taken from `to_crdt.rs` so the comparison is between
    /// two implementations and not one.
    fn rust_type(class: &Class) -> String {
        if class.is_enum() {
            return class.name().to_upper_camel_case();
        }
        match class.name().parse::<BuiltinTyp>() {
            Ok(BuiltinTyp::EByte) => "u8",
            Ok(BuiltinTyp::EShort) => "i16",
            Ok(BuiltinTyp::EInt) => "i32",
            Ok(BuiltinTyp::ELong) => "i64",
            Ok(BuiltinTyp::EFloat) => "f32",
            Ok(BuiltinTyp::EDouble) => "f64",
            Ok(BuiltinTyp::EChar) => "char",
            Ok(BuiltinTyp::EBoolean) => "bool",
            Ok(BuiltinTyp::EString) => "std::string::String",
            other => panic!("no Rust type for {other:?}"),
        }
        .to_string()
    }

    /// The log type of one leaf, over the attribute's declared Rust type.
    ///
    /// This is the readable half of the table: a `MergeRule` on the left, the
    /// construction the generator emits on the right.
    fn leaf_log(leaf: &LeafRule, rust: &str) -> String {
        match leaf {
            LeafRule::Text => "GraphLog<List<char>>".to_string(),
            LeafRule::Counter { .. } => format!("VecLog<Counter<{rust}>>"),
            LeafRule::Flag {
                wins: FlagWins::Enable,
            } => "VecLog<EWFlag>".to_string(),
            LeafRule::Flag {
                wins: FlagWins::Disable,
            } => "VecLog<DWFlag>".to_string(),
            LeafRule::Register { tie } | LeafRule::Enum { tie, .. } => {
                format!("VecLog<{}<{rust}>>", register_name(*tie))
            }
        }
    }

    fn register_name(tie: TieBreak) -> &'static str {
        match tie {
            TieBreak::MultiValue => "MVRegister",
            TieBreak::LastWriterWins => "LwwRegister",
            TieBreak::Fair => "FairRegister",
            TieBreak::PartialOrder => "PORegister",
            TieBreak::TotalOrder => "TORegister",
        }
    }

    /// The field type an `Attribute` rule implies.
    ///
    /// A set and a bag are the two shapes that drop the leaf log and hold the
    /// bare Rust value, which is `attribute.rs:199-217`.
    fn attribute_type(shape: &Shape, leaf: &LeafRule, rust: &str) -> String {
        match shape {
            Shape::Single => leaf_log(leaf, rust),
            Shape::Optional => format!("OptionLog<{}>", leaf_log(leaf, rust)),
            Shape::Sequence => format!("NestedListLog<{}>", leaf_log(leaf, rust)),
            Shape::OrderedSet => format!("ListLog<{}>", leaf_log(leaf, rust)),
            Shape::Set {
                tie: SetTie::AddWins,
            } => format!("VecLog<AWSet<{rust}>>"),
            Shape::Set {
                tie: SetTie::RemoveWins,
            } => format!("VecLog<RWSet<{rust}>>"),
            Shape::Bag => format!("AWBagLog<{rust}>"),
            Shape::Keyed { key } => {
                format!("UWMapLog<{},{}>", key_rust(*key), leaf_log(leaf, rust))
            }
        }
    }

    /// The Rust key type `containment.rs:150-163` compiles for a `uw-map`.
    fn key_rust(key: KeyKind) -> String {
        match key {
            KeyKind::Str => "std::string::String".to_string(),
            KeyKind::Bool => "bool".to_string(),
            KeyKind::Char => "char".to_string(),
            KeyKind::Num { num } => num_rust(num).to_string(),
            KeyKind::Enum { .. } => {
                panic!("no checked-in metamodel keys a `uw-map` by an enum literal")
            }
        }
    }

    /// The log ident a containment's target contributes: the family log when
    /// the target is abstract, an interface or has subclasses, the class's own
    /// log otherwise (`classifier/mod.rs:36-59`, restated).
    fn target_log(target: &Class) -> String {
        let kind = if target.is_abstract() || target.is_interface() || !target.sub().is_empty() {
            format!("{}Kind", target.name().to_upper_camel_case())
        } else {
            target.name().to_upper_camel_case()
        };
        format!("{kind}Log")
    }

    /// The field type a `Containment` rule implies.
    fn containment_type(shape: &Shape, target: &Class) -> String {
        let log = target_log(target);
        match shape {
            Shape::Single => log,
            Shape::Optional => format!("OptionLog<{log}>"),
            Shape::Sequence => format!("NestedListLog<{log}>"),
            Shape::Keyed { key } => format!("UWMapLog<{},{log}>", key_rust(*key)),
            other => panic!("a containment cannot be {other:?}"),
        }
    }

    /// The emitted fragment as a comparable string: the field name dropped,
    /// the private classifiers module dropped, `Box` dropped and every space
    /// squeezed out.
    ///
    /// `Box` — `BoxedLog` since Moirai v0.6 — is a Rust representation choice
    /// the cycle analysis makes so a log has a finite size; it changes no
    /// merge, and the rule does not name it.
    fn normalize(tokens: &proc_macro2::TokenStream) -> String {
        let rendered = tokens.to_string();
        let (_, typ) = rendered
            .split_once(" : ")
            .unwrap_or_else(|| panic!("a field fragment is `name : type`, got `{rendered}`"));
        let mut out = String::with_capacity(typ.len());
        for chunk in typ.replace("__classifiers :: ", "").split_whitespace() {
            out.push_str(chunk);
        }
        while let Some((start, opener)) = ["BoxedLog<", "Box<"]
            .iter()
            .filter_map(|opener| out.find(opener).map(|start| (start, *opener)))
            .min_by_key(|(start, _)| *start)
        {
            let width = opener.len();
            let mut depth = 0usize;
            let mut end = None;
            for (offset, ch) in out[start + width..].char_indices() {
                match ch {
                    '<' => depth += 1,
                    '>' if depth == 0 => {
                        end = Some(start + width + offset);
                        break;
                    }
                    '>' => depth -= 1,
                    _ => {}
                }
            }
            let end = end.expect("a balanced `Box<...>` or `BoxedLog<...>`");
            out = format!(
                "{}{}{}",
                &out[..start],
                &out[start + width..end],
                &out[end + 1..]
            );
        }
        out
    }

    /// **ip2** — for every structural feature of every class of the checked-in
    /// metamodels, the rule `merge_rule` derives names the construction
    /// `AttributeGenerator` or `ContainmentGenerator` emits for that feature.
    ///
    /// This is criterion I-A3 and leg 2 of the paper: the descriptor may be
    /// called *derived* only if what it publishes is what the generator would
    /// have compiled. The two sides are written independently — the table
    /// above against Léo's emitters — and a disagreement on any one of the
    /// features named in the failure is the falsification the criterion asks
    /// for.
    #[test]
    fn ip2_the_rule_names_the_construction_the_generator_emits() {
        let mut compared = 0usize;
        let mut references = 0usize;
        let mut unsupported = 0usize;

        for metamodel in METAMODELS {
            let parser = EcoreParser::from_file(example(metamodel))
                .unwrap_or_else(|e| panic!("{metamodel} should parse: {e}"));
            let ctx = &parser.ctx;
            let pack = crate::find_user_package(ctx).expect("a user package");
            let cycles = analyze_cycles(ctx).expect("the cycle analysis");

            for class_idx in pack.classes() {
                let class = &ctx[*class_idx];
                if class.is_enum() {
                    continue;
                }
                for feature in class.structural() {
                    let (rule, _) = merge_rule(feature, class, ctx);
                    let at = format!("{metamodel} `{}.{}`", class.name(), feature.name);

                    match &rule {
                        MergeRule::Unsupported { .. } => unsupported += 1,
                        // `process_structural_features` skips a non-containment
                        // reference, so there is no field to compare; the
                        // generated `bt_crdt` shows it as the empty
                        // `record!(DataFlowPort {})`, and
                        // `no_field_is_emitted_for_a_non_containment_reference`
                        // pins that.
                        MergeRule::Reference { many, target } => {
                            assert_eq!(
                                (feature.kind, feature.containment),
                                (structural::Typ::EReference, false),
                                "{at}: the rule says reference"
                            );
                            assert_eq!(
                                *many,
                                feature.bounds.ubound.is_none_or(|ubound| ubound > 1),
                                "{at}: multiplicity"
                            );
                            assert_eq!(
                                super::class_name(*target, ctx),
                                ctx[feature.typ.expect("a resolved target")].name(),
                                "{at}: target"
                            );
                            references += 1;
                        }
                        MergeRule::Attribute { shape, leaf } => {
                            let declared = ctx
                                .classes()
                                .get(*feature.typ.expect("a resolved type"))
                                .expect("a classifier");
                            let rust = rust_type(declared);
                            if let LeafRule::Counter { num, .. } = leaf {
                                assert_eq!(
                                    num_rust(*num),
                                    rust,
                                    "{at}: the counter's width is not the declared one"
                                );
                            }
                            let expected = attribute_type(shape, leaf, &rust);
                            let emitted = AttributeGenerator::new(feature, ctx)
                                .generate()
                                .unwrap_or_else(|e| panic!("{at} should generate: {e}"));
                            assert_eq!(
                                normalize(emitted.tokens()),
                                expected,
                                "{at}: {rule:?} does not name what the generator emits"
                            );
                            compared += 1;
                        }
                        MergeRule::Containment { shape, target } => {
                            let target = ctx.classes().get(target.index()).expect("a classifier");
                            let expected = containment_type(shape, target);
                            let emitted =
                                ContainmentGenerator::new(feature, class.idx, ctx, &cycles)
                                    .generate()
                                    .unwrap_or_else(|e| panic!("{at} should generate: {e}"));
                            assert_eq!(
                                normalize(emitted.tokens()),
                                expected,
                                "{at}: {rule:?} does not name what the generator emits"
                            );
                            compared += 1;
                        }
                    }
                }
            }
        }

        // The counts are asserted so a metamodel silently losing its features
        // — a parse that yields nothing, a package that resolves to the
        // builtins — cannot make this test vacuous.
        // Fifty on 2026-09-07, when `json.ecore`'s five keyed and transparent
        // features carried no rule and were counted as unsupported. D6's
        // amendment gives all five a rule, and each is compared against the
        // construction the generator emits like any other: `Object.entry`
        // against the `UWMapLog<std::string::String, JsonKindLog>`
        // `containment.rs` writes, and the four transparent fields against
        // what their own generator writes, which is what
        // `transparent_field_types` then lifts into the `JsonKind` union.
        // Sixty-nine and eleven on 2026-09-09, when `class_diagram.ecore`
        // joined the list: fourteen more attributes, two more references, and
        // the first `Shape::Set`, `DWFlag`, `LwwRegister`, `FairRegister`,
        // `PORegister`, `TORegister` and enum-typed attribute the rule and
        // the generator have ever been compared on.
        assert_eq!(
            (compared, references, unsupported),
            (69, 11, 0),
            "the census of what was compared moved"
        );
    }

    fn num_rust(num: NumKind) -> &'static str {
        match num {
            NumKind::U8 => "u8",
            NumKind::I16 => "i16",
            NumKind::I32 => "i32",
            NumKind::I64 => "i64",
            NumKind::F32 => "f32",
            NumKind::F64 => "f64",
        }
    }

    /// The other half of ip2's reference arm: a non-containment reference is
    /// not merely absent from the comparison, it is absent from the generated
    /// record. `DataFlowPort` declares exactly one feature, `entry`, and its
    /// record is empty.
    #[test]
    fn no_field_is_emitted_for_a_non_containment_reference() {
        let parser = EcoreParser::from_file(example("behavior_tree.ecore"))
            .expect("behavior_tree.ecore should parse");
        let pack = crate::find_user_package(&parser.ctx).expect("a user package");
        let (classifiers, _references, _package, _count) =
            crate::generate_from_parser(&parser, pack).expect("generation should succeed");
        let rendered = classifiers.build().to_string().replace(' ', "");
        assert!(
            rendered.contains("record!(DataFlowPort{})"),
            "`DataFlowPort.entry` reached the record: {rendered}"
        );
    }

    /// **ip3** — `bt.ecore` declares `ordered` and `unique` zero times, so the
    /// only facet in the whole file whose value Arachne rather than the
    /// modeller chose is `ordered` on its five multi-valued containments.
    ///
    /// Criterion I-A4 is falsified by any other count, and by a facet with no
    /// source at all. The bad day this prevents is a paper claiming derivation
    /// for a policy nobody wrote down.
    #[test]
    fn ip3_bt_reports_five_house_defaults_all_on_ordered() {
        let parser = EcoreParser::from_file(example("behavior_tree.ecore"))
            .expect("behavior_tree.ecore should parse");
        let ctx = &parser.ctx;
        let pack = crate::find_user_package(ctx).expect("a user package");

        let mut house: Vec<String> = Vec::new();
        let mut features = 0usize;
        for class_idx in pack.classes() {
            let class = &ctx[*class_idx];
            if class.is_enum() {
                continue;
            }
            for feature in class.structural() {
                features += 1;
                let (_, provenance) = merge_rule(feature, class, ctx);
                let at = format!("{}.{}", class.name(), feature.name);
                for (facet, source) in [
                    ("ordered", provenance.ordered),
                    ("unique", provenance.unique),
                    ("leaf", provenance.leaf),
                    ("presence", provenance.presence),
                ] {
                    if source == FacetSource::HouseDefault {
                        house.push(format!("{at}.{facet}"));
                    }
                }
            }
        }
        house.sort();

        assert_eq!(features, 16, "bt.ecore declares sixteen features");
        assert_eq!(
            house,
            [
                "Blackboard.entries.ordered",
                "ControlNode.children.ordered",
                "ExecutionNode.inflowports.ordered",
                "ExecutionNode.outflowports.ordered",
                "Root.behaviortrees.ordered",
            ],
            "the house defaults of bt.ecore moved"
        );
    }

    /// Every facet of every feature of every checked-in metamodel carries a
    /// source. `FacetSource` has no absent value, so this is really the claim
    /// that `merge_rule` returns for all of them, which the census pins.
    #[test]
    fn ip3_every_facet_of_every_feature_carries_a_source() {
        let mut sources: BTreeMap<String, usize> = BTreeMap::new();
        for metamodel in METAMODELS {
            let parser = EcoreParser::from_file(example(metamodel))
                .unwrap_or_else(|e| panic!("{metamodel} should parse: {e}"));
            let ctx = &parser.ctx;
            let pack = crate::find_user_package(ctx).expect("a user package");
            for class_idx in pack.classes() {
                let class = &ctx[*class_idx];
                if class.is_enum() {
                    continue;
                }
                for feature in class.structural() {
                    let (_, provenance) = merge_rule(feature, class, ctx);
                    for source in [
                        provenance.ordered,
                        provenance.unique,
                        provenance.leaf,
                        provenance.presence,
                    ] {
                        *sources.entry(format!("{source:?}")).or_default() += 1;
                    }
                }
            }
        }
        let total: usize = sources.values().sum();
        assert_eq!(total, 4 * 80, "four facets on each of eighty features");
        assert!(
            sources.contains_key("Declared")
                && sources.contains_key("EcoreDefault")
                && sources.contains_key("HouseDefault")
                && sources.contains_key("NotApplicable"),
            "the corpus exercises four of the five sources: {sources:?}"
        );
    }

    /// The 22-row class table of spec 11 §7.1, as the file writes it: slot,
    /// class, instantiable, direct supertypes, concrete closure.
    ///
    /// A `ClassSlot` is a dense position by sorted classifier name, so the row
    /// order below *is* the numbering, and a class inserted into `bt.ecore`
    /// renumbers everything after it. That is the property the table is worth
    /// asserting for.
    const SPEC_11_CLASSES: [(&str, bool, &[&str], &[&str]); 21] = [
        (
            "Action",
            false,
            &["ExecutionNode"],
            &["CloseDoor", "EnterRoom", "OpenDoor"],
        ),
        ("BehaviorTree", true, &[], &["BehaviorTree"]),
        ("Blackboard", true, &[], &["Blackboard"]),
        ("BlackboardEntry", true, &[], &["BlackboardEntry"]),
        ("CloseDoor", true, &["Action"], &["CloseDoor"]),
        ("Condition", false, &["ExecutionNode"], &["IsDoorOpen"]),
        (
            "ControlNode",
            false,
            &["TreeNode"],
            &["Fallback", "Sequence"],
        ),
        ("DataFlowPort", false, &[], &["InFlowPort", "OutFlowPort"]),
        ("Decorator", false, &["TreeNode"], &["Inverter"]),
        ("EnterRoom", true, &["Action"], &["EnterRoom"]),
        (
            "ExecutionNode",
            false,
            &["TreeNode"],
            &["CloseDoor", "EnterRoom", "IsDoorOpen", "OpenDoor"],
        ),
        ("Fallback", true, &["ControlNode"], &["Fallback"]),
        ("InFlowPort", true, &["DataFlowPort"], &["InFlowPort"]),
        ("Inverter", true, &["Decorator"], &["Inverter"]),
        ("IsDoorOpen", true, &["Condition"], &["IsDoorOpen"]),
        ("OpenDoor", true, &["Action"], &["OpenDoor"]),
        ("OutFlowPort", true, &["DataFlowPort"], &["OutFlowPort"]),
        ("Root", true, &[], &["Root"]),
        ("Sequence", true, &["ControlNode"], &["Sequence"]),
        ("SubTree", true, &["TreeNode"], &["SubTree"]),
        (
            "TreeNode",
            false,
            &[],
            &[
                "CloseDoor",
                "EnterRoom",
                "Fallback",
                "Inverter",
                "IsDoorOpen",
                "OpenDoor",
                "Sequence",
                "SubTree",
            ],
        ),
    ];

    /// The 16-row feature table of spec 11 §7.2: the `(class slot, feature
    /// slot)` key, the feature, and the `FacetSource` of `ordered` and
    /// `unique`.
    ///
    /// Spec 11 writes the rule column in a vocabulary of its own proposing —
    /// `Slot`, `Child`, `Children`, `Edge` — which is a *recommendation* for a
    /// later phase and not what the generator compiles today. Phase 5 mirrors
    /// the generator exactly (decision D5), so the rule column here is
    /// `moirai_semantics::MergeRule` and the two vocabularies are compared in
    /// the report rather than in an assertion. The slots and the facet sources
    /// are the same claim in both and are asserted.
    const SPEC_11_FEATURES: [(u16, u16, &str, FacetSource, FacetSource); 16] = [
        (
            17,
            0,
            "Root.behaviortrees",
            FacetSource::HouseDefault,
            FacetSource::NotApplicable,
        ),
        (
            17,
            1,
            "Root.main",
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        ),
        (
            1,
            0,
            "BehaviorTree.ID",
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        ),
        (
            1,
            1,
            "BehaviorTree.blackboard",
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        ),
        (
            1,
            2,
            "BehaviorTree.child",
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        ),
        (
            20,
            0,
            "TreeNode.ID",
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        ),
        (
            20,
            1,
            "TreeNode.name",
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        ),
        (
            10,
            0,
            "ExecutionNode.inflowports",
            FacetSource::HouseDefault,
            FacetSource::NotApplicable,
        ),
        (
            10,
            1,
            "ExecutionNode.outflowports",
            FacetSource::HouseDefault,
            FacetSource::NotApplicable,
        ),
        (
            8,
            0,
            "Decorator.child",
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        ),
        (
            6,
            0,
            "ControlNode.children",
            FacetSource::HouseDefault,
            FacetSource::NotApplicable,
        ),
        (
            7,
            0,
            "DataFlowPort.entry",
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        ),
        (
            2,
            0,
            "Blackboard.entries",
            FacetSource::HouseDefault,
            FacetSource::NotApplicable,
        ),
        (
            3,
            0,
            "BlackboardEntry.key",
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        ),
        (
            3,
            1,
            "BlackboardEntry.value",
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        ),
        (
            19,
            0,
            "SubTree.tree",
            FacetSource::NotApplicable,
            FacetSource::NotApplicable,
        ),
    ];

    /// **ip4** — the table `moirai_semantics::from_descriptor` builds from the
    /// descriptor this crate emits for `bt.ecore` reproduces spec 11 §7's
    /// class and feature tables, slot by slot.
    ///
    /// This is the round trip the interpreted path depends on: what Arachne
    /// derives, writes as JSON and a node reads back is one table, and its
    /// slots are the dense positions the delivery path indexes with. It also
    /// closes ip2's loop — ip2 checks the rule against the generator in
    /// memory, this checks that the rule survives the descriptor.
    #[test]
    fn ip4_from_descriptor_reproduces_spec_11_section_7() {
        let parser = EcoreParser::from_file(example("behavior_tree.ecore"))
            .expect("behavior_tree.ecore should parse");
        let pack = crate::find_user_package(&parser.ctx).expect("a user package");
        let descriptor =
            crate::codegen::descriptor::descriptor_json(&parser.ctx, pack).expect("a descriptor");
        let table = moirai_semantics::from_descriptor(&descriptor)
            .expect("bt.ecore needs neither a keyed nor a transparent feature");

        // §7.1, the class table.
        let derived: Vec<(String, bool, Vec<String>, Vec<String>)> = table
            .classes
            .iter()
            .enumerate()
            .map(|(index, class)| {
                assert_eq!(class.slot.index(), index, "a class slot is its position");
                let name = |slot: &moirai_semantics::ClassSlot| {
                    table.classes[slot.index()].name.to_string()
                };
                (
                    class.name.to_string(),
                    class.instantiable,
                    class.supers.iter().map(name).collect(),
                    class.concrete.iter().map(name).collect(),
                )
            })
            .collect();
        let expected: Vec<(String, bool, Vec<String>, Vec<String>)> = SPEC_11_CLASSES
            .iter()
            .map(|(name, instantiable, supers, concrete)| {
                (
                    (*name).to_string(),
                    *instantiable,
                    supers.iter().map(|name| (*name).to_string()).collect(),
                    concrete.iter().map(|name| (*name).to_string()).collect(),
                )
            })
            .collect();
        assert_eq!(derived, expected, "spec 11 §7.1's class table");

        // The enum, and the root. Enums are numbered into their own dense
        // space, so `Status` is E0 and addresses `enums`.
        assert_eq!(
            (
                table.enums.len(),
                table.enums[0].name.to_string(),
                table.enums[0]
                    .literals
                    .iter()
                    .map(|literal| literal.to_string())
                    .collect::<Vec<_>>(),
                table.roots.clone(),
            ),
            (
                1,
                "Status".to_string(),
                vec![
                    "RUNNING".to_string(),
                    "SUCCESS".to_string(),
                    "FAILURE".to_string()
                ],
                vec![moirai_semantics::ClassSlot(17)],
            ),
            "spec 11 §7.1's enum row and `root = Some(17)`"
        );

        // §7.2, the feature table: every key, in the order the spec lists it.
        let mut derived: Vec<(u16, u16, String, FacetSource, FacetSource)> = Vec::new();
        for class in &table.classes {
            for feature in &class.declared {
                derived.push((
                    class.slot.0,
                    feature.slot.0,
                    format!("{}.{}", class.name, feature.name),
                    feature.provenance.ordered,
                    feature.provenance.unique,
                ));
            }
        }
        derived.sort();
        let mut expected: Vec<(u16, u16, String, FacetSource, FacetSource)> = SPEC_11_FEATURES
            .iter()
            .map(|(class, feature, name, ordered, unique)| {
                (*class, *feature, (*name).to_string(), *ordered, *unique)
            })
            .collect();
        expected.sort();
        assert_eq!(derived, expected, "spec 11 §7.2's feature table");

        // The rules themselves, in the vocabulary phase 5 actually runs. Five
        // sequences, five single containments, five text slots and one
        // reference, which is the same shape §7.2's rule column describes in
        // its own words.
        let slot = |name: &str| {
            table
                .classes
                .iter()
                .find(|class| &*class.name == name)
                .unwrap_or_else(|| panic!("{name}"))
                .slot
        };
        let rule = |class: &str, feature: &str| -> MergeRule {
            let class = table
                .classes
                .iter()
                .find(|candidate| &*candidate.name == class)
                .unwrap_or_else(|| panic!("{class}"));
            *class
                .declared
                .iter()
                .find(|candidate| &*candidate.name == feature)
                .map(|candidate| &candidate.merge)
                .unwrap_or_else(|| panic!("{feature}"))
        };
        let text = MergeRule::Attribute {
            shape: Shape::Single,
            leaf: LeafRule::Text,
        };
        assert_eq!(
            [
                rule("ControlNode", "children"),
                rule("Root", "main"),
                rule("BehaviorTree", "ID"),
                rule("TreeNode", "name"),
                rule("DataFlowPort", "entry"),
            ],
            [
                MergeRule::Containment {
                    shape: Shape::Sequence,
                    target: slot("TreeNode")
                },
                MergeRule::Containment {
                    shape: Shape::Single,
                    target: slot("BehaviorTree")
                },
                text,
                MergeRule::Attribute {
                    shape: Shape::Optional,
                    leaf: LeafRule::Text
                },
                MergeRule::Reference {
                    many: false,
                    target: slot("BlackboardEntry")
                },
            ],
            "the rules the descriptor carries"
        );

        // Inheritance is flattened once, at parse time: a `Sequence` shows the
        // eight features of its four-deep supertype chain and nothing else.
        let sequence = &table.classes[slot("Sequence").index()];
        assert_eq!(
            sequence
                .visible
                .iter()
                .map(|(name, owner, _)| format!("{}.{name}", table.classes[owner.index()].name))
                .collect::<Vec<_>>(),
            vec![
                "TreeNode.ID".to_string(),
                "ControlNode.children".to_string(),
                "TreeNode.name".to_string(),
            ],
            "`Sequence` sees its own features and its supertypes'"
        );
    }

    /// **ip5's boundary, from the emitting side** — the descriptor this crate
    /// writes for `json.ecore` is read by `from_descriptor`, and the table it
    /// becomes carries the two forms D6 used to refuse: `Object.entry` keyed
    /// by a string onto `Json`, and all five concrete classes marked
    /// transparent onto the one feature each is represented by.
    ///
    /// This assertion was the refusal until 2026-09-08. The refusal itself is
    /// still reachable and still tested, in `moirai-semantics`'s own
    /// `ip5_a_keyed_feature_is_refused_with_a_sentence_naming_it`: a
    /// descriptor written before the amendment spells `Object.entry` as
    /// `unsupported`, which carries no rule, and a table cannot run what a
    /// descriptor does not say.
    #[test]
    fn from_descriptor_reads_the_json_descriptor_keyed_and_transparent() {
        let parser =
            EcoreParser::from_file(example("json.ecore")).expect("json.ecore should parse");
        let pack = crate::find_user_package(&parser.ctx).expect("a user package");
        let descriptor =
            crate::codegen::descriptor::descriptor_json(&parser.ctx, pack).expect("a descriptor");
        let table = moirai_semantics::from_descriptor(&descriptor)
            .expect("json.ecore's descriptor is a table this crate's parser reads");

        let class = |name: &str| {
            table
                .classes
                .iter()
                .find(|class| &*class.name == name)
                .unwrap_or_else(|| panic!("no class `{name}`"))
        };

        // `UWMapLog<std::string::String, JsonKindLog>`, as data.
        let object = class("Object");
        assert_eq!(
            object.declared[0].merge,
            MergeRule::Containment {
                shape: Shape::Keyed { key: KeyKind::Str },
                target: class("Json").slot,
            },
            "`Object.entry` is a map from a string onto whatever a `Json` is"
        );

        // Every concrete class is its one field and nothing else.
        let transparent: Vec<(String, String)> = table
            .classes
            .iter()
            .filter_map(|class| {
                class.transparent.map(|slot| {
                    (
                        class.name.to_string(),
                        class.visible[slot.index()].0.to_string(),
                    )
                })
            })
            .collect();
        assert_eq!(
            transparent,
            vec![
                ("Array".to_string(), "items".to_string()),
                ("Boolean".to_string(), "value".to_string()),
                ("Number".to_string(), "value".to_string()),
                ("Object".to_string(), "entry".to_string()),
                ("String".to_string(), "value".to_string()),
            ],
            "the five classes `classifiers.rs` renders as `JsonKind`'s variants"
        );
    }

    /// The house default that costs, spelled out: a multi-valued attribute
    /// with no `unique` is a bag or a list because Arachne says so, and EMF
    /// says the opposite. `kitchen_sink.ecore`'s `set` is the sharp case — the
    /// name says set, the silent facet makes it a bag.
    #[test]
    fn a_silent_unique_on_a_multi_valued_attribute_is_a_house_default() {
        let parser = EcoreParser::from_file(example("pet_metamodels/kitchen_sink.ecore"))
            .expect("kitchen_sink.ecore should parse");
        let ctx = &parser.ctx;
        let pack = crate::find_user_package(ctx).expect("a user package");
        let foo = pack
            .classes()
            .iter()
            .map(|idx| &ctx[*idx])
            .find(|class| class.name() == "Foo")
            .expect("Foo");
        let feature = |name: &str| -> &Structural {
            foo.structural()
                .iter()
                .find(|feature| feature.name == name)
                .unwrap_or_else(|| panic!("Foo.{name}"))
        };

        let (rule, provenance) = merge_rule(feature("set"), foo, ctx);
        assert_eq!(
            (rule, provenance.unique, provenance.ordered),
            (
                MergeRule::Attribute {
                    shape: Shape::Bag,
                    leaf: LeafRule::Counter {
                        num: NumKind::I16,
                        resettable: true,
                    },
                },
                FacetSource::HouseDefault,
                FacetSource::Declared,
            ),
            "`Foo.set` is a bag, because `unique` is silent and Arachne reads that as false"
        );

        // `myChar` writes no `lowerBound`, and since upstream's parser fixes that is Ecore's
        // `0`, not the `1` arachne used to read: the attribute is optional, and the source of
        // its presence is the Ecore default rather than anything the file declared.
        let (rule, provenance) = merge_rule(feature("myChar"), foo, ctx);
        assert_eq!(
            (rule, provenance.leaf, provenance.presence),
            (
                MergeRule::Attribute {
                    shape: Shape::Optional,
                    leaf: LeafRule::Register {
                        tie: TieBreak::MultiValue,
                    },
                },
                FacetSource::HouseDefault,
                FacetSource::EcoreDefault,
            ),
            "`Foo.myChar` is a multi-value register, which is Arachne's pick among five"
        );
    }

    /* ---------- the corpus census: `ip2` over a whole directory tree ---------- */

    /// Where the panic hook parks the message of the panic it just caught.
    static PANIC_MESSAGE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

    /// Replace the default hook with one that records instead of printing, so
    /// a corpus of thousands of files does not bury the census in backtraces.
    fn quiet_panics() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            std::panic::set_hook(Box::new(|info| {
                let payload = info
                    .payload()
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_string())
                    .or_else(|| info.payload().downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "panicked".to_string());
                let at = info.location().map_or_else(String::new, |loc| {
                    format!(" at {}:{}", loc.file(), loc.line())
                });
                *PANIC_MESSAGE.lock().expect("the panic slot") = Some(format!("{payload}{at}"));
            }));
        });
    }

    /// The file the census is inside right now, with its size and the moment
    /// it started, which is all the watchdog below needs.
    static CENSUS_CURRENT: std::sync::Mutex<Option<(String, u64, std::time::Instant)>> =
        std::sync::Mutex::new(None);

    /// A per-file deadline, because one pathological file in a corpus of
    /// thousands must not stall the run.
    ///
    /// A hang is not a panic and cannot be caught like one: the thread that is
    /// stuck cannot be unwound, so the watchdog writes the file's row itself
    /// with the status `hang` and ends the process. Every file already named
    /// in `files.csv` is skipped, so the next invocation resumes past it and
    /// the hung file stays in the record as a hang rather than vanishing.
    fn watchdog(files_path: PathBuf, seconds: u64) {
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(250));
                let stuck = match &*CENSUS_CURRENT.lock().expect("the census slot") {
                    Some((rel, bytes, started)) if started.elapsed().as_secs() >= seconds => {
                        Some((rel.clone(), *bytes))
                    }
                    _ => None,
                };
                let Some((rel, bytes)) = stuck else { continue };
                if let Ok(mut files) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&files_path)
                {
                    let _ = writeln!(
                        files,
                        "{}",
                        csv(&[
                            rel.clone(),
                            bytes.to_string(),
                            "hang".to_string(),
                            "0".to_string(),
                            "0".to_string(),
                            "0".to_string(),
                            String::new(),
                            String::new(),
                            format!("no answer in {seconds}s, ended by the census watchdog"),
                        ])
                    );
                    let _ = files.flush();
                }
                println!("{rel}: hang after {seconds}s");
                let _ = std::io::stdout().flush();
                std::process::exit(7);
            }
        });
    }

    /// Run `body`, turning a panic into an `Err` carrying its message.
    ///
    /// `merge_rule` and the two generators both `expect` their way through
    /// facts every checked-in metamodel happens to satisfy — a resolved
    /// `eType`, fewer classifiers than a `u16` addresses, a builtin the table
    /// has a Rust type for. A corpus nobody wrote for us does not, and a
    /// census that aborted on the first one would measure nothing.
    fn catching<T>(body: impl FnOnce() -> T) -> Result<T, String> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).map_err(|_| {
            PANIC_MESSAGE
                .lock()
                .expect("the panic slot")
                .take()
                .unwrap_or_else(|| "panicked".to_string())
        })
    }

    /// One error, flattened to a CSV field with the quoted source dropped.
    ///
    /// The parser's error prints its cause chain, then the line and column,
    /// then `current parser state:` followed by the bytes of the file it
    /// stopped inside. The chain and the position are the measurement and the
    /// bytes are somebody else's metamodel: 432 KB of them over ModelSet, and
    /// no part of a third-party corpus belongs in this repository or in the
    /// vault, so the field stops where the quoting starts.
    fn diagnostic(error: &str) -> String {
        let flat = error.lines().collect::<Vec<_>>().join(" | ");
        match flat.find("current parser state:") {
            Some(at) => format!("{}current parser state elided", &flat[..at]),
            None => flat,
        }
    }

    /// Every `.ecore` under `root`, sorted, skipping build and VCS directories.
    fn ecore_files(root: &Path) -> Vec<PathBuf> {
        fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            let mut here: Vec<PathBuf> = Vec::new();
            let mut deeper: Vec<PathBuf> = Vec::new();
            for entry in entries.flatten() {
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                let path = entry.path();
                if kind.is_dir() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name == "target" || name == "node_modules" || name == ".git" {
                        continue;
                    }
                    deeper.push(path);
                } else if kind.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("ecore"))
                {
                    here.push(path);
                }
            }
            here.sort();
            deeper.sort();
            found.append(&mut here);
            for dir in deeper {
                walk(&dir, found);
            }
        }
        let mut found = Vec::new();
        walk(root, &mut found);
        found
    }

    /// One CSV record, every field quoted so a type string full of commas and
    /// angle brackets survives the round trip.
    fn csv(fields: &[String]) -> String {
        fields
            .iter()
            .map(|field| format!("\"{}\"", field.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The `LeafLog` arm `moirai-interp` would hold for this rule, which is
    /// the vocabulary the two-path proof is stated in. A set and a bag replace
    /// the leaf log outright, so they name the arm themselves.
    fn leaf_arm(shape: &Shape, leaf: &LeafRule) -> String {
        match shape {
            Shape::Set {
                tie: SetTie::AddWins,
            } => "SetAw".to_string(),
            Shape::Set {
                tie: SetTie::RemoveWins,
            } => "SetRw".to_string(),
            Shape::Bag => "Bag".to_string(),
            _ => match leaf {
                LeafRule::Text => "Text".to_string(),
                LeafRule::Counter { num, resettable } => format!(
                    "{}{}",
                    if *resettable {
                        "Counter"
                    } else {
                        "SimpleCounter"
                    },
                    num_arm(*num)
                ),
                LeafRule::Flag {
                    wins: FlagWins::Enable,
                } => "FlagEw".to_string(),
                LeafRule::Flag {
                    wins: FlagWins::Disable,
                } => "FlagDw".to_string(),
                LeafRule::Register { tie } | LeafRule::Enum { tie, .. } => {
                    format!("Register{}", tie_arm(*tie))
                }
            },
        }
    }

    fn num_arm(num: NumKind) -> &'static str {
        match num {
            NumKind::U8 => "U8",
            NumKind::I16 => "I16",
            NumKind::I32 => "I32",
            NumKind::I64 => "I64",
            NumKind::F32 => "F32",
            NumKind::F64 => "F64",
        }
    }

    fn tie_arm(tie: TieBreak) -> &'static str {
        match tie {
            TieBreak::MultiValue => "Mv",
            TieBreak::LastWriterWins => "Lww",
            TieBreak::Fair => "Fair",
            TieBreak::PartialOrder => "Po",
            TieBreak::TotalOrder => "To",
        }
    }

    /// The shape's own name, in the descriptor's spelling.
    fn shape_name(shape: &Shape) -> String {
        match shape {
            Shape::Single => "single".to_string(),
            Shape::Optional => "optional".to_string(),
            Shape::Sequence => "sequence".to_string(),
            Shape::Set {
                tie: SetTie::AddWins,
            } => "set-aw".to_string(),
            Shape::Set {
                tie: SetTie::RemoveWins,
            } => "set-rw".to_string(),
            Shape::Bag => "bag".to_string(),
            Shape::Keyed { key } => format!("keyed-{}", key_name(*key)),
            Shape::OrderedSet => "ordered-set".to_string(),
        }
    }

    fn key_name(key: KeyKind) -> String {
        match key {
            KeyKind::Str => "str".to_string(),
            KeyKind::Bool => "bool".to_string(),
            KeyKind::Char => "char".to_string(),
            KeyKind::Num { num } => num_arm(num).to_ascii_lowercase(),
            KeyKind::Enum { .. } => "enum".to_string(),
        }
    }

    /// Whether this feature sits inside the constructions the oracles have
    /// proven two-path equal, and when it does not, which way it falls out.
    ///
    /// `ordered-set` is the cell with no construction: the generator warns and
    /// compiles a list, and `Shape::effective` degrades it the same way, so the
    /// two paths agree by both dropping the declaration rather than by
    /// honouring it. `reference` is the capability the interpreted path does
    /// not implement at all. `unsupported` is a behavioural flag, which carries
    /// no construction on either path.
    fn proof_standing(rule: &MergeRule) -> &'static str {
        match rule {
            MergeRule::Attribute { shape, leaf } => match (shape, leaf) {
                (Shape::OrderedSet, _) => "ordered-set",
                (
                    _,
                    LeafRule::Counter {
                        resettable: false, ..
                    },
                ) => "unreachable-arm",
                _ => "proven",
            },
            MergeRule::Containment { shape, .. } => match shape {
                Shape::Single | Shape::Optional | Shape::Sequence | Shape::Keyed { .. } => "proven",
                _ => "unreachable-arm",
            },
            MergeRule::Reference { .. } => "reference",
            MergeRule::Unsupported { .. } => "unsupported",
        }
    }

    /// Whether the file wrote a facet down at all: `true`, `false` or silence.
    fn facet(value: Option<bool>) -> String {
        match value {
            Some(true) => "true".to_string(),
            Some(false) => "false".to_string(),
            None => "absent".to_string(),
        }
    }

    /// The census of one feature: the derived rule, the type the generator
    /// emits, and whether they agree.
    ///
    /// This is `ip2`'s loop body with the assertion removed. It calls the same
    /// `attribute_type` and `containment_type` the assertion calls, so a
    /// disagreement found here is a disagreement `ip2` would have failed on.
    fn census_feature(
        rel: &str,
        class: &Class,
        feature: &Structural,
        ctx: &ecore_rs::ctx::Ctx,
        cycles: &crate::codegen::cycles::CycleAnalysis,
    ) -> Vec<String> {
        let (rule, provenance) = merge_rule(feature, class, ctx);
        let many = !matches!(
            (feature.bounds.lbound, feature.bounds.ubound),
            (0, Some(1)) | (0, Some(0)) | (1, Some(1))
        );

        // The generator runs first. When it refuses the feature there is no
        // type to compare against and no reason to ask the table for one,
        // which is also what keeps a metamodel the generator would reject from
        // being counted as a disagreement.
        let emitted = match &rule {
            MergeRule::Attribute { .. } => Some(
                AttributeGenerator::new(feature, ctx)
                    .generate()
                    .map(|fragment| normalize(fragment.tokens()))
                    .map_err(|error| error.to_string()),
            ),
            MergeRule::Containment { .. } => Some(
                ContainmentGenerator::new(feature, class.idx, ctx, cycles)
                    .generate()
                    .map(|fragment| normalize(fragment.tokens()))
                    .map_err(|error| error.to_string()),
            ),
            _ => None,
        };

        let (shape_column, leaf_column, arm, expected, emitted_column, agree) =
            match (&rule, emitted) {
                (MergeRule::Unsupported { reason }, _) => (
                    String::new(),
                    reason.as_str().to_string(),
                    String::new(),
                    String::new(),
                    String::new(),
                    "no-field".to_string(),
                ),
                (MergeRule::Reference { many, .. }, _) => (
                    if *many { "many" } else { "single" }.to_string(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    "no-field".to_string(),
                ),
                (_, Some(Err(error))) => (
                    match &rule {
                        MergeRule::Attribute { shape, .. }
                        | MergeRule::Containment { shape, .. } => shape_name(shape),
                        _ => String::new(),
                    },
                    String::new(),
                    String::new(),
                    String::new(),
                    error,
                    "generator-refused".to_string(),
                ),
                (MergeRule::Attribute { shape, leaf }, Some(Ok(emitted))) => {
                    let declared = feature
                        .typ
                        .and_then(|typ| ctx.classes().get(*typ))
                        .expect("a generated attribute has a resolved type");
                    let rust = rust_type(declared);
                    let expected = attribute_type(shape, leaf, &rust);
                    let agree = if expected == emitted { "yes" } else { "NO" };
                    (
                        shape_name(shape),
                        format!("{leaf:?}"),
                        leaf_arm(shape, leaf),
                        expected,
                        emitted,
                        agree.to_string(),
                    )
                }
                (MergeRule::Containment { shape, target }, Some(Ok(emitted))) => {
                    let target = ctx
                        .classes()
                        .get(target.index())
                        .expect("a containment target is a classifier");
                    let expected = containment_type(shape, target);
                    let agree = if expected == emitted { "yes" } else { "NO" };
                    (
                        shape_name(shape),
                        String::new(),
                        String::new(),
                        expected,
                        emitted,
                        agree.to_string(),
                    )
                }
                (_, None) => {
                    unreachable!("only a reference and an unsupported feature emit nothing")
                }
            };

        vec![
            rel.to_string(),
            class.name().to_string(),
            feature.name.clone(),
            match feature.kind {
                structural::Typ::EAttribute => "attribute",
                structural::Typ::EReference if feature.containment => "containment",
                structural::Typ::EReference => "reference",
            }
            .to_string(),
            match &rule {
                MergeRule::Attribute { .. } => "attribute",
                MergeRule::Containment { .. } => "containment",
                MergeRule::Reference { .. } => "reference",
                MergeRule::Unsupported { .. } => "unsupported",
            }
            .to_string(),
            many.to_string(),
            feature.bounds.lbound.to_string(),
            feature
                .bounds
                .ubound
                .map_or_else(|| "*".to_string(), |bound| bound.to_string()),
            facet(feature.ordered),
            facet(feature.unique),
            shape_column,
            leaf_column,
            arm,
            expected,
            emitted_column,
            agree,
            proof_standing(&rule).to_string(),
            format!("{:?}", provenance.ordered),
            format!("{:?}", provenance.unique),
            format!("{:?}", provenance.leaf),
            format!("{:?}", provenance.presence),
            datatype_annotation(feature).cloned().unwrap_or_default(),
        ]
    }

    /// The header of `results.csv`, one column per field `census_feature`
    /// returns.
    const CENSUS_COLUMNS: [&str; 22] = [
        "file",
        "class",
        "feature",
        "ecore_kind",
        "rule_kind",
        "multivalued",
        "lower_bound",
        "upper_bound",
        "ordered_declared",
        "unique_declared",
        "shape",
        "leaf",
        "leaf_arm",
        "expected_type",
        "emitted_type",
        "agree",
        "proof_standing",
        "src_ordered",
        "src_unique",
        "src_leaf",
        "src_presence",
        "annotation",
    ];

    /// The header of `files.csv`.
    const FILE_COLUMNS: [&str; 9] = [
        "file",
        "bytes",
        "status",
        "classes",
        "features",
        "compared",
        "descriptor",
        "table",
        "detail",
    ];

    /// **The corpus census** — `ip2` run as a measurement rather than as an
    /// assertion, over every `.ecore` file under `CENSUS_ROOT`.
    ///
    /// For each file it records whether the parser accepts it; for each
    /// structural feature of each class of its user package it records the
    /// rule `merge_rule` derives, the type `AttributeGenerator` or
    /// `ContainmentGenerator` emits, and whether the two agree. It asserts
    /// nothing at all: a disagreement is a row in `results.csv` with `NO` in
    /// the `agree` column, and the aggregate is computed from the CSV by the
    /// experiment's own script so it can be recomputed without a rebuild.
    ///
    /// It is restartable. Every file already named in `files.csv` is skipped
    /// and both files are appended to, so an interrupted run resumes where it
    /// stopped rather than starting over.
    ///
    /// Knobs, all from the environment because a test takes no arguments:
    /// `CENSUS_ROOT` is the directory walked, `CENSUS_OUT` the directory the
    /// two CSVs are written to, `CENSUS_LIMIT` an optional cap on how many
    /// files this invocation processes, `CENSUS_TIMEOUT` the per-file deadline
    /// in seconds, 60 by default and disabled by 0.
    #[test]
    #[ignore = "a corpus census, not a gate; run it from experiments/ip5-corpus-census/run.sh"]
    fn corpus_census_of_derived_rules_against_emitted_types() {
        let Ok(root) = std::env::var("CENSUS_ROOT") else {
            println!("CENSUS_ROOT is unset; nothing to census");
            return;
        };
        let root = PathBuf::from(root);
        let out = PathBuf::from(
            std::env::var("CENSUS_OUT").unwrap_or_else(|_| "target/census".to_string()),
        );
        let limit: usize = std::env::var("CENSUS_LIMIT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(usize::MAX);
        std::fs::create_dir_all(&out).expect("the census output directory");

        let results_path = out.join("results.csv");
        let files_path = out.join("files.csv");

        // Restart: every file already in `files.csv` has been censused.
        let mut done: BTreeMap<String, ()> = BTreeMap::new();
        if let Ok(text) = std::fs::read_to_string(&files_path) {
            for line in text.lines().skip(1) {
                if let Some(name) = line
                    .strip_prefix('"')
                    .and_then(|rest| rest.split('"').next())
                {
                    done.insert(name.to_string(), ());
                }
            }
        }

        let fresh = done.is_empty();
        let mut results = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&results_path)
            .expect("results.csv");
        let mut files = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&files_path)
            .expect("files.csv");
        if fresh {
            let header = |columns: &[&str]| {
                csv(&columns
                    .iter()
                    .map(|column| (*column).to_string())
                    .collect::<Vec<_>>())
            };
            writeln!(results, "{}", header(&CENSUS_COLUMNS)).expect("the results header");
            writeln!(files, "{}", header(&FILE_COLUMNS)).expect("the files header");
        }

        quiet_panics();
        let timeout: u64 = std::env::var("CENSUS_TIMEOUT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(60);
        if timeout > 0 {
            watchdog(files_path.clone(), timeout);
        }
        let corpus = ecore_files(&root);
        println!(
            "corpus: {} .ecore files under {}",
            corpus.len(),
            root.display()
        );

        let mut processed = 0usize;
        for path in &corpus {
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if done.contains_key(&rel) {
                continue;
            }
            if processed >= limit {
                break;
            }
            processed += 1;
            let bytes = std::fs::metadata(path).map_or(0, |meta| meta.len());
            *CENSUS_CURRENT.lock().expect("the census slot") =
                Some((rel.clone(), bytes, std::time::Instant::now()));
            println!("> {rel}");
            let _ = std::io::stdout().flush();

            let mut rows: Vec<String> = Vec::new();
            let mut status = "ok".to_string();
            let mut detail = String::new();
            let mut classes = 0usize;
            let mut features = 0usize;
            let mut compared = 0usize;
            let mut descriptor_status = String::new();
            let mut table_status = String::new();

            let parsed = catching(|| EcoreParser::from_file(path));
            match parsed {
                Err(panic) => {
                    status = "parse-panic".to_string();
                    detail = panic;
                }
                Ok(Err(error)) => {
                    status = "parse-error".to_string();
                    detail = error.to_string();
                }
                Ok(Ok(parser)) => {
                    let ctx = &parser.ctx;
                    match crate::find_user_package(ctx) {
                        Err(error) => {
                            status = "no-package".to_string();
                            detail = error.to_string();
                        }
                        Ok(pack) => match catching(|| analyze_cycles(ctx)) {
                            Err(panic) => {
                                status = "cycles-panic".to_string();
                                detail = panic;
                            }
                            Ok(Err(error)) => {
                                status = "cycles-error".to_string();
                                detail = error.to_string();
                            }
                            Ok(Ok(cycles)) => {
                                match catching(|| {
                                    crate::codegen::descriptor::descriptor_json(ctx, pack)
                                }) {
                                    Err(panic) => {
                                        descriptor_status = format!("panic: {panic}");
                                        table_status = "skipped".to_string();
                                    }
                                    Ok(Err(error)) => {
                                        descriptor_status = format!("error: {error}");
                                        table_status = "skipped".to_string();
                                    }
                                    Ok(Ok(descriptor)) => {
                                        descriptor_status = "ok".to_string();
                                        table_status = match catching(|| {
                                            moirai_semantics::from_descriptor(&descriptor)
                                        }) {
                                            Err(panic) => format!("panic: {panic}"),
                                            Ok(Err(error)) => format!("error: {error}"),
                                            Ok(Ok(_)) => "ok".to_string(),
                                        };
                                    }
                                }

                                for class_idx in pack.classes() {
                                    let class = &ctx[*class_idx];
                                    if class.is_enum() {
                                        continue;
                                    }
                                    classes += 1;
                                    for feature in class.structural() {
                                        features += 1;
                                        match catching(|| {
                                            census_feature(&rel, class, feature, ctx, &cycles)
                                        }) {
                                            Ok(fields) => {
                                                compared += 1;
                                                rows.push(csv(&fields));
                                            }
                                            Err(panic) => {
                                                rows.push(csv(&[
                                                    rel.clone(),
                                                    class.name().to_string(),
                                                    feature.name.clone(),
                                                    String::new(),
                                                    "panic".to_string(),
                                                    String::new(),
                                                    String::new(),
                                                    String::new(),
                                                    facet(feature.ordered),
                                                    facet(feature.unique),
                                                    String::new(),
                                                    String::new(),
                                                    String::new(),
                                                    String::new(),
                                                    panic,
                                                    "derivation-panic".to_string(),
                                                    "panic".to_string(),
                                                    String::new(),
                                                    String::new(),
                                                    String::new(),
                                                    String::new(),
                                                    String::new(),
                                                ]));
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }
                }
            }

            for row in &rows {
                writeln!(results, "{row}").expect("a results row");
            }
            writeln!(
                files,
                "{}",
                csv(&[
                    rel.clone(),
                    bytes.to_string(),
                    status.clone(),
                    classes.to_string(),
                    features.to_string(),
                    compared.to_string(),
                    descriptor_status,
                    table_status,
                    diagnostic(&detail),
                ])
            )
            .expect("a files row");
            results.flush().expect("flush results");
            files.flush().expect("flush files");
            *CENSUS_CURRENT.lock().expect("the census slot") = None;
            println!("{rel}: {status}, {features} features");
        }

        println!(
            "censused {processed} files this invocation; {} were already in {}",
            done.len(),
            files_path.display()
        );
    }
}
