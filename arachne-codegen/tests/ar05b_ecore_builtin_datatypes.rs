//! AR.05b (`cea-cdrt-knowledge/spec/items/AR.05b.md`): AR.04's follow-ups.
//!
//! - G1, G2: for a synthetic metamodel whose package carries a `urn:arachne:semantics` detail
//!   for one of Ecore's own attributes, the field the generator emits for that attribute is the
//!   construction the descriptor's merge rule names, and the rule is the register the detail
//!   names; without the detail both keep text. Ground truth: the fixture's details, and the table
//!   in `construction` below, written here and not taken from the generator.
//! - H1: `examples/sysml/sysml.ecore`'s header comment no longer claims byte identity with
//!   SysON's file, says it is XML-equal once the `urn:arachne:semantics` annotations are removed,
//!   and names the 12 expanded elements; no byte outside that comment moved (sha256 pinned at
//!   `347cd8c`), and the checked-in descriptor is unchanged.
//! - D1, D2: the doc of `descriptor.rs`'s `add_semantics` and the paragraph of `semantics.rs`'s
//!   module doc that names the entry's `annotation` key point to `feature_datatype` and to the
//!   package-level detail.
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-codegen --test ar05b_ecore_builtin_datatypes
//! ```

use std::{
    fs,
    path::{Path, PathBuf},
};

use arachne_codegen::{EcoreParser, descriptor_json, find_user_package, generate_from_parser};
use serde_json::Value;
use sha2::{Digest, Sha256};

const FIXTURE: &str = "arachne-codegen/tests/fixtures/ar05b/builtin_datatypes.ecore";
/// The package annotation G2 removes, exactly as the fixture writes it.
const PACKAGE_ANNOTATION: &str = r#"  <eAnnotations source="urn:arachne:semantics">
    <details key="ecore::EAnnotation.source" value="mv-register"/>
    <details key="ecore::ENamedElement.name" value="lww-register"/>
  </eAnnotations>
"#;

const SYSML_ECORE: &str = "examples/sysml/sysml.ecore";
const SYSML_DESCRIPTOR: &str = "examples/sysml/sysml.metamodel.json";
const SYSML_DESCRIPTOR_SHA256: &str =
    "896ba4d758c6d8adfe9764bb7779f3400212c02afd97959012936d4e113d4e47";
/// `examples/sysml/sysml.ecore` at `347cd8c` with its header comment (from `<!--` through `-->`
/// and the newline after it) cut out.
const SYSML_ECORE_BUT_COMMENT_SHA256: &str =
    "2844b7ddd1cebb5c82bca8ab240620a0ba0af3e7f7112d5235af27e21fdd4db5";

const DESCRIPTOR_RS: &str = "arachne-codegen/src/codegen/descriptor.rs";
const SEMANTICS_RS: &str = "arachne-codegen/src/codegen/semantics.rs";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(relative: &str) -> String {
    let path = repo().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("set-up: {}: {e}", path.display()))
}

/// The descriptor and the classifiers the generator emits, for one `.ecore` text.
fn describe_and_generate(text: &str) -> (Value, String) {
    let parser = EcoreParser::from_string(text).expect("set-up: the fixture parses");
    let pack = find_user_package(&parser.ctx).expect("set-up: a user package");
    let descriptor = descriptor_json(&parser.ctx, pack).expect("set-up: describe");
    let (classifiers, _, _, _) = generate_from_parser(&parser, pack).expect("set-up: generate");
    let code: String = classifiers
        .build()
        .to_string()
        .split_whitespace()
        .collect::<String>()
        .replace("__classifiers::", "");
    (descriptor, code)
}

/// The descriptor entry of one attribute.
fn attribute<'a>(descriptor: &'a Value, class: &str, name: &str) -> &'a Value {
    descriptor["classes"][class]["attributes"]
        .as_array()
        .unwrap_or_else(|| panic!("the descriptor has no class `{class}`"))
        .iter()
        .find(|entry| entry["name"] == name)
        .unwrap_or_else(|| panic!("the descriptor has no attribute `{class}.{name}`"))
}

/// The field type the generator wrote for `field` in `record!(<record> { ... })`, whitespace
/// and the `__classifiers::` path removed.
fn emitted_field(code: &str, record: &str, field: &str) -> String {
    let start = code
        .find(&format!("record!({record}{{"))
        .unwrap_or_else(|| panic!("the generator emits no record `{record}`"));
    let body = &code[start..];
    let body = &body[..body.find("});").expect("a record ends")];
    let at = [format!("{{{field}:"), format!(",{field}:")]
        .iter()
        .find_map(|prefix| body.find(prefix.as_str()).map(|at| at + prefix.len()))
        .unwrap_or_else(|| panic!("record `{record}` has no field `{field}`: {body}"));
    let rest = &body[at..];
    rest[..rest.find("=>").expect("a field type ends at `=>`")].to_string()
}

/// The construction a single-valued string attribute's rule names, stated here rather than taken
/// from the generator: the leaf's log, inside `OptionLog` for an optional attribute.
fn construction(merge: &Value) -> String {
    assert_eq!(merge["kind"], "attribute", "not an attribute rule: {merge}");
    let leaf = &merge["leaf"];
    let leaf = match (leaf["kind"].as_str(), leaf["tie"].as_str()) {
        (Some("text"), _) => "GraphLog<List<char>>".to_string(),
        (Some("register"), Some("mv")) => "VecLog<MVRegister<std::string::String>>".to_string(),
        (Some("register"), Some("lww")) => "VecLog<LwwRegister<std::string::String>>".to_string(),
        _ => panic!("no construction stated for leaf {leaf}"),
    };
    match merge["shape"]["kind"].as_str() {
        Some("optional") => format!("OptionLog<{leaf}>"),
        Some("single") => leaf,
        _ => panic!("no construction stated for shape {}", merge["shape"]),
    }
}

/// `(descriptor class, generated record, attribute)` for the attributes G1 and G2 read.
const FIELDS: [(&str, &str, &str); 3] = [
    ("ecore::EAnnotation", "EcoreEAnnotation", "source"),
    ("ecore::ENamedElement", "EcoreENamedElement", "name"),
    ("Part", "Part", "label"),
];

fn check(text: &str, expected: [(Value, Value); 3]) {
    let (descriptor, code) = describe_and_generate(text);
    let mut failures = Vec::new();
    for ((class, record, name), (annotation, leaf)) in FIELDS.iter().zip(expected) {
        let entry = attribute(&descriptor, class, name);
        if entry["annotation"] != annotation || entry["merge"]["leaf"] != leaf {
            failures.push(format!(
                "descriptor `{class}.{name}`: annotation {} and leaf {}, expected {annotation} and {leaf}",
                entry["annotation"], entry["merge"]["leaf"]
            ));
        }
        let expected = construction(&entry["merge"]);
        let emitted = emitted_field(&code, record, name);
        if emitted != expected {
            failures.push(format!(
                "`{class}.{name}`: the descriptor names `{expected}` and the generator emits `{emitted}`"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "AR.05b claim:\n{}",
        failures.join("\n")
    );
}

/// G1: with the package detail, the descriptor names the register the detail names and the
/// generator emits that register; the metamodel's own `label` stays text on both sides.
#[test]
fn ar05b_g1_the_generator_emits_the_register_a_package_detail_names() {
    let text = read(FIXTURE);
    assert_eq!(
        text.matches(PACKAGE_ANNOTATION).count(),
        1,
        "set-up: the fixture carries its package annotation once"
    );
    check(
        &text,
        [
            (
                Value::from("mv-register"),
                serde_json::json!({"kind": "register", "tie": "mv"}),
            ),
            (
                Value::from("lww-register"),
                serde_json::json!({"kind": "register", "tie": "lww"}),
            ),
            (Value::Null, serde_json::json!({"kind": "text"})),
        ],
    );
}

/// G2: the same metamodel without the package annotation: text on both sides, for all three.
#[test]
fn ar05b_g2_without_the_detail_the_generator_and_the_descriptor_keep_text() {
    let text = read(FIXTURE).replacen(PACKAGE_ANNOTATION, "", 1);
    assert!(
        !text.contains("eAnnotations"),
        "set-up: the package annotation was removed"
    );
    let text_leaf = || (Value::Null, serde_json::json!({"kind": "text"}));
    check(&text, [text_leaf(), text_leaf(), text_leaf()]);
}

/// Lower case, every run of whitespace or hyphens one space.
fn normalized(text: &str) -> String {
    text.to_lowercase()
        .replace('-', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// H1: the header comment of the annotated SysML copy says what is true, and nothing else in the
/// file or its descriptor moved.
#[test]
fn ar05b_h1_the_sysml_header_comment_says_xml_equal_not_byte_for_byte() {
    let bytes = fs::read(repo().join(SYSML_ECORE)).expect("set-up: the annotated ecore");
    let text = String::from_utf8(bytes.clone()).expect("set-up: UTF-8");
    let package = text.find("<ecore:EPackage").expect("set-up: a package");
    let start = text[..package]
        .find("<!--")
        .expect("AR.05b claim: the file keeps a header comment before its package");
    let end = start + text[start..].find("-->").expect("a comment ends") + "-->".len();
    assert!(
        !text[end..package].contains("<!--"),
        "AR.05b claim: one header comment"
    );
    let comment = normalized(&text[start + "<!--".len()..end - "-->".len()]);

    let mut failures = Vec::new();
    if !text.is_ascii() {
        failures.push("the file declares encoding ASCII and holds a non-ASCII byte".to_string());
    }
    let mut from = 0;
    while let Some(at) = comment[from..].find("byte for byte") {
        let at = from + at;
        if !comment[..at].ends_with("not ") {
            failures.push(format!(
                "the comment still claims byte identity: `{}`",
                &comment[at.saturating_sub(40)..(at + 13).min(comment.len())]
            ));
        }
        from = at + 1;
    }
    for (needle, why) in [
        ("xml", "it does not say the file is XML-equal to SysON's"),
        (
            "urn:arachne:semantics",
            "it does not name the annotations removed",
        ),
    ] {
        if !comment.contains(needle) {
            failures.push(format!("the comment lacks `{needle}`: {why}"));
        }
    }
    if !(comment.contains("12") || comment.contains("twelve")) {
        failures.push("the comment does not name the 12 expanded elements".to_string());
    }

    let mut rest = bytes[..start].to_vec();
    let after = if bytes.get(end) == Some(&b'\n') {
        end + 1
    } else {
        end
    };
    rest.extend_from_slice(&bytes[after..]);
    let rest_sha = format!("{:x}", Sha256::digest(&rest));
    if rest_sha != SYSML_ECORE_BUT_COMMENT_SHA256 {
        failures.push(format!(
            "a byte outside the header comment moved: sha256 {rest_sha}, expected {SYSML_ECORE_BUT_COMMENT_SHA256}"
        ));
    }
    let descriptor = fs::read(repo().join(SYSML_DESCRIPTOR)).expect("set-up: the descriptor");
    let descriptor_sha = format!("{:x}", Sha256::digest(&descriptor));
    if descriptor_sha != SYSML_DESCRIPTOR_SHA256 {
        failures.push(format!(
            "{SYSML_DESCRIPTOR} moved: sha256 {descriptor_sha}, expected {SYSML_DESCRIPTOR_SHA256}"
        ));
    }
    assert!(
        failures.is_empty(),
        "AR.05b claim:\n{}\ncomment: {comment}",
        failures.join("\n")
    );
}

/// The `///` doc block right above the line that starts with `item`, as one normalized string.
fn doc_above(source: &str, item: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.starts_with(item))
        .unwrap_or_else(|| panic!("set-up: no line starts with `{item}`"));
    let mut doc: Vec<&str> = lines[..at]
        .iter()
        .rev()
        .take_while(|line| line.trim_start().starts_with("///"))
        .map(|line| line.trim_start().trim_start_matches("///"))
        .collect();
    doc.reverse();
    doc.join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// D1: `add_semantics`'s doc says where the `annotation` key's value comes from.
#[test]
fn ar05b_d1_add_semantics_doc_points_to_feature_datatype() {
    let doc = doc_above(&read(DESCRIPTOR_RS), "fn add_semantics(");
    let mut failures = Vec::new();
    if !doc.contains("feature_datatype") {
        failures.push("it does not point to `feature_datatype`");
    }
    if !doc.to_lowercase().contains("package") {
        failures.push("it does not say a package-level detail can govern the value");
    }
    if doc.contains("string it wrote or `null`") {
        failures.push("it still says the value is the string the file wrote on the feature");
    }
    assert!(
        failures.is_empty(),
        "AR.05b claim: add_semantics's doc: {}\ndoc: {doc}",
        failures.join("; ")
    );
}

/// D2: `semantics.rs`'s module doc points to `feature_datatype` and the package-level detail,
/// and so does every paragraph of it that says what the entry's `annotation` key carries.
#[test]
fn ar05b_d2_semantics_module_doc_points_to_feature_datatype() {
    let source = read(SEMANTICS_RS);
    let module_doc: Vec<&str> = source
        .lines()
        .take_while(|line| line.starts_with("//!"))
        .map(|line| line.trim_start_matches("//!").trim())
        .collect();
    let paragraphs: Vec<String> = module_doc
        .split(|line| line.is_empty())
        .map(|lines| lines.join(" "))
        .collect();
    assert!(
        paragraphs
            .iter()
            .any(|paragraph| paragraph.contains("feature_datatype")
                && paragraph.to_lowercase().contains("package")),
        "AR.05b claim: no paragraph of semantics.rs's module doc points to `feature_datatype` \
         and the package-level detail"
    );
    for paragraph in paragraphs.iter().filter(|paragraph| {
        paragraph.contains("`annotation` key") || paragraph.contains("verbatim")
    }) {
        assert!(
            paragraph.contains("feature_datatype") && paragraph.to_lowercase().contains("package"),
            "AR.05b claim: semantics.rs's module doc says what the `annotation` key carries \
             without pointing to `feature_datatype` and the package-level detail: {paragraph}"
        );
    }
}
