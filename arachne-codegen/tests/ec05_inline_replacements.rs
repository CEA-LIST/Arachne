//! EC.05 (`cea-cdrt-knowledge/spec/items/EC.05.md`), registered exception of VALIDATION.md §2:
//! four inline tests of `src/codegen/descriptor.rs` held whole descriptor entries, which EC.05's
//! added keys (Cam, 2026-11-20: every shipped digest moves once) contradict. Each is replaced here
//! word for word, its entries carrying EC.05's keys as `tests/ec05/model.py` reads them from the
//! `.ecore` (bounds, `default`, `changeable`, `unsettable`, `eKeys`; `interface`,
//! `superTypeOrder`); the spec registers the inline hunk that deletes the originals.
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-codegen --test ec05_inline_replacements
//! ```

use std::path::{Path, PathBuf};

use arachne_codegen::{EcoreParser, descriptor_json, find_user_package};
use serde_json::{Value, json};

fn example(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples")
        .join(file)
}

/// The descriptor `arachne describe` renders for an example metamodel.
fn descriptor_of(ecore: &str) -> Value {
    let parser = EcoreParser::from_file(example(ecore))
        .unwrap_or_else(|e| panic!("{ecore} should parse: {e}"));
    let pack = find_user_package(&parser.ctx).expect("a user package");
    descriptor_json(&parser.ctx, pack).expect("descriptor should build")
}

fn bt_descriptor() -> Value {
    descriptor_of("behavior_tree.ecore")
}

fn builtins_descriptor() -> Value {
    descriptor_of("pet_metamodels/ecore_builtins.ecore")
}

/// Replaces `descriptor.rs`'s `bt_tree_node_declares_only_its_own_features`: a class carries its
/// own declarations only, each with the four `formatVersion` 2 keys and, from EC.05, its bounds
/// and flags as `behavior_tree.ecore` writes them (`ID` `lowerBound="1"`, `name` `0..1`).
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
                            "leaf": "declared", "presence": "declared"},
             "lowerBound": 1, "upperBound": 1, "default": null,
             "changeable": true, "unsettable": false, "eKeys": []},
            {"name": "name", "kind": "string", "many": false,
             "required": false, "isId": false,
             "facets": {"ordered": null, "unique": null}, "annotation": null,
             "merge": {"kind": "attribute", "shape": {"kind": "optional"},
                       "leaf": {"kind": "text"}},
             "provenance": {"ordered": "notApplicable", "unique": "notApplicable",
                            "leaf": "declared", "presence": "ecoreDefault"},
             "lowerBound": 0, "upperBound": 1, "default": null,
             "changeable": true, "unsettable": false, "eKeys": []},
        ])
    );
}

/// Replaces `descriptor.rs`'s `bt_control_node_containment_is_many_ordered_and_subclass_typed`.
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
                            "leaf": "notApplicable", "presence": "declared"},
             "lowerBound": 0, "upperBound": -1, "default": null,
             "changeable": true, "unsettable": false, "eKeys": []},
        ])
    );
}

/// Replaces `descriptor.rs`'s `ecore_classes_are_listed_under_a_package_qualified_key`; `EObject`
/// is no interface and declares no supertype.
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
            "attributes": [], "containments": [], "references": [],
            "interface": false, "superTypeOrder": []
        }),
        "`EObject` is concrete and has no feature, as `Ecore.ecore` declares it"
    );
    assert!(names.iter().filter(|name| name.contains("::")).count() == 5);
}

/// Replaces `descriptor.rs`'s `the_annotation_features_carry_their_rules_and_their_refusals`; the
/// containment of annotations is Léo's built-in `0..-1` (`arachne-parser/src/repr/ecore.rs`).
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
                           "leaf": "notApplicable", "presence": "declared"},
            "lowerBound": 0, "upperBound": -1, "default": null,
            "changeable": true, "unsettable": false, "eKeys": []
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
