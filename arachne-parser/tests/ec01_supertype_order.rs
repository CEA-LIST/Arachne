//! EC.01, parser fix 4: a class keeps its supertypes in the order `eSuperTypes` declares them.
//!
//! EMF's feature inheritance order and a generated record's `_super` fields follow declaration
//! order. Here `Both` declares `#//Second #//First`, while `First` is declared (and indexed) before
//! `Second`, so an index-ordered set yields `First, Second` and declaration order is
//! `Second, First`.

use ecore_rs::ctx::Ctx;

const MODEL: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore" name="order" nsURI="urn:order" nsPrefix="order">
  <eClassifiers xsi:type="ecore:EClass" name="First" abstract="true"/>
  <eClassifiers xsi:type="ecore:EClass" name="Second" abstract="true"/>
  <eClassifiers xsi:type="ecore:EClass" name="Third" abstract="true"/>
  <eClassifiers xsi:type="ecore:EClass" name="Both" eSuperTypes="#//Second #//First"/>
  <eClassifiers xsi:type="ecore:EClass" name="Three" eSuperTypes="#//Third #//First #//Second"/>
  <eClassifiers xsi:type="ecore:EClass" name="Forward" eSuperTypes="#//Later #//First"/>
  <eClassifiers xsi:type="ecore:EClass" name="Later" abstract="true"/>
</ecore:EPackage>
"##;

fn supertypes(ctx: &Ctx, class: &str) -> Vec<String> {
    let class = ctx
        .classes()
        .iter()
        .find(|c| c.name() == class)
        .unwrap_or_else(|| panic!("class `{class}` should exist"));
    class
        .sup()
        .iter()
        .map(|sup| ctx[*sup].name().to_string())
        .collect()
}

#[test]
fn supertypes_keep_declaration_order() {
    let ctx = Ctx::parse(MODEL).expect("the model should parse");
    assert_eq!(supertypes(&ctx, "Both"), ["Second", "First"]);
    assert_eq!(supertypes(&ctx, "Three"), ["Third", "First", "Second"]);
}

#[test]
fn a_forward_referenced_supertype_keeps_its_place() {
    let ctx = Ctx::parse(MODEL).expect("the model should parse");
    assert_eq!(supertypes(&ctx, "Forward"), ["Later", "First"]);
}
