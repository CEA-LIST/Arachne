pub mod codegen;
pub mod config;
pub mod error;
pub mod parser;
mod project;
mod utils;

use std::path::PathBuf;

pub use config::Config;
use ecore_rs::repr::{Class, Pack, idx, structural};
pub use error::{ArachneError, Result};
use heck::ToSnakeCase;
use log::{debug, info, warn};
pub use parser::EcoreParser;

use crate::{
    codegen::{
        classifier::{ClassGenerator, is_instantiable_class},
        cycles::analyze_cycles,
        generate::Generate,
        generator::Generator,
        package::PackageGenerator,
        read_as_ecore::ReadAsEcoreGenerator,
        reference::{ReferenceGenerator, analysis::analyze_references},
    },
    utils::topo::topological_sort,
};

const CLASSIFIERS_PATH_MOD: &str = "classifiers";
const REFERENCES_PATH_MOD: &str = "references";
const PACKAGE_PATH_MOD: &str = "package";

/// Metadata about the code generation process, including input/output paths, project/package names, and statistics about the generated code
#[derive(Debug, Clone)]
pub struct GenerationReport {
    pub input_path: PathBuf,
    pub output_dir: PathBuf,
    pub project_name: String,
    pub package_name: String,
    pub class_count: usize,
}

/// Main entry point for code generation
pub fn generate(config: Config) -> anyhow::Result<()> {
    generate_with_report(config).map(|_| ())
}

/// Main entry point for code generation with execution metadata.
pub fn generate_with_report(config: Config) -> anyhow::Result<GenerationReport> {
    info!("Validating configuration");
    config.validate()?;

    info!("Parsing ecore metamodel: {:?}", config.input_path);
    let parser = EcoreParser::from_file(&config.input_path)?;

    if parser
        .ctx
        .packs()
        .iter()
        .filter(|p| p.name() != "[root]" && p.name() != "[builtin]")
        .count()
        > 1
    {
        warn!(
            "Multiple packages found in the Ecore model. Only the first valid package will be used for code generation."
        );
    }

    // Find the first valid package (not [root] or [builtin]) to generate code for
    // TODO: Consider allowing the user to specify a package name in the config
    let pack = parser
        .ctx
        .packs()
        .iter()
        .find(|p| p.name() != "[root]" && p.name() != "[builtin]")
        .ok_or(ArachneError::NoValidPackageFound)?;

    let class_count = pack.classes().len();
    debug!(
        "Found package `{}` with {} classes",
        pack.name(),
        class_count
    );

    info!("Generating Rust tokens");
    let (classifiers, references, package, generated_class_count) =
        generate_from_parser(&parser, pack)?;

    // Emit any warnings collected during generation
    classifiers.emit_warnings();
    references.emit_warnings();
    package.emit_warnings();

    // Build the final TokenStream
    let classifiers_code = classifiers.build();
    let references_code = references.build();
    let package_code = package.build();

    // Choose a project name
    let project_name = config
        .project_name
        .clone()
        .or_else(|| Some(pack.name().to_snake_case()))
        .unwrap_or_else(|| "generated_crdt".to_string());

    info!("Writing generated project '{}'", project_name);
    // Write a full Rust project
    project::write_project(
        &config,
        &project_name,
        classifiers_code,
        references_code,
        package_code,
    )?;

    Ok(GenerationReport {
        input_path: config.input_path.clone(),
        output_dir: config.output_dir.clone(),
        project_name,
        package_name: pack.name().to_string(),
        class_count: generated_class_count,
    })
}

/// Generates code from a parsed Ecore context.
/// Returns the generated classifiers CRDT objects and the generated reference management code
pub fn generate_from_parser<'a>(
    parser: &'a EcoreParser,
    pack: &'a Pack,
) -> anyhow::Result<(Generator<'a>, Generator<'a>, Generator<'a>, usize)> {
    let mut classifiers = Generator::new(CLASSIFIERS_PATH_MOD);
    let mut references = Generator::new(REFERENCES_PATH_MOD);
    let mut package = Generator::new(PACKAGE_PATH_MOD);

    let cycle_analysis = analyze_cycles(&parser.ctx)?;

    let package_classes: Vec<idx::Class> = pack.classes().iter().copied().collect();
    let package_class_set: std::collections::HashSet<idx::Class> =
        package_classes.iter().copied().collect();

    let concrete_package_classes: Vec<idx::Class> = package_classes
        .iter()
        .copied()
        .filter(|class_idx| is_instantiable_class(&parser.ctx.classes()[**class_idx]))
        .collect();

    let concrete_containment_incoming =
        compute_concrete_containment_incoming(&parser.ctx, &package_classes, &package_class_set);

    let mut top_level_roots: Vec<idx::Class> = concrete_package_classes
        .iter()
        .copied()
        .filter(|class_idx| {
            let class = &parser.ctx.classes()[**class_idx];
            !class.is_enum()
                && !class.is_interface()
                && !concrete_containment_incoming.contains(class_idx)
        })
        .collect();

    if top_level_roots.is_empty() {
        debug!(
            "No top-level roots found based on concrete classes. Falling back to abstract/interface classes with concrete descendants and no external containers."
        );
        top_level_roots = package_classes
            .iter()
            .copied()
            .filter(|class_idx| {
                let class = &parser.ctx.classes()[**class_idx];
                !class.is_enum()
                    && (class.is_interface() || !class.is_concrete())
                    && has_concrete_descendant(&parser.ctx, *class_idx, &package_class_set)
                    && abstract_family_has_no_external_container(
                        &parser.ctx,
                        *class_idx,
                        &package_classes,
                        &package_class_set,
                    )
            })
            .collect();
    }

    if top_level_roots.is_empty() {
        return Err(ArachneError::RootClassNotFound(pack.name().to_string()).into());
    }

    let mut reachable_classes: std::collections::HashSet<idx::Class> =
        std::collections::HashSet::new();
    for root_idx in &top_level_roots {
        reachable_classes.extend(collect_reachable_classes(
            &parser.ctx,
            *root_idx,
            &package_class_set,
        ));
    }

    // Get all classes in the package
    let classes: Vec<&Class> = parser
        .ctx
        .classes()
        .iter()
        .filter(|c| reachable_classes.contains(&c.idx) || c.is_enum())
        .collect();

    // Sort classes topologically by inheritance hierarchy
    let sorted_classes = topological_sort(&parser.ctx, &classes);
    let reachable_package_classes: Vec<idx::Class> = package_classes
        .iter()
        .copied()
        .filter(|idx| reachable_classes.contains(idx))
        .collect();
    let reference_analysis = analyze_references(&parser.ctx, &reachable_package_classes);

    debug!(
        "Identified {} top-level root classes for package `{}`: `{}`",
        top_level_roots.len(),
        pack.name(),
        top_level_roots
            .iter()
            .map(|idx| parser.ctx.classes()[**idx].name())
            .collect::<Vec<_>>()
            .join("`, `")
    );

    info!("Generating classifiers...",);
    for class in &sorted_classes {
        let class_gen = ClassGenerator::new(class, &parser.ctx, &cycle_analysis);
        let fragment = class_gen.generate()?;
        classifiers.register(fragment);
    }

    info!("Generating reference manager...");
    let refs = ReferenceGenerator::new(
        &parser.ctx,
        reachable_package_classes.clone(),
        top_level_roots.clone(),
        &cycle_analysis,
    );
    let fragment = refs.generate()?;
    references.register(fragment);

    info!("Generating package...");
    let generated_class_count = reachable_package_classes.len();
    let package_gen = PackageGenerator::new(
        &parser.ctx,
        pack.idx,
        top_level_roots.clone(),
        &reference_analysis,
    );
    let fragment = package_gen.generate()?;
    package.register(fragment);

    let read_as_ecore_gen = ReadAsEcoreGenerator::new(&parser.ctx, pack.idx, top_level_roots);
    let fragment = read_as_ecore_gen.generate()?;
    package.register(fragment);

    Ok((classifiers, references, package, generated_class_count))
}

fn collect_reachable_classes(
    ctx: &ecore_rs::ctx::Ctx,
    root_class: idx::Class,
    package_classes: &std::collections::HashSet<idx::Class>,
) -> std::collections::HashSet<idx::Class> {
    let mut reachable = std::collections::HashSet::new();
    let mut stack = vec![root_class];

    while let Some(class_idx) = stack.pop() {
        if !package_classes.contains(&class_idx) || !reachable.insert(class_idx) {
            continue;
        }

        let class = &ctx.classes()[*class_idx];

        for parent in class.sup() {
            stack.push(*parent);
        }

        if !class.sub().is_empty() {
            for sub in class.sub() {
                stack.push(*sub);
            }
        }

        for feature in class.structural() {
            if feature.kind == structural::Typ::EReference
                && feature.containment
                && let Some(target) = feature.typ
            {
                stack.push(target);
            }
        }
    }

    reachable
}

fn has_concrete_descendant(
    ctx: &ecore_rs::ctx::Ctx,
    class_idx: idx::Class,
    package_classes: &std::collections::HashSet<idx::Class>,
) -> bool {
    let mut stack: Vec<idx::Class> = ctx.classes()[*class_idx].sub().iter().copied().collect();

    while let Some(candidate) = stack.pop() {
        if !package_classes.contains(&candidate) {
            continue;
        }

        let class = &ctx.classes()[*candidate];
        if is_instantiable_class(class) {
            return true;
        }

        stack.extend(class.sub().iter().copied());
    }

    false
}

fn concrete_descendants_in_package(
    ctx: &ecore_rs::ctx::Ctx,
    class_idx: idx::Class,
    package_classes: &std::collections::HashSet<idx::Class>,
) -> std::collections::HashSet<idx::Class> {
    let mut result = std::collections::HashSet::new();
    let mut stack = vec![class_idx];

    while let Some(candidate) = stack.pop() {
        if !package_classes.contains(&candidate) {
            continue;
        }

        let class = &ctx.classes()[*candidate];
        if is_instantiable_class(class) {
            result.insert(candidate);
        }

        stack.extend(class.sub().iter().copied());
    }

    result
}

fn compute_concrete_containment_incoming(
    ctx: &ecore_rs::ctx::Ctx,
    package_classes: &[idx::Class],
    package_class_set: &std::collections::HashSet<idx::Class>,
) -> std::collections::HashSet<idx::Class> {
    let mut incoming = std::collections::HashSet::new();

    for &source_class_idx in package_classes {
        let source_concretes =
            concrete_descendants_in_package(ctx, source_class_idx, package_class_set);
        if source_concretes.is_empty() {
            continue;
        }

        for feature in ctx.classes()[*source_class_idx].structural() {
            if feature.kind != structural::Typ::EReference || !feature.containment {
                continue;
            }

            let Some(target_class_idx) = feature.typ else {
                continue;
            };
            if !package_class_set.contains(&target_class_idx) {
                continue;
            }

            let target_concretes =
                concrete_descendants_in_package(ctx, target_class_idx, package_class_set);
            for target in target_concretes {
                if source_concretes.iter().any(|source| *source != target) {
                    incoming.insert(target);
                }
            }
        }
    }

    incoming
}

fn abstract_family_has_no_external_container(
    ctx: &ecore_rs::ctx::Ctx,
    class_idx: idx::Class,
    package_classes: &[idx::Class],
    package_class_set: &std::collections::HashSet<idx::Class>,
) -> bool {
    let family = concrete_descendants_in_package(ctx, class_idx, package_class_set);
    if family.is_empty() {
        return false;
    }
    let family_context = collect_reachable_classes(ctx, class_idx, package_class_set);

    for &source_class_idx in package_classes {
        let source_concretes =
            concrete_descendants_in_package(ctx, source_class_idx, package_class_set);
        if source_concretes.is_empty() {
            continue;
        }

        for feature in ctx.classes()[*source_class_idx].structural() {
            if feature.kind != structural::Typ::EReference || !feature.containment {
                continue;
            }

            let Some(target_class_idx) = feature.typ else {
                continue;
            };
            if !package_class_set.contains(&target_class_idx) {
                continue;
            }

            let target_concretes =
                concrete_descendants_in_package(ctx, target_class_idx, package_class_set);
            if target_concretes
                .iter()
                .any(|target| family.contains(target))
                && source_concretes
                    .iter()
                    .any(|source| !family_context.contains(source))
            {
                return false;
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{EcoreParser, generate_from_parser};

    fn normalize(code: impl ToString) -> String {
        code.to_string()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    }

    fn generate_modules_from_parser(parser: &EcoreParser) -> (String, String) {
        println!("{:?}", parser.ctx.packs().len());
        let pack = parser
            .ctx
            .packs()
            .iter()
            .find(|p| p.name() != "[root]" && p.name() != "[builtin]")
            .expect("package should exist");
        let (classifiers, references, _package, _generated_class_count) =
            generate_from_parser(&parser, pack).expect("generation should succeed");

        (
            normalize(classifiers.build()),
            normalize(references.build()),
        )
    }

    fn generate_modules_from_file(path: impl AsRef<Path>) -> (String, String) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
        let parser = EcoreParser::from_file(path).expect("ecore should parse");
        generate_modules_from_parser(&parser)
    }

    fn generate_modules_from_str(ecore: &str) -> (String, String) {
        let parser = EcoreParser::from_string(ecore).expect("ecore should parse");
        generate_modules_from_parser(&parser)
    }

    fn generate_package_from_parser(parser: &EcoreParser) -> String {
        let pack = parser
            .ctx
            .packs()
            .iter()
            .find(|p| p.name() != "[root]" && p.name() != "[builtin]")
            .expect("package should exist");
        let (_classifiers, _references, package, _generated_class_count) =
            generate_from_parser(&parser, pack).expect("generation should succeed");

        normalize(package.build())
    }

    fn generate_package_from_file(path: impl AsRef<Path>) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
        let parser = EcoreParser::from_file(path).expect("ecore should parse");
        generate_package_from_parser(&parser)
    }

    fn generate_package_from_str(ecore: &str) -> String {
        let parser = EcoreParser::from_string(ecore).expect("ecore should parse");
        generate_package_from_parser(&parser)
    }

    #[test]
    fn unique_ordered_many_attribute_uses_graph_list() {
        let (classifiers, _references) =
            generate_modules_from_file("../examples/pet_metamodels/kitchen_sink.ecore");

        assert!(
            classifiers.contains("unique_list:__classifiers::GraphLog<__classifiers::List<i16>>")
        );
        assert!(!classifiers.contains("__classifiers::ListLog"));
    }

    #[test]
    fn vec_log_attributes_import_vec_log() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Model">
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="visibilities" unique="false" upperBound="-1" eType="#//Visibility"/>
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="ids" ordered="false" upperBound="-1" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EEnum" name="Visibility">
        <eLiterals name="Final"/>
        <eLiterals name="Initial" value="1"/>
    </eClassifiers>
</ecore:EPackage>
"##;

        let (classifiers, _references) = generate_modules_from_str(ecore);

        assert!(classifiers.contains("pubusemoirai_protocol::state::po_log::VecLog;"));
        assert!(classifiers.contains(
            "visibilities:__classifiers::NestedListLog<__classifiers::VecLog<__classifiers::MVRegister<Visibility>>>"
        ));
        assert!(classifiers.contains("ids:__classifiers::VecLog<__classifiers::AWSet<i32>>"));
    }

    #[test]
    fn concrete_superclass_with_subclasses_emits_family_union() {
        let (classifiers, _references) = generate_modules_from_file(
            "../examples/pet_metamodels/concrete_inherits_concrete.ecore",
        );

        assert!(classifiers.contains("__classifiers::record!(A{"));
        assert!(classifiers.contains("__classifiers::union!(AKind=A(A,ALog)|B(B,BLog));"));
    }

    #[test]
    fn model_enum_does_not_collide_with_polymorphic_kind() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="typeslibrary"
    nsURI="http://example.org/typeslibrary"
    nsPrefix="typeslibrary">
    <eClassifiers xsi:type="ecore:EClass" name="TypesLibrary" abstract="true">
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="kind" eType="#//TypesLibraryKind"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="NativeTypesLibrary" eSuperTypes="#//TypesLibrary"/>
    <eClassifiers xsi:type="ecore:EClass" name="UserDefinedTypesLibrary" eSuperTypes="#//TypesLibrary"/>
    <eClassifiers xsi:type="ecore:EEnum" name="TypesLibraryKind">
        <eLiterals name="LogicalTypes"/>
        <eLiterals name="PhysicalTypes" value="1"/>
    </eClassifiers>
</ecore:EPackage>
"##;

        let (classifiers, _references) = generate_modules_from_str(ecore);

        assert!(classifiers.contains(
            "__classifiers::union!(TypesLibraryKind=NativeTypesLibrary(NativeTypesLibrary,NativeTypesLibraryLog)|UserDefinedTypesLibrary(UserDefinedTypesLibrary,UserDefinedTypesLibraryLog));"
        ));
        assert!(classifiers.contains("pubenumTypesLibraryKindModel{"));
        assert!(classifiers.contains(
            "kind:__classifiers::OptionLog<__classifiers::VecLog<__classifiers::MVRegister<TypesLibraryKindModel>>>"
        ));
        assert!(!classifiers.contains("pubenumTypesLibraryKind{"));
    }

    #[test]
    fn containment_typed_by_concrete_superclass_uses_family_log() {
        let (classifiers, _references) = generate_modules_from_file(
            "../examples/pet_metamodels/concrete_polymorphic_targets.ecore",
        );

        assert!(classifiers.contains("__classifiers::union!(AKind=A(A,ALog)|B(BKind,BKindLog));"));
        assert!(classifiers.contains("__classifiers::union!(BKind=B(B,BLog)|C(C,CLog));"));
        assert!(classifiers.contains("D{child:__classifiers::OptionLog<AKindLog>,}"));
    }

    #[test]
    fn recursive_containment_uses_boxed_log() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Model">
        <eStructuralFeatures xsi:type="ecore:EReference" name="root" eType="#//Select" containment="true"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Select">
        <eStructuralFeatures xsi:type="ecore:EReference" name="union" eType="#//Union" containment="true"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Union">
        <eStructuralFeatures xsi:type="ecore:EReference" name="select" eType="#//Select" containment="true"/>
    </eClassifiers>
</ecore:EPackage>
"##;

        let (classifiers, _references) = generate_modules_from_str(ecore);

        assert!(classifiers.contains("pubusemoirai_protocol::state::log::BoxedLog;"));
        assert!(classifiers.contains(
            "Select{union:__classifiers::OptionLog<__classifiers::BoxedLog<UnionLog>>,}"
        ));
        assert!(!classifiers.contains("OptionLog<Box<UnionLog>>"));
    }

    #[test]
    fn parallel_recursive_containment_edges_are_all_boxed() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Model">
        <eStructuralFeatures xsi:type="ecore:EReference" name="rules" upperBound="-1" eType="#//FollowBy" containment="true"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="FollowBy">
        <eStructuralFeatures xsi:type="ecore:EReference" name="leftSide" eType="#//TerminalExpression" containment="true"/>
        <eStructuralFeatures xsi:type="ecore:EReference" name="rightSide" upperBound="-1" eType="#//TerminalExpression" containment="true"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="TerminalExpression">
        <eStructuralFeatures xsi:type="ecore:EReference" name="everyExpression" eType="#//FollowBy" containment="true"/>
        <eStructuralFeatures xsi:type="ecore:EReference" name="betweenParenthesis" eType="#//FollowBy" containment="true"/>
    </eClassifiers>
</ecore:EPackage>
"##;

        let (classifiers, _references) = generate_modules_from_str(ecore);

        assert!(classifiers.contains(
            "left_side:__classifiers::OptionLog<__classifiers::BoxedLog<TerminalExpressionLog>>"
        ));
        assert!(classifiers.contains(
            "right_side:__classifiers::NestedListLog<__classifiers::BoxedLog<TerminalExpressionLog>>"
        ));
        assert!(classifiers.contains("every_expression:__classifiers::OptionLog<FollowByLog>"));
    }

    #[test]
    fn non_containment_reference_typed_by_concrete_superclass_expands_to_family() {
        let (_classifiers, references) = generate_modules_from_file(
            "../examples/pet_metamodels/concrete_polymorphic_targets.ecore",
        );

        assert!(references.contains("DTargetEdge[0,1]"));
        assert!(references.contains("DToA:DId->AId(DTargetEdge)"));
        assert!(references.contains("DToB:DId->BId(DTargetEdge)"));
        assert!(references.contains("DToC:DId->CId(DTargetEdge)"));
    }

    #[test]
    fn reference_vertex_matchers_use_sink_kind() {
        let (_classifiers, references) = generate_modules_from_file("../examples/conference.ecore");

        assert!(references.contains("instance_from_sink_kind"));
        assert!(references.contains("\"Session\""));
        assert!(references.contains("Instance::SessionId"));
        assert!(references.contains("\"Person\""));
        assert!(references.contains("Instance::PersonId"));
        assert!(!references.contains("pub fn instance_from_path"));
        assert!(!references.contains("__references::Field(\"track_super\")"));
    }

    #[test]
    fn multiple_inheritance() {
        let (classifiers, _references) =
            generate_modules_from_file("../examples/pet_metamodels/multiple_inheritance.ecore");

        println!("classifiers: {}", classifiers);

        assert!(classifiers.contains("AKind=C(C,CLog)"));
        assert!(classifiers.contains("BKind=C(C,CLog)"));
    }

    #[test]
    fn interface_is_generated_like_abstract_class() {
        let ecore = include_str!("../../examples/pet_metamodels/kitchen_sink.ecore").replace(
            r#"name="Abstract" abstract="true""#,
            r#"name="Abstract" interface="true""#,
        );

        let (classifiers, references) = generate_modules_from_str(&ecore);

        assert!(classifiers.contains("__classifiers::union!(AbstractKind=Baz(Baz,BazLog));"));
        assert!(classifiers.contains("__classifiers::record!(Abstract{"));
        assert!(classifiers.contains(
            "name:__classifiers::OptionLog<__classifiers::GraphLog<__classifiers::List<char>>>"
        ));
        assert!(references.contains("BazFooEdge[1,1]"));
    }

    #[test]
    fn abstract_classes_without_concrete_descendants_are_removed_from_generated_code() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="NamedElement">
        <eStructuralFeatures xsi:type="ecore:EReference" name="constraints" upperBound="-1" eType="#//Constraint" containment="true"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Parameter" abstract="true" eSuperTypes="#//NamedElement">
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="kind" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Constraint" abstract="true">
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="body" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Method" eSuperTypes="#//NamedElement">
        <eStructuralFeatures xsi:type="ecore:EReference" name="parameters" upperBound="-1" eType="#//Parameter" containment="true"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Model">
        <eStructuralFeatures xsi:type="ecore:EReference" name="methods" upperBound="-1" eType="#//Method" containment="true"/>
    </eClassifiers>
</ecore:EPackage>
"##;

        let (classifiers, _references) = generate_modules_from_str(ecore);

        assert!(!classifiers.contains("ParameterKind"));
        assert!(!classifiers.contains("ConstraintKind"));
        assert!(!classifiers.contains("parameters:"));
        assert!(!classifiers.contains("constraints:"));
        assert!(
            classifiers
                .contains("__classifiers::record!(Method{named_element_super:NamedElementLog,});")
        );
        assert!(classifiers.contains("__classifiers::record!(NamedElement{});"));
    }

    #[test]
    fn reference_side_effects_use_event_disambiguators() {
        let package = generate_package_from_file("../examples/conference.ecore");

        assert!(package.contains("letmutreference_effect_disambiguator=0u32;"));
        assert!(package.contains("sink.kind().and_then"));
        assert!(package.contains("instance_from_sink_kind(kind,sink.path())"));
        assert!(package.contains("reference_effect_disambiguator+=1;"));
        assert!(package.contains("__package::ProtocolEvent::unfold_with_disambiguator"));
    }

    #[test]
    fn root_classifier_named_event_does_not_collide_with_protocol_event() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="stateMachine"
    nsURI="http://example.org/state-machine"
    nsPrefix="stateMachine">
    <eClassifiers xsi:type="ecore:EClass" name="Event"/>
</ecore:EPackage>
"##;

        let package = generate_package_from_str(ecore);

        assert!(package.contains("pubusemoirai_protocol::event::EventasProtocolEvent;"));
        assert!(package.contains("Event(crate::classifiers::Event)"));
        assert!(package.contains("event:__package::ProtocolEvent<Self::Op>"));
        assert!(package.contains("__package::ProtocolEvent::unfold(event.clone(),o)"));
        assert!(!package.contains("Event(__package::Event)"));
    }

    #[test]
    fn package_generates_read_as_ecore_query() {
        let package = generate_package_from_file("../examples/class_hierarchy.ecore");

        assert!(package.contains("pubstructReadAsEcore;"));
        assert!(package.contains("impl__package::QueryOperationforReadAsEcore"));
        assert!(package.contains("impl__package::EvalNested<ReadAsEcore>forClassHierarchyLog"));
        assert!(package.contains("XMLElement::new(\"xmi:XMI\")"));
        assert!(package.contains(
            "document_root.add_attribute(\"xmlns:class_hierarchy\",\"http://www.example.org/class_hierarchy\")"
        ));
        assert!(package.contains("XMLElement::new(\"class_hierarchy:Package\")"));
        assert!(!package.contains("XMLElement::new(\"ecore:EPackage\")"));
    }

    #[test]
    fn read_as_ecore_dispatches_polymorphic_roots_to_concrete_eclasses() {
        let package = generate_package_from_file("../examples/json.ecore");

        assert!(package.contains("match&self.json_log.child"));
        assert!(package.contains("crate::classifiers::JsonKindChild::Array(_)"));
        assert!(package.contains("crate::classifiers::JsonKindChild::Object(_)"));
        assert!(package.contains("XMLElement::new(\"json:Array\")"));
        assert!(package.contains("XMLElement::new(\"json:Object\")"));
        assert!(package.contains("XMLElement::new(\"json:String\")"));
        assert!(package.contains("XMLElement::new(\"json:Number\")"));
        assert!(package.contains("XMLElement::new(\"json:Boolean\")"));
    }
}
