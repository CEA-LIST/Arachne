//! Metamodel descriptor emission.
//!
//! The generated CRDT crates keep none of the metamodel the generator read:
//! `classifiers.rs` is pure CRDT type aliases. This module writes the part a
//! *client* needs back out as data — a `metamodel.json` the generated
//! `network_node` serves on `GET /api/metamodel`, so an editor can shape
//! itself to whatever node it connects to without compiling against the
//! generated types.
//!
//! # Descriptor format (`formatVersion` 2)
//!
//! ```json
//! {
//!   "formatVersion": 2,
//!   "package": "behaviortree",
//!   "nsURI": "http://www.example.org/behaviortree",
//!   "rootClasses": ["Root"],
//!   "classes": {
//!     "TreeNode": {
//!       "abstract": true,
//!       "superTypes": [],
//!       "attributes": [
//!         {"name": "ID", "kind": "string", "many": false,
//!          "required": true, "isId": true,
//!          "facets": {"ordered": null, "unique": null}, "annotation": null,
//!          "merge": {"kind": "attribute", "shape": {"kind": "single"},
//!                    "leaf": {"kind": "text"}},
//!          "provenance": {"ordered": "notApplicable", "unique": "notApplicable",
//!                         "leaf": "declared", "presence": "declared"}}
//!       ],
//!       "containments": [
//!         {"name": "children", "target": "TreeNode", "many": true,
//!          "required": false, "ordered": true,
//!          "facets": {"ordered": null, "unique": null}, "annotation": null,
//!          "merge": {"kind": "containment", "shape": {"kind": "sequence"},
//!                    "target": "TreeNode"},
//!          "provenance": {"ordered": "houseDefault", "unique": "notApplicable",
//!                         "leaf": "notApplicable", "presence": "declared"}}
//!       ],
//!       "references": [
//!         {"name": "entry", "target": "BlackboardEntry", "many": false,
//!          "required": false,
//!          "facets": {"ordered": null, "unique": null}, "annotation": null,
//!          "merge": {"kind": "reference", "many": false,
//!                    "target": "BlackboardEntry"},
//!          "provenance": {"ordered": "notApplicable", "unique": "notApplicable",
//!                         "leaf": "notApplicable", "presence": "notApplicable"}}
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
//! - A class carrying `urn:arachne:representation` `kind="transparent"` adds
//!   one key, `"transparent"`, naming the field it is represented by. The key
//!   is absent on every other class.
//! - The class set is the one code generation reaches from the root classes,
//!   so descriptor and generated crate describe the same metamodel slice.
//!
//! # What version 2 added, and why nothing broke
//!
//! Version 1 described a metamodel for an *editor*: enough shape to draw a
//! form. It said nothing about how two concurrent writes to a feature settle,
//! so a node that wanted to merge by the metamodel had to be compiled against
//! it. Version 2 adds, on every attribute, containment and reference entry:
//!
//! - `facets` — `ordered` and `unique` exactly as the `.ecore` file wrote
//!   them, `null` where it was silent.
//! - `annotation` — the `urn:arachne:semantics` `datatype` string it wrote,
//!   or `null`.
//! - `merge` — the CRDT construction the generator compiles for this feature,
//!   as data, in the closed vocabulary of `moirai_semantics::MergeRule`.
//! - `provenance` — where each of the four facets of that rule came from:
//!   `declared`, `ecoreDefault`, `houseDefault`, `annotation` or
//!   `notApplicable`. This is what keeps "derived" an honest word: a policy
//!   nobody wrote down says so in the file that publishes it.
//!
//! The derivation is [`crate::codegen::semantics::merge_rule`], and its module
//! documents every reading it takes. **Keys were added and none removed**, so
//! the phase 4 `Schema` parser (`deployment/conformance.rs`) and the model
//! editor keep working on a version 2 descriptor: both read only the keys they
//! name.
//!
//! # Identity
//!
//! A descriptor's identity on the model plane is `{nsURI, digest}`, where the
//! digest is [`metamodel_digest`]: a function of the descriptor's content and
//! nothing else, so a reformatted file keeps its models and an edited one
//! does not.

use std::collections::HashSet;

use ecore_rs::{
    ctx::Ctx,
    repr::{Class, Pack, builtin, idx, structural},
};
use log::warn;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::codegen::{ecore, semantics};
use crate::error::{ArachneError, Result};

/// Version of the descriptor layout above. Bump on any breaking change so
/// clients can refuse what they do not understand.
const FORMAT_VERSION: u64 = 2;

/// Builds the JSON metamodel descriptor for `pack`.
///
/// Fails with [`ArachneError::RootClassNotFound`] when the package has no
/// root class — the same condition under which code generation fails.
pub fn descriptor_json(ctx: &Ctx, pack: &Pack) -> Result<Value> {
    // The class set is the one code generation reaches, computed the same way
    // `generate_from_parser` computes it so that the descriptor and the crate
    // describe one slice: the package's classes, then Ecore's own but
    // `EObject`, which the generator never emits.
    let package_classes: Vec<idx::Class> = pack.classes().iter().copied().collect();
    let ecore_classes: Vec<idx::Class> = match ctx.ecore_pack() {
        Some(ecore_pack) => ctx[ecore_pack]
            .classes()
            .iter()
            .copied()
            .filter(|class| !ecore::is_eobject(ctx, *class))
            .collect(),
        None => Vec::new(),
    };
    let generated: Vec<idx::Class> = package_classes
        .iter()
        .chain(ecore_classes.iter())
        .copied()
        .collect();
    let generated_set: HashSet<idx::Class> = generated.iter().copied().collect();

    let roots = crate::compute_top_level_roots(ctx, &package_classes, &generated, &generated_set);
    if roots.is_empty() {
        return Err(ArachneError::RootClassNotFound(pack.name().to_string()));
    }

    // `EObject` is *listed* although it is not generated. It has no feature,
    // so listing it adds no rule and no field; what it adds is that every
    // name the descriptor writes resolves. A metamodel may declare `EObject`
    // as a supertype (SysON's does, and so does the pet metamodel) and may
    // type a feature by it, and `moirai-semantics` refuses a supertype or a
    // target it cannot find. Dropping the supertype silently would also
    // contradict EMF, where an explicit `EObject` supertype is visible in the
    // reflective API even though it contributes nothing (spec 05 §2.4).
    let mut described_set = generated_set.clone();
    if let Some(eobject) = ecore::eobject(ctx) {
        described_set.insert(eobject);
    }

    let mut reachable: HashSet<idx::Class> = HashSet::new();
    for root in &roots {
        reachable.extend(crate::collect_reachable_classes(ctx, *root, &described_set));
    }
    // An abstract class of Ecore that nothing generated extends is left out,
    // exactly as `generate_from_parser` leaves it out.
    reachable.retain(|class| {
        !ctx.is_ecore_class(*class)
            || !crate::codegen::classifier::is_uninhabited_polymorphic_class(ctx, &ctx[*class])
    });

    let mut root_names: Vec<String> = roots
        .iter()
        .map(|idx| semantics::descriptor_class_name(ctx, *idx))
        .collect();
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
        let key = semantics::descriptor_class_name(ctx, class.idx);
        if classes
            .insert(key.clone(), class_descriptor(ctx, class, &reachable))
            .is_some()
        {
            // Two classifiers on one key would silently become one class with
            // one of the two feature sets. `ecore::` cannot be spelled in a
            // well-formed Ecore name, so this is unreachable for a metamodel
            // EMF would accept; it is checked because the descriptor is
            // emitted for metamodels EMF has never seen.
            return Err(ArachneError::DuplicateDescriptorClass(key));
        }
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

/// The digest half of a metamodel's identity: SHA-256, lowercase hex, over
/// the compact `serde_json` serialization of the parsed descriptor.
///
/// Taken over the parsed value and never over file bytes, so pretty and
/// compact renderings of one descriptor agree. `serde_json` runs without
/// `preserve_order` in this crate, so every object serializes with its keys
/// sorted and the order a file lists them in cannot reach the digest either;
/// a changed class, attribute, enum or `nsURI` can. The generated node binary
/// carries a copy of this rule and the editor computes it with a key-sorted
/// stringify, and `examples/fixtures/metamodel-digests.json` is where the
/// three are held to agree.
pub fn metamodel_digest(descriptor: &Value) -> String {
    format!("{:x}", Sha256::digest(descriptor.to_string()))
}

/// Describes one class: declared features partitioned into attributes,
/// containments and plain references, plus its super types.
fn class_descriptor(ctx: &Ctx, class: &Class, included: &HashSet<idx::Class>) -> Value {
    let mut super_types: Vec<String> = class
        .sup()
        .iter()
        .copied()
        .filter(|sup| included.contains(sup))
        .map(|sup| semantics::descriptor_class_name(ctx, sup))
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
                add_semantics(&mut attribute, ctx, class, feature);
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
                let target = semantics::descriptor_class_name(ctx, target);
                if feature.containment {
                    let mut containment = json!({
                        "name": feature.name,
                        "target": target,
                        "many": many,
                        "required": required,
                        "ordered": feature.ordered.unwrap_or(true),
                    });
                    add_semantics(&mut containment, ctx, class, feature);
                    containments.push(containment);
                } else {
                    let mut reference = json!({
                        "name": feature.name,
                        "target": target,
                        "many": many,
                        "required": required,
                    });
                    add_semantics(&mut reference, ctx, class, feature);
                    references.push(reference);
                }
            }
        }
    }

    let mut entry = json!({
        "abstract": class.is_abstract() || class.is_interface(),
        "superTypes": super_types,
        "attributes": attributes,
        "containments": containments,
        "references": references,
    });
    // A `urn:arachne:representation` `kind="transparent"` class is compiled
    // as its one named field and gets no record of its own
    // (`classifier/mod.rs:565-567`). The name is written here so a reader of
    // the descriptor renders the class the same way, and the key is absent
    // on every other class so nothing that read a version 2 descriptor before
    // 2026-09-08 sees a change.
    if let Some(field) = semantics::transparent_field(class) {
        entry["transparent"] = json!(field);
    }
    entry
}

/// Adds the four `formatVersion` 2 keys to one feature entry: the facets as
/// the `.ecore` file wrote them, the `urn:arachne:semantics` `datatype` string
/// it wrote or `null`, the merge rule the generator would compile, and where
/// each facet of that rule came from.
///
/// Nothing is removed and nothing existing moves, which is what keeps the
/// phase 4 `Schema` parser (`deployment/conformance.rs`) and the model editor
/// reading these descriptors unchanged: both read only the keys they name.
fn add_semantics(entry: &mut Value, ctx: &Ctx, class: &Class, feature: &structural::Structural) {
    let (merge, provenance) = semantics::merge_rule(feature, class, ctx);
    entry["facets"] = semantics::facets_json(feature);
    entry["annotation"] = semantics::datatype_annotation(feature)
        .map(|value| json!(value))
        .unwrap_or(Value::Null);
    entry["merge"] = semantics::merge_json(&merge, ctx);
    entry["provenance"] = semantics::provenance_json(&provenance);
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
    use std::path::{Path, PathBuf};

    use serde_json::{Map, Value, json};

    use super::metamodel_digest;
    use crate::EcoreParser;

    fn example(file: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples")
            .join(file)
    }

    /// The descriptor `arachne describe` renders for an example metamodel.
    fn descriptor_of(ecore: &str) -> Value {
        let parser = EcoreParser::from_file(example(ecore))
            .unwrap_or_else(|e| panic!("{ecore} should parse: {e}"));
        let pack = crate::find_user_package(&parser.ctx).expect("a user package");
        super::descriptor_json(&parser.ctx, pack).expect("descriptor should build")
    }

    fn bt_descriptor() -> Value {
        descriptor_of("behavior_tree.ecore")
    }

    fn read_json(path: &Path) -> Value {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{} should be readable: {e}", path.display()));
        serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{} should be JSON: {e}", path.display()))
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

    /// A class carries its own declarations only, and every one of them carries
    /// the four `formatVersion` 2 keys: the facets as written, the annotation,
    /// the merge rule and its provenance.
    ///
    /// `TreeNode.ID` is the shape of a required text slot, its `presence`
    /// `declared` because `lowerBound="1"` differs from Ecore's `0..1` default;
    /// `TreeNode.name` the same leaf under an `OptionLog`, and although it
    /// writes `lowerBound="0" upperBound="1"` it resolves to that default, so
    /// the conservative reading calls it `ecoreDefault` rather than claim a
    /// declaration the parsed feature no longer distinguishes.
    #[test]
    fn bt_tree_node_declares_only_its_own_features() {
        let descriptor = bt_descriptor();
        let tree_node = &descriptor["classes"]["TreeNode"];
        assert_eq!(
            tree_node["attributes"],
            json!([
                {"name": "ID", "kind": "string", "many": false,
                 "required": true, "isId": false,
                 "facets": {"ordered": null, "unique": null}, "annotation": null,
                 "merge": {"kind": "attribute", "shape": {"kind": "single"},
                           "leaf": {"kind": "text"}},
                 "provenance": {"ordered": "notApplicable", "unique": "notApplicable",
                                "leaf": "declared", "presence": "declared"}},
                {"name": "name", "kind": "string", "many": false,
                 "required": false, "isId": false,
                 "facets": {"ordered": null, "unique": null}, "annotation": null,
                 "merge": {"kind": "attribute", "shape": {"kind": "optional"},
                           "leaf": {"kind": "text"}},
                 "provenance": {"ordered": "notApplicable", "unique": "notApplicable",
                                "leaf": "declared", "presence": "ecoreDefault"}},
            ])
        );
    }

    /// The one entry the whole provenance argument rests on. `bt.ecore` writes
    /// no `ordered` anywhere, and a behaviour tree's `Sequence` runs its
    /// children left to right, so the sequence this compiles to is Arachne's
    /// decision and the descriptor says so: `houseDefault`, with `facets`
    /// showing the file was silent.
    #[test]
    fn bt_control_node_containment_is_many_ordered_and_subclass_typed() {
        let descriptor = bt_descriptor();
        let control_node = &descriptor["classes"]["ControlNode"];
        assert_eq!(
            control_node["containments"],
            json!([
                {"name": "children", "target": "TreeNode", "many": true,
                 "required": false, "ordered": true,
                 "facets": {"ordered": null, "unique": null}, "annotation": null,
                 "merge": {"kind": "containment", "shape": {"kind": "sequence"},
                           "target": "TreeNode"},
                 "provenance": {"ordered": "houseDefault", "unique": "notApplicable",
                                "leaf": "notApplicable", "presence": "declared"}},
            ])
        );
    }

    /// The digest is a function of the descriptor's content: rendering it
    /// pretty, compact or with its keys in another order gives one digest, a
    /// second generation from the same `.ecore` gives the same one, and one
    /// changed attribute gives another.
    #[test]
    fn mp5_the_descriptor_digest_is_stable_under_reformatting_and_changes_under_edits() {
        let descriptor = bt_descriptor();
        let digest = metamodel_digest(&descriptor);

        let pretty: Value = serde_json::from_str(&format!("{descriptor:#}")).unwrap();
        let compact: Value = serde_json::from_str(&descriptor.to_string()).unwrap();
        let reversed = Value::Object(descriptor.as_object().unwrap().iter().rev().fold(
            Map::new(),
            |mut map, (key, value)| {
                map.insert(key.clone(), value.clone());
                map
            },
        ));
        assert_eq!(
            [
                metamodel_digest(&pretty),
                metamodel_digest(&compact),
                metamodel_digest(&reversed),
                metamodel_digest(&bt_descriptor()),
            ],
            [
                digest.clone(),
                digest.clone(),
                digest.clone(),
                digest.clone()
            ],
            "a formatter or a second generation moved the digest"
        );

        let mut edited = descriptor.clone();
        let name = edited["classes"]["TreeNode"]["attributes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|attribute| attribute["name"] == "name")
            .expect("TreeNode declares `name`");
        name["required"] = json!(true);
        assert_ne!(
            metamodel_digest(&edited),
            digest,
            "making `TreeNode.name` required left the digest unchanged"
        );
    }

    /// The fixture the editor (mp6) and the generated crate (mp13) read: the
    /// checked-in descriptors and their digests, as this crate computes them.
    #[test]
    fn the_digest_fixture_names_the_checked_in_descriptors() {
        let fixture = read_json(&example("fixtures/metamodel-digests.json"));
        for (file, ecore) in [
            ("bt.metamodel.json", "behavior_tree.ecore"),
            ("uml.metamodel.json", "SimpleUML.ecore"),
            ("json.metamodel.json", "json.ecore"),
        ] {
            let generated = descriptor_of(ecore);
            let checked_in = read_json(&example(file));
            let digest = metamodel_digest(&generated);
            assert_eq!(
                metamodel_digest(&checked_in),
                digest,
                "examples/{file} differs from `arachne describe examples/{ecore}`"
            );
            assert_eq!(
                fixture[file],
                json!({ "nsURI": generated["nsURI"], "digest": digest }),
                "the fixture entry for {file} is stale; `arachne digest examples/{file}` prints the current one"
            );
        }
    }

    /// A non-containment reference stays a reference, and its rule says so
    /// with every facet `notApplicable`: no field is emitted for it
    /// (`classifier/mod.rs:104-117`), so there is nothing for a facet to
    /// describe.
    #[test]
    fn bt_non_containment_reference_stays_a_reference() {
        let descriptor = bt_descriptor();
        let port = &descriptor["classes"]["DataFlowPort"];
        assert_eq!(
            port["references"],
            json!([
                {"name": "entry", "target": "BlackboardEntry",
                 "many": false, "required": false,
                 "facets": {"ordered": null, "unique": null}, "annotation": null,
                 "merge": {"kind": "reference", "many": false,
                           "target": "BlackboardEntry"},
                 "provenance": {"ordered": "notApplicable", "unique": "notApplicable",
                                "leaf": "notApplicable", "presence": "notApplicable"}},
            ])
        );
    }

    /// The two forms D6 used to keep on the generated path, as `json.ecore`
    /// writes them since the decision was amended on 2026-09-08: a `uw-map`
    /// containment publishes the `keyed` shape and the class its values are,
    /// not the `Entry` its `.ecore` names; and a class the
    /// `urn:arachne:representation` annotation makes transparent publishes the
    /// field it is represented by under its own `transparent` key while its
    /// features publish ordinary rules.
    #[test]
    fn json_publishes_the_keyed_shape_and_the_transparent_field() {
        let descriptor = descriptor_of("json.ecore");
        assert_eq!(
            (
                &descriptor["classes"]["Object"]["containments"][0]["merge"],
                &descriptor["classes"]["Object"]["containments"][0]["annotation"],
                &descriptor["classes"]["Object"]["transparent"],
            ),
            (
                &json!({
                    "kind": "containment",
                    "shape": {"kind": "keyed", "key": {"kind": "str"}},
                    "target": "Json"
                }),
                &json!("uw-map"),
                &json!("entry"),
            )
        );
        assert_eq!(
            (
                &descriptor["classes"]["Array"]["containments"][0]["merge"],
                &descriptor["classes"]["Array"]["transparent"],
                &descriptor["classes"]["String"]["attributes"][0]["merge"],
                &descriptor["classes"]["String"]["transparent"],
            ),
            (
                &json!({
                    "kind": "containment",
                    "shape": {"kind": "sequence"},
                    "target": "Json"
                }),
                &json!("items"),
                &json!({"kind": "attribute", "shape": {"kind": "single"},
                        "leaf": {"kind": "text"}}),
                &json!("value"),
            )
        );
        // `Entry` is not transparent and keeps its own rules; nothing reaches
        // it any more, because the keyed rule above names `Json` directly.
        assert_eq!(descriptor["classes"]["Entry"].get("transparent"), None);
        assert_eq!(descriptor["classes"]["Json"].get("transparent"), None);
    }

    /// The format version is what the editor and the interpreted node gate on,
    /// so it is asserted rather than left to the fixture digest.
    #[test]
    fn the_descriptor_declares_format_version_two() {
        assert_eq!(bt_descriptor()["formatVersion"], json!(2));
    }

    /* ---------- Ecore's own classes ---------- */

    fn builtins_descriptor() -> Value {
        descriptor_of("pet_metamodels/ecore_builtins.ecore")
    }

    /// Ecore's classes are listed under a package-qualified key and a
    /// metamodel's own are not, which is what lets a metamodel declaring its
    /// own `EAnnotation` be described beside Ecore's. ModelSet holds 36 that
    /// do.
    ///
    /// `EObject` is listed although it is never generated: it has no feature,
    /// so it adds no rule, and listing it is what makes every supertype and
    /// every target the descriptor writes resolve to a class the descriptor
    /// lists — which `moirai-semantics` requires and which dropping the
    /// supertype silently would buy at the price of contradicting EMF, where
    /// an explicit `EObject` supertype is visible in the reflective API.
    #[test]
    fn ecore_classes_are_listed_under_a_package_qualified_key() {
        let descriptor = builtins_descriptor();
        let mut names: Vec<&str> = descriptor["classes"]
            .as_object()
            .expect("classes")
            .keys()
            .map(String::as_str)
            .collect();
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
        assert_eq!(
            descriptor["classes"]["Model"]["superTypes"],
            json!(["ecore::EObject"])
        );
        assert_eq!(
            descriptor["classes"]["Port"]["superTypes"],
            json!(["ecore::ENamedElement"])
        );
        assert_eq!(
            descriptor["classes"]["ecore::EObject"],
            json!({
                "abstract": false, "superTypes": [],
                "attributes": [], "containments": [], "references": []
            }),
            "`EObject` is concrete and has no feature, as `Ecore.ecore` declares it"
        );
        // A `::` is not a character an Ecore name can hold, so the key cannot
        // be one a metamodel's own class claims.
        assert!(names.iter().filter(|name| name.contains("::")).count() == 5);
    }

    /// The four features of `EAnnotation` that carry a construction and the
    /// three that do not, as data: the ordered containment of annotations,
    /// the optional `source`, the `details` map to a register over an
    /// optional text, and the two `EObject`-typed features and the transient
    /// back-pointer recorded with their reason rather than dropped.
    #[test]
    fn the_annotation_features_carry_their_rules_and_their_refusals() {
        let descriptor = builtins_descriptor();
        let annotation = &descriptor["classes"]["ecore::EAnnotation"];
        let rule = |array: &str, name: &str| -> Value {
            annotation[array]
                .as_array()
                .unwrap_or_else(|| panic!("`{array}`"))
                .iter()
                .find(|entry| entry["name"] == json!(name))
                .unwrap_or_else(|| panic!("`EAnnotation.{name}`"))["merge"]
                .clone()
        };

        assert_eq!(
            descriptor["classes"]["ecore::EModelElement"]["containments"][0],
            json!({
                "name": "eAnnotations", "target": "ecore::EAnnotation",
                "many": true, "required": false, "ordered": true,
                "facets": {"ordered": null, "unique": null}, "annotation": null,
                "merge": {"kind": "containment", "shape": {"kind": "sequence"},
                          "target": "ecore::EAnnotation"},
                "provenance": {"ordered": "houseDefault", "unique": "notApplicable",
                               "leaf": "notApplicable", "presence": "declared"}
            })
        );
        assert_eq!(
            rule("attributes", "source"),
            json!({"kind": "attribute", "shape": {"kind": "optional"},
                   "leaf": {"kind": "text"}}),
            "a silent `lowerBound` is Ecore's 0, so `source` is optional text"
        );
        assert_eq!(
            rule("containments", "details"),
            json!({"kind": "attribute",
                   "shape": {"kind": "keyed", "key": {"kind": "str"}},
                   "leaf": {"kind": "optionalRegister"}}),
            "EMF's map entry convention makes `details` a map, and its optional \
             value a register so that a key put with no value is read back"
        );
        assert_eq!(
            rule("containments", "contents"),
            json!({"kind": "unsupported", "reason": "eObjectContainment"})
        );
        assert_eq!(
            rule("references", "references"),
            json!({"kind": "unsupported", "reason": "eObjectReference"})
        );
        assert_eq!(
            rule("references", "eModelElement"),
            json!({"kind": "unsupported", "reason": "transient"}),
            "the back-pointer of a containment, which `analysis.rs:74` drops"
        );
        // The map is Ecore's own convention and not a `urn:arachne:semantics`
        // annotation, so the two collection facets are sourced to Ecore.
        let details = descriptor["classes"]["ecore::EAnnotation"]["containments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == json!("details"))
            .unwrap();
        assert_eq!(
            details["provenance"],
            json!({"ordered": "ecoreDefault", "unique": "ecoreDefault",
                   "leaf": "houseDefault", "presence": "declared"})
        );
        assert_eq!(details["annotation"], Value::Null);
    }

    /// The descriptor `arachne describe` emits is one `moirai-semantics`
    /// parses into a table, which is the whole point of emitting it: the
    /// supertypes resolve, the targets resolve, the inherited `eAnnotations`
    /// is visible on every annotated class, and the three refusals cost the
    /// features they name and nothing else.
    #[test]
    fn the_builtins_descriptor_parses_into_a_table() {
        let descriptor = builtins_descriptor();
        let table = moirai_semantics::from_descriptor(&descriptor)
            .expect("the built-ins descriptor is a table");
        let class = |name: &str| {
            table
                .classes
                .iter()
                .find(|class| &*class.name == name)
                .unwrap_or_else(|| panic!("no class `{name}`"))
        };
        assert_eq!(table.classes.len(), 9);
        for name in ["Part", "Port", "ecore::EAnnotation"] {
            assert!(
                class(name)
                    .visible
                    .iter()
                    .any(|(feature, _, _)| &**feature == "eAnnotations"),
                "`{name}` should see the inherited `eAnnotations`"
            );
        }
        assert_eq!(
            table.roots,
            vec![class("Model").slot],
            "`EObject` is listed and is still not a root"
        );
        let unsupported = table
            .classes
            .iter()
            .flat_map(|class| class.declared.iter())
            .filter(|feature| {
                matches!(
                    feature.merge,
                    moirai_semantics::MergeRule::Unsupported { .. }
                )
            })
            .count();
        assert_eq!(
            unsupported, 4,
            "two `EObject` references, one containment, one transient"
        );
    }
}
