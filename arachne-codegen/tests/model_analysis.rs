use arachne_codegen::{EcoreParser, codegen::analysis::GenerationPlan, generate_from_parser};

fn parse(classes: &str) -> EcoreParser {
    EcoreParser::from_string(&format!(
        r#"<ecore:EPackage xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
            xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" name="analysis">
            {classes}
        </ecore:EPackage>"#
    ))
    .unwrap()
}

#[test]
fn diamond_inheritance_has_one_concrete_member_and_one_reference() {
    let parser = parse(
        r##"
        <eClassifiers xsi:type="ecore:EClass" name="Root">
            <eStructuralFeatures xsi:type="ecore:EReference" name="children"
                containment="true" upperBound="-1" eType="#//Top"/>
        </eClassifiers>
        <eClassifiers xsi:type="ecore:EClass" name="Top" abstract="true">
            <eStructuralFeatures xsi:type="ecore:EReference" name="peer" eType="#//Top"/>
        </eClassifiers>
        <eClassifiers xsi:type="ecore:EClass" name="Left" abstract="true" eSuperTypes="#//Top"/>
        <eClassifiers xsi:type="ecore:EClass" name="Right" interface="true" eSuperTypes="#//Top"/>
        <eClassifiers xsi:type="ecore:EClass" name="Leaf" eSuperTypes="#//Left #//Right"/>
        <eClassifiers xsi:type="ecore:EClass" name="Empty" abstract="true" eSuperTypes="#//Top"/>
    "##,
    );
    let pack = parser
        .ctx
        .packs()
        .iter()
        .find(|p| p.name() == "analysis")
        .unwrap();
    let plan = GenerationPlan::new(&parser.ctx, pack).unwrap();
    let class = |name| {
        parser
            .ctx
            .classes()
            .iter()
            .find(|c| c.name() == name)
            .unwrap()
            .idx
    };
    let top = class("Top");
    let leaf = class("Leaf");
    assert_eq!(plan.model.info(top).concrete_family, [leaf]);
    assert_eq!(plan.model.info(top).family.len(), 4);
    assert!(plan.model.info(class("Empty")).uninhabited);
    assert!(!plan.model.info(top).family.contains(&class("Empty")));
    assert!(plan.model.info(top).wrapper_descendant);
    assert_eq!(plan.roots, [class("Root")]);
    assert_eq!(plan.references.refs.len(), 1);
    let reference = &plan.references.refs[0];
    assert_eq!(
        (reference.source_class, reference.target_class),
        (leaf, leaf)
    );
    assert_eq!(reference.reference_name, "peer");
}

#[test]
fn reachability_and_generation_order_preserve_their_distinct_roles() {
    let parser = parse(
        r##"
        <eClassifiers xsi:type="ecore:EClass" name="Child" eSuperTypes="#//Parent"/>
        <eClassifiers xsi:type="ecore:EClass" name="Parent" abstract="true"/>
        <eClassifiers xsi:type="ecore:EEnum" name="State">
            <eLiterals name="Ready"/>
        </eClassifiers>
    "##,
    );
    let pack = parser
        .ctx
        .packs()
        .iter()
        .find(|p| p.name() == "analysis")
        .unwrap();
    let plan = GenerationPlan::new(&parser.ctx, pack).unwrap();
    let reachable_names = plan
        .reachable_classes
        .iter()
        .map(|idx| parser.ctx.class(*idx).name())
        .collect::<Vec<_>>();
    let generation_names = plan
        .generation_order
        .iter()
        .map(|idx| parser.ctx.class(*idx).name())
        .collect::<Vec<_>>();
    assert_eq!(reachable_names, ["Child", "Parent"]);
    assert_eq!(generation_names, ["Parent", "Child", "State"]);
    let (_, _, _, _, generated_class_count) = generate_from_parser(&parser, pack).unwrap();
    assert_eq!(generated_class_count, 2);
}

#[test]
fn cyclic_inheritance_returns_a_generation_error() {
    let parser = parse(
        r##"
        <eClassifiers xsi:type="ecore:EClass" name="A" eSuperTypes="#//B"/>
        <eClassifiers xsi:type="ecore:EClass" name="B" eSuperTypes="#//A"/>
    "##,
    );
    let pack = parser
        .ctx
        .packs()
        .iter()
        .find(|p| p.name() == "analysis")
        .unwrap();
    let error = generate_from_parser(&parser, pack)
        .err()
        .expect("inheritance cycles must be rejected");
    assert!(error.to_string().contains("Inheritance cycle"), "{error}");
}

#[test]
fn parallel_references_share_the_same_arc_names_in_graph_and_xmi() {
    let parser = parse(
        r##"
        <eClassifiers xsi:type="ecore:EClass" name="Root">
            <eStructuralFeatures xsi:type="ecore:EReference" name="first" eType="#//Root"/>
            <eStructuralFeatures xsi:type="ecore:EReference" name="second" eType="#//Root"/>
        </eClassifiers>
    "##,
    );
    let pack = parser
        .ctx
        .packs()
        .iter()
        .find(|p| p.name() == "analysis")
        .unwrap();
    let (_, references, _, xmi, _) = generate_from_parser(&parser, pack).unwrap();
    let references = references.build().to_string();
    let xmi = xmi.build().to_string();
    assert!(references.contains("RootToRoot2"));
    assert!(xmi.contains("Ref :: RootToRoot2"));
    assert!(references.contains("RootFirstEdge") && references.contains("RootSecondEdge"));
    assert!(xmi.contains("root::first") && xmi.contains("root::second"));
}

#[test]
fn cascading_name_collisions_are_consistent_across_modules_and_runs() {
    let parser = parse(
        r##"
        <eClassifiers xsi:type="ecore:EClass" name="Query"/>
        <eClassifiers xsi:type="ecore:EClass" name="QueryValue">
            <eStructuralFeatures xsi:type="ecore:EReference" name="target" eType="#//QueryValueModel"/>
        </eClassifiers>
        <eClassifiers xsi:type="ecore:EClass" name="QueryValueModel"/>
    "##,
    );
    let pack = parser
        .ctx
        .packs()
        .iter()
        .find(|p| p.name() == "analysis")
        .unwrap();
    let modules = || {
        let (classifiers, references, package, xmi, _) =
            generate_from_parser(&parser, pack).unwrap();
        [classifiers, references, package, xmi].map(|g| g.build().to_string())
    };
    let first = modules();
    assert_eq!(first, modules(), "generation must be deterministic");
    for code in &first {
        assert!(code.contains("QueryValueModel2"), "{code}");
        assert!(code.contains("QueryValueModel"), "{code}");
    }
    assert!(first[1].contains("QueryValueModel2Id"));
    assert!(first[1].contains("QueryValueModelId"));
}
