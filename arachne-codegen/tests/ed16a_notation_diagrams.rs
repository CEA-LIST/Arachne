//! ED.16a (`cea-cdrt-knowledge/spec/items/ED.16a.md`): Arachne's notation metamodel holds several
//! diagrams for one model, each with its own content, under decision D5 (a) of the diagram
//! notation plan. The level-1 half of the oracle; the level-3 half is Moirai's
//! `moirai-interp/tests/ed16a_notation_diagrams_l3.rs`.
//!
//! Ground truth, none of it computed by Arachne:
//! - `tests/fixtures/ed16a/notation-9d551713.metamodel.json`: the descriptor shipped at `dc16103`,
//!   copied byte for byte at the spec commit (sha256 `8c98687d…`, digest `9d551713…`).
//! - `tests/fixtures/ed16a/notation-expected.metamodel.json`: that descriptor with the spec's two
//!   additions written in by hand (section 4 of the spec): `Diagram.diagrams`, a many-valued
//!   containment of `Diagram` declared after `edges`, and `Node.visible`, a required `EBoolean`
//!   annotated `lww-register` declared after `height`. Each entry is a copy of its neighbour
//!   (`children`, `height`) with the name, kind and target changed, as `descriptor.rs` writes them.
//! - `NEW_DIGEST` and `NEW_FILE_SHA`: that fixture's digest (sorted-key compact JSON, SHA-256) and
//!   the sha256 of its pretty rendering, computed at registration.
//! - `OTHERS`: the sha256 of every other file under `examples/` at `dc16103`, by `sha256sum`.
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-codegen --test ed16a_notation_diagrams -- --nocapture
//! ```
//!
//! A claim failure panics with `ED16A claim:`; a harness failure with `set-up:`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use arachne_codegen::{EcoreParser, descriptor_json, find_user_package, metamodel_digest};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

// SI.05 (`cea-cdrt-knowledge/spec/items/SI.05.md`, section 6.2): from SI.05 on `describe` names the
// identity rule on every in-scope reference. This file guards everything else, so what it reads is
// `describe`'s output with that rule read back as today's (`si05_support::read_back`), the
// identity before SI.05; `si05_identity_reference_emitter.rs` holds the rule itself.
#[path = "si05_support/mod.rs"]
mod si05_support;

const ECORE: &str = "examples/notation.ecore";
const DESCRIPTOR: &str = "examples/notation.metamodel.json";
const OLD_FIXTURE: &str = "arachne-codegen/tests/fixtures/ed16a/notation-9d551713.metamodel.json";
const EXPECTED_FIXTURE: &str =
    "arachne-codegen/tests/fixtures/ed16a/notation-expected.metamodel.json";
const OLD_DIGEST: &str = "9d551713bd2d1b1cfd8c38e591ed33a454706305278f0d1c1dc7d3589167506c";
const NEW_DIGEST: &str = "12e7d2beab5a6d0a44090ed0512228c170a19879a7ac25093d817d58fe5c4de1";
const NEW_FILE_SHA: &str = "04ebb5c15dad375284d4e5e681a5e634130525c614517d373763e6040eead03f";
/// SI.10 (`cea-cdrt-knowledge/spec/items/SI.10.md`): the shipped notation descriptor is
/// regenerated with SI.05's identity rule (`Edge.source` and `Edge.target` move), so C1 reads it
/// back as today's before comparing, and the digest fixture names it by this digest.
/// `si10_shipped_descriptors.rs` holds its bytes.
const SI10_DIGEST: &str = "14377158866c6dd249915cdadbc5579efb64a3e742b5bd7aff1ced4396f57b86";
const NS_URI: &str = "http://cea.fr/crdt/notation/0.1";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_json(relative: &str) -> Result<Value, String> {
    let text = fs::read_to_string(repo().join(relative))
        .map_err(|e| format!("{relative} cannot be read: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{relative} is not JSON: {e}"))
}

fn fixture(relative: &str) -> Value {
    read_json(relative).unwrap_or_else(|why| panic!("set-up: {why}"))
}

/// What `arachne describe <ecore>` writes, as a value, or why it writes nothing.
fn try_describe(ecore: &Path) -> Result<Value, String> {
    let parser = EcoreParser::from_file(ecore)
        .map_err(|e| format!("{} does not parse: {e}", ecore.display()))?;
    let pack = find_user_package(&parser.ctx)
        .map_err(|e| format!("{} has no user package: {e}", ecore.display()))?;
    descriptor_json(&parser.ctx, pack)
        .map(si05_support::read_back)
        .map_err(|e| format!("{} is not describable: {e}", ecore.display()))
}

/// The entry named `name` in `class`'s `array`, with its position.
fn entry<'a>(
    descriptor: &'a Value,
    class: &str,
    array: &str,
    name: &str,
) -> Option<(usize, &'a Value)> {
    descriptor["classes"][class][array]
        .as_array()?
        .iter()
        .enumerate()
        .find(|(_, e)| e["name"] == name)
}

/// The names of `class`'s `array`, in order.
fn names(descriptor: &Value, class: &str, array: &str) -> Vec<String> {
    descriptor["classes"][class][array]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|e| e["name"].as_str().unwrap_or("?").to_string())
                .collect()
        })
        .unwrap_or_default()
}

/* ---------- premises: the fixtures are what the spec says they are ---------- */

#[test]
fn ed16a_premise_the_fixtures_are_the_registered_ones() {
    let old = fixture(OLD_FIXTURE);
    let expected = fixture(EXPECTED_FIXTURE);
    assert_eq!(
        metamodel_digest(&old),
        OLD_DIGEST,
        "set-up: the old fixture moved"
    );
    assert_eq!(
        metamodel_digest(&expected),
        NEW_DIGEST,
        "set-up: the expected fixture moved"
    );
    let rendered = format!("{expected:#}\n");
    assert_eq!(
        sha256_hex(rendered.as_bytes()),
        NEW_FILE_SHA,
        "set-up: the expected fixture does not render to the registered bytes"
    );
}

/* ---------- C1: the shipped descriptor holds several diagrams, each with its own nodes ---------- */

/// C1. `examples/notation.metamodel.json` is, value for value, the hand-derived descriptor: the
/// one shipped at `dc16103` with `Diagram.diagrams` (a containment of `Diagram`, many, a
/// sequence, after `edges`) and `Node.visible` (a required `bool`, an LWW register, after
/// `height`) added, and nothing else changed: `rootClasses` stays `["Diagram"]`, no class is
/// added, no other entry moves. Its digest is `NEW_DIGEST`.
#[test]
fn ed16a_c1_the_descriptor_is_the_old_one_plus_diagrams_and_visible() {
    let expected = fixture(EXPECTED_FIXTURE);
    // SI.10: read back as today's (see `SI10_DIGEST`).
    let shipped = match read_json(DESCRIPTOR) {
        Ok(v) => si05_support::read_back(v),
        Err(why) => panic!("ED16A claim: {why}"),
    };
    let mut fail = Vec::new();

    let diagrams = entry(&shipped, "Diagram", "containments", "diagrams");
    match diagrams {
        None => fail.push(format!(
            "Diagram holds no containment `diagrams` (containments {:?})",
            names(&shipped, "Diagram", "containments")
        )),
        Some((pos, got)) => {
            let (want_pos, want) =
                entry(&expected, "Diagram", "containments", "diagrams").expect("set-up: fixture");
            if pos != want_pos || got != want {
                fail.push(format!(
                    "Diagram.diagrams at {pos} is {got}, not at {want_pos} {want}"
                ));
            }
        }
    }
    match entry(&shipped, "Node", "attributes", "visible") {
        None => fail.push(format!(
            "Node has no attribute `visible` (attributes {:?})",
            names(&shipped, "Node", "attributes")
        )),
        Some((pos, got)) => {
            let (want_pos, want) =
                entry(&expected, "Node", "attributes", "visible").expect("set-up: fixture");
            if pos != want_pos || got != want {
                fail.push(format!(
                    "Node.visible at {pos} is {got}, not at {want_pos} {want}"
                ));
            }
        }
    }
    if shipped["rootClasses"] != json!(["Diagram"]) {
        fail.push(format!(
            "rootClasses is {}, not [\"Diagram\"]: a view written today has a Diagram root",
            shipped["rootClasses"]
        ));
    }
    if shipped["nsURI"] != json!(NS_URI) || shipped["package"] != json!("notation") {
        fail.push(format!(
            "nsURI {} package {}, not {NS_URI} notation",
            shipped["nsURI"], shipped["package"]
        ));
    }
    // Everything else, value for value.
    let shipped_classes: Vec<String> = shipped["classes"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let expected_classes: Vec<String> = expected["classes"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    if shipped_classes != expected_classes {
        fail.push(format!(
            "classes {shipped_classes:?}, not {expected_classes:?}"
        ));
    }
    for class in &expected_classes {
        for array in ["attributes", "containments", "references"] {
            let got = names(&shipped, class, array);
            let want = names(&expected, class, array);
            if got != want {
                fail.push(format!("{class}.{array} is {got:?}, not {want:?}"));
            }
        }
    }
    let digest = metamodel_digest(&shipped);
    if shipped != expected && fail.is_empty() {
        fail.push("the descriptor differs from the expected one outside the named entries".into());
    }
    println!(
        "ED16A-C1 shipped digest {digest}; old {OLD_DIGEST}; registered new {NEW_DIGEST}; \
         equal to the expected descriptor {}",
        shipped == expected
    );
    if digest != NEW_DIGEST {
        fail.push(format!("digest {digest}, not the registered {NEW_DIGEST}"));
    }
    assert!(
        fail.is_empty(),
        "ED16A claim: the notation descriptor does not hold several diagrams with their own \
         nodes as section 4 designs it:\n  {}",
        fail.join("\n  ")
    );
}

/// C1, the identity half: the checked-in descriptor is what `arachne describe
/// examples/notation.ecore` writes, byte for byte (`{descriptor:#}` and a newline), and those
/// bytes are the registered ones.
#[test]
fn ed16a_c1_the_checked_in_descriptor_is_what_arachne_describe_writes() {
    let mut fail = Vec::new();
    match try_describe(&repo().join(ECORE)) {
        Err(why) => fail.push(why),
        Ok(described) => {
            let rendered = format!("{described:#}\n");
            match fs::read(repo().join(DESCRIPTOR)) {
                Err(e) => fail.push(format!("{DESCRIPTOR}: {e}")),
                Ok(bytes) => {
                    // SI.10: read back as today's (see `SI10_DIGEST`).
                    let bytes = si05_support::read_back_bytes(bytes);
                    if bytes != rendered.as_bytes() {
                        fail.push(format!(
                            "{DESCRIPTOR} differs from `arachne describe {ECORE}` (digest of \
                             the described {})",
                            metamodel_digest(&described)
                        ));
                    }
                    let sha = sha256_hex(&bytes);
                    println!("ED16A-C1 {DESCRIPTOR} sha256 {sha}, registered {NEW_FILE_SHA}");
                    if sha != NEW_FILE_SHA {
                        fail.push(format!("{DESCRIPTOR} sha256 {sha}, not {NEW_FILE_SHA}"));
                    }
                }
            }
        }
    }
    assert!(
        fail.is_empty(),
        "ED16A claim: the shipped notation descriptor is not the described one:\n  {}",
        fail.join("\n  ")
    );
}

/* ---------- C3: nothing else under examples/ moves; the digest fixture names the new one ---------- */

/// C3. Every file under `examples/` but `notation.ecore`, `notation.metamodel.json`,
/// `README.md` and the digest fixture keeps its bytes (`sha256sum` at `dc16103`); every other
/// descriptor keeps its digest; no file is added; and `fixtures/metamodel-digests.json` names
/// the notation descriptor by `NEW_DIGEST` and the three others as before.
#[test]
fn ed16a_c3_nothing_else_under_examples_moves() {
    const OTHERS: [(&str, &str); 18] = [
        (
            "GENERIC_NETWORK_GUIDE.md",
            "01ab74a6ad9ea4e0f7cf0465b10661f8953d095eafef3bb87d684ccc9c15adac",
        ),
        (
            "SimpleUML.ecore",
            "31296010f3cde494863c4d55b4fd0d1adaf56023dc07ccc6d694c6bec0be411c",
        ),
        (
            "UML.ecore",
            "421e95a6b1946e9705a136011a0fc02aa7bf9b2871cba3e83bf11f11234176b6",
        ),
        (
            "behavior_tree.ecore",
            "d658b5c5b7a71dbdc49dda87d20a151c25336aabc16cd2c38b5b05f41dd663d5",
        ),
        (
            "bt.metamodel.json",
            // SI.10: regenerated with the identity rule; ff452510... before.
            "d3b20675a5df3f62e824b25467f00840ed506171e300c4b1aa0e787df995bd4e",
        ),
        (
            "class_diagram.ecore",
            "b8645c0fb97cbe9debed4af2937b2e63ac71ab0b18bd23542aca611267a6a05b",
        ),
        (
            "class_hierarchy.ecore",
            "bc1d7f1a40522d9905e6576caf72422a87f42c307a936084f6fdeb5e3cdbf7d3",
        ),
        (
            "conference.ecore",
            "4b30cbcf9411caa86ca4a6e74ef835b9944747d319fa3a6440b5d505741a0346",
        ),
        (
            "json.ecore",
            "fd88ee6b27546e51d9948fccb709ba95e082a0ed343b171aae1e71a94adc4b9c",
        ),
        (
            "json.metamodel.json",
            "efdbde163d711b39e102ae7dc98585c3cb981ad9be82914ba4f4270e10113ec1",
        ),
        (
            "pet_metamodels/abstract_inherits_concrete.ecore",
            "d11e29a90768c6d9c3226a3d8289b0158733972b99bfd8c8daa41a1bfeb1b6e6",
        ),
        (
            "pet_metamodels/concrete_inherits_concrete.ecore",
            "b2277e3bfa6951675e0575fddf6548175ed17c83ff275044ec944ceb2198be57",
        ),
        (
            "pet_metamodels/concrete_polymorphic_targets.ecore",
            "16dba6831820fee59f1072c029696e674de623c04b781d0fc98982d548aa89d8",
        ),
        (
            "pet_metamodels/ecore_builtins.ecore",
            "108465fab2d43a002c81a49fcf2f6dc1b4aa0da3fa6a7f64059e9704d1c33149",
        ),
        (
            "pet_metamodels/kitchen_sink.ecore",
            "dd765685c7072f6f0250c70bba8d1214d37250c5cb5af114d9e93e2c9c7d4a1b",
        ),
        (
            "pet_metamodels/multiple_inheritance.ecore",
            "ce39e5d5a6566e814588e44d2f81028bdc1226f5ae3b7937e8292d14cda7b7bc",
        ),
        (
            "umlclass.ecore",
            "d2baec8b4f99d36d6a5b53420f31ade7422160c64cba4c87d9e7cebed9618437",
        ),
        (
            "umlclass.metamodel.json",
            // SI.10: regenerated with the identity rule; a770d938... before.
            "9600cc395935773caa15518c3a3e1d337aaccfa31f1d5073deae53dc2064ec79",
        ),
    ];
    // Every checked-in descriptor but notation keeps its digest (umlclass's taken at dc16103 by
    // the same sorted-key compact SHA-256; ED.10 did not pin it).
    const DIGESTS: [(&str, &str); 4] = [
        (
            "bt.metamodel.json",
            // SI.10; b899ec33... before.
            "f19f730e4aaa7320d23e6b211a7856e7aa9042b56cef67cb3c0cb36091ad6822",
        ),
        (
            "uml.metamodel.json",
            // SI.10; 21c50dea... before.
            "5d34af887ace294f008c96403a4e6dd442f374d4e23639ec1620c04a4cf301f3",
        ),
        (
            "json.metamodel.json",
            "a9d83505175d8d6708b86c7eb64d356d64197f066f752c1a9ea842c76a45bc30",
        ),
        (
            "umlclass.metamodel.json",
            // SI.10; a44920ad... before.
            "999ed54f0657c2c8e27162645b0676e94be56a16d9058429707a104d1e620dc9",
        ),
    ];
    const UML_FILE: (&str, &str) = (
        "uml.metamodel.json",
        // SI.10: regenerated with the identity rule; b000aa0d... before.
        "8c50a7a89aedad54fd19c6939798ddf7ef1c3b7c4a7444a2ddf467748dc4c727",
    );
    const MAY_CHANGE: [&str; 4] = [
        "notation.ecore",
        "notation.metamodel.json",
        "README.md",
        "fixtures/metamodel-digests.json",
    ];
    // AR.04 (spec/items/AR.04.md, section 6): the annotated SysML metamodel and its descriptor
    // are added under `examples/sysml/`, a directory no node's METAMODEL_DIR reads.
    const AR04_ADDED: [&str; 2] = ["sysml/sysml.ecore", "sysml/sysml.metamodel.json"];
    let examples = repo().join("examples");
    let mut fail = Vec::new();
    for (file, today) in OTHERS.iter().chain(std::iter::once(&UML_FILE)) {
        match fs::read(examples.join(file)) {
            Ok(bytes) => {
                let sha = sha256_hex(&bytes);
                if sha != *today {
                    fail.push(format!("{file}: sha256 {sha}, at dc16103 {today}"));
                }
            }
            Err(e) => fail.push(format!("{file}: {e}")),
        }
    }
    for (file, today) in DIGESTS {
        let digest = read_json(&format!("examples/{file}"))
            .map(|v| metamodel_digest(&v))
            .unwrap_or_else(|why| why);
        if digest != today {
            fail.push(format!("{file}: digest {digest}, at dc16103 {today}"));
        }
    }
    // The digest fixture: notation's entry is the new digest, the others are as at dc16103.
    match read_json("examples/fixtures/metamodel-digests.json") {
        Err(why) => fail.push(why),
        Ok(fixture) => {
            let want = json!({
                "bt.metamodel.json": {"nsURI": "http://www.example.org/behaviortree", "digest": DIGESTS[0].1},
                "uml.metamodel.json": {"nsURI": "http:///SimpleUML.ecore", "digest": DIGESTS[1].1},
                "json.metamodel.json": {"nsURI": "http://www.example.org/json", "digest": DIGESTS[2].1},
                "notation.metamodel.json": {"nsURI": NS_URI, "digest": SI10_DIGEST},
            });
            if fixture != want {
                fail.push(format!(
                    "fixtures/metamodel-digests.json is {fixture}, not {want}"
                ));
            }
            let shipped_digest = read_json(DESCRIPTOR)
                .map(|v| metamodel_digest(&v))
                .unwrap_or_else(|why| why);
            if fixture["notation.metamodel.json"]["digest"] != json!(shipped_digest) {
                fail.push(format!(
                    "the digest fixture names notation by {}, the shipped descriptor's digest \
                     is {shipped_digest}",
                    fixture["notation.metamodel.json"]["digest"]
                ));
            }
        }
    }
    // No file added.
    let mut present = Vec::new();
    let mut todo = vec![examples.clone()];
    while let Some(dir) = todo.pop() {
        for entry in fs::read_dir(&dir)
            .expect("set-up: listing examples/")
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                todo.push(path);
            } else {
                present.push(
                    path.strip_prefix(&examples)
                        .expect("under examples/")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    present.sort();
    for p in &present {
        let known = OTHERS.iter().any(|(f, _)| f == p)
            || UML_FILE.0 == p
            || MAY_CHANGE.contains(&p.as_str())
            || AR04_ADDED.contains(&p.as_str());
        if !known {
            fail.push(format!("unexpected file examples/{p}"));
        }
    }
    println!(
        "ED16A-C3 files guarded {}, files under examples/ {}, differing or unexpected {}",
        OTHERS.len() + 1,
        present.len(),
        fail.len()
    );
    assert!(
        fail.is_empty(),
        "ED16A claim: something else under examples/ moved:\n  {}",
        fail.join("\n  ")
    );
}
