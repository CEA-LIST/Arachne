//! EC.01, parser fix 5: a `#//a/X` fragment is resolved from the document root, as EMF spells
//! every fragment, not from the package being parsed.
//!
//! Reproducer in ModelSet: `wrwei/OMG-specification-implementations/omg.spem/model/spem.ecore`
//! lines 383 to 386, where inside `<eSubpackages name="uma">` a containment reads
//! `eType="#//uma/Artifact"`. Every path here names a package already open or defined, so no
//! package is forward-referenced: Léo's refusal of a forward-referenced package that a later
//! element defines (`rejects_interpackage_links`, kept by Cam on 2026-11-12) is not reached.

use ecore_rs::ctx::Ctx;

const MODEL: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore" name="spem" nsURI="urn:spem" nsPrefix="spem">
  <eClassifiers xsi:type="ecore:EClass" name="Same"/>
  <eClassifiers xsi:type="ecore:EClass" name="Top"/>
  <eSubpackages name="uma" nsURI="urn:spem/uma" nsPrefix="uma">
    <eClassifiers xsi:type="ecore:EClass" name="Same"/>
    <eClassifiers xsi:type="ecore:EClass" name="Artifact">
      <eStructuralFeatures xsi:type="ecore:EReference" name="containedArtifact" upperBound="-1"
          eType="#//uma/Artifact" containment="true"/>
      <eStructuralFeatures xsi:type="ecore:EReference" name="top" eType="#//Top"/>
      <eStructuralFeatures xsi:type="ecore:EReference" name="rootSame" eType="#//Same"/>
      <eStructuralFeatures xsi:type="ecore:EReference" name="umaSame" eType="#//uma/Same"/>
    </eClassifiers>
    <eSubpackages name="inner" nsURI="urn:spem/uma/inner" nsPrefix="inner">
      <eClassifiers xsi:type="ecore:EClass" name="Deep">
        <eStructuralFeatures xsi:type="ecore:EReference" name="back" eType="#//uma/Artifact"/>
      </eClassifiers>
    </eSubpackages>
  </eSubpackages>
</ecore:EPackage>
"##;

/// The type of `owner.feature`, as `(package name, class name)`.
fn resolved(ctx: &Ctx, owner: &str, feature: &str) -> (String, String) {
    let owner = ctx
        .classes()
        .iter()
        .find(|c| c.name() == owner && c.structural().iter().any(|f| f.name == feature))
        .unwrap_or_else(|| panic!("class `{owner}` with `{feature}` should exist"));
    let typ = owner
        .structural()
        .iter()
        .find(|f| f.name == feature)
        .unwrap()
        .typ
        .unwrap_or_else(|| panic!("`{feature}` should have a resolved type"));
    let class = &ctx[typ];
    (
        ctx[class.path.last()].name().to_string(),
        class.name().to_string(),
    )
}

fn pair(pack: &str, class: &str) -> (String, String) {
    (pack.to_string(), class.to_string())
}

#[test]
fn a_root_relative_path_into_the_current_subpackage_parses() {
    Ctx::parse(MODEL).expect("`#//uma/Artifact` inside `uma` should parse");
}

#[test]
fn every_fragment_resolves_from_the_document_root() {
    let ctx = Ctx::parse(MODEL).expect("the model should parse");
    assert_eq!(
        resolved(&ctx, "Artifact", "containedArtifact"),
        pair("uma", "Artifact")
    );
    assert_eq!(resolved(&ctx, "Artifact", "top"), pair("spem", "Top"));
    assert_eq!(resolved(&ctx, "Artifact", "rootSame"), pair("spem", "Same"));
    assert_eq!(resolved(&ctx, "Artifact", "umaSame"), pair("uma", "Same"));
    assert_eq!(resolved(&ctx, "Deep", "back"), pair("uma", "Artifact"));
}

#[test]
fn a_path_to_a_missing_class_is_still_refused() {
    let broken = MODEL.replace("eType=\"#//Top\"", "eType=\"#//uma/Missing\"");
    assert!(
        Ctx::parse(broken).is_err(),
        "a fragment naming no class must still refuse the file"
    );
}
