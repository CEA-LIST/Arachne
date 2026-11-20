//! The enum literals of an Ecore file as it writes them.
//!
//! The parser keeps one text per literal, `value` or else `literal`, so which of the two a file
//! wrote is read here, from the XML, and written under `enumLiterals` in the descriptor.

use std::collections::{BTreeMap, BTreeSet};

use roxmltree::{Document, Node};
use serde_json::{Value, json};

/// The `{name, value, literal}` list of each enum, by the key the descriptor writes it under.
pub(crate) type Literals = BTreeMap<String, Vec<Value>>;

/// An EInt `value` as a number, any other text as it is.
pub(crate) fn value_json(value: &str) -> Value {
    value
        .parse::<i32>()
        .map_or_else(|_| json!(value), |n| json!(n))
}

/// The enums of the package `text` declares and of its subpackages, by root-relative key. A text
/// roxmltree refuses has none: the parser answers for it.
pub(crate) fn enum_literals(text: &str) -> Literals {
    let mut found = Literals::new();
    classifiers(text, &mut |key, node| {
        if has_type(node, "EEnum") {
            let literals = node
                .children()
                .filter(|c| c.is_element() && c.tag_name().name() == "eLiterals")
                .map(|c| {
                    json!({
                        "name": c.attribute("name"),
                        "value": c.attribute("value").map(value_json),
                        "literal": c.attribute("literal"),
                    })
                })
                .collect();
            found.entry(key).or_insert(literals);
        }
    });
    found
}

/// The keys of the datatypes `text` declares that carry nothing but a name, an `instanceClassName`
/// and annotations of a source the descriptor reads: the descriptor says all of such a datatype.
pub(crate) fn plain_datatypes(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    classifiers(text, &mut |key, node| {
        let attributes = node.attributes().all(|a| {
            matches!(
                (a.namespace(), a.name()),
                (Some(XSI), "type") | (None, "name" | "instanceClassName")
            )
        });
        let children = node.children().filter(Node::is_element).all(|c| {
            c.tag_name().name() == "eAnnotations"
                && matches!(
                    c.attribute("source"),
                    Some(
                        "urn:arachne:semantics"
                            | "urn:arachne:representation"
                            | "subsets"
                            | "redefines"
                    )
                )
        });
        if has_type(node, "EDataType") && attributes && children {
            found.insert(key);
        }
    });
    found
}

const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";

fn has_type(node: Node, local: &str) -> bool {
    node.attributes()
        .any(|a| a.name() == "type" && a.value().rsplit(':').next() == Some(local))
}

/// Calls `visit` with the root-relative key and element of every classifier of the package `text`
/// declares and of its subpackages.
fn classifiers(text: &str, visit: &mut impl FnMut(String, Node)) {
    if let Ok(document) = Document::parse(text) {
        package(document.root_element(), "", visit);
    }
}

fn package(node: Node, path: &str, visit: &mut impl FnMut(String, Node)) {
    for child in node.children().filter(Node::is_element) {
        let name = child.attribute("name").unwrap_or("");
        match child.tag_name().name() {
            "EPackage" | "eSubpackages" => package(child, &format!("{path}{name}::"), visit),
            "eClassifiers" => visit(format!("{path}{name}"), child),
            _ => {}
        }
    }
}
