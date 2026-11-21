//! Ecore's own classes, as the parser builds them in when a metamodel refers to one of them (see
//! `ecore_rs::repr::ecore`).

use ecore_rs::{
    ctx::Ctx,
    repr::{Class, Structural, annot::Val, ecore::Typ, idx, structural},
};

use crate::codegen::{
    annotation::{UwMapSpec, uw_map_spec},
    feature::bounds::{BoundKind, normalize_bounds},
};

/// The `instanceClassName`s EMF recognises a map entry class by.
const MAP_ENTRY_INSTANCE_CLASS_NAMES: [&str; 2] = ["java.util.Map$Entry", "java.util.Map.Entry"];

/// Ecore's `EObject`, if the metamodel refers to Ecore's classes.
pub fn eobject(ctx: &Ctx) -> Option<idx::Class> {
    ctx.ecore_classes().get(&Typ::EObject).copied()
}

/// True if `class` is Ecore's `EObject`, the class of every object.
///
/// `EObject` has no structural feature and is never generated: a supertype `EObject` adds
/// nothing, and a feature typed by it holds or refers to an object of any class, which is not
/// generated either.
pub fn is_eobject(ctx: &Ctx, class: idx::Class) -> bool {
    eobject(ctx) == Some(class)
}

/// The map `feature` is by EMF's convention, if it is a many-valued containment of Ecore's own
/// map entry class, `EStringToStringMapEntry`.
///
/// EMF recognises a map entry class by its `instanceClassName`, `java.util.Map$Entry`, and its
/// features `key` and `value`, and a many-valued containment of such a class is a map (an `EMap`)
/// whose keys are unique. Only Ecore's own entry class is recognised here, so a metamodel's own
/// entry classes are generated as before, as contained objects, unless annotated as `uw-map`.
pub fn map_entry_spec(ctx: &Ctx, feature: &Structural) -> Option<UwMapSpec> {
    if feature.kind != structural::Typ::EReference
        || !feature.containment
        || !matches!(
            normalize_bounds(feature.bounds, &feature.name).0,
            BoundKind::Many
        )
    {
        return None;
    }
    let entry = feature.typ?;
    if !ctx.is_ecore_class(entry) {
        return None;
    }
    let entry = &ctx[entry];
    let is_map_entry = entry
        .instance_class_name()
        .is_some_and(|name| MAP_ENTRY_INSTANCE_CLASS_NAMES.contains(&name));
    let has_feature = |name: &str| entry.structural().iter().any(|f| f.name == name);

    (is_map_entry && has_feature("key") && has_feature("value")).then(|| UwMapSpec {
        key_feature: "key".to_string(),
        value_feature: "value".to_string(),
    })
}

/// The `datatype` a metamodel gives an attribute of one of Ecore's own classes, or `None`.
///
/// No element of a metamodel file declares `EAnnotation.source` or
/// `EStringToStringMapEntry.key` and `.value`: Arachne builds Ecore's classes in, so there is no
/// feature to annotate. A metamodel names them instead in a `urn:arachne:semantics` annotation
/// on one of its own packages, one detail per feature, keyed by the feature as the descriptor
/// lists it and valued by the spelling a feature-level `datatype` takes:
///
/// ```xml
/// <eAnnotations source="urn:arachne:semantics">
///   <details key="ecore::EAnnotation.source" value="mv-register"/>
/// </eAnnotations>
/// ```
///
/// Only the attributes of Ecore's own classes are reached this way; a metamodel's own feature
/// carries its annotation itself. A metamodel that writes no such detail is described and generated as
/// before.
pub fn ecore_feature_datatype<'a>(
    ctx: &'a Ctx,
    class: &Class,
    feature: &Structural,
) -> Option<&'a Val> {
    if feature.kind != structural::Typ::EAttribute || !ctx.is_ecore_class(class.idx) {
        return None;
    }
    let key = format!(
        "{}::{}.{}",
        ecore_rs::repr::ecore::PACKAGE_NAME,
        class.name(),
        feature.name
    );
    ctx.pack_indices()
        .filter(|pack| Some(*pack) != ctx.ecore_pack())
        .flat_map(|pack| ctx[pack].annotations())
        .filter(|annot| annot.source() == "urn:arachne:semantics")
        .find_map(|annot| annot.details().get(&key))
}

/// The map a containment is generated as, if any: by its `uw-map` annotation, or by EMF's map
/// entry convention if it contains Ecore's own `EStringToStringMapEntry`.
pub fn map_spec(ctx: &Ctx, feature: &Structural) -> Option<UwMapSpec> {
    uw_map_spec(feature).or_else(|| map_entry_spec(ctx, feature))
}
