//! SI.05 (`cea-cdrt-knowledge/spec/items/SI.05.md`), claim R6 and contract K1 at the emitter:
//! `arachne describe` names the new reference rule on every reference in scope, keeps today's
//! rule on a reference that declares an `eOpposite` and on an `EObject`-typed one, moves nothing
//! else, and so gives a metamodel with an in-scope reference a new digest. The checked-in
//! descriptors keep their bytes (scoping decision S1: no migration).
//!
//! Ground truth, none of it computed by the emitter under test:
//! - the descriptors `arachne describe` wrote at d81cae8, checked in (`examples/*.metamodel.json`)
//!   or copied under `tests/fixtures/si05/` and `generated/simpleuml_crdt/tests/fixtures/si05/`,
//!   moved by the rule of section 3.1 (`si05_support::identity_rule`), which reads one key of
//!   the entry it moves (`eOpposite`) and no class or feature name;
//! - the digests below, computed from those moved descriptors with Python's `json` and `hashlib`
//!   (compact, keys sorted, UTF-8), as the spec records them.
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-codegen --test si05_identity_reference_emitter -- --nocapture
//! ```

#[path = "si05_support/mod.rs"]
mod si05_support;

use std::{
    fs,
    path::{Path, PathBuf},
};

use arachne_codegen::{EcoreParser, descriptor_json, find_user_package, metamodel_digest};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use si05_support::{render, with_identity_rule};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn describe(ecore: &Path) -> Value {
    let parser = EcoreParser::from_file(ecore)
        .unwrap_or_else(|e| panic!("set-up: {} should parse: {e}", ecore.display()));
    let pack = find_user_package(&parser.ctx)
        .unwrap_or_else(|e| panic!("set-up: {} has no user package: {e}", ecore.display()));
    descriptor_json(&parser.ctx, pack)
        .unwrap_or_else(|e| panic!("set-up: describing {}: {e}", ecore.display()))
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("set-up: reading {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("set-up: {} is not JSON: {e}", path.display()))
}

fn sha256_of(path: &Path) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            fs::read(path).unwrap_or_else(|e| panic!("set-up: reading {}: {e}", path.display()))
        )
    )
}

/// Every `.ecore` of `examples/` that `arachne describe` reads (`UML.ecore` it does not, today
/// or after), and the board fixture.
const ECORES: [&str; 16] = [
    "examples/behavior_tree.ecore",
    "examples/class_diagram.ecore",
    "examples/class_hierarchy.ecore",
    "examples/conference.ecore",
    "examples/json.ecore",
    "examples/notation.ecore",
    "examples/SimpleUML.ecore",
    "examples/umlclass.ecore",
    "examples/pet_metamodels/abstract_inherits_concrete.ecore",
    "examples/pet_metamodels/concrete_inherits_concrete.ecore",
    "examples/pet_metamodels/concrete_polymorphic_targets.ecore",
    "examples/pet_metamodels/ecore_builtins.ecore",
    "examples/pet_metamodels/kitchen_sink.ecore",
    "examples/pet_metamodels/multiple_inheritance.ecore",
    "examples/sysml/sysml.ecore",
    "arachne-codegen/tests/fixtures/si05/si05board.ecore",
];

/// How many references of each `.ecore` are in scope, counted at d81cae8 from the descriptor
/// (`merge.kind == "reference"` and no `eOpposite`): 1, 2, 3, 2, 0, 2, 4, 8, 0, 0, 1, 1, 2, 0,
/// 34, 2; 62 in all.
const IN_SCOPE: [usize; 16] = [1, 2, 3, 2, 0, 2, 4, 8, 0, 0, 1, 1, 2, 0, 34, 2];

/// K1 at the emitter, entry by entry: an in-scope reference is described on the identity rule
/// with the `many` and `target` it had, an `eOpposite` one on today's rule, an unsupported one
/// as it was; and nothing else of any descriptor moves.
#[test]
fn si05_e1_every_in_scope_reference_is_described_on_the_identity_rule() {
    let mut wrong = Vec::new();
    let mut total = 0;
    for (ecore, want) in ECORES.iter().zip(IN_SCOPE) {
        let described = describe(&repo().join(ecore));
        let mut identity = 0;
        for (class, body) in described["classes"].as_object().expect("classes") {
            for entry in body["references"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[])
            {
                let name = format!("{ecore}: {class}.{}", entry["name"]);
                let merge = &entry["merge"];
                match merge["kind"].as_str() {
                    Some("identityReference") => {
                        identity += 1;
                        if entry.get("eOpposite").is_some() {
                            wrong.push(format!(
                                "{name} declares an eOpposite and is on the identity rule"
                            ));
                        }
                        let expect = json!({"kind": "identityReference", "many": entry["many"], "target": entry["target"]});
                        if merge != &expect {
                            wrong.push(format!("{name}: merge {merge}, expected {expect}"));
                        }
                    }
                    Some("reference") => {
                        if entry.get("eOpposite").is_none() {
                            wrong.push(format!(
                                "{name} is in scope and still on today's rule {merge}"
                            ));
                        }
                    }
                    Some("unsupported") => {}
                    other => wrong.push(format!("{name}: a merge of kind {other:?}")),
                }
            }
        }
        println!("SI05-E1 {ecore}: {identity} references on the identity rule (expected {want})");
        if identity != want {
            wrong.push(format!(
                "{ecore}: {identity} references on the identity rule, expected {want}"
            ));
        }
        total += identity;
    }
    println!(
        "SI05-E1 {} metamodels, {total} references on the identity rule",
        ECORES.len()
    );
    assert!(
        wrong.is_empty(),
        "K1 at the emitter:\n  {}",
        wrong.join("\n  ")
    );
    assert_eq!(total, 62);
}

/// The checked-in descriptors, the `.ecore` each is described from, its sha256 at d81cae8, and
/// the digest the new rule gives it (unchanged for `json`, which has no reference).
const CHECKED_IN: [(&str, &str, &str, &str); 6] = [
    (
        "examples/bt.metamodel.json",
        "examples/behavior_tree.ecore",
        "ff4525108d4ecb65d297002c7bc02f22978966d347af3678712de07a5c6e1263",
        "f19f730e4aaa7320d23e6b211a7856e7aa9042b56cef67cb3c0cb36091ad6822",
    ),
    (
        "examples/uml.metamodel.json",
        "examples/SimpleUML.ecore",
        "b000aa0d2b562a41763d3e77829fd9363224fdcade914d42846b9cb0c54690a5",
        "5d34af887ace294f008c96403a4e6dd442f374d4e23639ec1620c04a4cf301f3",
    ),
    (
        "examples/notation.metamodel.json",
        "examples/notation.ecore",
        "04ebb5c15dad375284d4e5e681a5e634130525c614517d373763e6040eead03f",
        "14377158866c6dd249915cdadbc5579efb64a3e742b5bd7aff1ced4396f57b86",
    ),
    (
        "examples/umlclass.metamodel.json",
        "examples/umlclass.ecore",
        "a770d938ae503aff62f395b6f3cf241a4776459da5b2a04d5ba5e3cddc892edd",
        "999ed54f0657c2c8e27162645b0676e94be56a16d9058429707a104d1e620dc9",
    ),
    (
        "examples/json.metamodel.json",
        "examples/json.ecore",
        "efdbde163d711b39e102ae7dc98585c3cb981ad9be82914ba4f4270e10113ec1",
        "a9d83505175d8d6708b86c7eb64d356d64197f066f752c1a9ea842c76a45bc30",
    ),
    (
        "examples/sysml/sysml.metamodel.json",
        "examples/sysml/sysml.ecore",
        "896ba4d758c6d8adfe9764bb7779f3400212c02afd97959012936d4e113d4e47",
        "a75d8af47bf99f51b908f909a468c75c629a505de80b5379f19085943d6ae066",
    ),
];

/// R6: each checked-in descriptor is what `arachne describe` writes once moved by the rule and
/// by nothing else, byte for byte, so a metamodel with an in-scope reference has a new digest,
/// the one registered; and the checked-in file itself keeps its bytes (S1, no migration).
#[test]
fn si05_e2_the_emitter_moves_each_checked_in_descriptor_by_the_rule_alone() {
    let mut wrong = Vec::new();
    for (file, ecore, file_sha, new_digest) in CHECKED_IN {
        let path = repo().join(file);
        let checked_in = read_json(&path);
        let described = describe(&repo().join(ecore));
        let expected = with_identity_rule(&checked_in);
        if render(&described) != render(&expected) {
            wrong.push(format!(
                "{file}: describe({ecore}) is not the checked-in descriptor moved by the rule"
            ));
        }
        let digest = metamodel_digest(&described);
        println!(
            "SI05-E2 {file}: today {}, described {digest}, registered {new_digest}",
            metamodel_digest(&checked_in)
        );
        if digest != new_digest {
            wrong.push(format!(
                "{file}: describe gives digest {digest}, registered {new_digest}"
            ));
        }
        if sha256_of(&path) != file_sha {
            wrong.push(format!(
                "{file} moved: sha256 {}, registered {file_sha} (S1: no migration)",
                sha256_of(&path)
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "R6 at the emitter:\n  {}",
        wrong.join("\n  ")
    );
}

/// The descriptors the SI.05 oracles run are the emitter's: the board fixture (here and, byte
/// for byte, in Moirai), and the two the twin suite gives the interpreted path.
#[test]
fn si05_e3_the_oracles_descriptors_are_what_describe_writes() {
    let pairs = [
        (
            "arachne-codegen/tests/fixtures/si05/si05board.ecore",
            "arachne-codegen/tests/fixtures/si05/si05board.metamodel.json",
        ),
        (
            "examples/SimpleUML.ecore",
            "generated/simpleuml_crdt/tests/fixtures/si05/simpleuml.identity.metamodel.json",
        ),
        (
            "examples/class_hierarchy.ecore",
            "generated/simpleuml_crdt/tests/fixtures/si05/class_hierarchy.identity.metamodel.json",
        ),
    ];
    let mut wrong = Vec::new();
    for (ecore, fixture) in pairs {
        let described = render(&describe(&repo().join(ecore)));
        let checked = fs::read_to_string(repo().join(fixture)).expect("the fixture is there");
        if described != checked {
            wrong.push(format!("describe({ecore}) is not {fixture}"));
        }
    }
    // The fixtures are themselves today's descriptors moved by the rule: the premise of the
    // twin suite and of the board's Moirai tests, checked before any emitter change.
    let today = read_json(
        &repo().join("arachne-codegen/tests/fixtures/si05/si05board.today.metamodel.json"),
    );
    let moved = fs::read_to_string(
        repo().join("arachne-codegen/tests/fixtures/si05/si05board.metamodel.json"),
    )
    .expect("fixture");
    assert_eq!(
        render(&with_identity_rule(&today)),
        moved,
        "set-up: the board fixture is today's board moved by the rule"
    );
    assert!(
        wrong.is_empty(),
        "the oracles' descriptors:\n  {}",
        wrong.join("\n  ")
    );
}

/// The premise of E2 and E3, which holds before SI.05 and must hold after: the fixtures of
/// today's descriptors are what `arachne describe` wrote at d81cae8, moved by nothing.
#[test]
fn si05_e0_premise_the_today_fixtures_are_the_checked_in_bytes() {
    for (file, _, file_sha, _) in CHECKED_IN {
        assert_eq!(
            sha256_of(&repo().join(file)),
            file_sha,
            "{file} is not the registered file"
        );
    }
    assert_eq!(
        sha256_of(
            &repo().join("arachne-codegen/tests/fixtures/si05/si05board.today.metamodel.json")
        ),
        "fc23f6a843538307aa09275202c947e816c93e05e9f4857691bca2229138d95c"
    );
    assert_eq!(
        sha256_of(&repo().join(
            "generated/simpleuml_crdt/tests/fixtures/si05/class_hierarchy.today.metamodel.json"
        )),
        "5992e8ac6663e42a1a0810184f1b5d7940dc32c8e58f989efe3e58ed71634431"
    );
}
