//! SI.05 (`cea-cdrt-knowledge/spec/items/SI.05.md`, section 6.3): the replacements of the three
//! inline tests the identity rule contradicts, under VALIDATION.md section 2's registered
//! exception. The hunk the executor applies is `cea-cdrt-knowledge/spec/items/SI.05-inline-hunk-
//! arachne.diff`; each test below says which old assertion it replaces and what the old one
//! guarded.
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-codegen --test si05_inline_replacements
//! ```

#[path = "si05_support/mod.rs"]
mod si05_support;

use std::{
    fs,
    path::{Path, PathBuf},
};

use arachne_codegen::{EcoreParser, descriptor_json, find_user_package, metamodel_digest};
use serde_json::{Value, json};

use si05_support::with_identity_rule;

fn example(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples")
        .join(file)
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display())),
    )
    .unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

fn describe(ecore: &str) -> Value {
    let parser = EcoreParser::from_file(example(ecore))
        .unwrap_or_else(|e| panic!("{ecore} should parse: {e}"));
    let pack = find_user_package(&parser.ctx).expect("a user package");
    descriptor_json(&parser.ctx, pack).expect("a descriptor")
}

/// Replaces `descriptor.rs`'s `the_digest_fixture_names_the_checked_in_descriptors`, whose first
/// assertion held `digest(examples/<file>) == digest(arachne describe <ecore>)`. That guarded two
/// things: the digest fixture the editor (mp6) and the generated crate (mp13) read names the
/// checked-in descriptors, and those descriptors are what the emitter writes. The first still
/// holds and is asserted here as it was. The second is now "what the emitter writes is the
/// checked-in descriptor moved by the identity rule" (scoping decision S1: the checked-in files
/// are the descriptors of the models created under them and are not regenerated).
#[test]
fn si05_r1_the_digest_fixture_names_the_checked_in_descriptors_and_the_emitter_moves_them_by_the_rule()
 {
    let fixture = read_json(&example("fixtures/metamodel-digests.json"));
    for (file, ecore) in [
        ("bt.metamodel.json", "behavior_tree.ecore"),
        ("uml.metamodel.json", "SimpleUML.ecore"),
        ("json.metamodel.json", "json.ecore"),
    ] {
        let checked_in = read_json(&example(file));
        assert_eq!(
            fixture[file],
            json!({ "nsURI": checked_in["nsURI"], "digest": metamodel_digest(&checked_in) }),
            "the fixture entry for {file} does not name the checked-in descriptor"
        );
        assert_eq!(
            describe(ecore),
            with_identity_rule(&checked_in),
            "`arachne describe examples/{ecore}` is not examples/{file} moved by the identity rule"
        );
    }
}

/// Replaces `descriptor.rs`'s `bt_non_containment_reference_stays_a_reference`, which held
/// `DataFlowPort.entry` as `{"kind": "reference", "many": false, "target": "BlackboardEntry"}`
/// with every facet `notApplicable`. It guarded that a non-containment reference is described as
/// a reference and that its provenance says no facet applies (no field is emitted for it). Both
/// still hold; the rule's kind is SI.05's.
#[test]
fn si05_r2_the_bt_non_containment_reference_is_described_on_the_identity_rule() {
    let descriptor = describe("behavior_tree.ecore");
    assert_eq!(
        descriptor["classes"]["DataFlowPort"]["references"],
        json!([
            {"name": "entry", "target": "BlackboardEntry",
             "many": false, "required": false,
             "facets": {"ordered": null, "unique": null}, "annotation": null,
             "merge": {"kind": "identityReference", "many": false,
                       "target": "BlackboardEntry"},
             "provenance": {"ordered": "notApplicable", "unique": "notApplicable",
                            "leaf": "notApplicable", "presence": "notApplicable"}},
        ])
    );
}

/// Replaces the fifth element of `semantics.rs`'s `ip4_from_descriptor_reproduces_spec_11_section_7`
/// rule array, which held the table's rule for `DataFlowPort.entry` to be
/// `MergeRule::Reference { many: false, target: BlackboardEntry }`. It guarded that the table a
/// node parses from the emitted descriptor carries the reference's multiplicity and target slot.
/// Both still hold, under the identity rule's tag, read through the table's own serialization
/// (the table travels in the log, decision D2).
#[test]
fn si05_r3_the_table_carries_the_bt_reference_on_the_identity_rule() {
    let table = moirai_semantics::from_descriptor(&describe("behavior_tree.ecore"))
        .unwrap_or_else(|e| panic!("the bt descriptor is not a table: {e}"));
    let slot = |name: &str| {
        table
            .classes
            .iter()
            .find(|c| &*c.name == name)
            .unwrap_or_else(|| panic!("{name}"))
            .slot
    };
    let port = &table.classes[slot("DataFlowPort").index()];
    let entry = port
        .declared
        .iter()
        .find(|f| &*f.name == "entry")
        .expect("DataFlowPort declares entry");
    assert_eq!(
        serde_json::to_value(entry.merge).expect("a rule serializes"),
        json!({"kind": "identityReference", "many": false, "target": slot("BlackboardEntry").0})
    );
}
