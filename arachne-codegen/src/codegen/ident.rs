/// This module contains functions for generating valid Rust identifiers from arbitrary strings,
/// ensuring that they do not conflict with Rust keywords or other generated identifiers.
use ecore_rs::{
    ctx::Ctx,
    repr::{Class, idx},
};
use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::Span;
use syn::Ident;

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "become", "box", "break", "const", "continue", "crate", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let",
    "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];
const MACRO_GENERATED_TYPE_SUFFIXES: &[&str] =
    &["Value", "Log", "Child", "ChildValue", "Container"];
const CLASH_RESOLUTION_SUFFIXES: &str = "Model";

pub fn rust_ident(name: impl AsRef<str>) -> Ident {
    let mut name = sanitize_ident(name.as_ref());
    if RUST_KEYWORDS.contains(&name.as_str()) {
        name.push('_');
    }
    Ident::new(&name, Span::call_site())
}

pub fn type_ident(name: &str) -> Ident {
    let name = name.to_upper_camel_case();
    rust_ident(if name.is_empty() {
        "Unnamed".to_string()
    } else {
        name
    })
}

pub fn type_ident_with_suffix(name: &str, suffix: &str) -> Ident {
    let name = name.to_upper_camel_case();
    let name = if name.is_empty() {
        "Unnamed".to_string()
    } else {
        name
    };
    rust_ident(format!("{name}{suffix}"))
}

pub fn classifier_type_ident(ctx: &Ctx, class: &Class) -> Ident {
    rust_ident(classifier_type_name(ctx, class.idx))
}

pub fn classifier_type_ident_with_suffix(ctx: &Ctx, class: &Class, suffix: &str) -> Ident {
    rust_ident(format!(
        "{}{}",
        classifier_type_name(ctx, class.idx),
        suffix
    ))
}

pub fn value_ident(name: &str) -> Ident {
    let name = name.to_snake_case();
    let mut name = if name.is_empty() {
        "unnamed".to_string()
    } else {
        name
    };
    if RUST_KEYWORDS.contains(&name.as_str()) {
        name.push_str("_field");
    }
    rust_ident(name)
}

pub fn value_ident_with_suffix(name: &str, suffix: &str) -> Ident {
    let name = name.to_snake_case();
    let name = if name.is_empty() {
        "unnamed".to_string()
    } else {
        name
    };
    rust_ident(format!("{name}_{suffix}"))
}

fn sanitize_ident(name: &str) -> String {
    let mut out = String::new();

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }

    let trimmed = out.trim_matches('_');
    let mut out = if trimmed.is_empty() {
        "unnamed".to_string()
    } else {
        trimmed.to_string()
    };

    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out.insert(0, '_');
    }

    out
}

fn classifier_type_name(ctx: &Ctx, class_idx: idx::Class) -> String {
    let mut names = ctx
        .classes()
        .iter()
        .map(|class| (class.idx, type_ident(class.name()).to_string()))
        .collect::<Vec<_>>();

    let mut changed = true;
    while changed {
        changed = false;

        for i in 0..names.len() {
            let original = names[i].1.clone();
            let mut candidate = names[i].1.clone();
            let mut attempt = 0;

            while classifier_name_conflicts(i, &candidate, &names) {
                attempt += 1;
                candidate = if attempt == 1 {
                    format!("{original}{CLASH_RESOLUTION_SUFFIXES}")
                } else {
                    format!("{original}{CLASH_RESOLUTION_SUFFIXES}{attempt}")
                };
            }

            if names[i].1 != candidate {
                names[i].1 = candidate;
                changed = true;
            }
        }
    }

    names
        .into_iter()
        .find_map(|(idx, name)| (idx == class_idx).then_some(name))
        .unwrap_or_else(|| "Unnamed".to_string())
}

fn classifier_name_conflicts(
    candidate_index: usize,
    candidate: &str,
    names: &[(idx::Class, String)],
) -> bool {
    names
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != candidate_index)
        .any(|(_, (_, name))| {
            candidate == name
                || MACRO_GENERATED_TYPE_SUFFIXES
                    .iter()
                    .any(|suffix| candidate == format!("{name}{suffix}"))
        })
}

#[cfg(test)]
mod tests {
    use ecore_rs::ctx::Ctx;

    use super::{
        classifier_type_ident, classifier_type_ident_with_suffix, type_ident, value_ident,
    };

    #[test]
    fn avoids_rust_keywords() {
        assert_eq!(value_ident("type").to_string(), "type_field");
        assert_eq!(value_ident("self").to_string(), "self_field");
        assert_eq!(type_ident("Self").to_string(), "Self_");
        assert_eq!(value_ident("gen").to_string(), "gen_field");
    }

    #[test]
    fn sanitizes_invalid_identifier_characters() {
        assert_eq!(value_ident("1 invalid-name").to_string(), "_1_invalid_name");
        assert_eq!(type_ident("_").to_string(), "Unnamed");
    }

    #[test]
    fn avoids_names_generated_by_record_macros() {
        let ctx = Ctx::parse(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="collision"
    nsURI="http://example.org/collision"
    nsPrefix="collision">
    <eClassifiers xsi:type="ecore:EClass" name="Query">
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="name" lowerBound="1" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="QueryValue">
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="value" lowerBound="1" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    </eClassifiers>
</ecore:EPackage>
"#,
        )
        .expect("ecore should parse");

        let query = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Query")
            .expect("Query should exist");
        let query_value = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "QueryValue")
            .expect("QueryValue should exist");

        assert_eq!(classifier_type_ident(&ctx, query).to_string(), "Query");
        assert_eq!(
            classifier_type_ident(&ctx, query_value).to_string(),
            "QueryValueModel"
        );
        assert_eq!(
            classifier_type_ident_with_suffix(&ctx, query_value, "Log").to_string(),
            "QueryValueModelLog"
        );
    }
}
