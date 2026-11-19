//! EC.01, parser fix 3: `eOpposite` is kept on the structural feature as written, not dropped.
//!
//! The value is raw (`#//Class/feature`) and unresolved; nothing keeps the two ends in step. The
//! warning about it stays (Léo's `names_the_feature_when_dropping_eopposite` and the CLI
//! diagnostics test check its wording), only the value is no longer lost.

use ecore_rs::ctx::Ctx;

const MODEL: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore" name="school" nsURI="urn:school" nsPrefix="school">
  <eClassifiers xsi:type="ecore:EClass" name="Teacher">
    <eStructuralFeatures xsi:type="ecore:EReference" name="advises" upperBound="-1"
        eType="#//Student" eOpposite="#//Student/advisor"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="office" eType="#//Room"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Student">
    <eStructuralFeatures xsi:type="ecore:EReference" name="advisor" eType="#//Teacher"
        eOpposite="#//Teacher/advises"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Room"/>
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
fn eopposite_is_kept_as_written() {
    let ctx = Ctx::parse(MODEL).expect("the model should parse");
    assert_eq!(
        feature(&ctx, "Teacher", "advises").opposite(),
        Some("#//Student/advisor")
    );
    assert_eq!(
        feature(&ctx, "Student", "advisor").opposite(),
        Some("#//Teacher/advises")
    );
}

#[test]
fn a_reference_without_eopposite_has_none() {
    let ctx = Ctx::parse(MODEL).expect("the model should parse");
    assert_eq!(feature(&ctx, "Teacher", "office").opposite(), None);
}
