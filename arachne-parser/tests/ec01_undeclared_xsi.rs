//! EC.01, parser change 6: a document that uses the `xsi` prefix without declaring it is read as
//! if it declared `xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"`, and only then.
//!
//! Reproducer in ModelSet: `srijanighosh87/MasterThesisDev/com.kaleidoscope.core.aux.simpletree/
//! model/SimpleTree.ecore` writes `xsi:type` 22 times and declares only `xmi` and `ecore`.
//! roxmltree refuses the undeclared prefix; the parser of `akira/interpreter-modelstore` f90795e
//! never checked declarations and describes the file, so without this change the candidate loses
//! it (Cam, 2026-11-12: keep it).
//!
//! Only `xsi`, and only when the document declares it nowhere: a document that declares `xsi`
//! (to the standard namespace, to another one, or on one element only), and any other undeclared
//! prefix, behave exactly as in Léo's parser at ebc3141. The strings in `TODAY_*` are the errors
//! `Ctx::parse` returned at ebc3141 (roxmltree 0.21.1, as pinned in `Cargo.lock`).

use ecore_rs::ctx::Ctx;

const XSI_DECL: &str = r#" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#;

/// The shape of SimpleTree.ecore: `xsi:type` on every classifier and feature, no `xmlns:xsi`.
const UNDECLARED: &str = r##"<?xml version="1.0" encoding="ASCII"?>
<ecore:EPackage xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI" xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="tree" nsURI="urn:tree" nsPrefix="tree">
  <eClassifiers xsi:type="ecore:EClass" name="TreeElement" abstract="true">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name" lowerBound="1"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Node" eSuperTypes="#//TreeElement">
    <eStructuralFeatures xsi:type="ecore:EReference" name="children" upperBound="-1"
        eType="#//Node" containment="true"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="attribute" upperBound="-1"
        eType="#//Attribute" containment="true" eOpposite="#//Attribute/node"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="kind" eType="#//Kind"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Attribute" eSuperTypes="#//TreeElement">
    <eStructuralFeatures xsi:type="ecore:EReference" name="node" lowerBound="1" eType="#//Node"
        eOpposite="#//Node/attribute"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="value" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EEnum" name="Kind">
    <eLiterals name="folder"/>
    <eLiterals name="file" value="1"/>
  </eClassifiers>
</ecore:EPackage>
"##;

/// The same document with the standard declaration written on the root's first line, so every
/// later line and column is the same as in [`UNDECLARED`].
fn declared(model: &str) -> String {
    model.replacen(" xmlns:xmi=", &format!("{XSI_DECL} xmlns:xmi="), 1)
}

/// Every class as Léo's parser holds it (its `Debug`), in index order.
fn classes(ctx: &Ctx) -> Vec<String> {
    ctx.classes()
        .iter()
        .map(|class| {
            let sup: Vec<&str> = class.sup().iter().map(|idx| ctx[*idx].name()).collect();
            format!("{class:?} sup={sup:?}")
        })
        .collect()
}

fn error(model: &str) -> String {
    match Ctx::parse(model) {
        Ok(_) => "parsed".to_string(),
        Err(err) => err.to_string(),
    }
}

#[test]
fn an_undeclared_xsi_prefix_is_read_as_the_standard_namespace() {
    let ctx = Ctx::parse(UNDECLARED).expect("`xsi:type` without `xmlns:xsi` should parse");
    let twin = Ctx::parse(declared(UNDECLARED)).expect("the declared twin parses");
    assert_eq!(
        classes(&ctx),
        classes(&twin),
        "the undeclared document must parse to what its declared twin parses to"
    );
    for name in ["TreeElement", "Node", "Attribute", "Kind"] {
        assert!(
            ctx.classes().iter().any(|c| c.name() == name),
            "`{name}` should be parsed"
        );
    }

    // Positions in diagnostics are those of the input as given: an unsupported attribute on line
    // 18 is reported exactly as in the declared twin.
    let bad = UNDECLARED.replacen(r#"name="value""#, r#"name="value" bogus="1""#, 1);
    let got = error(&bad);
    assert_ne!(
        got, "parsed",
        "an unsupported attribute must still refuse the file"
    );
    assert_eq!(got, error(&declared(&bad)));
}

/// `foo:note` on line 20, an undeclared prefix other than `xsi`.
fn with_foo(model: &str) -> String {
    model.replacen(r#"name="Kind""#, r#"name="Kind" foo:note="x""#, 1)
}

const TODAY_FOO: &str =
    "Failed to parse XML for `ecore:EPackage`: an unknown namespace prefix 'foo' at 20:52";

#[test]
fn an_undeclared_prefix_other_than_xsi_is_still_refused() {
    // `xsi` declared, `foo` not: refused exactly as today.
    assert_eq!(error(&with_foo(&declared(UNDECLARED))), TODAY_FOO);
    // Neither declared: `xsi` is read as the standard namespace, `foo` is still refused, with
    // the same error and position as in the declared twin.
    assert_eq!(error(&with_foo(UNDECLARED)), TODAY_FOO);
}

const TODAY_OTHER_URI: &str = "Unsupported namespaced attribute `type` in `eClassifiers`\n\
    at `eClassifiers` named `TreeElement`, line 4, column 3\n\
    at `EPackage` named `tree`, line 2, column 1";
const TODAY_NESTED_ONLY: &str =
    "Failed to parse XML for `ecore:EPackage`: an unknown namespace prefix 'xsi' at 8:17";

#[test]
fn a_document_that_declares_xsi_is_read_as_today() {
    // Declared to the standard namespace on the root: parses, as Léo's whole suite does.
    Ctx::parse(declared(UNDECLARED)).expect("a declared `xsi` parses as today");

    // Declared to another namespace: `xsi:type` is not the standard attribute, as today.
    let other = UNDECLARED.replacen(
        " xmlns:xmi=",
        r#" xmlns:xsi="http://example.org/not-xsi" xmlns:xmi="#,
        1,
    );
    assert_eq!(error(&other), TODAY_OTHER_URI);

    // Declared on the first classifier only: `xsi:type` on a later classifier is outside its
    // scope and refused, as today.
    let nested = UNDECLARED.replacen(
        r#"<eClassifiers xsi:type="ecore:EClass" name="TreeElement""#,
        &format!(r#"<eClassifiers{XSI_DECL} xsi:type="ecore:EClass" name="TreeElement""#),
        1,
    );
    assert_eq!(error(&nested), TODAY_NESTED_ONLY);
}
