pub mod codegen;
pub mod config;
mod deployment;
pub mod error;
pub mod parser;
mod project;
mod utils;

use std::path::PathBuf;

pub use codegen::descriptor::{descriptor_json, metamodel_digest};
pub use config::{Config, MoiraiPathStyle};
use ecore_rs::repr::{Class, Pack, idx, structural};
pub use error::{ArachneError, Result};
use heck::ToSnakeCase;
use log::{debug, info, warn};
pub use parser::EcoreParser;

use crate::{
    codegen::{
        classifier::{ClassGenerator, is_instantiable_class, is_uninhabited_polymorphic_class},
        cycles::analyze_cycles,
        ecore::is_eobject,
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
        .filter(|p| {
            p.name() != "[root]"
                && p.name() != "[builtin]"
                && Some(p.idx) != parser.ctx.ecore_pack()
        })
        .count()
        > 1
    {
        warn!(
            "Multiple packages found in the Ecore model. Only the first valid package will be used for code generation."
        );
    }

    let pack = find_user_package(&parser.ctx)?;

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

    // The metamodel the generated types encode, re-emitted as data so the
    // node can serve it to clients (`GET /api/metamodel`).
    let descriptor = codegen::descriptor::descriptor_json(&parser.ctx, pack)?;

    info!("Writing generated project '{}'", project_name);
    // Write a full Rust project
    project::write_project(
        &config,
        &project_name,
        &pack.name().to_string(),
        classifiers_code,
        references_code,
        package_code,
        &descriptor,
    )?;

    Ok(GenerationReport {
        input_path: config.input_path.clone(),
        output_dir: config.output_dir.clone(),
        project_name,
        package_name: pack.name().to_string(),
        class_count: generated_class_count,
    })
}

/// The package generation targets: the first that is neither the parser's
/// `[root]` nor `[builtin]` bookkeeping package, nor the built-in Ecore
/// package the parser adds when a metamodel names one of Ecore's own classes.
// TODO: Consider allowing the user to specify a package name in the config
pub fn find_user_package(ctx: &ecore_rs::ctx::Ctx) -> Result<&Pack> {
    ctx.packs()
        .iter()
        .find(|p| {
            p.name() != "[root]" && p.name() != "[builtin]" && Some(p.idx) != ctx.ecore_pack()
        })
        .ok_or(ArachneError::NoValidPackageFound)
}

/// Generates code from a parsed Ecore context.
/// Returns the generated classifiers CRDT objects and the generated reference management code
///
/// Ecore's own classes that the package's classes extend or use (see `ecore_rs::repr::ecore`)
/// are generated with them, and take part in the analyses below as the package's classes do,
/// except `EObject`, which has no feature and is never generated: a supertype `EObject` adds no
/// field, and neither a containment nor a reference typed by `EObject` is generated, each with a
/// warning naming the feature.
pub fn generate_from_parser<'a>(
    parser: &'a EcoreParser,
    pack: &'a Pack,
) -> anyhow::Result<(Generator<'a>, Generator<'a>, Generator<'a>, usize)> {
    let mut classifiers = Generator::new(CLASSIFIERS_PATH_MOD);
    let mut references = Generator::new(REFERENCES_PATH_MOD);
    let mut package = Generator::new(PACKAGE_PATH_MOD);

    let cycle_analysis = analyze_cycles(&parser.ctx)?;

    let package_classes: Vec<idx::Class> = pack.classes().iter().copied().collect();
    // The classes the analyses below range over: the package's, then Ecore's but `EObject`.
    let ecore_classes: Vec<idx::Class> = match parser.ctx.ecore_pack() {
        Some(ecore_pack) => parser.ctx[ecore_pack]
            .classes()
            .iter()
            .copied()
            .filter(|class_idx| !is_eobject(&parser.ctx, *class_idx))
            .collect(),
        None => Vec::new(),
    };
    let package_classes_and_ecore: Vec<idx::Class> = package_classes
        .iter()
        .chain(ecore_classes.iter())
        .copied()
        .collect();
    let package_class_set: std::collections::HashSet<idx::Class> =
        package_classes_and_ecore.iter().copied().collect();

    let top_level_roots = compute_top_level_roots(
        &parser.ctx,
        &package_classes,
        &package_classes_and_ecore,
        &package_class_set,
    );

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
    // An abstract class of Ecore that no generated class extends, such as `ENamedElement` reached
    // as a subclass of `EModelElement`, is left out instead of being skipped with a warning.
    reachable_classes.retain(|class_idx| {
        !parser.ctx.is_ecore_class(*class_idx)
            || !is_uninhabited_polymorphic_class(&parser.ctx, &parser.ctx[*class_idx])
    });

    // Get all classes in the package
    let classes: Vec<&Class> = parser
        .ctx
        .classes()
        .iter()
        .filter(|c| reachable_classes.contains(&c.idx) || c.is_enum())
        .collect();

    // Sort classes topologically by inheritance hierarchy
    let sorted_classes = topological_sort(&parser.ctx, &classes);
    // The package's classes and the classes of Ecore generated with them.
    let reachable_package_classes: Vec<idx::Class> = package_classes_and_ecore
        .iter()
        .copied()
        .filter(|idx| reachable_classes.contains(idx))
        .collect();
    let generated_class_count = reachable_package_classes
        .iter()
        .filter(|idx| !parser.ctx.is_ecore_class(**idx))
        .count();
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

/// Computes the top-level root classes of a package: the instantiable,
/// non-enum, non-interface classes no other instantiable class contains —
/// falling back to abstract classes and interfaces with concrete descendants
/// and no external container when there is no such instantiable class.
///
/// `package_classes` are the package's own classes, the only candidates for a
/// root. `package_classes_and_ecore` adds the built-in Ecore classes the
/// package reaches, which contain and are contained like the package's own and
/// so must be searched when asking whether a candidate has a container.
///
/// Shared by code generation and metamodel-descriptor emission, so both name
/// the same roots. Empty when the package has no viable root; callers decide
/// whether that is an error.
fn compute_top_level_roots(
    ctx: &ecore_rs::ctx::Ctx,
    package_classes: &[idx::Class],
    package_classes_and_ecore: &[idx::Class],
    package_class_set: &std::collections::HashSet<idx::Class>,
) -> Vec<idx::Class> {
    let concrete_containment_incoming =
        compute_concrete_containment_incoming(ctx, package_classes_and_ecore, package_class_set);

    let mut top_level_roots: Vec<idx::Class> = package_classes
        .iter()
        .copied()
        .filter(|class_idx| {
            let class = &ctx.classes()[**class_idx];
            codegen::classifier::is_instantiable_class(class)
                && !class.is_enum()
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
                let class = &ctx.classes()[**class_idx];
                !class.is_enum()
                    && (class.is_interface() || !class.is_concrete())
                    && has_concrete_descendant(ctx, *class_idx, package_class_set)
                    && abstract_family_has_no_external_container(
                        ctx,
                        *class_idx,
                        package_classes_and_ecore,
                        package_class_set,
                    )
            })
            .collect();
    }

    top_level_roots
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

    /// A key put without a value stays in the map, so an optional attribute value cannot be an
    /// `OptionLog`, whose unset state `UWMapLog` does not read.
    #[test]
    fn uw_map_with_an_optional_attribute_value_uses_a_register_over_option() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Model">
        <eStructuralFeatures xsi:type="ecore:EReference" name="entries" upperBound="-1" eType="#//Entry" containment="true">
            <eAnnotations source="urn:arachne:semantics">
                <details key="datatype" value="uw-map"/>
            </eAnnotations>
        </eStructuralFeatures>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Entry">
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="key" lowerBound="1" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="value" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    </eClassifiers>
</ecore:EPackage>
"##;

        let (classifiers, _references) = generate_modules_from_str(ecore);

        assert!(classifiers.contains(
            "entries:__classifiers::UWMapLog<std::string::String,__classifiers::VecLog<__classifiers::MVRegister<Option<std::string::String>>>>"
        ));
        assert!(classifiers.contains("pubusemoirai_crdt::register::mv_register::MVRegister;"));
        assert!(!classifiers.contains("record!(Entry"));
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

    /// A metamodel with a transparent class and an enumeration whose Ecore
    /// name is not already upper camel case, which is the pair the ModelSet
    /// census of 2026-09-09 found thirty-one instances of the second half of
    /// and no instance of both halves at once.
    ///
    /// `swmlTypes` is the shape of the real name: ModelSet carries `SWMLTypes`
    /// in six files, `types` in four and `Is_Style` in two, and every
    /// enumeration of the fourteen checked-in `.ecore` files is a fixed point
    /// of `to_upper_camel_case`, which is why nothing in the repository could
    /// reach this.
    const NON_CAMEL_ENUM_WITH_A_TRANSPARENT_CLASS: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore" name="casing" nsURI="http://www.example.org/casing" nsPrefix="casing">
    <eClassifiers xsi:type="ecore:EEnum" name="swmlTypes">
        <eLiterals name="Text"/>
        <eLiterals name="Number" value="1"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Value" abstract="true"/>
    <eClassifiers xsi:type="ecore:EClass" name="Tagged" eSuperTypes="#//Value">
        <eAnnotations source="urn:arachne:representation">
            <details key="kind" value="transparent"/>
            <details key="field" value="tag"/>
        </eAnnotations>
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="tag" lowerBound="1" eType="#//swmlTypes"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Root">
        <eStructuralFeatures xsi:type="ecore:EAttribute" name="tag" lowerBound="1" eType="#//swmlTypes"/>
        <eStructuralFeatures xsi:type="ecore:EReference" name="values" upperBound="-1" eType="#//Value" containment="true"/>
    </eClassifiers>
</ecore:EPackage>
"##;

    /// The enumeration is declared under one name and every field that names
    /// it uses that name.
    ///
    /// Before arachne fixed this, `generate_enum` declared the Rust enum under
    /// the raw Ecore name while `transparent_field_types` typed the
    /// transparent variant's field by the upper camel cased one, so this
    /// metamodel emitted a `MVRegister<SwmlTypes>` field against an
    /// `enum swmlTypes` declaration and the generated crate did not compile.
    /// The rest of the generator spells every Rust type it makes from a
    /// classifier's name upper camel case, so that is the spelling all four
    /// enumeration sites now use.
    #[test]
    fn a_non_camel_enum_is_declared_and_referred_to_under_one_upper_camel_name() {
        let (classifiers, _references) =
            generate_modules_from_str(NON_CAMEL_ENUM_WITH_A_TRANSPARENT_CLASS);

        assert!(
            classifiers.contains("enumSwmlTypes{"),
            "the enum should be declared as `SwmlTypes`, not under its raw Ecore name: {classifiers}"
        );
        assert!(
            classifiers.contains("MVRegister<SwmlTypes>"),
            "every field typed by the enum should name `SwmlTypes`: {classifiers}"
        );
        assert!(
            !classifiers.contains("swmlTypes"),
            "no site should keep the raw Ecore spelling `swmlTypes`: {classifiers}"
        );
    }

    #[test]
    fn concrete_superclass_with_subclasses_emits_family_union() {
        let (classifiers, _references) = generate_modules_from_file(
            "../examples/pet_metamodels/concrete_inherits_concrete.ecore",
        );

        assert!(classifiers.contains("__classifiers::record!(A{"));
        assert!(classifiers.contains("__classifiers::union!(AKind=A(A,ALog=>AValue)|B(B,BLog=>BValue));"));
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
            "__classifiers::union!(TypesLibraryKind=NativeTypesLibrary(NativeTypesLibrary,NativeTypesLibraryLog=>NativeTypesLibraryValue)|UserDefinedTypesLibrary(UserDefinedTypesLibrary,UserDefinedTypesLibraryLog=>UserDefinedTypesLibraryValue));"
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

        assert!(classifiers.contains("__classifiers::union!(AKind=A(A,ALog=>AValue)|B(BKind,BKindLog=>BKindValue));"));
        assert!(classifiers.contains("__classifiers::union!(BKind=B(B,BLog=>BValue)|C(C,CLog=>CValue));"));
        assert!(classifiers.contains("D{child:__classifiers::OptionLog<AKindLog>=>Option<AKindValue>,}"));
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
            "Select{union:__classifiers::OptionLog<__classifiers::BoxedLog<UnionLog>>=>Option<Box<UnionValue>>,}"
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

        assert!(classifiers.contains("AKind=C(C,CLog=>CValue)"));
        assert!(classifiers.contains("BKind=C(C,CLog=>CValue)"));
    }

    #[test]
    fn interface_is_generated_like_abstract_class() {
        let ecore = include_str!("../../examples/pet_metamodels/kitchen_sink.ecore").replace(
            r#"name="Abstract" abstract="true""#,
            r#"name="Abstract" interface="true""#,
        );

        let (classifiers, references) = generate_modules_from_str(&ecore);

        assert!(classifiers.contains("__classifiers::union!(AbstractKind=Baz(Baz,BazLog=>BazValue));"));
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
                .contains("__classifiers::record!(Method{named_element_super:NamedElementLog=>NamedElementValue,});")
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

    /// A reference typed by Ecore's `EObject` refers to an object of any class, which would need
    /// an arc to a vertex of any kind: it is refused, with a warning naming the feature, and
    /// nothing is emitted for it.
    #[test]
    fn reference_to_any_object_is_refused_with_a_warning() {
        use crate::codegen::{
            cycles::analyze_cycles,
            generate::Generate,
            package::PackageGenerator,
            reference::{ReferenceGenerator, analysis::analyze_references},
        };

        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Model">
        <eStructuralFeatures xsi:type="ecore:EReference" name="parts" upperBound="-1" eType="#//Part" containment="true"/>
        <eStructuralFeatures xsi:type="ecore:EReference" name="ports" upperBound="-1" eType="#//Port" containment="true"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Part">
        <eStructuralFeatures xsi:type="ecore:EReference" name="subject" eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EObject"/>
        <eStructuralFeatures xsi:type="ecore:EReference" name="port" eType="#//Port"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Port"/>
</ecore:EPackage>
"##;

        let parser = EcoreParser::from_string(ecore).expect("ecore should parse");
        let ctx = &parser.ctx;
        let pack = ctx
            .packs()
            .iter()
            .find(|p| p.name() == "test")
            .expect("package should exist");
        let classes: Vec<_> = pack.classes().iter().copied().collect();
        let model = *classes
            .iter()
            .find(|idx| ctx[**idx].name() == "Model")
            .unwrap();
        let cycles = analyze_cycles(ctx).expect("cycle analysis should succeed");
        let analysis = analyze_references(ctx, &classes);

        let references = normalize(
            ReferenceGenerator::new(ctx, classes.clone(), vec![model], &cycles)
                .generate()
                .expect("references should generate")
                .tokens(),
        );
        // `Part.port` is generated as any reference is; nothing at all is emitted for
        // `Part.subject`: no edge type, no arc, no vertex kind, and no `EcoreEObject` anywhere.
        assert!(references.contains("PartPortEdge[0,1]"));
        assert!(references.contains("PartToPort:PartId->PortId(PartPortEdge)"));
        assert!(references.contains("vertices{PartId,PortId}"));
        assert!(!references.contains("Subject"), "{references}");
        assert!(!references.contains("EcoreEObject"), "{references}");
        assert!(
            !references.contains("object_from_sink_kind"),
            "{references}"
        );

        let package = normalize(
            PackageGenerator::new(ctx, pack.idx, vec![model], &analysis)
                .generate()
                .expect("package should generate")
                .tokens(),
        );
        // No object is given a second vertex: the package adds one vertex per sink, and no more.
        assert!(package.contains("__package::instance_from_sink_kind(kind,sink.path())"));
        assert!(!package.contains("object_from_sink_kind"), "{package}");
        assert_eq!(package.matches("ReferenceManager::AddVertex").count(), 1);

        let warnings: Vec<String> = analysis.warnings.iter().map(|w| w.message()).collect();
        assert_eq!(
            warnings,
            vec![
                "Reference `Part.subject` refers to an object of any class (it is typed by Ecore's `EObject`), which is not supported: it is not generated. It would need an arc whose target is a vertex of any kind, which Moirai's typed graph does not offer."
                    .to_string()
            ]
        );
    }

    /// Generates `ecore` and returns its classifiers, references and package, normalized, and the
    /// warnings of the three.
    fn generate_all_from_parser(parser: &EcoreParser) -> (String, String, String, Vec<String>) {
        let pack = parser
            .ctx
            .packs()
            .iter()
            .find(|p| p.name() != "[root]" && p.name() != "[builtin]")
            .expect("package should exist");
        let (classifiers, references, package, _generated_class_count) =
            generate_from_parser(parser, pack).expect("generation should succeed");
        let warnings = [&classifiers, &references, &package]
            .iter()
            .flat_map(|generator| generator.warning_messages())
            .collect();

        (
            normalize(classifiers.build()),
            normalize(references.build()),
            normalize(package.build()),
            warnings,
        )
    }

    /// `Element` extends Ecore's `EModelElement` and `Port` its `ENamedElement`, `Model` extends
    /// `EObject`, and `Part.subject` is typed by `EObject`: the classes of Ecore they use are
    /// generated with them, under the `Ecore` prefix, and `EObject` is not.
    #[test]
    fn ecore_classes_a_package_uses_are_generated_with_it() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/pet_metamodels/ecore_builtins.ecore");
        let parser = EcoreParser::from_file(path).expect("ecore should parse");
        let (classifiers, references, _package, warnings) = generate_all_from_parser(&parser);

        // An explicit `EObject` supertype adds no field.
        assert!(classifiers.contains(
            "__classifiers::record!(Model{parts:__classifiers::NestedListLog<PartLog>=>Vec<PartValue>,});"
        ));
        assert!(!classifiers.contains("EcoreEObject"));
        // Inherited features, as from any supertype.
        assert!(classifiers.contains(
            "__classifiers::record!(Element{e_model_element_super:EcoreEModelElementLog=>EcoreEModelElementValue,"
        ));
        assert!(classifiers.contains(
            "__classifiers::record!(Port{e_named_element_super:EcoreENamedElementLog=>EcoreENamedElementValue,});"
        ));
        assert!(classifiers.contains(
            "__classifiers::record!(EcoreENamedElement{e_model_element_super:EcoreEModelElementLog=>EcoreEModelElementValue,name:__classifiers::OptionLog<__classifiers::GraphLog<__classifiers::List<char>>>=>Option<Vec<char>>,});"
        ));
        assert!(classifiers.contains(
            "__classifiers::union!(EcoreEModelElementKind=EcoreEAnnotation(EcoreEAnnotation,EcoreEAnnotationLog=>EcoreEAnnotationValue)|EcoreENamedElement(EcoreENamedElementKind,EcoreENamedElementKindLog=>EcoreENamedElementKindValue)|Element(ElementKind,ElementKindLog=>ElementKindValue));"
        ));
        // `eAnnotations` is an ordered containment, `source` an optional string, `details` a map
        // from a key to an optional string; `contents` and `eModelElement` are not generated.
        assert!(classifiers.contains(
            "__classifiers::record!(EcoreEModelElement{e_annotations:__classifiers::NestedListLog<__classifiers::BoxedLog<EcoreEAnnotationLog>>=>Vec<Box<EcoreEAnnotationValue>>,});"
        ));
        assert!(classifiers.contains(
            "__classifiers::record!(EcoreEAnnotation{e_model_element_super:EcoreEModelElementLog=>EcoreEModelElementValue,source:__classifiers::OptionLog<__classifiers::GraphLog<__classifiers::List<char>>>=>Option<Vec<char>>,details:__classifiers::UWMapLog<std::string::String,__classifiers::VecLog<__classifiers::MVRegister<Option<std::string::String>>>>=>__classifiers::HashMap<std::string::String,__classifiers::HashSet<Option<std::string::String>>>,});"
        ));
        assert!(!classifiers.contains("EcoreEStringToStringMapEntry"));

        // A reference typed by `EObject` is not generated at all; `EModelElement` is expanded over
        // the generated classes, as any class is.
        assert!(!references.contains("EcoreEObject"), "{references}");
        assert!(!references.contains("Subject"), "{references}");
        assert!(!references.contains("References"), "{references}");
        assert!(
            references
                .contains("PortToEcoreEAnnotation:PortId->EcoreEAnnotationId(PortAnnotatedEdge)")
        );
        assert!(references.contains("PortToPart:PortId->PartId(PortAnnotatedEdge)"));
        assert!(references.contains("PortToPort:PortId->PortId(PortAnnotatedEdge)"));
        assert!(!references.contains("EModelElementEdge"));

        assert!(
            warnings
                .iter()
                .any(|w| w
                    .starts_with("Containment `EAnnotation.contents` holds objects of any class")),
            "{warnings:#?}"
        );
        for feature in ["Part.subject", "EAnnotation.references"] {
            assert!(
                warnings.iter().any(|w| *w
                    == format!(
                        "Reference `{feature}` refers to an object of any class (it is typed by Ecore's `EObject`), which is not supported: it is not generated. It would need an arc whose target is a vertex of any kind, which Moirai's typed graph does not offer."
                    )),
                "{warnings:#?}"
            );
        }
        assert!(
            warnings.iter().any(|w| w.starts_with(
                "Reference `EAnnotation.eModelElement` of Ecore's own classes is transient"
            )),
            "{warnings:#?}"
        );
    }

    /// A feature typed by one of Ecore's classes is generated as for any class, over the family of
    /// that class among the generated classes, and contains its objects as any containment does.
    #[test]
    fn features_typed_by_ecore_classes_are_generated_like_any_feature() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Model">
        <eStructuralFeatures xsi:type="ecore:EReference" name="notes" upperBound="-1"
            eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EAnnotation" containment="true"/>
        <eStructuralFeatures xsi:type="ecore:EReference" name="elements" upperBound="-1"
            eType="ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EModelElement" containment="true"/>
    </eClassifiers>
    <eClassifiers xsi:type="ecore:EClass" name="Part"
        eSuperTypes="http://www.eclipse.org/emf/2002/Ecore#//EModelElement"/>
</ecore:EPackage>
"##;

        let parser = EcoreParser::from_string(ecore).expect("ecore should parse");
        let (classifiers, _references, package, _warnings) = generate_all_from_parser(&parser);

        assert!(classifiers.contains(
            "__classifiers::record!(Model{notes:__classifiers::NestedListLog<EcoreEAnnotationLog>=>Vec<EcoreEAnnotationValue>,elements:__classifiers::NestedListLog<EcoreEModelElementKindLog>=>Vec<EcoreEModelElementKindValue>,});"
        ));
        assert!(classifiers.contains(
            "__classifiers::union!(EcoreEModelElementKind=EcoreEAnnotation(EcoreEAnnotation,EcoreEAnnotationLog=>EcoreEAnnotationValue)|Part(Part,PartLog=>PartValue));"
        ));
        // `Part` is contained through `Model.elements`, so `Model` is the only root. The only
        // non-containment reference the built-ins bring in is `EAnnotation.references`, typed by
        // `EObject` and refused, so the package has no reference manager at all.
        assert!(package.contains("pubenumTest{Model(crate::classifiers::Model)}"));
        assert!(!package.contains("Part(crate::classifiers::Part)"));
        assert!(!package.contains("AddReference"), "{package}");
    }
}
