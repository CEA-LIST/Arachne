use ecore_rs::ctx::Ctx;

const MODEL: &str = r##"<?xml version='1.0'?>
<?tool keep='this'?>
<m:EPackage xmlns:m='http://www.eclipse.org/emf/2002/Ecore'
    xmlns:t='http://www.w3.org/2001/XMLSchema-instance'
    xmlns:z='http://www.omg.org/XMI' z:version='2.0' name='adapter'>
    <!-- Forward references, aliased QNames and reordered attributes. -->
    <eClassifiers name='Child' t:type='m:EClass' eSuperTypes='#//Base'>
        <eAnnotations references='#//Base&#x20;#//Child' source='notes'>
            <details value='List&lt;𝔸&gt; &amp; &quot;»&quot; &#10;' key='documentation'></details>
        </eAnnotations>
        <eStructuralFeatures name='label' t:type='m:EAttribute' lowerBound='1'
            eType='m:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString'
            defaultValueLiteral='&apos;» &amp; ℕ &#x1D538;&apos;'/>
        <eStructuralFeatures name='parent' t:type='m:EReference' eType='#//Base'/>
        <eOperations name='lookup' eType='#//Base' ordered='0' unique='1'>
            <eParameters name='node' eType='#//Base'></eParameters>
        </eOperations>
    </eClassifiers>
    <eClassifiers t:type='m:EClass' name='Base' abstract='1'/>
    <eSubpackages name='nested' nsURI='urn:nested' nsPrefix='nested'>
        <eClassifiers t:type='m:EClass' name='Inner'/>
    </eSubpackages>
</m:EPackage>"##;

#[test]
fn parses_aliases_entities_and_forward_links() {
    // Run with both a prefixed root and a default Ecore namespace.
    for xml in [
        MODEL.to_string(),
        MODEL
            .replace("m:EPackage", "EPackage")
            .replace(
                "xmlns:m=",
                "xmlns='http://www.eclipse.org/emf/2002/Ecore' xmlns:m=",
            )
            .replace("m:EDataType ", "EDataType "),
    ] {
        let ctx = Ctx::parse(xml).expect("well-formed Ecore should parse");
        let child = ctx.classes().iter().find(|c| c.name() == "Child").unwrap();
        let base = ctx.classes().iter().find(|c| c.name() == "Base").unwrap();
        assert!(base.is_abstract());
        assert!(child.sup().contains(&base.idx));
        assert!(base.sub().contains(&child.idx));
        let annotation = &child.annotations()[0];
        assert_eq!(annotation.references(), ["#//Base", "#//Child"]);
        assert_eq!(annotation.details()["documentation"], "List<𝔸> & \"»\" \n");
        let label = &child.structural()[0];
        assert_eq!(ctx.class(label.typ.unwrap()).name(), "EString");
        assert_eq!(label.default_value_literal.as_deref(), Some("'» & ℕ 𝔸'"));
        assert_eq!(label.bounds.lbound, 1);
        assert_eq!(child.structural()[1].typ, Some(base.idx));
        let operation = &child.operations()[0];
        assert_eq!(operation.typ(), Some(base.idx));
        assert_eq!(operation.ordered(), Some(false));
        assert_eq!(operation.unique(), Some(true));
        assert_eq!(operation.parameters()[0].typ(), Some(base.idx));
        assert!(ctx
            .packs()
            .iter()
            .any(|p| p.name() == "nested" && p.ns_uri() == Some("urn:nested")));
        assert!(ctx.classes().iter().any(|c| c.name() == "Inner"));
    }
}

#[test]
fn rejects_malformed_xml_without_panicking() {
    for xml in [
        MODEL.replace("</m:EPackage>", "</m:EClass>"),
        MODEL.replace("name='adapter'", "name='adapter' name='duplicate'"),
        MODEL.replace("List&lt;", "List&unknown;"),
        MODEL.replace("</m:EPackage>", "</m:EPackage><extra/>"),
        MODEL.replace("List&lt;", "List<"),
        MODEL.replace("xmlns:t=", "xmlns:other="),
    ] {
        let result = std::panic::catch_unwind(|| Ctx::parse(&xml));
        let error = result
            .expect("invalid Unicode XML must not panic")
            .err()
            .expect("malformed XML must be rejected")
            .to_string();
        assert!(error.contains("Failed to parse XML"), "{error}");
        assert!(
            error.contains(" at "),
            "syntax errors should include source position: {error}"
        );
    }
}

#[test]
fn rejects_unsupported_ecore_with_element_positions() {
    for (xml, message) in [
        (
            MODEL.replace("t:type='m:EClass'", "t:type='z:EClass'"),
            "Unsupported Ecore type",
        ),
        (
            MODEL.replace("name='Child'", "name='Child' mystery='yes'"),
            "Unsupported attribute",
        ),
        (
            MODEL.replace("abstract='1'", "abstract='maybe'"),
            "Expected boolean",
        ),
        (
            MODEL.replace("lowerBound='1'", "lowerBound='NaN'"),
            "lower bound",
        ),
        (
            MODEL.replace(
                "<!-- Forward references, aliased QNames and reordered attributes. -->",
                "<eTypeParameters name='T'/>",
            ),
            "Unsupported package element",
        ),
        (
            MODEL.replace(
                "xmlns:m='http://www.eclipse.org/emf/2002/Ecore'",
                "xmlns:m='urn:wrong'",
            ),
            "ecore:EPackage",
        ),
        (
            MODEL.replace("m:EDataType ", "z:EDataType "),
            "Unsupported Ecore type",
        ),
        (
            MODEL
                .replace(
                    "xmlns:z='http://www.omg.org/XMI'",
                    "xmlns:ecore='urn:wrong' xmlns:z='http://www.omg.org/XMI'",
                )
                .replace("m:EDataType ", "ecore:EDataType "),
            "Unsupported Ecore type",
        ),
    ] {
        let error = Ctx::parse(xml)
            .err()
            .expect("unsupported Ecore must be rejected")
            .to_string();
        assert!(error.contains(message), "expected {message:?}: {error}");
        assert!(
            error.contains("line ") && error.contains("column "),
            "{error}"
        );
    }
}

#[test]
fn rejects_duplicate_annotation_keys() {
    let xml = MODEL.replace(
        "</eAnnotations>",
        "<details key='documentation' value='duplicate'/></eAnnotations>",
    );
    let error = Ctx::parse(xml)
        .err()
        .expect("duplicate keys must be rejected")
        .to_string();
    assert!(
        error.contains("annotation details key/value binding"),
        "{error}"
    );
}
