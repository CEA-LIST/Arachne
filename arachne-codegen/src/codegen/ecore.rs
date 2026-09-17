//! Ecore's own classes, as the parser builds them in when a metamodel refers to one of them (see
//! `ecore_rs::repr::ecore`).

use ecore_rs::{
    ctx::Ctx,
    repr::{Structural, ecore::Typ, idx, structural},
};

use crate::codegen::{
    annotation::UwMapSpec,
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
/// nothing, and a non-containment reference typed by it refers to an object of any class.
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
