/////////////////////////////////////////////////////////////////////////////
//  Copyright (c) 2025, 2026 CEA LIST.
//
//  This program and the accompanying materials
//  are made available under the terms of the Apache License, Version 2.0
//  which accompanies this distribution, and is available at
//  https://www.apache.org/licenses/LICENSE-2.0
//
//  SPDX-License-Identifier: Apache-2.0
//
/////////////////////////////////////////////////////////////////////////////

//! Under the generated crate's `serde` feature, every type the generator writes
//! for a log derives serde as the operation types do: the package log, the
//! projected logs and the enumerations the logs store.

use std::path::Path;

use arachne_codegen::{EcoreParser, generate_from_parser};

const SERDE: &str = r#"#[cfg_attr(feature="serde",derive(serde::Serialize,serde::Deserialize))]"#;

const EXAMPLES: [&str; 11] = [
    "SimpleUML",
    "behavior_tree",
    "class_hierarchy",
    "conference",
    "json",
    "pub_crawl",
    "pet_metamodels/abstract_inherits_concrete",
    "pet_metamodels/concrete_inherits_concrete",
    "pet_metamodels/concrete_polymorphic_targets",
    "pet_metamodels/kitchen_sink",
    "pet_metamodels/multiple_inheritance",
];

/// The four generated modules of an example, whitespace removed.
fn generated(example: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../examples/{example}.ecore"));
    let parser = EcoreParser::from_file(path).expect("example should parse");
    let pack = parser
        .ctx
        .packs()
        .iter()
        .find(|p| p.name() != "[root]" && p.name() != "[builtin]")
        .expect("package should exist");
    let (classifiers, references, package, read_as_ecore, _) =
        generate_from_parser(&parser, pack).expect("generation should succeed");
    [classifiers, references, package, read_as_ecore]
        .into_iter()
        .flat_map(|module| module.build().to_string().chars().collect::<Vec<_>>())
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// The names of the structs and enums declared without the serde attribute
/// among their outer attributes.
fn without_serde(code: &str) -> Vec<String> {
    let mut missing = Vec::new();
    for keyword in ["pubstruct", "pubenum"] {
        for (at, _) in code.match_indices(keyword) {
            let name: String = code[at + keyword.len()..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let mut attributes = &code[..at];
            let mut serde = false;
            while attributes.ends_with(']') {
                let start = attributes.rfind("#[").expect("an attribute opens");
                serde |= &attributes[start..] == SERDE;
                attributes = &attributes[..start];
            }
            if !serde {
                missing.push(name);
            }
        }
    }
    missing
}

#[test]
fn every_generated_log_type_derives_serde_behind_the_feature() {
    let missing: Vec<String> = EXAMPLES
        .iter()
        .flat_map(|example| {
            without_serde(&generated(example))
                .into_iter()
                // Rejections are not stored in a log; ReadAsEcore is a query marker.
                .filter(|name| !name.ends_with("Rejection") && name != "ReadAsEcore")
                .map(move |name| format!("{example}: {name}"))
        })
        .collect();
    assert!(missing.is_empty(), "types without serde: {missing:#?}");
}
