//! EC.01, parser fix 2: `eKeys` is accepted and its raw paths are kept on the feature, in document
//! order, split on whitespace as EMF writes several keys. They are not resolved.

use ecore_rs::ctx::Ctx;

const MODEL: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore" name="keyed" nsURI="urn:keyed" nsPrefix="keyed">
  <eClassifiers xsi:type="ecore:EClass" name="Catalog">
    <eStructuralFeatures xsi:type="ecore:EReference" name="items" upperBound="-1"
        eType="#//Item" containment="true" eKeys="#//Item/name"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="pairs" upperBound="-1"
        eType="#//Item" containment="true" eKeys="#//Item/name #//Item/code"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="plain" upperBound="-1"
        eType="#//Item" containment="true"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Item">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="code"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
  </eClassifiers>
</ecore:EPackage>
"##;

fn feature<'a>(ctx: &'a Ctx, class: &str, name: &str) -> &'a ecore_rs::repr::Structural {
    ctx.classes()
        .iter()
        .find(|c| c.name() == class)
        .unwrap_or_else(|| panic!("class `{class}` should exist"))
        .structural()
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("feature `{class}.{name}` should exist"))
}

#[test]
fn a_model_with_ekeys_parses() {
    Ctx::parse(MODEL).expect("a model using eKeys should parse");
}

#[test]
fn ekeys_are_kept_raw_in_document_order() {
    let ctx = Ctx::parse(MODEL).expect("the model should parse");
    assert_eq!(feature(&ctx, "Catalog", "items").keys(), ["#//Item/name"]);
    assert_eq!(
        feature(&ctx, "Catalog", "pairs").keys(),
        ["#//Item/name", "#//Item/code"]
    );
    assert!(feature(&ctx, "Catalog", "plain").keys().is_empty());
}
