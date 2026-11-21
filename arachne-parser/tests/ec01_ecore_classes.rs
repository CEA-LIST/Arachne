//! EC.01, parser fix 1: Ecore's own classes are accepted as types and as supertypes.
//!
//! `EObject`, `EModelElement`, `ENamedElement`, `EAnnotation` and `EStringToStringMapEntry`,
//! named by Ecore's nsURI (`http://www.eclipse.org/emf/2002/Ecore#//X`, with or without a leading
//! `ecore:EClass`) or by the plugin spelling
//! (`platform:/plugin/org.eclipse.emf.ecore/model/Ecore.ecore#//X`), resolve to classes of a
//! package `ecore` that the context adds the first time a metamodel names one. `EObject` is
//! concrete and `EModelElement` and `ENamedElement` are abstract, as `Ecore.ecore` declares them.
//! A metamodel that names none of them gets no such package.

use ecore_rs::ctx::Ctx;

const MODEL: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore" name="meta" nsURI="urn:meta" nsPrefix="meta">
  <eClassifiers xsi:type="ecore:EClass" name="Element"
      eSuperTypes="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EModelElement">
    <eStructuralFeatures xsi:type="ecore:EReference" name="target"
        eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EObject"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="notes" upperBound="-1"
        eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EAnnotation" containment="true"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="props" upperBound="-1"
        eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EStringToStringMapEntry"
        containment="true"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Named"
      eSuperTypes="platform:/plugin/org.eclipse.emf.ecore/model/Ecore.ecore#//ENamedElement #//Element"/>
  <eClassifiers xsi:type="ecore:EClass" name="Root">
    <eStructuralFeatures xsi:type="ecore:EReference" name="elements" upperBound="-1"
        eType="#//Element" containment="true"/>
  </eClassifiers>
</ecore:EPackage>
"##;

const PLAIN: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore" name="plain" nsURI="urn:plain" nsPrefix="plain">
  <eClassifiers xsi:type="ecore:EClass" name="Root"/>
</ecore:EPackage>
"##;

fn class<'a>(ctx: &'a Ctx, name: &str) -> &'a ecore_rs::repr::Class {
    ctx.classes()
        .iter()
        .find(|c| c.name() == name)
        .unwrap_or_else(|| panic!("class `{name}` should exist"))
}

fn feature_type<'a>(ctx: &'a Ctx, owner: &str, feature: &str) -> &'a ecore_rs::repr::Class {
    let typ = class(ctx, owner)
        .structural()
        .iter()
        .find(|f| f.name == feature)
        .unwrap_or_else(|| panic!("feature `{owner}.{feature}` should exist"))
        .typ
        .unwrap_or_else(|| panic!("`{owner}.{feature}` should have a resolved type"));
    &ctx[typ]
}

#[test]
fn a_model_naming_ecore_classes_parses() {
    Ctx::parse(MODEL).expect("a model naming Ecore's own classes should parse");
}

#[test]
fn ecore_classes_resolve_as_feature_types() {
    let ctx = Ctx::parse(MODEL).expect("the model should parse");
    for (feature, expected) in [
        ("target", "EObject"),
        ("notes", "EAnnotation"),
        ("props", "EStringToStringMapEntry"),
    ] {
        let typ = feature_type(&ctx, "Element", feature);
        assert_eq!(typ.name(), expected);
        assert!(
            ctx.is_ecore_class(typ.idx),
            "`{expected}` is one of Ecore's classes"
        );
    }
}

#[test]
fn ecore_classes_resolve_as_supertypes_in_both_spellings() {
    let ctx = Ctx::parse(MODEL).expect("the model should parse");
    // Sorted: the order of supertypes is fix 4's business, not this one's.
    let names = |name: &str| -> Vec<String> {
        let mut names: Vec<String> = class(&ctx, name)
            .sup()
            .iter()
            .map(|sup| ctx[*sup].name().to_string())
            .collect();
        names.sort();
        names
    };
    assert_eq!(names("Element"), ["EModelElement"]);
    assert_eq!(names("Named"), ["ENamedElement", "Element"]);
}

#[test]
fn ecore_classes_are_abstract_as_ecore_declares_them() {
    let ctx = Ctx::parse(MODEL).expect("the model should parse");
    assert!(
        !feature_type(&ctx, "Element", "target").is_abstract(),
        "EObject is concrete"
    );
    let sup = class(&ctx, "Element").sup().iter().next().copied().unwrap();
    assert!(ctx[sup].is_abstract(), "EModelElement is abstract");
    let ecore = ctx.ecore_pack().expect("the model names Ecore's classes");
    assert_eq!(ctx[ecore].name(), "ecore");
}

#[test]
fn a_model_naming_no_ecore_class_gets_no_ecore_package() {
    let ctx = Ctx::parse(PLAIN).expect("the plain model should parse");
    assert!(ctx.ecore_pack().is_none());
    assert!(ctx.packs().iter().all(|p| p.name() != "ecore"));
}
