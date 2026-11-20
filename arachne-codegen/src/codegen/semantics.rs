//! The merge policy of one structural feature, derived as data.
//!
//! # What this module is for
//!
//! The generator decides a feature's CRDT once, at generation, by three
//! matches: `datatype/to_crdt.rs` maps the Ecore builtin to a CRDT family and
//! a Rust width, `annotation.rs`'s `datatype_override_in` replaces that family
//! from a closed set of spellings when a `urn:arachne:semantics` `datatype`
//! governs the feature (its own detail or, for an attribute of one of Ecore's
//! own classes, the package-level detail that names it), and
//! `feature/attribute.rs` combines the result with the triple
//! `(bound_kind, unique.unwrap_or(true), ordered.unwrap_or(true))` to pick the
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
//!   to say, and Arachne's own rule supplies the value. Every leaf but
//!   `EString`'s is the sharp case: Ecore names the type and Arachne picks the
//!   CRDT family, a counter's resettability or a register's tie-break on top of
//!   it. A silent `unique` used to be the sharpest case of all and is not one
//!   any more: Ecore defaults it to `true` and the rule reads it as `true`.
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
//! annotation itself is carried verbatim under the entry's `annotation` key:
//! the `datatype` value [`feature_datatype`] answers, which is the feature's
//! own detail or, for an attribute of one of Ecore's own classes, the
//! package-level detail that names it.
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
    repr::{Class, Structural, annot::Val, builtin::Typ as BuiltinTyp, ecore, idx, structural},
};
use moirai_semantics::{
    ClassSlot, FacetSource, FlagWins, KeyKind, LeafRule, MergeRule, NumKind, Provenance, SetTie,
    Shape, TieBreak, UnsupportedReason,
};
use serde_json::{Value, json};

use crate::codegen::ecore::{ecore_feature_datatype, is_eobject, map_spec};

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
    if let Some(reason) = unsupported_reason(feature, class, ctx) {
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
        // A non-containment reference is an arc of the generated reference
        // graph, which names its target by creation, so its rule is SI.05's
        // reference by store identity; one that declares an `eOpposite` keeps
        // today's string rule (Cam's decision 3). `process_structural_features`
        // emits no field for either, so neither has a facet to source.
        structural::Typ::EReference if !feature.containment => (
            if feature.opposite().is_some() {
                MergeRule::Reference {
                    many: presence == Presence::Many,
                    target: target_slot(feature),
                }
            } else {
                MergeRule::IdentityReference {
                    many: presence == Presence::Many,
                    target: target_slot(feature),
                }
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
            let datatype = feature_datatype(feature, class, ctx);
            let (leaf, leaf_source) = leaf_rule(feature, datatype, ctx);
            let (shape, ordered, unique) = attribute_shape(feature, presence, datatype);
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
        MergeRule::IdentityReference { many, target } => json!({
            "kind": "identityReference",
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

/// The `urn:arachne:semantics` `datatype` value that governs an attribute of
/// `class`, the class declaring it: its own annotation, or, for a feature of
/// one of Ecore's own classes, which no metamodel file declares, the detail a
/// package annotation of the metamodel names it by
/// (`annotation.rs`'s `ecore_feature_datatype`). What the descriptor writes
/// under the entry's `annotation` key, and what [`merge_rule`] reads.
pub fn feature_datatype<'a>(
    feature: &'a Structural,
    class: &Class,
    ctx: &'a Ctx,
) -> Option<&'a Val> {
    datatype_annotation(feature).or_else(|| ecore_feature_datatype(ctx, class, feature))
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

/// The classifier a slot produced by this module names, under the key the
/// descriptor lists it by.
pub fn class_name(slot: ClassSlot, ctx: &Ctx) -> String {
    let index = idx::Class::from(slot.index());
    descriptor_class_name(ctx, index)
}

/// The key the descriptor lists a classifier under.
///
/// A metamodel's own classifier keeps its name. One of Ecore's own is
/// qualified by its package — `ecore::EAnnotation` — and the reason is that
/// the descriptor keys `classes` by name while EMF keys a classifier by
/// *(package nsURI, name)*: ModelSet holds 36 metamodels of their own that
/// declare a class called `EAnnotation`, and EMF lets one of them coexist
/// with Ecore's without a diagnostic (spec 05 §2.5 and §8f). A bare name
/// would silently merge the two.
///
/// `::` is what makes the key safe rather than merely unlikely. A reserved
/// prefix is not: `EcoreEAnnotation` is a perfectly good Ecore class name and
/// a metamodel may declare it, which is why the *generated* path, whose names
/// are Rust identifiers and cannot hold a `::`, has to carry a clash
/// resolution beside its prefix (`ident.rs`). A descriptor key is a JSON
/// string, so it can hold the one character an Ecore name cannot: EMF
/// requires a classifier name to be a well-formed Java identifier
/// (`EcoreValidator`'s `WellFormedName`), and `descriptor.rs` refuses a
/// descriptor whose two classifiers would land on one key anyway, so the
/// guarantee does not rest on a metamodel being well-formed.
///
/// The two spellings therefore differ across the paths — `ecore::EAnnotation`
/// here and `EcoreEAnnotation` there — and the census's `Agreement::Nominal`
/// is where that is recorded and checked.
pub fn descriptor_class_name(ctx: &Ctx, class: idx::Class) -> String {
    let name = ctx
        .classes()
        .get(*class)
        .expect("a slot this module minted indexes the context it was minted from")
        .name();
    if ctx.is_ecore_class(class) {
        format!("{}::{name}", ecore::PACKAGE_NAME)
    } else {
        name.to_string()
    }
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
    let spec = map_spec(ctx, feature)?;
    if feature.kind != structural::Typ::EReference
        || !feature.containment
        || presence_of(feature) != Presence::Many
    {
        return None;
    }
    // Which of the two ways this containment became a map, which is what the
    // two collection facets are sourced to: a `urn:arachne:semantics`
    // `datatype="uw-map"` annotation, or EMF's own convention that a
    // many-valued containment of a class whose `instanceClassName` is
    // `java.util.Map$Entry` *is* a map (spec 05 §3.4). The second is Ecore's
    // rule and not Arachne's, so it is an `ecoreDefault` and not an
    // `annotation`.
    let from_annotation = datatype_annotation(feature)
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("uw-map"));
    let collection_source = if from_annotation {
        FacetSource::Annotation
    } else {
        FacetSource::EcoreDefault
    };

    let entry = ctx.classes().get(*feature.typ?)?;
    let key_feature = entry
        .structural()
        .iter()
        .find(|f| f.name == spec.key_feature)?;
    let value_feature = entry
        .structural()
        .iter()
        .find(|f| f.name == spec.value_feature)?;
    if key_feature.kind != structural::Typ::EAttribute {
        return None;
    }
    // `containment.rs:108-116`: single-valued, or an optional *attribute*.
    let value_presence = presence_of(value_feature);
    if value_presence != Presence::Single
        && !(value_presence == Presence::Optional
            && value_feature.kind == structural::Typ::EAttribute)
    {
        return None;
    }
    let key = key_kind(key_feature, ctx)?;
    let shape = Shape::Keyed { key };

    match value_feature.kind {
        // An optional attribute value: `containment.rs`'s
        // `uw_map_optional_value_log_type` compiles
        // `VecLog<MVRegister<Option<T>>>` for it and refuses a `datatype`
        // annotation on it, so neither the declared type nor any annotation
        // picks the family — Arachne does, to keep a key whose value is
        // absent readable at all.
        structural::Typ::EAttribute if value_presence == Presence::Optional => {
            if datatype_annotation(value_feature).is_some() {
                return None;
            }
            let class = value_feature
                .typ
                .filter(|typ| ctx.classes().get(**typ).is_some_and(Class::is_enum))
                .map(slot);
            Some((
                MergeRule::Attribute {
                    shape,
                    leaf: LeafRule::OptionalRegister { class },
                },
                Provenance {
                    ordered: collection_source,
                    unique: collection_source,
                    leaf: FacetSource::HouseDefault,
                    presence: presence_source(feature),
                },
            ))
        }
        structural::Typ::EAttribute => {
            let (leaf, leaf_source) =
                leaf_rule(value_feature, datatype_annotation(value_feature), ctx);
            Some((
                MergeRule::Attribute { shape, leaf },
                Provenance {
                    // The map replaced the collection outright, so neither
                    // facet the file wrote decided anything.
                    ordered: collection_source,
                    unique: collection_source,
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
                ordered: collection_source,
                unique: collection_source,
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
        Ok(BuiltinTyp::EByte) => KeyKind::Num { num: NumKind::I8 },
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
/// rendered as that feature. The three behavioural flags and the two
/// `EObject` forms below are what is left.
///
/// The three flags come first because they are what the *file* declares, and
/// the generated path reads them first too: `analysis.rs:74` drops a
/// transient reference of one of Ecore's own classes — `EAnnotation`'s
/// back-pointer `eModelElement`, and nothing else in the corpus — before it
/// ever looks at the target.
fn unsupported_reason(feature: &Structural, class: &Class, ctx: &Ctx) -> Option<UnsupportedReason> {
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
    // A feature typed by `EObject` holds or refers to an object of *any*
    // class, and the generator emits nothing for either form:
    // `classifier/mod.rs:173-181` writes no field for such a containment and
    // `reference/analysis.rs:82-88` no arc for such a reference, each with a
    // warning naming the feature. The descriptor says the same thing as data,
    // so a reader is told what is missing rather than left to notice.
    if feature.kind == structural::Typ::EReference
        && feature.typ.is_some_and(|typ| is_eobject(ctx, typ))
    {
        return Some(if feature.containment {
            UnsupportedReason::EObjectContainment
        } else {
            UnsupportedReason::EObjectReference
        });
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
/// outcomes on top of it, as data. `datatype` is the annotation's spelling
/// that governs the feature, if any ([`feature_datatype`]).
fn leaf_rule(feature: &Structural, datatype: Option<&Val>, ctx: &Ctx) -> (LeafRule, FacetSource) {
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
            Ok(BuiltinTyp::EByte) => (counter(NumKind::I8), FacetSource::HouseDefault),
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
    let Some(spelling) = datatype else {
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
fn attribute_shape(
    feature: &Structural,
    presence: Presence,
    datatype: Option<&Val>,
) -> (Shape, FacetSource, FacetSource) {
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

    let unique = feature.unique.unwrap_or(true);
    let ordered = feature.ordered.unwrap_or(true);
    let shape = match (unique, ordered) {
        (false, true) => Shape::Sequence,
        // `GraphLog<List<T>>`, a construction of its own and not a sequence:
        // `attribute.rs:152-162`.
        (true, true) => Shape::OrderedSet,
        (false, false) => Shape::Bag,
        (true, false) => Shape::Set {
            tie: set_tie(datatype),
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
        // Ecore's own default is `true` — `ecore.ecore` gives
        // `ETypedElement.unique` the `defaultValueLiteral` `true` — and
        // Arachne takes it unchanged, which is what makes this an Ecore
        // default and no longer the house rule it was.
        FacetSource::EcoreDefault
    };
    (shape, ordered_source, unique_source)
}

/// Which side of a concurrent add and remove wins, from the `aw-set` and
/// `rw-set` spellings; add-wins when the file names neither.
fn set_tie(datatype: Option<&Val>) -> SetTie {
    match datatype.map(|value| value.trim().to_ascii_lowercase()) {
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
        // Written by hand for the same reason the enum leaf is: the slot this
        // module mints indexes the `Ctx` and the descriptor carries names.
        // The key is absent when the value is not enum-typed, which is what
        // `moirai-semantics`'s `#[serde(default)]` reads back as `None`.
        LeafRule::OptionalRegister { class } => match class {
            Some(class) => json!({
                "kind": "optionalRegister",
                "class": class_name(*class, ctx),
            }),
            None => json!({ "kind": "optionalRegister" }),
        },
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
    use std::path::{Path, PathBuf};

    use ecore_rs::repr::{Structural, idx};
    use moirai_semantics::{
        FacetSource, FlagWins, KeyKind, LeafRule, MergeRule, NumKind, SetTie, Shape, TieBreak,
    };

    use super::merge_rule;
    use crate::EcoreParser;
    use crate::codegen::ecore::is_eobject;

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
    /// plus `pet_metamodels/ecore_builtins.ecore`, the only checked-in file
    /// that reaches Ecore's own classes: the ordered containment of
    /// annotations, the `details` map to a register over an optional value,
    /// and the three features that carry no construction on either path.
    const METAMODELS: [&str; 6] = [
        "behavior_tree.ecore",
        "SimpleUML.ecore",
        "json.ecore",
        "pet_metamodels/kitchen_sink.ecore",
        "class_diagram.ecore",
        "pet_metamodels/ecore_builtins.ecore",
    ];

    /// Every class a generated crate would hold, in the order
    /// `generate_from_parser` ranges over them: the package's own, then
    /// Ecore's but `EObject`, which has no feature and is never generated.
    fn generated_classes(ctx: &ecore_rs::ctx::Ctx, pack: &ecore_rs::repr::Pack) -> Vec<idx::Class> {
        let ecore_classes = ctx.ecore_pack().into_iter().flat_map(|pack| {
            ctx[pack]
                .classes()
                .iter()
                .copied()
                .filter(|class| !is_eobject(ctx, *class))
                .collect::<Vec<_>>()
        });
        pack.classes()
            .iter()
            .copied()
            .chain(ecore_classes)
            .filter(|class| !ctx[*class].is_enum())
            .collect()
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
            for class_idx in &generated_classes(ctx, pack) {
                let class = &ctx[*class_idx];
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
        assert_eq!(total, 4 * 94, "four facets on each of ninety-four features");
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
        // sequences, five single containments and five text slots, which is
        // the same shape §7.2's rule column describes in its own words. The
        // one reference, `DataFlowPort.entry`, is on SI.05's identity rule and
        // is held in `tests/si05_inline_replacements.rs`.
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

    /// The house default that used to cost, and what it costs now that it is
    /// gone: a multi-valued attribute with no `unique` is unique, because
    /// Ecore says so. `kitchen_sink.ecore`'s `set` is the sharp case — the
    /// name says set, the silent facet used to make it a bag, and now it is
    /// the set it is called.
    #[test]
    fn a_silent_unique_on_a_multi_valued_attribute_is_an_ecore_default() {
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
                    shape: Shape::Set {
                        tie: SetTie::AddWins,
                    },
                    leaf: LeafRule::Counter {
                        num: NumKind::I16,
                        resettable: true,
                    },
                },
                FacetSource::EcoreDefault,
                FacetSource::Declared,
            ),
            "`Foo.set` is a set, because `unique` is silent and Ecore reads that as true"
        );

        // Both facets silent on a multi-valued attribute is an ordered set,
        // which is the case the corpus meets 259 times and the one that used
        // to be read as a sequence.
        let (rule, provenance) = merge_rule(feature("bounds0inf"), foo, ctx);
        assert_eq!(
            (rule, provenance.unique, provenance.ordered),
            (
                MergeRule::Attribute {
                    shape: Shape::OrderedSet,
                    leaf: LeafRule::Counter {
                        num: NumKind::I16,
                        resettable: true,
                    },
                },
                FacetSource::EcoreDefault,
                FacetSource::EcoreDefault,
            ),
            "`Foo.bounds0inf` is an ordered set, because both facets default to true"
        );

        // `unique="false"` written down is the one way left to a sequence.
        let (rule, provenance) = merge_rule(feature("simpleList"), foo, ctx);
        assert_eq!(
            (rule, provenance.unique, provenance.ordered),
            (
                MergeRule::Attribute {
                    shape: Shape::Sequence,
                    leaf: LeafRule::Flag {
                        wins: FlagWins::Enable,
                    },
                },
                FacetSource::Declared,
                FacetSource::EcoreDefault,
            ),
            "`Foo.simpleList` is a sequence, because the file writes `unique=\"false\"`"
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
}
