use arachne_codegen::{EcoreParser, generate_from_parser};

fn model(feature: &str, helpers: &str, transparent: bool) -> String {
    let (target, inheritance, representation) = if transparent {
        (
            "Base",
            "eSuperTypes=\"#//Base\"",
            r#"<eAnnotations source="urn:arachne:representation">
                <details key="kind" value="transparent"/>
                <details key="field" value="values"/>
            </eAnnotations>"#,
        )
    } else {
        ("Owner", "", "")
    };
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
        <ecore:EPackage xmlns:xmi="http://www.omg.org/XMI"
            xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
            xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
            name="mappings" nsURI="urn:mappings" nsPrefix="mappings">
            <eClassifiers xsi:type="ecore:EClass" name="Root">
                <eStructuralFeatures xsi:type="ecore:EReference" name="item"
                    lowerBound="1" containment="true" eType="#//{target}"/>
            </eClassifiers>
            <eClassifiers xsi:type="ecore:EClass" name="Base" abstract="true"/>
            <eClassifiers xsi:type="ecore:EClass" name="Owner" {inheritance}>
                {representation}{feature}
            </eClassifiers>
            {helpers}
        </ecore:EPackage>
        "##
    )
}

fn modules(model: &str) -> anyhow::Result<(String, String)> {
    let parser = EcoreParser::from_string(model)?;
    let pack = parser
        .ctx
        .packs()
        .iter()
        .find(|p| p.name() == "mappings")
        .unwrap();
    let (classifiers, _, _, xmi, _) = generate_from_parser(&parser, pack)?;
    let normalize = |code: String| code.chars().filter(|c| !c.is_whitespace()).collect();
    Ok((
        normalize(classifiers.build().to_string()),
        normalize(xmi.build().to_string()),
    ))
}

fn attribute(properties: &str, typ: &str, annotation: &str) -> String {
    format!(
        r#"<eStructuralFeatures xsi:type="ecore:EAttribute" name="values"
        {properties} eType="{typ}">{annotation}</eStructuralFeatures>"#
    )
}

const INT: &str = "ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt";
const ENUM: &str = r#"<eClassifiers xsi:type="ecore:EEnum" name="Choice">
    <eLiterals name="First"/><eLiterals name="Second" value="1"/>
</eClassifiers>"#;
const MAP: &str = r##"<eStructuralFeatures xsi:type="ecore:EReference" name="values"
    upperBound="-1" containment="true" eType="#//Entry">
    <eAnnotations source="urn:arachne:semantics"><details key="datatype" value="uw-map"/></eAnnotations>
</eStructuralFeatures>"##;

#[test]
fn optional_transparent_attributes_and_containments_use_optional_commands() {
    for feature in [
        attribute("lowerBound=\"0\"", INT, ""),
        r##"<eStructuralFeatures xsi:type="ecore:EReference" name="values"
            eType="#//Node" containment="true"/>"##
            .to_string(),
    ] {
        let helpers = r#"<eClassifiers xsi:type="ecore:EClass" name="Node"/>"#;
        let (classifiers, _) = modules(&model(&feature, helpers, true)).unwrap();
        assert!(
            classifiers.contains("typeBaseOwner=moirai_crdt::option::Optional<"),
            "{classifiers}"
        );
    }
}

#[test]
fn collections_use_the_same_storage_and_xmi_read_types_in_both_representations() {
    for (properties, log, value) in [
        (
            "unique=\"true\" ordered=\"true\"",
            "GraphLog<__classifiers::List<i32>>",
            "Vec<i32>",
        ),
        (
            "unique=\"false\" ordered=\"true\"",
            "NestedListLog<__classifiers::VecLog<__classifiers::Counter<i32>>>",
            "Vec<i32>",
        ),
        (
            "unique=\"false\" ordered=\"false\"",
            "AWBagLog<i32>",
            "rustc_hash::FxHashMap<i32,usize>",
        ),
        (
            "unique=\"true\" ordered=\"false\"",
            "VecLog<__classifiers::AWSet<i32>>",
            "rustc_hash::FxHashSet<i32>",
        ),
    ] {
        let feature = attribute(&format!("upperBound=\"-1\" {properties}"), INT, "");
        for transparent in [false, true] {
            let (classifiers, xmi) = modules(&model(&feature, "", transparent)).unwrap();
            assert!(
                classifiers.contains(&format!("__classifiers::{log}")),
                "{classifiers}"
            );
            assert!(xmi.contains(&format!("xmi_read::<_,{value}>")), "{xmi}");
        }
    }
}

#[test]
fn uw_map_values_must_be_required_and_single_in_both_representations() {
    for bounds in [
        "lowerBound=\"0\"",
        "lowerBound=\"1\" upperBound=\"-1\"",
        "lowerBound=\"1\" upperBound=\"2\"",
    ] {
        for kind in ["ecore:EAttribute", "ecore:EReference"] {
            let (typ, containment) = if kind == "ecore:EAttribute" {
                (INT, "")
            } else {
                ("#//Node", "containment=\"true\"")
            };
            let entry = format!(
                r##"<eClassifiers xsi:type="ecore:EClass" name="Entry">
                <eStructuralFeatures xsi:type="ecore:EAttribute" name="key" lowerBound="1" eType="{INT}"/>
                <eStructuralFeatures xsi:type="{kind}" name="value" {bounds} {containment} eType="{typ}"/>
            </eClassifiers><eClassifiers xsi:type="ecore:EClass" name="Node"/>"##
            );
            for transparent in [false, true] {
                let error = modules(&model(MAP, &entry, transparent)).unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains("required and single-valued (1..1)"),
                    "{error}"
                );
            }
        }
    }
}

#[test]
fn xmi_qualifies_enum_values_in_both_representations() {
    for properties in [
        "lowerBound=\"1\"",
        "lowerBound=\"0\"",
        "upperBound=\"-1\" unique=\"false\"",
    ] {
        let feature = attribute(properties, "#//Choice", "");
        for transparent in [false, true] {
            let (_, xmi) = modules(&model(&feature, ENUM, transparent)).unwrap();
            assert!(
                xmi.contains("rustc_hash::FxHashSet<__read_as_ecore::Choice>"),
                "{xmi}"
            );
        }
    }
}
