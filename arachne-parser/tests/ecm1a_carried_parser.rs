//! EC.M1 piece 1 (`cea-cdrt-knowledge/spec/items/EC.M1.md`, section 5): the parser tests of our
//! line that the merge removes with `arachne-parser/src/ctx.rs`, carried as they were inline at
//! `f90795e` (lines 1571 to 2327 of its `#[cfg(test)] mod tests`), run here against the merged
//! parser through its public API. Changes from `f90795e`, and nothing else: `crate::` is
//! `ecore_rs::`; the imports below; the pretty prints are read from `fixtures/ecm1a/pretty/`
//! (byte for byte `f90795e:arachne-parser/rsc/pretty/`).

use ecore_rs::{ctx::Ctx, repr::Class};

const ECORE_NS_URI: &str = "http://www.eclipse.org/emf/2002/Ecore";

/// The five classes of Ecore that metamodels use, written as `Ecore.ecore` declares them,
/// except that `EString` is the builtin datatype and `EObject` has none of its operations,
/// whose types are Ecore classes outside these five.
const ECORE_SUBSET: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore" name="ecore" nsURI="http://www.eclipse.org/emf/2002/Ecore" nsPrefix="ecore">
  <eClassifiers xsi:type="ecore:EClass" name="EAnnotation" eSuperTypes="#//EModelElement">
<eAnnotations source="http://www.eclipse.org/emf/2002/Ecore">
  <details key="constraints" value="WellFormed WellFormedSourceURI"/>
</eAnnotations>
<eStructuralFeatures xsi:type="ecore:EAttribute" name="source" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
<eStructuralFeatures xsi:type="ecore:EReference" name="details" upperBound="-1"
    eType="#//EStringToStringMapEntry" containment="true" resolveProxies="false"/>
<eStructuralFeatures xsi:type="ecore:EReference" name="eModelElement" eType="#//EModelElement"
    transient="true" resolveProxies="false" eOpposite="#//EModelElement/eAnnotations"/>
<eStructuralFeatures xsi:type="ecore:EReference" name="contents" upperBound="-1"
    eType="#//EObject" containment="true" resolveProxies="false"/>
<eStructuralFeatures xsi:type="ecore:EReference" name="references" upperBound="-1"
    eType="#//EObject"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="EModelElement" abstract="true">
<eOperations name="getEAnnotation" eType="#//EAnnotation">
  <eParameters name="source" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
</eOperations>
<eStructuralFeatures xsi:type="ecore:EReference" name="eAnnotations" upperBound="-1"
    eType="#//EAnnotation" containment="true" resolveProxies="false" eOpposite="#//EAnnotation/eModelElement"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="ENamedElement" abstract="true" eSuperTypes="#//EModelElement">
<eAnnotations source="http://www.eclipse.org/emf/2002/Ecore">
  <details key="constraints" value="WellFormedName"/>
</eAnnotations>
<eStructuralFeatures xsi:type="ecore:EAttribute" name="name" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="EObject"/>
  <eClassifiers xsi:type="ecore:EClass" name="EStringToStringMapEntry" instanceClassName="java.util.Map$Entry">
<eStructuralFeatures xsi:type="ecore:EAttribute" name="key" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
<eStructuralFeatures xsi:type="ecore:EAttribute" name="value" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
  </eClassifiers>
</ecore:EPackage>
"##;

/// A user metamodel whose `Element` extends Ecore's `EModelElement`, as SysON's does, and
/// whose `Part` has features typed by Ecore classes.
const EXTENDS_ECORE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
xmlns:xmi="http://www.omg.org/XMI"
xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
name="test"
nsURI="http://example.org/test"
nsPrefix="test">
<eClassifiers xsi:type="ecore:EClass" name="Element" abstract="true"
    eSuperTypes="http://www.eclipse.org/emf/2002/Ecore#//EModelElement">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="elementId"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
</eClassifiers>
<eClassifiers xsi:type="ecore:EClass" name="Part" eSuperTypes="#//Element">
    <eStructuralFeatures xsi:type="ecore:EReference" name="payload" upperBound="-1"
        eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EObject" containment="true"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="notes" upperBound="-1"
        eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EAnnotation"/>
</eClassifiers>
</ecore:EPackage>
"##;

fn class_named<'ctx>(ctx: &'ctx Ctx, pack: ecore_rs::repr::idx::Pack, name: &str) -> &'ctx Class {
    ctx.classes()
        .iter()
        .find(|class| class.path.last() == pack && class.name() == name)
        .unwrap_or_else(|| panic!("no class `{name}` in package `{}`", ctx[pack].name()))
}

fn pack_named(ctx: &Ctx, name: &str) -> ecore_rs::repr::idx::Pack {
    ctx.packs()
        .iter()
        .find(|pack| pack.name() == name)
        .unwrap_or_else(|| panic!("no package `{name}`"))
        .idx
}

/// Everything the context holds about a class, with classes named by package and name
/// instead of by index.
fn describe_class(ctx: &Ctx, class: &Class) -> String {
    use std::fmt::Write;

    let name = |idx: Option<ecore_rs::repr::idx::Class>| {
        idx.map(|idx| format!("{}/{}", ctx[ctx[idx].path.last()].name(), ctx[idx].name()))
    };
    let annots = |out: &mut String, annots: &[ecore_rs::repr::Annot]| {
        for annot in annots {
            let mut details: Vec<_> = annot.details().iter().collect();
            details.sort();
            writeln!(
                out,
                "  annotation {:?} {details:?} {:?}",
                annot.source(),
                annot.references()
            )
            .unwrap();
        }
    };

    let mut out = String::new();
    writeln!(
        out,
        "{} {} abstract={} interface={} inst_name={:?} instance_class_name={:?} literals={}",
        class.name(),
        class.typ(),
        class.is_abstract(),
        class.is_interface(),
        class.inst_name(),
        class.instance_class_name(),
        class.literals().len(),
    )
    .unwrap();
    let sups: Vec<_> = class.sup().iter().map(|idx| name(Some(*idx))).collect();
    writeln!(out, "  supertypes {sups:?}").unwrap();
    annots(&mut out, class.annotations());
    for s in class.structural() {
        writeln!(
            out,
            "  feature {} {} {:?} {:?} {} containment={} iD={} ordered={:?} changeable={:?} \
            volatile={:?} transient={:?} derived={:?} unsettable={:?} unique={:?} \
            default={:?}/{:?} resolve_proxies={:?}",
            s.name,
            s.kind,
            name(s.typ),
            s.typ_path,
            s.bounds,
            s.containment,
            s.is_id,
            s.ordered,
            s.changeable,
            s.volatile,
            s.transient,
            s.derived,
            s.unsettable,
            s.unique,
            s.default_value,
            s.default_value_literal,
            s.resolve_proxies,
        )
        .unwrap();
        annots(&mut out, s.annotations());
    }
    for op in class.operations() {
        writeln!(
            out,
            "  operation {} {:?} {} ordered={:?} unique={:?}",
            op.name(),
            name(op.typ()),
            op.bounds(),
            op.ordered(),
            op.unique(),
        )
        .unwrap();
        for param in op.parameters() {
            writeln!(
                out,
                "    parameter {} {:?} {} ordered={:?} unique={:?}",
                param.name(),
                name(param.typ()),
                param.bounds(),
                param.ordered(),
                param.unique(),
            )
            .unwrap();
        }
        annots(&mut out, op.annotations());
    }
    out
}

macro_rules! example {
    ($ecore:literal, $pretty:literal) => {
        (
            $ecore,
            include_str!(concat!("../../", $ecore)),
            include_str!(concat!("fixtures/ecm1a/pretty/", $pretty)),
        )
    };
}

/// The pretty print of each example as the parser printed it before Ecore's own classes were
/// built in, except that `multiple_inheritance.ecore` lists the supertypes of `C` in declared
/// order (`aas.ecore` is left out: it refers to UML and is refused).
const EXAMPLES: [(&str, &str, &str); 12] = [
    example!("examples/behavior_tree.ecore", "behavior_tree.pretty"),
    example!("examples/class_hierarchy.ecore", "class_hierarchy.pretty"),
    example!("examples/conference.ecore", "conference.pretty"),
    example!("examples/json.ecore", "json.pretty"),
    example!(
        "examples/pet_metamodels/abstract_inherits_concrete.ecore",
        "abstract_inherits_concrete.pretty"
    ),
    example!(
        "examples/pet_metamodels/concrete_inherits_concrete.ecore",
        "concrete_inherits_concrete.pretty"
    ),
    example!(
        "examples/pet_metamodels/concrete_polymorphic_targets.ecore",
        "concrete_polymorphic_targets.pretty"
    ),
    example!(
        "examples/pet_metamodels/kitchen_sink.ecore",
        "kitchen_sink.pretty"
    ),
    example!(
        "examples/pet_metamodels/multiple_inheritance.ecore",
        "multiple_inheritance.pretty"
    ),
    example!(
        "arachne-parser/rsc/AbstractEcore.ecore",
        "AbstractEcore.pretty"
    ),
    example!("arachne-parser/rsc/bt.ecore", "bt.pretty"),
    example!(
        "arachne-parser/rsc/ExampleEcore.ecore",
        "ExampleEcore.pretty"
    ),
];

/// A metamodel that never refers to an Ecore class must not see any of them: same packages,
/// same class indices, same pretty print.
#[test]
fn examples_without_ecore_classes_parse_as_before() {
    for (path, ecore, pretty) in EXAMPLES {
        let ctx = Ctx::parse(ecore).unwrap_or_else(|e| panic!("{path} refused: {e}"));
        assert!(
            ctx.packs()
                .iter()
                .all(|pack| pack.ns_uri() != Some(ECORE_NS_URI)),
            "{path} has a package for Ecore"
        );
        assert_eq!(
            format!("{}\n", ctx.to_pretty_string()),
            pretty,
            "{path} does not parse as before"
        );
    }
}

/// `resolve_etype` knows Ecore's datatypes and nothing else of Ecore, so a class extending
/// `EModelElement` or a feature typed by `EObject` is refused.
#[test]
fn injects_ecore_classes_once_when_referenced() {
    let ctx = Ctx::parse(EXTENDS_ECORE).unwrap_or_else(|e| panic!("refused: {e}"));

    let ecore_packs: Vec<_> = ctx
        .packs()
        .iter()
        .filter(|pack| pack.ns_uri() == Some(ECORE_NS_URI))
        .collect();
    assert_eq!(ecore_packs.len(), 1, "expected one Ecore package");
    let ecore = ecore_packs[0];
    assert_eq!(ecore.name(), "ecore");
    assert_eq!(ecore.ns_prefix(), Some("ecore"));
    assert_eq!(ecore.sup(), Some(ctx.top_pack()));
    assert!(ctx[ctx.top_pack()].has_sub(ecore.idx));
    let mut names: Vec<_> = ecore.classes().iter().map(|idx| ctx[*idx].name()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "EAnnotation",
            "EModelElement",
            "ENamedElement",
            "EObject",
            "EStringToStringMapEntry"
        ]
    );
    assert_eq!(ctx[ctx.builtin_pack()].classes().len(), 10);

    let test = pack_named(&ctx, "test");
    let element = class_named(&ctx, test, "Element");
    let sups: Vec<_> = element.sup().iter().copied().collect();
    assert_eq!(sups, [class_named(&ctx, ecore.idx, "EModelElement").idx]);
    assert_eq!(
        element.structural()[0].typ,
        Some(ctx.builtins()[&ecore_rs::repr::builtin::Typ::EString])
    );

    let part = class_named(&ctx, test, "Part");
    let typ = |feature: &str| {
        part.structural()
            .iter()
            .find(|candidate| candidate.name == feature)
            .and_then(|feature| feature.typ)
    };
    assert_eq!(
        typ("payload"),
        Some(class_named(&ctx, ecore.idx, "EObject").idx)
    );
    assert_eq!(
        typ("notes"),
        Some(class_named(&ctx, ecore.idx, "EAnnotation").idx)
    );
}

/// The classes built in for Ecore hold what the parser would hold for them if it read their
/// declarations in `Ecore.ecore`.
#[test]
fn built_in_ecore_classes_match_their_declarations() {
    let injected = Ctx::parse(EXTENDS_ECORE).unwrap_or_else(|e| panic!("refused: {e}"));
    let declared = Ctx::parse(ECORE_SUBSET).unwrap_or_else(|e| panic!("refused: {e}"));
    let injected_pack = pack_named(&injected, "ecore");
    let declared_pack = pack_named(&declared, "ecore");

    for name in [
        "EAnnotation",
        "EModelElement",
        "ENamedElement",
        "EObject",
        "EStringToStringMapEntry",
    ] {
        assert_eq!(
            describe_class(&injected, class_named(&injected, injected_pack, name)),
            describe_class(&declared, class_named(&declared, declared_pack, name)),
        );
    }
}

/// A metamodel that is Ecore itself names its own classes by Ecore's URI; they must resolve
/// to the file's classes and no second Ecore package may appear.
#[test]
fn ecore_itself_resolves_ecore_uris_to_its_own_classes() {
    let probe = r##"<eClassifiers xsi:type="ecore:EClass" name="Probe" eSuperTypes="http://www.eclipse.org/emf/2002/Ecore#//EModelElement">
<eStructuralFeatures xsi:type="ecore:EReference" name="payload" eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EObject"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="EAnnotation""##;
    let ecore = ECORE_SUBSET.replacen(
        r#"<eClassifiers xsi:type="ecore:EClass" name="EAnnotation""#,
        probe,
        1,
    );
    let ctx = Ctx::parse(&ecore).unwrap_or_else(|e| panic!("refused: {e}"));

    assert_eq!(
        ctx.packs()
            .iter()
            .filter(|pack| pack.ns_uri() == Some(ECORE_NS_URI))
            .count(),
        1,
        "a second Ecore package was added"
    );
    let own = pack_named(&ctx, "ecore");
    let probe = class_named(&ctx, own, "Probe");
    let sups: Vec<_> = probe.sup().iter().copied().collect();
    assert_eq!(sups, [class_named(&ctx, own, "EModelElement").idx]);
    assert_eq!(
        probe.structural()[0].typ,
        Some(class_named(&ctx, own, "EObject").idx)
    );
}

/// A user metamodel naming Ecore's classes in the given `eSuperTypes` and `eType` spellings.
fn naming_ecore(super_types: &str, etype: &str) -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
xmlns:xmi="http://www.omg.org/XMI"
xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
name="test"
nsURI="http://example.org/test"
nsPrefix="test">
<eClassifiers xsi:type="ecore:EClass" name="Element" eSuperTypes="{super_types}">
    <eStructuralFeatures xsi:type="ecore:EReference" name="payload" eType="{etype}"/>
</eClassifiers>
</ecore:EPackage>
"##
    )
}

fn ecore_with_annotation_value(value: &str) -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
xmlns:xmi="http://www.omg.org/XMI"
xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
name="test"
nsURI="http://example.org/test"
nsPrefix="test">
<eClassifiers xsi:type="ecore:EClass" name="Root">
    <eAnnotations source="http://www.eclipse.org/emf/2002/GenModel">
        <details key="documentation" value="{value}"/>
    </eAnnotations>
</eClassifiers>
</ecore:EPackage>
"##
    )
}

/// `until_char_is` advances its byte cursor by one per `char` and then slices
/// the source by that cursor, so a multi-byte character near the end of an
/// attribute value puts the slice end inside the character and the parse
/// panics with "is not a char boundary".
#[test]
fn parses_a_non_ascii_attribute_value_ending_in_a_multibyte_char() {
    for value in ["»", "«»x", "ℕx", "the «Focus»."] {
        let ecore = ecore_with_annotation_value(value);
        let parsed = std::panic::catch_unwind(|| Ctx::parse(&ecore).map(|_| ()));
        match parsed {
            Ok(Ok(())) => {}
            Ok(Err(e)) => panic!("value {value:?} refused: {e:?}"),
            Err(panic) => panic!(
                "value {value:?} panicked: {}",
                panic.downcast_ref::<String>().cloned().unwrap_or_default()
            ),
        }
    }
}

/// `resolve_etype` recognises Ecore's classes by Ecore's namespace URI only, so the
/// `platform:/plugin` location of `Ecore.ecore`, which the Ecore editor inserts by default and
/// SysON's metamodel uses, is refused.
#[test]
fn resolves_the_platform_spelling_of_ecore_classes() {
    const NS_URI: &str = "http://www.eclipse.org/emf/2002/Ecore#//";
    const PLATFORM: &str = "platform:/plugin/org.eclipse.emf.ecore/model/Ecore.ecore#//";

    for (super_types, etype) in [
        (format!("{NS_URI}EModelElement"), format!("{NS_URI}EObject")),
        (
            format!("{PLATFORM}EModelElement"),
            format!("ecore:EClass {PLATFORM}EObject"),
        ),
        (
            format!("{PLATFORM}EModelElement"),
            format!("{PLATFORM}EObject"),
        ),
        (
            format!("{PLATFORM}EModelElement {NS_URI}ENamedElement"),
            format!("ecore:EClass {NS_URI}EObject"),
        ),
    ] {
        let ecore = naming_ecore(&super_types, &etype);
        let ctx = Ctx::parse(&ecore)
            .unwrap_or_else(|e| panic!("`{super_types}` and `{etype}` refused: {e}"));
        let ecore_packs: Vec<_> = ctx
            .packs()
            .iter()
            .filter(|pack| pack.ns_uri() == Some(ECORE_NS_URI))
            .map(|pack| pack.idx)
            .collect();
        assert_eq!(ecore_packs.len(), 1, "`{super_types}`: one Ecore package");
        let ecore = ecore_packs[0];

        let element = class_named(&ctx, pack_named(&ctx, "test"), "Element");
        let sups: Vec<_> = element.sup().iter().map(|idx| ctx[*idx].name()).collect();
        let expected: Vec<_> = super_types
            .split_whitespace()
            .map(|token| token.rsplit('/').next().unwrap())
            .collect();
        assert_eq!(sups, expected, "`{super_types}`");
        assert!(element
            .sup()
            .iter()
            .all(|idx| ctx[*idx].path.last() == ecore));
        assert_eq!(
            element.structural()[0].typ,
            Some(class_named(&ctx, ecore, "EObject").idx),
            "`{etype}`"
        );
    }
}

/// Ecore itself may name its classes by the `platform:/plugin` location of `Ecore.ecore`,
/// which is the file itself.
#[test]
fn ecore_itself_resolves_platform_uris_to_its_own_classes() {
    let probe = r##"<eClassifiers xsi:type="ecore:EClass" name="Probe" eSuperTypes="platform:/plugin/org.eclipse.emf.ecore/model/Ecore.ecore#//EModelElement">
<eStructuralFeatures xsi:type="ecore:EReference" name="payload" eType="ecore:EClass platform:/plugin/org.eclipse.emf.ecore/model/Ecore.ecore#//EObject"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="EAnnotation""##;
    let ecore = ECORE_SUBSET.replacen(
        r#"<eClassifiers xsi:type="ecore:EClass" name="EAnnotation""#,
        probe,
        1,
    );
    let ctx = Ctx::parse(&ecore).unwrap_or_else(|e| panic!("refused: {e}"));

    assert_eq!(ctx.packs().len(), 3, "a package was added to Ecore itself");
    let own = pack_named(&ctx, "ecore");
    let probe = class_named(&ctx, own, "Probe");
    let sups: Vec<_> = probe.sup().iter().copied().collect();
    assert_eq!(sups, [class_named(&ctx, own, "EModelElement").idx]);
    assert_eq!(
        probe.structural()[0].typ,
        Some(class_named(&ctx, own, "EObject").idx)
    );
}

/// A name of Ecore outside the classes built in falls through to the generic refusal, which
/// says `eType` for a supertype and does not say which of Ecore's classes are supported.
#[test]
fn refuses_other_ecore_classes_naming_the_supported_ones() {
    for (super_types, etype, class) in [
        (
            "http://www.eclipse.org/emf/2002/Ecore#//EClass",
            "ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EObject",
            "EClass",
        ),
        (
            "http://www.eclipse.org/emf/2002/Ecore#//EModelElement",
            "ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EReference",
            "EReference",
        ),
        (
            "platform:/plugin/org.eclipse.emf.ecore/model/Ecore.ecore#//EPackage",
            "ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EObject",
            "EPackage",
        ),
    ] {
        let error = match Ctx::parse(naming_ecore(super_types, etype)) {
            Ok(_) => panic!("`{class}` accepted"),
            Err(e) => e.to_string(),
        };
        assert!(
            error.contains(&format!("`{class}`")),
            "the error does not name `{class}`: {error}"
        );
        for supported in [
            "EAnnotation",
            "EModelElement",
            "ENamedElement",
            "EObject",
            "EStringToStringMapEntry",
        ] {
            assert!(
                error.contains(&format!("`{supported}`")),
                "the error does not list `{supported}`: {error}"
            );
        }
    }
}

/// The same one-per-`char` cursor, where the lag lands on a character
/// boundary: the value slice stops short of the closing quote and the file
/// is refused as malformed although it is well-formed XML.
#[test]
fn parses_a_non_ascii_attribute_value_followed_by_ascii() {
    let ecore = ecore_with_annotation_value("a guillemet » inside an attribute value");
    if let Err(e) = Ctx::parse(&ecore) {
        panic!("refused: {e:?}");
    }
}

/// Three-byte (`ℕ`) and four-byte (`𝔸`) characters, at the end of the value,
/// in the middle of it, and on their own: the value is read back whole.
#[test]
fn parses_attribute_values_with_three_and_four_byte_chars() {
    for value in ["ℕ", "𝔸", "x ℕ", "x 𝔸", "ℕ𝔸 in the middle of a value", "𝔸ℕ𝔸"]
    {
        let ecore = ecore_with_annotation_value(value);
        let ctx = Ctx::parse(&ecore).unwrap_or_else(|e| panic!("value {value:?} refused: {e:?}"));
        let root = ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Root")
            .expect("Root classifier should exist");
        let read = root
            .annotations()
            .iter()
            .find_map(|annot| annot.details().get("documentation"))
            .expect("documentation detail should exist");
        assert_eq!(read, value);
    }
}

/// EMF resolves `Ecore.ecore#//X` against the metamodel's own location and `#//X` in the
/// metamodel itself, so neither means the built-in Ecore; both keep their behaviour, and so do
/// Ecore's datatypes.
#[test]
fn relative_ecore_spellings_and_datatypes_resolve_as_before() {
    for (super_types, etype) in [
        (
            "Ecore.ecore#//EModelElement",
            "ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString",
        ),
        ("", "ecore:EClass Ecore.ecore#//EObject"),
    ] {
        let error = match Ctx::parse(naming_ecore(super_types, etype)) {
            Ok(_) => panic!("`{super_types}` and `{etype}` accepted"),
            Err(e) => e.to_string(),
        };
        assert!(
            error.contains("unsupported `eType` path `") && error.contains("Ecore.ecore#//"),
            "unexpected refusal: {error}"
        );
    }

    // no class of the metamodel is named `EObject`: an undefined forward reference
    let error = match Ctx::parse(naming_ecore("#//EObject", "#//EObject")) {
        Ok(_) => panic!("`#//EObject` accepted with no local `EObject`"),
        Err(e) => e.to_string(),
    };
    assert!(
        error.contains("failed to finalize parsing context"),
        "unexpected refusal: {error}"
    );

    let ecore = naming_ecore(
        "#//EObject",
        "ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString",
    )
    .replace(
        "</ecore:EPackage>",
        r#"<eClassifiers xsi:type="ecore:EClass" name="EObject"/></ecore:EPackage>"#,
    );
    let ctx = Ctx::parse(&ecore).unwrap_or_else(|e| panic!("refused: {e}"));
    assert!(ctx
        .packs()
        .iter()
        .all(|pack| pack.ns_uri() != Some(ECORE_NS_URI)));
    let test = pack_named(&ctx, "test");
    let element = class_named(&ctx, test, "Element");
    let sups: Vec<_> = element.sup().iter().copied().collect();
    assert_eq!(sups, [class_named(&ctx, test, "EObject").idx]);
    assert_eq!(
        element.structural()[0].typ,
        Some(ctx.builtins()[&ecore_rs::repr::builtin::Typ::EString])
    );
}

/// `class` parses the whole body of a class before it resolves the supertypes, so a supertype
/// that fails to resolve is reported at the end of the class, where the parser stands.
#[test]
fn reports_a_failed_supertype_at_its_class() {
    let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
xmlns:xmi="http://www.omg.org/XMI"
xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
name="test"
nsURI="http://example.org/test"
nsPrefix="test">
<eClassifiers xsi:type="ecore:EClass" name="Base"/>
<eClassifiers xsi:type="ecore:EClass" name="Derived" eSuperTypes="#//Base other.ecore#//Base">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="label"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="count"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
</eClassifiers>
<eClassifiers xsi:type="ecore:EClass" name="Next"/>
</ecore:EPackage>
"##;
    let line = 1 + ecore
        .lines()
        .position(|line| line.contains(r#"name="Derived""#))
        .unwrap();

    let error = match Ctx::parse(ecore) {
        Ok(_) => panic!("`other.ecore#//Base` accepted"),
        Err(e) => e.to_string(),
    };
    assert!(
        error.contains(&format!("line {line}, ")),
        "the error is not reported at line {line}: {error}"
    );
    assert!(
        error.contains("`Derived`") && error.contains("other.ecore#//Base"),
        "the error names neither `Derived` nor its supertype: {error}"
    );
}

/// `Class` keeps its supertypes in a `BTreeSet`, so they come back ordered by class index,
/// that is by first mention in the file, and not in the order `eSuperTypes` declares them,
/// which EMF uses to order inherited features and to pick the implementation's base class.
#[test]
fn keeps_supertypes_in_declaration_order() {
    let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
xmlns:xmi="http://www.omg.org/XMI"
xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
name="test"
nsURI="http://example.org/test"
nsPrefix="test">
<eClassifiers xsi:type="ecore:EClass" name="A" abstract="true"/>
<eClassifiers xsi:type="ecore:EClass" name="B" abstract="true"/>
<eClassifiers xsi:type="ecore:EClass" name="C" abstract="true"/>
<eClassifiers xsi:type="ecore:EClass" name="D" eSuperTypes="#//C #//A #//B"/>
<eClassifiers xsi:type="ecore:EClass" name="E" eSuperTypes="#//Later #//A"/>
<eClassifiers xsi:type="ecore:EClass" name="F" eSuperTypes="#//B #//A #//B"/>
<eClassifiers xsi:type="ecore:EClass" name="Later" abstract="true"/>
</ecore:EPackage>
"##;

    let ctx = Ctx::parse(ecore).unwrap_or_else(|e| panic!("refused: {e}"));
    let test = pack_named(&ctx, "test");
    let sups = |name: &str| {
        class_named(&ctx, test, name)
            .sup()
            .iter()
            .map(|idx| ctx[*idx].name().to_string())
            .collect::<Vec<_>>()
    };

    assert_eq!(sups("D"), ["C", "A", "B"]);
    assert_eq!(sups("E"), ["Later", "A"]);
    assert_eq!(sups("F"), ["B", "A"]);

    let subs = |name: &str| {
        class_named(&ctx, test, name)
            .sub()
            .iter()
            .map(|idx| ctx[*idx].name().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(subs("A"), ["D", "E", "F"]);
    assert_eq!(subs("B"), ["D", "F"]);
    assert_eq!(subs("Later"), ["E"]);

    assert!(ctx.to_pretty_string().contains("supers: C, A, B"));
}

/// `class_structural` knows no `eKeys` attribute, so a reference that declares the attributes
/// identifying its targets is refused.
#[test]
fn parses_reference_keys() {
    let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
xmlns:xmi="http://www.omg.org/XMI"
xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
name="test"
nsURI="http://example.org/test"
nsPrefix="test">
<eClassifiers xsi:type="ecore:EClass" name="Container">
    <eStructuralFeatures xsi:type="ecore:EReference" name="items" upperBound="-1"
        eType="#//Item" containment="true" eKeys="#//Item/name  #//Item/version"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="plain" upperBound="-1"
        eType="#//Item" containment="true"/>
</eClassifiers>
<eClassifiers xsi:type="ecore:EClass" name="Item">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="version"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
</eClassifiers>
</ecore:EPackage>
"##;

    let ctx = Ctx::parse(ecore).unwrap_or_else(|e| panic!("refused: {e}"));
    let container = class_named(&ctx, pack_named(&ctx, "test"), "Container");
    assert_eq!(container.structural().len(), 2);
    assert_eq!(
        container.structural()[0].keys(),
        ["#//Item/name", "#//Item/version"]
    );
    assert!(container.structural()[1].keys().is_empty());
}
