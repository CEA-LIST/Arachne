//! The enum literals of an Ecore file as it writes them.
//!
//! The parser keeps one text per literal, `value` or else `literal`, so which of the two a file
//! wrote is read here, from the XML, and written under `enumLiterals` in the descriptor.

use std::collections::BTreeMap;

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
    if let Ok(document) = Document::parse(text) {
        package(document.root_element(), "", &mut found);
    }
    found
}

fn package(node: Node, path: &str, found: &mut Literals) {
    for child in node.children().filter(Node::is_element) {
        let name = child.attribute("name").unwrap_or("");
        match child.tag_name().name() {
            "EPackage" | "eSubpackages" => package(child, &format!("{path}{name}::"), found),
            "eClassifiers"
                if child.attributes().any(|a| {
                    a.name() == "type" && a.value().rsplit(':').next() == Some("EEnum")
                }) =>
            {
                let literals = child
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
                found.entry(format!("{path}{name}")).or_insert(literals);
            }
            _ => {}
        }
    }
}
