use arachne_codegen::{EcoreParser, codegen::analysis::GenerationPlan, generate_from_parser};

fn parser(classes: &str) -> EcoreParser {
    EcoreParser::from_string(&format!(
        r#"<ecore:EPackage xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
            xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" name="unsupported">
            {classes}
        </ecore:EPackage>"#
    ))
    .unwrap()
}

fn modules(classes: &str) -> ([String; 4], Vec<String>) {
    let parser = parser(classes);
    let pack = parser
        .ctx
        .packs()
        .iter()
        .find(|p| p.name() == "unsupported")
        .unwrap();
    let (classifiers, references, package, xmi, _) = generate_from_parser(&parser, pack).unwrap();
    let generators = [classifiers, references, package, xmi];
    let warnings = generators
        .iter()
        .flat_map(|g| g.warnings())
        .map(|w| w.message())
        .collect();
    let code = generators.map(|g| {
        g.build()
            .to_string()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    });
    (code, warnings)
}

const INT: &str = "ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt";

#[test]
fn unsupported_properties_skip_fields_and_xmi_but_keep_each_warning_once() {
    for properties in [
        "changeable=\"false\"",
        "derived=\"true\"",
        "volatile=\"true\"",
        "transient=\"true\"",
        "unsettable=\"true\"",
        "resolveProxies=\"true\"",
        "defaultValue=\"7\"",
        "defaultValueLiteral=\"7\"",
    ] {
        let (code, warnings) = modules(&format!(
            r##"
            <eClassifiers xsi:type="ecore:EClass" name="Root">
                <eStructuralFeatures xsi:type="ecore:EAttribute" name="omitted" {properties} lowerBound="1" eType="{INT}"/>
                <eStructuralFeatures xsi:type="ecore:EAttribute" name="visible" lowerBound="1" eType="{INT}"/>
            </eClassifiers>
            <eClassifiers xsi:type="ecore:EClass" name="Child" eSuperTypes="#//Root"/>
        "##
        ));
        assert!(code[0].contains("visible:"));
        assert!(code[3].contains(".visible()"));
        assert!(code.iter().all(|c| !c.contains("omitted")), "{code:?}");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("Root.omitted") && warnings[0].contains("was skipped"));
    }
}

#[test]
fn supported_boolean_values_do_not_skip_features() {
    let (code, warnings) = modules(&format!(
        r#"
        <eClassifiers xsi:type="ecore:EClass" name="Root">
            <eStructuralFeatures xsi:type="ecore:EAttribute" name="visible" lowerBound="1" eType="{INT}"
                changeable="true" derived="false" volatile="false" transient="false"
                unsettable="false" resolveProxies="false"/>
        </eClassifiers>
    "#
    ));
    assert!(code[0].contains("visible:"));
    assert!(code[3].contains(".visible()"));
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn skipped_containment_does_not_affect_roots_or_boxing() {
    let classes = r##"
        <eClassifiers xsi:type="ecore:EClass" name="Root">
            <eStructuralFeatures xsi:type="ecore:EReference" name="omitted" containment="true" eType="#//Child" derived="true"/>
        </eClassifiers>
        <eClassifiers xsi:type="ecore:EClass" name="Child">
            <eStructuralFeatures xsi:type="ecore:EReference" name="parent" containment="true" eType="#//Root"/>
        </eClassifiers>
    "##;
    let parser = parser(classes);
    let pack = parser
        .ctx
        .packs()
        .iter()
        .find(|p| p.name() == "unsupported")
        .unwrap();
    let plan = GenerationPlan::new(&parser.ctx, pack).unwrap();
    assert_eq!(plan.roots.len(), 1);
    assert_eq!(parser.ctx.class(plan.roots[0]).name(), "Child");
    let (code, warnings) = modules(classes);
    assert!(!code[0].contains("omitted:"));
    assert!(!code[0].contains("BoxedLog"));
    assert!(!code[3].contains(".omitted()"));
    assert_eq!(warnings.len(), 1);
}

#[test]
fn skipped_graph_references_have_no_edges_or_xmi_links() {
    let (code, warnings) = modules(
        r##"
        <eClassifiers xsi:type="ecore:EClass" name="Root">
            <eStructuralFeatures xsi:type="ecore:EReference" name="omitted" eType="#//Root" derived="true"/>
            <eStructuralFeatures xsi:type="ecore:EReference" name="visible" eType="#//Root"/>
        </eClassifiers>
    "##,
    );
    assert!(code[1].contains("RootVisibleEdge"));
    assert!(!code[1].contains("RootOmittedEdge"));
    assert!(!code[3].contains("root::omitted"));
    assert!(code[3].contains("root::visible"));
    assert_eq!(warnings.len(), 1);
}

#[test]
fn skipped_transparent_field_falls_back_to_record() {
    let (code, warnings) = modules(&format!(
        r##"
        <eClassifiers xsi:type="ecore:EClass" name="Base" abstract="true"/>
        <eClassifiers xsi:type="ecore:EClass" name="Owner" eSuperTypes="#//Base">
            <eAnnotations source="urn:arachne:representation">
                <details key="kind" value="transparent"/><details key="field" value="omitted"/>
            </eAnnotations>
            <eStructuralFeatures xsi:type="ecore:EAttribute" name="omitted" derived="true" lowerBound="1" eType="{INT}"/>
            <eStructuralFeatures xsi:type="ecore:EAttribute" name="visible" lowerBound="1" eType="{INT}"/>
        </eClassifiers>
    "##
    ));
    assert!(code[0].contains("visible:") && code[0].contains("OwnerLog"));
    assert!(!code[0].contains("typeBaseOwner="));
    assert!(!code[3].contains(".omitted()"));
    assert!(code[3].contains(".visible()"));
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("ordinary record")));
}

#[test]
fn maps_with_skipped_keys_or_values_are_skipped_with_dependency_warnings() {
    for dependency in ["key", "value"] {
        let (code, warnings) = modules(&format!(
            r##"
            <eClassifiers xsi:type="ecore:EClass" name="Root">
                <eStructuralFeatures xsi:type="ecore:EReference" name="entries" containment="true" upperBound="-1" eType="#//Entry">
                    <eAnnotations source="urn:arachne:semantics"><details key="datatype" value="uw-map"/></eAnnotations>
                </eStructuralFeatures>
            </eClassifiers>
            <eClassifiers xsi:type="ecore:EClass" name="Entry">
                <eStructuralFeatures xsi:type="ecore:EAttribute" name="key" lowerBound="1" eType="{INT}" {key}/>
                <eStructuralFeatures xsi:type="ecore:EAttribute" name="value" lowerBound="1" eType="{INT}" {value}/>
            </eClassifiers>
        "##,
            key = if dependency == "key" {
                "derived=\"true\""
            } else {
                ""
            },
            value = if dependency == "value" {
                "derived=\"true\""
            } else {
                ""
            }
        ));
        assert!(!code[0].contains("entries:"));
        assert!(!code[0].contains("UWMap"));
        assert!(!code[3].contains(".entries()"));
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("Root.entries") && w.contains(&format!("Entry.{dependency}")))
        );
    }
}

#[test]
fn skipped_features_bypass_codegen_type_validation_and_keep_all_reasons() {
    let (code, warnings) = modules(
        r#"
        <eClassifiers xsi:type="ecore:EClass" name="Root">
            <eStructuralFeatures xsi:type="ecore:EAttribute" name="omitted" derived="true" volatile="true"
                eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//Object"/>
        </eClassifiers>
    "#,
    );
    assert!(!code[0].contains("omitted"));
    assert_eq!(warnings.len(), 2, "{warnings:?}");
}
