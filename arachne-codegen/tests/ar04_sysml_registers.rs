//! AR.04 (`cea-cdrt-knowledge/spec/items/AR.04.md`): SysON's SysML descriptor stops encoding
//! names, identifiers and literal values as text and counters. Our fork carries an annotated copy
//! of SysON's `sysml.ecore`, `examples/sysml/sysml.ecore`, whose `urn:arachne:semantics`
//! annotations make every replicated string attribute but `Comment.body` and
//! `TextualRepresentation.body`, and `LiteralInteger.value` and `LiteralRational.value`, merge as
//! multi-value registers; its descriptor is checked in beside it as
//! `examples/sysml/sysml.metamodel.json`; Arachne's defaults do not move, so no other
//! metamodel's descriptor does.
//!
//! Ground truth, none of it computed by Arachne:
//! - R1: SysON's `sysml.ecore` (`tests/fixtures/ar02/sysml.ecore`, sha256 `82463c8f...`) read
//!   by the small XML reader below, and the claim's list of attributes, written out here.
//! - R2, R3: `tests/fixtures/ar04/sysml.ar04.metamodel.json`, today's descriptor (AR.02's
//!   fixture, digest `ebd58069...`) with the 17 claimed entries turned into multi-value registers
//!   by `tests/fixtures/ar04/registers.py`, Python alone; its digest `2df00538...`.
//! - R4: the digests `arachne describe` wrote at `b9b85e7` for every describable `.ecore` of
//!   this repository, the bytes of the checked-in descriptors and of `to_crdt.rs` at `b9b85e7`.
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-codegen --test ar04_sysml_registers -- --nocapture
//! ```

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use arachne_codegen::{EcoreParser, descriptor_json, find_user_package, metamodel_digest};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// SysON's metamodel, byte for byte (AR.02's fixture).
const SYSON_ECORE: &str = "arachne-codegen/tests/fixtures/ar02/sysml.ecore";
const SYSON_ECORE_SHA256: &str = "82463c8f21d1a688603866ec02c6d4880b4e9224e34e9833a464aed0bd8f5440";
/// The annotated copy and its descriptor, where the spec puts them.
const ANNOTATED_ECORE: &str = "examples/sysml/sysml.ecore";
const ANNOTATED_DESCRIPTOR: &str = "examples/sysml/sysml.metamodel.json";
/// What the annotated copy must describe to (`registers.py`).
const EXPECTED: &str = "arachne-codegen/tests/fixtures/ar04/sysml.ar04.metamodel.json";
const EXPECTED_SHA256: &str = "896ba4d758c6d8adfe9764bb7779f3400212c02afd97959012936d4e113d4e47";
const EXPECTED_DIGEST: &str = "2df00538ec6fc36639611b6e57d0926e8422bce4aa7a17d4083bdbcd1f761457";
/// SysON's own `.ecore`, unannotated, describes to this today and must after.
const SYSON_DIGEST: &str = "ebd580696f97d9339c148f5b29db5d0ff7b0928d78fb05efc7c206811405e627";

const SEMANTICS: &str = "urn:arachne:semantics";

/// The claimed attributes SysON's `.ecore` declares itself, as `(class, attribute)`: every
/// replicated string attribute but the two bodies, and the two literal numbers (spec section 3,
/// C1). Ecore's own three (`EAnnotation.source`, `EStringToStringMapEntry.key` and `.value`) are
/// declared by no element of the file and are checked on the descriptor (R2).
const IN_FILE: [(&str, &str); 14] = [
    ("Element", "elementId"),
    ("Element", "aliasIds"),
    ("Element", "declaredName"),
    ("Element", "declaredShortName"),
    ("Membership", "memberName"),
    ("Membership", "memberShortName"),
    ("RequirementDefinition", "reqId"),
    ("RequirementUsage", "reqId"),
    ("OperatorExpression", "operator"),
    ("TextualRepresentation", "language"),
    ("Comment", "locale"),
    ("LiteralString", "value"),
    ("LiteralInteger", "value"),
    ("LiteralRational", "value"),
];
/// All 17 claimed descriptor entries, as `(class, attribute)`.
const CLAIMED: [(&str, &str); 17] = [
    ("Element", "elementId"),
    ("Element", "aliasIds"),
    ("Element", "declaredName"),
    ("Element", "declaredShortName"),
    ("Membership", "memberName"),
    ("Membership", "memberShortName"),
    ("RequirementDefinition", "reqId"),
    ("RequirementUsage", "reqId"),
    ("OperatorExpression", "operator"),
    ("TextualRepresentation", "language"),
    ("Comment", "locale"),
    ("LiteralString", "value"),
    ("ecore::EAnnotation", "source"),
    ("ecore::EStringToStringMapEntry", "key"),
    ("ecore::EStringToStringMapEntry", "value"),
    ("LiteralInteger", "value"),
    ("LiteralRational", "value"),
];
/// The two that stay collaborative text.
const KEPT_AS_TEXT: [(&str, &str); 2] = [("Comment", "body"), ("TextualRepresentation", "body")];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn sha256_of(path: &Path) -> Result<String, String> {
    fs::read(path)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// What `arachne describe <ecore>` writes, as a value.
fn describe(ecore: &Path) -> Result<Value, String> {
    let parser = EcoreParser::from_file(ecore)
        .map_err(|e| format!("{} does not parse: {e}", ecore.display()))?;
    let pack = find_user_package(&parser.ctx)
        .map_err(|e| format!("{}: no user package: {e}", ecore.display()))?;
    descriptor_json(&parser.ctx, pack).map_err(|e| format!("describing {}: {e}", ecore.display()))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} is not JSON: {e}", path.display()))
}

/* ---------- a small XML reader, enough for an .ecore file ---------- */

/// One element: its name, its attributes (entity-decoded, order-free) and its child elements in
/// order. Comments, the XML declaration and whitespace are not part of it; other text is kept as
/// a child named `#text`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct El {
    name: String,
    attrs: BTreeMap<String, String>,
    children: Vec<El>,
}

fn decode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let Some(end) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let entity = &tail[1..end];
        let ch = match entity {
            "quot" => Some('"'),
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "apos" => Some('\''),
            _ if entity.starts_with("#x") => u32::from_str_radix(&entity[2..], 16)
                .ok()
                .and_then(char::from_u32),
            _ if entity.starts_with('#') => {
                entity[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
            _ => None,
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=end]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

fn parse_xml(text: &str) -> Result<El, String> {
    let mut stack: Vec<El> = vec![El {
        name: "#document".into(),
        attrs: BTreeMap::new(),
        children: Vec::new(),
    }];
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<!--") {
            let end = after.find("-->").ok_or("an unterminated comment")?;
            rest = &after[end + 3..];
        } else if let Some(after) = rest.strip_prefix("<?") {
            let end = after.find("?>").ok_or("an unterminated declaration")?;
            rest = &after[end + 2..];
        } else if let Some(after) = rest.strip_prefix("</") {
            let end = after.find('>').ok_or("an unterminated end tag")?;
            let name = after[..end].trim();
            let done = stack.pop().ok_or("an end tag with nothing open")?;
            if done.name != name {
                return Err(format!("</{name}> closes <{}>", done.name));
            }
            stack
                .last_mut()
                .ok_or("an end tag past the document")?
                .children
                .push(done);
            rest = &after[end + 1..];
        } else if let Some(after) = rest.strip_prefix('<') {
            // A start tag: the name, then `key="value"` pairs, then `>` or `/>`.
            let mut i = 0;
            let bytes = after.as_bytes();
            while i < bytes.len()
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b'>'
                && bytes[i] != b'/'
            {
                i += 1;
            }
            let mut el = El {
                name: after[..i].to_string(),
                attrs: BTreeMap::new(),
                children: Vec::new(),
            };
            let closed;
            loop {
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i >= bytes.len() {
                    return Err(format!("an unterminated <{}>", el.name));
                }
                if bytes[i] == b'>' {
                    closed = false;
                    i += 1;
                    break;
                }
                if after[i..].starts_with("/>") {
                    closed = true;
                    i += 2;
                    break;
                }
                let eq = after[i..]
                    .find('=')
                    .ok_or_else(|| format!("an attribute with no value in <{}>", el.name))?
                    + i;
                let key = after[i..eq].trim().to_string();
                let open = after[eq + 1..]
                    .find('"')
                    .ok_or("an unquoted attribute value")?
                    + eq
                    + 1;
                let close = after[open + 1..]
                    .find('"')
                    .ok_or("an unterminated attribute value")?
                    + open
                    + 1;
                el.attrs.insert(key, decode(&after[open + 1..close]));
                i = close + 1;
            }
            rest = &after[i..];
            if closed {
                stack
                    .last_mut()
                    .ok_or("a tag past the document")?
                    .children
                    .push(el);
            } else {
                stack.push(el);
            }
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let chunk = rest[..end].trim();
            if !chunk.is_empty() {
                stack
                    .last_mut()
                    .ok_or("text past the document")?
                    .children
                    .push(El {
                        name: "#text".into(),
                        attrs: BTreeMap::from([("text".to_string(), decode(chunk))]),
                        children: Vec::new(),
                    });
            }
            rest = &rest[end..];
        }
    }
    let mut document = stack.pop().ok_or("no document")?;
    if !stack.is_empty() {
        return Err(format!("<{}> is never closed", document.name));
    }
    if document.children.len() != 1 {
        return Err(format!("{} top-level elements", document.children.len()));
    }
    Ok(document.children.remove(0))
}

fn is_semantics(el: &El) -> bool {
    el.name == "eAnnotations" && el.attrs.get("source").map(String::as_str) == Some(SEMANTICS)
}

/// `el` with every `urn:arachne:semantics` annotation taken out, at every depth.
fn without_semantics(el: &El) -> El {
    El {
        name: el.name.clone(),
        attrs: el.attrs.clone(),
        children: el
            .children
            .iter()
            .filter(|c| !is_semantics(c))
            .map(without_semantics)
            .collect(),
    }
}

/// Where two trees first differ, as a path of element names, or None.
fn first_difference(a: &El, b: &El, at: &str) -> Option<String> {
    let here = format!(
        "{at}/{}{}",
        a.name,
        a.attrs
            .get("name")
            .map(|n| format!("[{n}]"))
            .unwrap_or_default()
    );
    if a.name != b.name || a.attrs != b.attrs {
        return Some(format!(
            "{here}: {:?} against {:?}",
            (&a.name, &a.attrs),
            (&b.name, &b.attrs)
        ));
    }
    for (x, y) in a.children.iter().zip(&b.children) {
        if let Some(d) = first_difference(x, y, &here) {
            return Some(d);
        }
    }
    if a.children.len() != b.children.len() {
        return Some(format!(
            "{here}: {} children against {}",
            a.children.len(),
            b.children.len()
        ));
    }
    None
}

/// Every `(class, feature, element)` of the package's classes.
fn features(root: &El) -> Vec<(String, String, &El)> {
    let mut out = Vec::new();
    for class in root.children.iter().filter(|c| c.name == "eClassifiers") {
        if class.attrs.get("xsi:type").map(String::as_str) != Some("ecore:EClass") {
            continue;
        }
        let cname = class.attrs.get("name").cloned().unwrap_or_default();
        for f in class
            .children
            .iter()
            .filter(|c| c.name == "eStructuralFeatures")
        {
            out.push((
                cname.clone(),
                f.attrs.get("name").cloned().unwrap_or_default(),
                f,
            ));
        }
    }
    out
}

/// Whether `feature` carries `urn:arachne:semantics` `datatype` = `mv-register`.
fn carries_mv_register(feature: &El) -> bool {
    feature
        .children
        .iter()
        .filter(|c| is_semantics(c))
        .any(|annotation| {
            annotation.children.iter().any(|d| {
                d.name == "details"
                    && d.attrs.get("key").map(String::as_str) == Some("datatype")
                    && d.attrs.get("value").map(String::as_str) == Some("mv-register")
            })
        })
}

/* ---------- R1: the annotated copy is SysON's file plus the annotations ---------- */

#[test]
fn ar04_r1_the_annotated_copy_is_syson_s_ecore_with_mv_register_on_the_claimed_attributes() {
    let repo = repo();
    let syson_sha = sha256_of(&repo.join(SYSON_ECORE)).unwrap_or_else(|e| panic!("set-up: {e}"));
    assert_eq!(
        syson_sha, SYSON_ECORE_SHA256,
        "set-up: the SysON fixture moved"
    );
    let syson =
        parse_xml(&fs::read_to_string(repo.join(SYSON_ECORE)).expect("set-up: SysON's ecore"))
            .unwrap_or_else(|e| panic!("set-up: reading SysON's ecore: {e}"));
    let text = fs::read_to_string(repo.join(ANNOTATED_ECORE));
    let annotated = match text {
        Ok(text) => {
            parse_xml(&text).map_err(|e| format!("{ANNOTATED_ECORE} is not well-formed: {e}"))
        }
        Err(e) => Err(format!("{ANNOTATED_ECORE}: {e}")),
    };
    let mut fail = Vec::new();
    match &annotated {
        Err(why) => fail.push(why.clone()),
        Ok(annotated) => {
            if let Some(d) = first_difference(&without_semantics(annotated), &syson, "") {
                fail.push(format!(
                    "without its urn:arachne:semantics annotations it is not SysON's file: {d}"
                ));
            }
            let all = features(annotated);
            for (class, feature) in IN_FILE {
                match all.iter().find(|(c, f, _)| c == class && f == feature) {
                    None => fail.push(format!("{class}.{feature}: not declared")),
                    Some((_, _, el)) if !carries_mv_register(el) => fail.push(format!(
                        "{class}.{feature}: no urn:arachne:semantics datatype=mv-register"
                    )),
                    Some(_) => {}
                }
            }
            for (class, feature, el) in &all {
                let claimed = IN_FILE.iter().any(|(c, f)| c == class && f == feature);
                if !claimed && el.children.iter().any(is_semantics) {
                    fail.push(format!(
                        "{class}.{feature}: annotated, and the claim does not name it"
                    ));
                }
            }
            for (class, feature) in KEPT_AS_TEXT {
                if !all.iter().any(|(c, f, _)| c == class && f == feature) {
                    fail.push(format!("{class}.{feature}: not declared"));
                }
            }
        }
    }
    println!(
        "AR04-R1 claimed in the file {}, failures {}",
        IN_FILE.len(),
        fail.len()
    );
    for line in &fail {
        println!("AR04-R1 {line}");
    }
    assert!(
        fail.is_empty(),
        "AR04 claim: the annotated copy:\n  {}",
        fail.join("\n  ")
    );
}

/* ---------- R2, R3: the descriptor it makes, and the one checked in ---------- */

fn entry<'a>(descriptor: &'a Value, class: &str, attribute: &str) -> Option<&'a Value> {
    descriptor["classes"][class]["attributes"]
        .as_array()?
        .iter()
        .find(|e| e["name"] == attribute)
}

#[test]
fn ar04_r2_the_annotated_copy_describes_the_claimed_attributes_as_mv_registers_and_nothing_else_moves()
 {
    let repo = repo();
    let expected = read_json(&repo.join(EXPECTED)).unwrap_or_else(|e| panic!("set-up: {e}"));
    assert_eq!(
        metamodel_digest(&expected),
        EXPECTED_DIGEST,
        "set-up: the AR.04 fixture moved"
    );
    let mut fail = Vec::new();
    match describe(&repo.join(ANNOTATED_ECORE)) {
        Err(why) => fail.push(why),
        Ok(described) => {
            let digest = metamodel_digest(&described);
            println!("AR04-R2 digest {digest}, expected {EXPECTED_DIGEST}");
            for (class, attribute) in CLAIMED {
                let got = entry(&described, class, attribute);
                let ok = got.is_some_and(|e| {
                    e["merge"]["leaf"] == serde_json::json!({"kind": "register", "tie": "mv"})
                        && e["annotation"] == "mv-register"
                        && e["provenance"]["leaf"] == "annotation"
                });
                if !ok {
                    fail.push(format!(
                        "{class}.{attribute}: leaf {}, annotation {}, provenance.leaf {}",
                        got.map(|e| e["merge"]["leaf"].to_string())
                            .unwrap_or_else(|| "absent".into()),
                        got.map(|e| e["annotation"].to_string()).unwrap_or_default(),
                        got.map(|e| e["provenance"]["leaf"].to_string())
                            .unwrap_or_default(),
                    ));
                }
            }
            for (class, attribute) in KEPT_AS_TEXT {
                let leaf = entry(&described, class, attribute).map(|e| e["merge"]["leaf"].clone());
                if leaf != Some(serde_json::json!({"kind": "text"})) {
                    fail.push(format!("{class}.{attribute}: leaf {leaf:?}, not text"));
                }
            }
            if described != expected {
                let mut moved = 0;
                for (class, body) in expected["classes"].as_object().expect("classes") {
                    for array in ["attributes", "containments", "references"] {
                        let want = body[array].as_array().map(Vec::as_slice).unwrap_or(&[]);
                        let got = described["classes"][class][array]
                            .as_array()
                            .map(Vec::as_slice)
                            .unwrap_or(&[]);
                        for (i, w) in want.iter().enumerate() {
                            if got.get(i) != Some(w) {
                                moved += 1;
                                if moved <= 12 {
                                    fail.push(format!(
                                        "{class}.{array}[{i}] ({}): {} against the fixture's {}",
                                        w["name"],
                                        got.get(i)
                                            .map(Value::to_string)
                                            .unwrap_or_else(|| "absent".into()),
                                        w
                                    ));
                                }
                            }
                        }
                    }
                }
                fail.push(format!(
                    "the descriptor is not the fixture: {moved} entries differ, digest {digest} against {EXPECTED_DIGEST}"
                ));
            }
        }
    }
    println!(
        "AR04-R2 entries claimed {}, failures {}",
        CLAIMED.len(),
        fail.len()
    );
    for line in &fail {
        println!("AR04-R2 {line}");
    }
    assert!(
        fail.is_empty(),
        "AR04 claim: the descriptor of the annotated copy:\n  {}",
        fail.join("\n  ")
    );
}

#[test]
fn ar04_r3_the_checked_in_descriptor_is_the_one_describe_writes() {
    let repo = repo();
    let expected_sha = sha256_of(&repo.join(EXPECTED)).unwrap_or_else(|e| panic!("set-up: {e}"));
    assert_eq!(
        expected_sha, EXPECTED_SHA256,
        "set-up: the AR.04 fixture moved"
    );
    let sha = sha256_of(&repo.join(ANNOTATED_DESCRIPTOR));
    let described = describe(&repo.join(ANNOTATED_ECORE));
    let checked_in = read_json(&repo.join(ANNOTATED_DESCRIPTOR));
    println!(
        "AR04-R3 {ANNOTATED_DESCRIPTOR}: sha256 {sha:?}, equal to describe {:?}",
        match (&described, &checked_in) {
            (Ok(d), Ok(c)) => Some(d == c),
            _ => None,
        }
    );
    assert!(
        sha.as_deref() == Ok(EXPECTED_SHA256)
            && matches!((&described, &checked_in), (Ok(d), Ok(c)) if d == c),
        "AR04 claim: {ANNOTATED_DESCRIPTOR} is the fixture's bytes ({EXPECTED_SHA256}) and what describe \
         writes for {ANNOTATED_ECORE}: sha256 {sha:?}"
    );
}

/* ---------- R4: every other descriptor, and Arachne's defaults, as today ---------- */

#[test]
fn ar04_r4_no_other_metamodel_and_no_default_moves() {
    // `arachne describe` at b9b85e7 (the CLI built from it), 2026-10-09. `aas.ecore` and
    // `UML.ecore` are not describable and are left out.
    const TODAY: [(&str, &str); 27] = [
        (SYSON_ECORE, SYSON_DIGEST),
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
            "arachne-codegen/tests/generated/st05cycles/st05cycles.ecore",
            "9d9d975b11d2c6d5835d342528b019a50af6461f04dafc359951dd841b205edd",
        ),
        (
            "arachne-codegen/tests/generated/st05probe/st05probe.ecore",
            "b024a1ca61d904df16c5f498b664a09cee0493c12a6503d8da1f8b250eeb9e1e",
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
            "arachne-codegen/tests/generated/st05rec/st05rec.ecore",
            "f87aa5d72cf4b49be2b83bdf73fceffb59c12793bca2e3f345fb1885b98b7c0f",
        ),
        (
            "arachne-codegen/tests/generated/st05/st05.ecore",
            "d43cae9a05bd099529d24894d2d7c6c7aac76e7ea380df4c4c28209f47132ec3",
        ),
        (
            "arachne-parser/rsc/AbstractEcore.ecore",
            "b6dde99121707ec0aeae484e9362568394eeeff54f00b976a5157ddb31ffc245",
        ),
        (
            "arachne-parser/rsc/bt.ecore",
            "fe18315d3ae37a6317691d133827b4f51d741ca661e42e44e36716d98be71204",
        ),
        (
            "arachne-parser/rsc/ExampleEcore.ecore",
            "0f3cdf919b06f7118850c0c396fc49fc156e26fd41f224889d76052825eb26cb",
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
        (
            "examples/SimpleUML.ecore",
            "21c50dea71938b861233b72116f5ead094fe1bbb3d67bf623864f52c3ead1c3f",
        ),
        (
            "examples/umlclass.ecore",
            "a44920adc661d308b7bbb9a8e4ccf1549a093546a241c0e33b28596de8000f3b",
        ),
    ];
    // The checked-in descriptors, the digest fixture and Arachne's defaults, file bytes at b9b85e7.
    const FILES: [(&str, &str); 7] = [
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
            "04ebb5c15dad375284d4e5e681a5e634130525c614517d373763e6040eead03f",
        ),
        (
            "examples/umlclass.metamodel.json",
            "a770d938ae503aff62f395b6f3cf241a4776459da5b2a04d5ba5e3cddc892edd",
        ),
        (
            "examples/uml.metamodel.json",
            "b000aa0d2b562a41763d3e77829fd9363224fdcade914d42846b9cb0c54690a5",
        ),
        (
            "examples/fixtures/metamodel-digests.json",
            "9d217db433c8a2d72b9badc5264869b3f2b65b84ad95cfe3047c79a244476879",
        ),
        (
            "arachne-codegen/src/codegen/datatype/to_crdt.rs",
            "2de8d2e8927b67d61f81d5055ac6a92809991aa259a96227e216602411dd4218",
        ),
    ];
    let repo = repo();
    let mut differing = Vec::new();
    for (ecore, today) in TODAY {
        let digest = describe(&repo.join(ecore))
            .map(|d| metamodel_digest(&d))
            .unwrap_or_else(|why| why);
        if digest != today {
            differing.push(format!("{ecore}: {digest}, today {today}"));
        }
    }
    for (file, today) in FILES {
        let sha = sha256_of(&repo.join(file)).unwrap_or_else(|why| why);
        if sha != today {
            differing.push(format!("{file}: sha256 {sha}, today {today}"));
        }
    }
    println!(
        "AR04-R4 descriptors compared {}, files compared {}, differing {}",
        TODAY.len(),
        FILES.len(),
        differing.len()
    );
    for line in &differing {
        println!("AR04-R4 {line}");
    }
    assert!(
        differing.is_empty(),
        "AR04 claim: another metamodel's descriptor, or a default, moved:\n  {}",
        differing.join("\n  ")
    );
}
