//! AR.02 (`cea-cdrt-knowledge/spec/items/AR.02.md`): the descriptor `arachne describe` writes
//! carries, on every reference entry (under `containments` or `references`), the declarations
//! of the `.ecore` that say which end is whose: `eOpposite`, the name of the opposite reference
//! on the target class, and `subsets` and `redefines`, one `{"class", "feature"}` object per
//! entry of the `references` of an `EAnnotation` whose `source` is that word, in document order,
//! which is how SysON's `sysml.ecore` declares them. A key is written only when the `.ecore`
//! declares at least one such thing, so a descriptor whose `.ecore` declares none is the
//! descriptor of today, byte for byte, and keeps its digest.
//!
//! Ground truth, none of it computed by Arachne:
//! - D1: the entries of FeatureTyping, Subclassification, Subsetting, Specialization,
//!   Relationship and Element, read by hand from `sysml.ecore` lines 601-611, 873-888,
//!   1399-1419, 1574-1588, 1661-1698 and written below as literals.
//! - D2: `fixtures/ar02/sysml.ar02.metamodel.json`, the descriptor of today (`demo/sysml.json`,
//!   digest `5168d5f6...`, what `arachne describe` wrote at `5e78ba7`) with the declarations
//!   added by `fixtures/ar02/declarations.py`, which reads the `.ecore` with Python's XML parser.
//! - D3: SimpleUML's, AbstractEcore's and ExampleEcore's opposites, read by hand, and their
//!   digests before and after, computed the same way.
//! - D4: the digests `arachne describe` wrote at `5e78ba7` for every `.ecore` of this repository
//!   that declares none of the three, and the bytes of the checked-in descriptors.
//! - D5: rule R-e of AG.02 (`cea-cdrt-knowledge/spec/items/AG.02.md`, section 3.4), run here over
//!   the descriptor instead of the `.ecore`, must give AG.02's table on SysML and nothing on
//!   SimpleUML. It reads no class or feature name.
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-codegen --test ar02_descriptor_declarations -- --nocapture
//! ```

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

use arachne_codegen::{EcoreParser, descriptor_json, find_user_package, metamodel_digest};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// The three keys this item adds, and nothing else.
const KEYS: [&str; 3] = ["eOpposite", "subsets", "redefines"];

/// `repos/arachne/sysml.ecore`, SysON's metamodel, copied byte for byte.
const SYSML_ECORE_SHA256: &str = "82463c8f21d1a688603866ec02c6d4880b4e9224e34e9833a464aed0bd8f5440";
/// The SysML descriptor of today: `demo/sysml.json`, what alice serves, what `arachne describe`
/// writes at `5e78ba7`.
const SYSML_DIGEST_TODAY: &str = "5168d5f66ef4811bb7e4e2258628dd2051d5fa451c8093479aa3b1304193692e";
/// The same with the declarations: `fixtures/ar02/sysml.ar02.metamodel.json`.
const SYSML_DIGEST_AR02: &str = "ebd580696f97d9339c148f5b29db5d0ff7b0928d78fb05efc7c206811405e627";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn fixture(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ar02")
        .join(file)
}

/// What `arachne describe <ecore>` writes, as a value.
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
    let bytes =
        fs::read(path).unwrap_or_else(|e| panic!("set-up: reading {}: {e}", path.display()));
    format!("{:x}", Sha256::digest(bytes))
}

/// Every feature entry of a descriptor, as `(class, array, entry)`.
fn entries(descriptor: &Value) -> Vec<(String, &'static str, &Value)> {
    let mut out = Vec::new();
    for (class, body) in descriptor["classes"].as_object().expect("classes") {
        for array in ["attributes", "containments", "references"] {
            for entry in body[array].as_array().map(Vec::as_slice).unwrap_or(&[]) {
                out.push((class.clone(), array, entry));
            }
        }
    }
    out
}

/// The descriptor with the three keys taken off every entry: what it would be if the `.ecore`
/// declared none of them.
fn strip(descriptor: &Value) -> Value {
    let mut out = descriptor.clone();
    for (_, body) in out["classes"].as_object_mut().expect("classes").iter_mut() {
        for array in ["attributes", "containments", "references"] {
            if let Some(list) = body[array].as_array_mut() {
                for entry in list {
                    let entry = entry.as_object_mut().expect("an entry is an object");
                    for key in KEYS {
                        entry.remove(key);
                    }
                }
            }
        }
    }
    out
}

/// The three keys of one entry, as an object holding only those present.
fn declared(descriptor: &Value, class: &str, feature: &str) -> Value {
    let entry = ["containments", "references", "attributes"]
        .iter()
        .filter_map(|array| descriptor["classes"][class][*array].as_array())
        .flatten()
        .find(|entry| entry["name"] == json!(feature))
        .unwrap_or_else(|| panic!("set-up: the descriptor has no entry `{class}.{feature}`"));
    let mut out = Map::new();
    for key in KEYS {
        if let Some(value) = entry.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(out)
}

fn r(class: &str, feature: &str) -> Value {
    json!({"class": class, "feature": feature})
}

/// `(class, feature, the three keys as declared)`, read by hand from the `.ecore`.
type HandRead = (&'static str, &'static str, Value);

fn check_hand_read(descriptor: &Value, what: &str, rows: &[HandRead]) -> Vec<String> {
    let mut wrong = Vec::new();
    for (class, feature, want) in rows {
        let got = declared(descriptor, class, feature);
        let line = format!("AR02-{what} {class}.{feature}: {got}");
        println!("{line}");
        if &got != want {
            wrong.push(format!(
                "{class}.{feature}: described {got}, the .ecore declares {want}"
            ));
        }
    }
    wrong
}

/// D1. SysML's relationship entries, hand-read from the `.ecore`. Today every one of them is
/// `{}`, and `subclassifier` and `superclassifier`, `subsettingFeature` and `subsettedFeature`,
/// are the same entry but for their names.
#[test]
fn ar02_d1_sysml_describes_the_declarations_hand_read_from_the_ecore() {
    let ecore = fixture("sysml.ecore");
    assert_eq!(
        sha256_of(&ecore),
        SYSML_ECORE_SHA256,
        "set-up: fixtures/ar02/sysml.ecore is not SysON's"
    );
    let descriptor = describe(&ecore);
    let rows: Vec<HandRead> = vec![
        // sysml.ecore:607-608
        (
            "Element",
            "ownedRelationship",
            json!({"eOpposite": "owningRelatedElement"}),
        ),
        // :1403-1410
        (
            "Relationship",
            "ownedRelatedElement",
            json!({"eOpposite": "owningRelationship",
                   "subsets": [r("Relationship", "relatedElement")]}),
        ),
        (
            "Relationship",
            "owningRelatedElement",
            json!({"eOpposite": "ownedRelationship",
                   "subsets": [r("Relationship", "relatedElement")]}),
        ),
        // :1574-1588
        (
            "Specialization",
            "general",
            json!({"redefines": [r("Relationship", "target")]}),
        ),
        (
            "Specialization",
            "owningType",
            json!({"eOpposite": "ownedSpecialization",
                   "subsets": [r("Relationship", "owningRelatedElement"),
                               r("Specialization", "specific")]}),
        ),
        (
            "Specialization",
            "specific",
            json!({"redefines": [r("Relationship", "source")]}),
        ),
        // :873-888
        (
            "FeatureTyping",
            "owningFeature",
            json!({"eOpposite": "ownedTyping",
                   "redefines": [r("Specialization", "owningType")],
                   "subsets": [r("FeatureTyping", "typedFeature")]}),
        ),
        (
            "FeatureTyping",
            "type",
            json!({"redefines": [r("Specialization", "general")]}),
        ),
        (
            "FeatureTyping",
            "typedFeature",
            json!({"redefines": [r("Specialization", "specific")]}),
        ),
        // :1661-1675
        (
            "Subclassification",
            "owningClassifier",
            json!({"eOpposite": "ownedSubclassification",
                   "redefines": [r("Specialization", "owningType")]}),
        ),
        (
            "Subclassification",
            "subclassifier",
            json!({"redefines": [r("Specialization", "specific")]}),
        ),
        (
            "Subclassification",
            "superclassifier",
            json!({"redefines": [r("Specialization", "general")]}),
        ),
        // :1683-1698
        (
            "Subsetting",
            "owningFeature",
            json!({"eOpposite": "ownedSubsetting",
                   "redefines": [r("Specialization", "owningType")],
                   "subsets": [r("Subsetting", "subsettingFeature")]}),
        ),
        (
            "Subsetting",
            "subsettedFeature",
            json!({"redefines": [r("Specialization", "general")]}),
        ),
        (
            "Subsetting",
            "subsettingFeature",
            json!({"redefines": [r("Specialization", "specific")]}),
        ),
    ];
    let wrong = check_hand_read(&descriptor, "D1", &rows);

    // The symmetry AG.02 section 3.2 found is gone: the two ends differ by more than a name.
    let without_name = |class: &str, feature: &str| -> Value {
        let mut entry = ["containments", "references"]
            .iter()
            .filter_map(|array| descriptor["classes"][class][*array].as_array())
            .flatten()
            .find(|entry| entry["name"] == json!(feature))
            .cloned()
            .unwrap_or_else(|| panic!("set-up: no `{class}.{feature}`"));
        entry.as_object_mut().unwrap().remove("name");
        entry
    };
    let mut symmetric = Vec::new();
    for (class, left, right) in [
        ("Subclassification", "subclassifier", "superclassifier"),
        ("Subsetting", "subsettingFeature", "subsettedFeature"),
    ] {
        if without_name(class, left) == without_name(class, right) {
            symmetric.push(format!("{class}: {left} and {right}"));
        }
    }
    println!(
        "AR02-D1 entries {}, differing from the .ecore {}, pairs still the same entry but for the name {}",
        rows.len(),
        wrong.len(),
        symmetric.len()
    );
    assert!(
        wrong.is_empty() && symmetric.is_empty(),
        "AR02 claim: the SysML descriptor does not carry the declarations of the .ecore:\n  {}\n  \
         still the same entry but for the name: {symmetric:?}",
        wrong.join("\n  ")
    );
}

/// D2. The whole SysML descriptor: today's, plus the declarations, and nothing else.
#[test]
fn ar02_d2_sysml_is_todays_descriptor_plus_the_declarations_and_nothing_else() {
    let expected = read_json(&fixture("sysml.ar02.metamodel.json"));
    assert_eq!(
        (
            metamodel_digest(&expected),
            metamodel_digest(&strip(&expected))
        ),
        (
            SYSML_DIGEST_AR02.to_string(),
            SYSML_DIGEST_TODAY.to_string()
        ),
        "set-up: the fixture is not today's descriptor plus the declarations"
    );
    let described = describe(&fixture("sysml.ecore"));

    let mut counts = BTreeMap::from([
        ("entries", 0usize),
        ("eOpposite", 0),
        ("subsets", 0),
        ("redefines", 0),
        ("on attributes", 0),
    ]);
    for (_, array, entry) in entries(&described) {
        let keys: Vec<&str> = KEYS
            .iter()
            .copied()
            .filter(|key| entry.get(*key).is_some())
            .collect();
        if keys.is_empty() {
            continue;
        }
        *counts.get_mut("entries").unwrap() += 1;
        if array == "attributes" {
            *counts.get_mut("on attributes").unwrap() += 1;
        }
        for key in keys {
            *counts.get_mut(key).unwrap() += match &entry[key] {
                Value::Array(list) => list.len(),
                _ => 1,
            };
        }
    }
    let digest = metamodel_digest(&described);
    let stripped = metamodel_digest(&strip(&described));
    println!(
        "AR02-D2 sysml: digest {digest} (today {SYSML_DIGEST_TODAY}), without the three keys \
         {stripped}; entries carrying a key {}, eOpposite {}, subsets {}, redefines {}, on \
         attributes {}; equal to the fixture {}",
        counts["entries"],
        counts["eOpposite"],
        counts["subsets"],
        counts["redefines"],
        counts["on attributes"],
        described == expected
    );
    let mut wrong = Vec::new();
    if stripped != SYSML_DIGEST_TODAY {
        wrong.push(format!(
            "without the three keys the descriptor is not today's: {stripped}"
        ));
    }
    if counts
        != BTreeMap::from([
            ("entries", 293),
            ("eOpposite", 70),
            ("subsets", 224),
            ("redefines", 120),
            ("on attributes", 0),
        ])
    {
        wrong.push(format!(
            "counts {counts:?}, the .ecore declares 293 entries, 70 eOpposite, 224 subsets, 120 \
             redefines, none on an attribute"
        ));
    }
    if described != expected {
        let expected_entries: HashMap<(String, String), &Value> = entries(&expected)
            .into_iter()
            .map(|(class, _, entry)| {
                (
                    (class, entry["name"].as_str().unwrap_or("").to_string()),
                    entry,
                )
            })
            .collect();
        let differing: Vec<String> = entries(&described)
            .into_iter()
            .filter(|(class, _, entry)| {
                expected_entries.get(&(
                    class.clone(),
                    entry["name"].as_str().unwrap_or("").to_string(),
                )) != Some(entry)
            })
            .take(10)
            .map(|(class, _, entry)| format!("{class}.{}: {entry}", entry["name"]))
            .collect();
        wrong.push(format!(
            "the descriptor differs from the fixture (digest {digest}, expected \
             {SYSML_DIGEST_AR02}); first differing entries:\n    {}",
            differing.join("\n    ")
        ));
    }
    assert!(
        wrong.is_empty(),
        "AR02 claim: the SysML descriptor is not today's plus the .ecore's declarations:\n  {}",
        wrong.join("\n  ")
    );
}

/// D3. The other metamodels of the repository that declare an opposite: their descriptors
/// carry it, and nothing else moves. SimpleUML's checked-in descriptor is the one `describe`
/// writes.
#[test]
fn ar02_d3_every_other_metamodel_that_declares_an_opposite_carries_it() {
    let repo = repo();
    let cases: [(&str, &str, &str, Vec<HandRead>); 3] = [
        (
            "examples/SimpleUML.ecore",
            "12d1a371fd995bce2c44e696e3714d59dae58fe04ff6450382a1e02b0a8a27dd",
            "21c50dea71938b861233b72116f5ead094fe1bbb3d67bf623864f52c3ead1c3f",
            // SimpleUML.ecore:13, 23, 34, 62
            vec![
                ("Package", "ownedElements", json!({"eOpposite": "owner"})),
                (
                    "Packageable",
                    "owner",
                    json!({"eOpposite": "ownedElements"}),
                ),
                ("DataType", "attributes", json!({"eOpposite": "owner"})),
                ("Property", "owner", json!({"eOpposite": "attributes"})),
            ],
        ),
        (
            "arachne-parser/rsc/AbstractEcore.ecore",
            "f7ccba64b8f14c2e8992ce6f205d2fe2d8176f43209acc6f3d72c3ab79323721",
            "b6dde99121707ec0aeae484e9362568394eeeff54f00b976a5157ddb31ffc245",
            // AbstractEcore.ecore:17, 25
            vec![
                // Both ends are named `refersClass2`, as the file declares them.
                (
                    "ConcreteClass1",
                    "refersClass2",
                    json!({"eOpposite": "refersClass2"}),
                ),
                (
                    "ConcreteClass2",
                    "refersClass2",
                    json!({"eOpposite": "refersClass2"}),
                ),
            ],
        ),
        (
            "arachne-parser/rsc/ExampleEcore.ecore",
            "f5722f0b5b44bbe0818ef4dda2078c1a95281dfbc5289ae344a8d96ece919970",
            "0f3cdf919b06f7118850c0c396fc49fc156e26fd41f224889d76052825eb26cb",
            // ExampleEcore.ecore:16, 24
            vec![
                ("Teacher", "advises", json!({"eOpposite": "advisor"})),
                ("Student", "advisor", json!({"eOpposite": "advises"})),
            ],
        ),
    ];
    let mut wrong = Vec::new();
    for (ecore, today, ar02, rows) in cases {
        let described = describe(&repo.join(ecore));
        wrong.extend(check_hand_read(&described, "D3", &rows));
        let (digest, stripped) = (
            metamodel_digest(&described),
            metamodel_digest(&strip(&described)),
        );
        let carrying = entries(&described)
            .iter()
            .filter(|(_, _, entry)| KEYS.iter().any(|key| entry.get(*key).is_some()))
            .count();
        println!(
            "AR02-D3 {ecore}: digest {digest} (expected {ar02}), without the three keys {stripped} \
             (today {today}), entries carrying a key {carrying} (declared {})",
            rows.len()
        );
        if digest != ar02 || stripped != today || carrying != rows.len() {
            wrong.push(format!(
                "{ecore}: digest {digest}, expected {ar02}; without the keys {stripped}, today \
                 {today}; {carrying} entries carry a key, {} declared",
                rows.len()
            ));
        }
    }
    // The descriptor the examples hold for SimpleUML is the one `arachne describe` writes, which
    // the editor, the agent and the digest fixture read.
    let checked_in = read_json(&repo.join("examples/uml.metamodel.json"));
    let described = describe(&repo.join("examples/SimpleUML.ecore"));
    println!(
        "AR02-D3 examples/uml.metamodel.json: digest {}, equal to `describe examples/SimpleUML.ecore` {}",
        metamodel_digest(&checked_in),
        checked_in == described
    );
    if checked_in != described {
        wrong.push(format!(
            "examples/uml.metamodel.json (digest {}) is not what `arachne describe \
             examples/SimpleUML.ecore` writes (digest {})",
            metamodel_digest(&checked_in),
            metamodel_digest(&described)
        ));
    }
    assert!(
        wrong.is_empty(),
        "AR02 claim: a metamodel that declares an opposite is not described with it, or \
         something else moved:\n  {}",
        wrong.join("\n  ")
    );
}

/// D4. The guard: every `.ecore` of the repository that declares none of the three is described
/// as it is today, and the checked-in descriptors of those languages keep their bytes.
#[test]
fn ar02_d4_a_descriptor_whose_ecore_declares_none_is_unchanged() {
    // `arachne describe` at 5e78ba7 (the CLI built from it), 2026-09-30.
    const TODAY: [(&str, &str); 22] = [
        (
            "arachne-codegen/tests/generated/st01/st01.ecore",
            "7840af5dac657b02e69b3acdf2e301f3627c6f43c90f3a6c5299a54a7a646b4c",
        ),
        (
            "arachne-codegen/tests/generated/st02/st02.ecore",
            "632d63c7e4fc3640c83b43c14defc566cc84bb79a1b70fe59858b3da896138e8",
        ),
        (
            "arachne-codegen/tests/generated/st04/st04.ecore",
            "db63631a94b02cdea8249d6adb85ec0b68f627328526a3c2f5afe06baf765385",
        ),
        (
            "arachne-codegen/tests/generated/st05/st05.ecore",
            "d43cae9a05bd099529d24894d2d7c6c7aac76e7ea380df4c4c28209f47132ec3",
        ),
        (
            "arachne-codegen/tests/generated/st05cycles/st05cycles.ecore",
            "9d9d975b11d2c6d5835d342528b019a50af6461f04dafc359951dd841b205edd",
        ),
        (
            "arachne-codegen/tests/generated/st05probe/st05probe.ecore",
            "b024a1ca61d904df16c5f498b664a09cee0493c12a6503d8da1f8b250eeb9e1e",
        ),
        (
            "arachne-codegen/tests/generated/st05rec/st05rec.ecore",
            "f87aa5d72cf4b49be2b83bdf73fceffb59c12793bca2e3f345fb1885b98b7c0f",
        ),
        (
            "arachne-codegen/tests/generated/st05recdirect/st05recdirect.ecore",
            "cea1b8712dda40ab519ac33efb223e1e4a9f7f84a63ba0fb5279f68d79e4d29f",
        ),
        (
            "arachne-codegen/tests/generated/st05recindirect/st05recindirect.ecore",
            "a2908a952fc9253f013f62672c23402807c3dcde8e08b8ceb0f62e9e433cf37f",
        ),
        (
            "arachne-parser/rsc/bt.ecore",
            "fe18315d3ae37a6317691d133827b4f51d741ca661e42e44e36716d98be71204",
        ),
        (
            "examples/behavior_tree.ecore",
            "b899ec335e7dc46303d5a7871a1370a12db8858718bea57dc6f58f01c474feb5",
        ),
        (
            "examples/class_diagram.ecore",
            "7e581f337610060484554af5533b43cdb36c566e41e9b3911c3e5a86d074eaaa",
        ),
        (
            "examples/class_hierarchy.ecore",
            "57bb56875ded530df91deda2b181e1041ef76da7d72934f21946a37392b92453",
        ),
        (
            "examples/conference.ecore",
            "608b6584c08f338ae684aeaf1473cab047cc1a14c6afb31bffc051394c72ae57",
        ),
        (
            "examples/json.ecore",
            "a9d83505175d8d6708b86c7eb64d356d64197f066f752c1a9ea842c76a45bc30",
        ),
        (
            "examples/notation.ecore",
            // ED.16a (spec/items/ED.16a.md, decision D5 (a)): `Diagram.diagrams` and
            // `Node.visible` added; 9d551713... before.
            "12e7d2beab5a6d0a44090ed0512228c170a19879a7ac25093d817d58fe5c4de1",
        ),
        (
            "examples/pet_metamodels/abstract_inherits_concrete.ecore",
            "11c127c866fdffca580a78513820a2e48b017b909a61b449259c43e2c0ba4859",
        ),
        (
            "examples/pet_metamodels/concrete_inherits_concrete.ecore",
            "e297e0ec84a8f3948e2f446f15c8ddce34482f8e06b840617c31d138ea578060",
        ),
        (
            "examples/pet_metamodels/concrete_polymorphic_targets.ecore",
            "6cfb23dfd282aca3eefe8be44a9205a0037fbb66acf471c75355a6b2f1045393",
        ),
        (
            "examples/pet_metamodels/ecore_builtins.ecore",
            "a17b955f0ab50c59aad3703579a6a8e7f13f88ecd4c2cc2412a5a661cf5fa68f",
        ),
        (
            "examples/pet_metamodels/kitchen_sink.ecore",
            "e26e2ca28f6c354c14c632d783ce445e36080ae17a0f6baa8591581f435f89dc",
        ),
        (
            "examples/pet_metamodels/multiple_inheritance.ecore",
            "0eb0faa36ff2b22761595fc598511680884e2a4432b4c54188df51d8fb7711c4",
        ),
    ];
    // The checked-in descriptors of those languages, file bytes.
    const FILES: [(&str, &str); 3] = [
        (
            "examples/bt.metamodel.json",
            "ff4525108d4ecb65d297002c7bc02f22978966d347af3678712de07a5c6e1263",
        ),
        (
            "examples/json.metamodel.json",
            "efdbde163d711b39e102ae7dc98585c3cb981ad9be82914ba4f4270e10113ec1",
        ),
        (
            "examples/notation.metamodel.json",
            // ED.16a; 8c98687d... before.
            "04ebb5c15dad375284d4e5e681a5e634130525c614517d373763e6040eead03f",
        ),
    ];
    let repo = repo();
    let mut differing = Vec::new();
    for (ecore, today) in TODAY {
        let described = describe(&repo.join(ecore));
        let digest = metamodel_digest(&described);
        if digest != today {
            differing.push(format!("{ecore}: {digest}, today {today}"));
        }
    }
    for (file, today) in FILES {
        let sha = sha256_of(&repo.join(file));
        if sha != today {
            differing.push(format!("{file}: file sha256 {sha}, today {today}"));
        }
    }
    println!(
        "AR02-D4 descriptors compared {}, checked-in files {}, differing {}",
        TODAY.len(),
        FILES.len(),
        differing.len()
    );
    for line in &differing {
        println!("AR02-D4 {line}");
    }
    assert!(
        differing.is_empty(),
        "AR02 claim: a descriptor whose .ecore declares none of the three moved:\n  {}",
        differing.join("\n  ")
    );
}

/* ---------- D5: AG.02's rule R-e over a descriptor ---------- */

/// One feature as R-e reads it. Nothing here is a class or feature name of any language.
struct Feat {
    is_ref: bool,
    /// Listed under `containments`, and its target class: where a new element may be created.
    containment: Option<String>,
    derived: bool,
    many: bool,
    /// `Target/eOpposite`, the opposite qualified by the class that declares it.
    opp: Option<String>,
    /// `subsets` then `redefines`, qualified.
    up: Vec<String>,
    redefines: Vec<String>,
}

/// The descriptor as R-e reads it: per class, its supertypes, whether it is abstract, and its
/// features in the order the descriptor lists them.
struct Model {
    classes: BTreeMap<String, (Vec<String>, bool, Vec<(String, Feat)>)>,
}

fn qualified(list: &Value) -> Vec<String> {
    list.as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .map(|r| {
            format!(
                "{}/{}",
                r["class"].as_str().unwrap_or(""),
                r["feature"].as_str().unwrap_or("")
            )
        })
        .collect()
}

impl Model {
    fn of(descriptor: &Value) -> Self {
        let mut classes = BTreeMap::new();
        for (name, body) in descriptor["classes"].as_object().expect("classes") {
            let supers = body["superTypes"]
                .as_array()
                .map(|s| {
                    s.iter()
                        .filter_map(|s| s.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let mut feats = Vec::new();
            for array in ["attributes", "containments", "references"] {
                for f in body[array].as_array().map(Vec::as_slice).unwrap_or(&[]) {
                    let merge = &f["merge"];
                    let mut up = qualified(&f["subsets"]);
                    up.extend(qualified(&f["redefines"]));
                    feats.push((
                        f["name"].as_str().unwrap_or("").to_string(),
                        Feat {
                            is_ref: array != "attributes",
                            containment: (array == "containments")
                                .then(|| f["target"].as_str().unwrap_or("").to_string()),
                            derived: merge["kind"] == json!("unsupported")
                                && merge["reason"] == json!("derived"),
                            many: f["many"] == json!(true),
                            opp: f["eOpposite"]
                                .as_str()
                                .map(|o| format!("{}/{o}", f["target"].as_str().unwrap_or(""))),
                            up,
                            redefines: qualified(&f["redefines"]),
                        },
                    ));
                }
            }
            classes.insert(
                name.clone(),
                (supers, body["abstract"] == json!(true), feats),
            );
        }
        Model { classes }
    }

    /// The class and its supertypes, in the order AG.02's script walks them.
    fn supers(&self, class: &str) -> Vec<String> {
        let (mut out, mut todo) = (Vec::<String>::new(), vec![class.to_string()]);
        while let Some(x) = todo.pop() {
            if let Some((sup, _, _)) = self.classes.get(&x) {
                if !out.contains(&x) {
                    out.push(x.clone());
                    todo.extend(sup.iter().cloned());
                }
            }
        }
        out
    }

    /// R-e: the owning end of an element of `class` created in containment `holder` (`Class/feature`)
    /// of its owner. AG.02 section 7's `owning_end`, line for line, over the descriptor.
    fn owning_end(&self, class: &str, holder: &str) -> BTreeSet<String> {
        let mut q: Vec<(String, &Feat)> = Vec::new();
        for s in self.supers(class) {
            for (name, feat) in &self.classes[&s].2 {
                q.push((format!("{s}/{name}"), feat));
            }
        }
        let index: HashMap<&str, &Feat> = q.iter().map(|(n, f)| (n.as_str(), *f)).collect();
        let (holder_class, holder_feature) = holder.split_once('/').expect("Class/feature");
        let opposite = match index.get(holder) {
            Some(f) => f.opp.clone(),
            None => self.classes.get(holder_class).and_then(|(_, _, feats)| {
                feats
                    .iter()
                    .find(|(n, _)| n == holder_feature)
                    .and_then(|(_, f)| f.opp.clone())
            }),
        };
        let Some(opposite) = opposite else {
            return BTreeSet::new();
        };
        let mut redefined_by: HashMap<&str, &str> = HashMap::new();
        for (n, f) in &q {
            for r in &f.redefines {
                redefined_by.insert(r.as_str(), n.as_str());
            }
        }
        let fin = |start: &str| -> String {
            let mut n = start.to_string();
            let mut steps = 0;
            while let Some(next) = redefined_by.get(n.as_str()) {
                n = next.to_string();
                steps += 1;
                assert!(steps < 1000, "set-up: a redefinition cycle through {n}");
            }
            n
        };
        let closure = |n: &str| -> BTreeSet<String> {
            let (mut seen, mut todo) = (BTreeSet::from([n.to_string()]), vec![n.to_string()]);
            while let Some(x) = todo.pop() {
                if let Some(f) = index.get(x.as_str()) {
                    for m in &f.up {
                        if seen.insert(m.clone()) {
                            todo.push(m.clone());
                        }
                    }
                }
            }
            seen
        };
        let mut ends = BTreeSet::new();
        for (d, f) in &q {
            if redefined_by.contains_key(d.as_str()) || !f.derived {
                continue;
            }
            let reach = closure(d);
            if !reach.contains(&opposite) {
                continue;
            }
            for s in reach.iter().filter(|s| *s != d && **s != opposite) {
                let end = fin(s);
                if let Some(e) = index.get(end.as_str()) {
                    if e.is_ref && !e.derived && !e.many {
                        ends.insert(
                            end.split_once('/')
                                .map(|(_, f)| f)
                                .unwrap_or("")
                                .to_string(),
                        );
                    }
                }
            }
        }
        ends
    }
}

/// D5. AG.02's R-e, over the descriptor `describe` writes: on SysML, for an element created in
/// `Element.ownedRelationship`, the owning end of every concrete relationship class is the one
/// AG.02's table (section 3.4) gives from the `.ecore`, `typedFeature`, `subclassifier` and
/// `subsettingFeature` among them; on SimpleUML, nothing, whatever containment holds it.
#[test]
fn ar02_d5_rule_r_e_over_the_descriptor_finds_the_owning_ends_on_sysml_and_nothing_on_simpleuml() {
    let sysml = Model::of(&describe(&fixture("sysml.ecore")));
    let holder = "Element/ownedRelationship";
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut judged = 0;
    for (class, (_, is_abstract, _)) in &sysml.classes {
        if *is_abstract || !sysml.supers(class).iter().any(|s| s == "Relationship") {
            continue;
        }
        judged += 1;
        let ends = sysml.owning_end(class, holder);
        if !ends.is_empty() {
            found.insert(class.clone(), ends);
        }
    }
    // AG.02, section 3.4, the table R-e gives on the `.ecore`.
    let ag02: BTreeMap<String, BTreeSet<String>> = [
        ("Annotation", "annotatedElement"),
        ("ConjugatedPortTyping", "typedFeature"),
        ("Conjugation", "conjugatedType"),
        ("Disjoining", "typeDisjoined"),
        ("FeatureInverting", "featureInverted"),
        ("FeatureTyping", "typedFeature"),
        ("PortConjugation", "conjugatedType"),
        ("Redefinition", "redefiningFeature"),
        ("Specialization", "specific"),
        ("Subclassification", "subclassifier"),
        ("Subsetting", "subsettingFeature"),
        ("TypeFeaturing", "featureOfType"),
    ]
    .into_iter()
    .map(|(c, e)| (c.to_string(), BTreeSet::from([e.to_string()])))
    .collect();
    for (class, ends) in &found {
        println!("AR02-D5 sysml {class} {ends:?}");
    }
    let three: Vec<(&str, Option<&BTreeSet<String>>)> =
        ["FeatureTyping", "Subclassification", "Subsetting"]
            .into_iter()
            .map(|c| (c, found.get(c)))
            .collect();
    println!(
        "AR02-D5 sysml: concrete relationship classes {judged}, with an owning end {}; \
         FeatureTyping {:?}, Subclassification {:?}, Subsetting {:?}; equal to AG.02's table {}",
        found.len(),
        three[0].1,
        three[1].1,
        three[2].1,
        found == ag02
    );

    let uml = Model::of(&describe(&repo().join("examples/SimpleUML.ecore")));
    let mut uml_found = Vec::new();
    let mut uml_judged = 0;
    for (holder_class, (_, _, feats)) in &uml.classes {
        for (name, feat) in feats {
            let Some(target) = &feat.containment else {
                continue;
            };
            for (class, (_, is_abstract, _)) in &uml.classes {
                if *is_abstract || !uml.supers(class).contains(target) {
                    continue;
                }
                uml_judged += 1;
                let ends = uml.owning_end(class, &format!("{holder_class}/{name}"));
                if !ends.is_empty() {
                    uml_found.push(format!("{class} in {holder_class}.{name}: {ends:?}"));
                }
            }
        }
    }
    println!(
        "AR02-D5 simpleuml: holder and class pairs judged {uml_judged}, with an owning end {}",
        uml_found.len()
    );
    assert!(
        judged == 62 && found == ag02 && uml_found.is_empty(),
        "AR02 claim: R-e over the descriptor does not give AG.02's owning ends ({judged} classes \
         judged, 62 expected):\n  found {found:?}\n  AG.02 {ag02:?}\n  SimpleUML {uml_found:?}"
    );
}
