//! Metamodel descriptor emission.
//!
//! The generated CRDT crates keep none of the metamodel the generator read:
//! `classifiers.rs` is pure CRDT type aliases. This module writes the part a
//! *client* needs back out as data — a `metamodel.json` the generated
//! `network_node` serves on `GET /api/metamodel`, so an editor can shape
//! itself to whatever node it connects to without compiling against the
//! generated types.
//!
//! # Descriptor format (`formatVersion` 1)
//!
//! ```json
//! {
//!   "formatVersion": 1,
//!   "package": "behaviortree",
//!   "nsURI": "http://www.example.org/behaviortree",
//!   "rootClasses": ["Root"],
//!   "classes": {
//!     "TreeNode": {
//!       "abstract": true,
//!       "superTypes": [],
//!       "attributes": [
//!         {"name": "ID", "kind": "string", "many": false,
//!          "required": true, "isId": true}
//!       ],
//!       "containments": [
//!         {"name": "children", "target": "TreeNode", "many": true,
//!          "required": false, "ordered": true}
//!       ],
//!       "references": [
//!         {"name": "entry", "target": "BlackboardEntry", "many": false,
//!          "required": false}
//!       ]
//!     }
//!   },
//!   "enums": {"Status": ["RUNNING", "SUCCESS", "FAILURE"]}
//! }
//! ```
//!
//! Rules:
//! - `kind` is one of `string`, `int`, `float`, `bool`, `enum`; an `enum`
//!   attribute additionally names its enum class under `"enum"`.
//! - `required` means `lowerBound >= 1`; `many` means the upper bound is
//!   unbounded or greater than one.
//! - A class lists its **declared** features only, plus `superTypes`; clients
//!   flatten the inheritance closure themselves. Enum classes appear only
//!   under `enums`.
//! - The class set is the one code generation reaches from the root classes,
//!   so descriptor and generated crate describe the same metamodel slice.

use std::collections::HashSet;

use ecore_rs::{
    ctx::Ctx,
    repr::{Class, Pack, builtin, idx, structural},
};
use log::warn;
use serde_json::{Map, Value, json};

use crate::error::{ArachneError, Result};

/// Version of the descriptor layout above. Bump on any breaking change so
/// clients can refuse what they do not understand.
const FORMAT_VERSION: u64 = 1;

/// Builds the JSON metamodel descriptor for `pack`.
///
/// Fails with [`ArachneError::RootClassNotFound`] when the package has no
/// root class — the same condition under which code generation fails.
pub fn descriptor_json(ctx: &Ctx, pack: &Pack) -> Result<Value> {
    let package_classes: Vec<idx::Class> = pack.classes().iter().copied().collect();
    let package_class_set: HashSet<idx::Class> = package_classes.iter().copied().collect();

    let roots = crate::compute_top_level_roots(ctx, &package_classes, &package_class_set);
    if roots.is_empty() {
        return Err(ArachneError::RootClassNotFound(pack.name().to_string()));
    }

    let mut reachable: HashSet<idx::Class> = HashSet::new();
    for root in &roots {
        reachable.extend(crate::collect_reachable_classes(
            ctx,
            *root,
            &package_class_set,
        ));
    }

    let mut root_names: Vec<&str> = roots.iter().map(|idx| ctx[*idx].name()).collect();
    root_names.sort_unstable();

    // Sorted maps so the emitted descriptor is deterministic.
    let mut classes = Map::new();
    let mut included: Vec<&Class> = reachable
        .iter()
        .map(|idx| &ctx[*idx])
        .filter(|class| !class.is_enum())
        .collect();
    included.sort_unstable_by_key(|class| class.name());
    for class in &included {
        classes.insert(
            class.name().to_string(),
            class_descriptor(ctx, class, &reachable),
        );
    }

    // Every enum of the package, whether a feature reaches it or not: an enum
    // only used by operations is still part of the metamodel's vocabulary.
    let mut enums = Map::new();
    let mut package_enums: Vec<&Class> = package_classes
        .iter()
        .map(|idx| &ctx[*idx])
        .filter(|class| class.is_enum())
        .collect();
    package_enums.sort_unstable_by_key(|class| class.name());
    for class in package_enums {
        let literals: Vec<&str> = class.literals().iter().map(|lit| lit.name()).collect();
        enums.insert(class.name().to_string(), json!(literals));
    }

    Ok(json!({
        "formatVersion": FORMAT_VERSION,
        "package": pack.name(),
        "nsURI": pack.ns_uri(),
        "rootClasses": root_names,
        "classes": classes,
        "enums": enums,
    }))
}

/// Describes one class: declared features partitioned into attributes,
/// containments and plain references, plus its super types.
fn class_descriptor(ctx: &Ctx, class: &Class, included: &HashSet<idx::Class>) -> Value {
    let mut super_types: Vec<&str> = class
        .sup()
        .iter()
        .copied()
        .filter(|sup| included.contains(sup))
        .map(|sup| ctx[sup].name())
        .collect();
    super_types.sort_unstable();

    let mut attributes = Vec::new();
    let mut containments = Vec::new();
    let mut references = Vec::new();

    for feature in class.structural() {
        let many = feature
            .bounds
            .ubound
            .map(|ubound| ubound > 1)
            .unwrap_or(true);
        let required = feature.bounds.lbound >= 1;

        match feature.kind {
            structural::Typ::EAttribute => {
                let Some((kind, enum_name)) = attribute_kind(ctx, class, feature) else {
                    continue;
                };
                let mut attribute = json!({
                    "name": feature.name,
                    "kind": kind,
                    "many": many,
                    "required": required,
                    "isId": feature.is_id,
                });
                if let Some(enum_name) = enum_name {
                    attribute["enum"] = json!(enum_name);
                }
                attributes.push(attribute);
            }
            structural::Typ::EReference => {
                let Some(target) = feature.typ else {
                    warn!(
                        "`{}.{}`: externally-typed reference has no resolved target; \
                         omitted from the descriptor",
                        class.name(),
                        feature.name
                    );
                    continue;
                };
                let target = ctx[target].name();
                if feature.containment {
                    containments.push(json!({
                        "name": feature.name,
                        "target": target,
                        "many": many,
                        "required": required,
                        "ordered": feature.ordered.unwrap_or(true),
                    }));
                } else {
                    references.push(json!({
                        "name": feature.name,
                        "target": target,
                        "many": many,
                        "required": required,
                    }));
                }
            }
        }
    }

    json!({
        "abstract": class.is_abstract() || class.is_interface(),
        "superTypes": super_types,
        "attributes": attributes,
        "containments": containments,
        "references": references,
    })
}

/// The descriptor `kind` of an attribute, with the enum class name when the
/// kind is `enum`. `None` means the attribute cannot be described and is
/// omitted (already warned about).
fn attribute_kind(
    ctx: &Ctx,
    class: &Class,
    feature: &structural::Structural,
) -> Option<(&'static str, Option<String>)> {
    let Some(target) = feature.typ else {
        warn!(
            "`{}.{}`: externally-typed attribute has no resolved type; \
             omitted from the descriptor",
            class.name(),
            feature.name
        );
        return None;
    };
    let target = &ctx[target];

    if target.is_enum() {
        return Some(("enum", Some(target.name().to_string())));
    }

    let kind = match target.name().parse::<builtin::Typ>() {
        Ok(builtin::Typ::EString) | Ok(builtin::Typ::EChar) => "string",
        Ok(builtin::Typ::EByte)
        | Ok(builtin::Typ::EShort)
        | Ok(builtin::Typ::EInt)
        | Ok(builtin::Typ::ELong) => "int",
        Ok(builtin::Typ::EFloat) | Ok(builtin::Typ::EDouble) => "float",
        Ok(builtin::Typ::EBoolean) => "bool",
        // `Object` and custom EDataTypes have no structure to offer a typed
        // editor; a string field is the honest lowest common denominator.
        Ok(builtin::Typ::Object) | Err(()) => {
            warn!(
                "`{}.{}`: attribute type `{}` has no typed editor mapping; \
                 described as `string`",
                class.name(),
                feature.name,
                target.name()
            );
            "string"
        }
    };
    Some((kind, None))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::{Value, json};

    use crate::EcoreParser;

    fn bt_descriptor() -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/bt.ecore");
        let parser = EcoreParser::from_file(path).expect("bt.ecore should parse");
        let pack = crate::find_user_package(&parser.ctx).expect("bt has a user package");
        super::descriptor_json(&parser.ctx, pack).expect("descriptor should build")
    }

    #[test]
    fn bt_names_root_as_its_only_root_class() {
        let descriptor = bt_descriptor();
        assert_eq!(descriptor["rootClasses"], json!(["Root"]));
    }

    #[test]
    fn bt_enum_appears_under_enums_and_not_under_classes() {
        let descriptor = bt_descriptor();
        assert_eq!(
            (
                &descriptor["enums"]["Status"],
                descriptor["classes"].get("Status")
            ),
            (&json!(["RUNNING", "SUCCESS", "FAILURE"]), None)
        );
    }

    #[test]
    fn bt_tree_node_declares_only_its_own_features() {
        let descriptor = bt_descriptor();
        let tree_node = &descriptor["classes"]["TreeNode"];
        assert_eq!(
            tree_node["attributes"],
            json!([
                {"name": "ID", "kind": "string", "many": false,
                 "required": true, "isId": false},
                {"name": "name", "kind": "string", "many": false,
                 "required": false, "isId": false},
            ])
        );
    }

    #[test]
    fn bt_control_node_containment_is_many_ordered_and_subclass_typed() {
        let descriptor = bt_descriptor();
        let control_node = &descriptor["classes"]["ControlNode"];
        assert_eq!(
            control_node["containments"],
            json!([
                {"name": "children", "target": "TreeNode", "many": true,
                 "required": false, "ordered": true},
            ])
        );
    }

    #[test]
    fn bt_non_containment_reference_stays_a_reference() {
        let descriptor = bt_descriptor();
        let port = &descriptor["classes"]["DataFlowPort"];
        assert_eq!(
            port["references"],
            json!([
                {"name": "entry", "target": "BlackboardEntry",
                 "many": false, "required": false},
            ])
        );
    }
}
