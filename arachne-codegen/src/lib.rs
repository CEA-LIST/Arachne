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
use log::{debug, info};
pub use parser::EcoreParser;

use crate::{
    codegen::{
        classifier::ClassGenerator,
        cycles::analyze_cycles,
        generate::Generate,
        generator::Generator,
        package::PackageGenerator,
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
        .filter(|class_idx| parser.ctx.classes()[**class_idx].is_concrete())
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
                    && !class.is_interface()
                    && !class.is_concrete()
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
    let package_gen =
        PackageGenerator::new(&parser.ctx, pack.idx, top_level_roots, &reference_analysis);
    let fragment = package_gen.generate()?;
    package.register(fragment);

    Ok((
        classifiers,
        references,
        package,
        reachable_package_classes.len(),
    ))
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
        if class.is_concrete() {
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
        if class.is_concrete() {
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

    #[test]
    fn concrete_superclass_with_subclasses_emits_family_union() {
        let (classifiers, _references) =
            generate_modules_from_file("../examples/concrete_inherits_concrete.ecore");

        assert!(classifiers.contains("__classifiers::record!(A{"));
        assert!(classifiers.contains("__classifiers::union!(AKind=A(A,ALog)|B(B,BLog));"));
    }

    #[test]
    fn containment_typed_by_concrete_superclass_uses_family_log() {
        let (classifiers, _references) =
            generate_modules_from_file("../examples/concrete_polymorphic_targets.ecore");

        assert!(classifiers.contains("__classifiers::union!(AKind=A(A,ALog)|B(BKind,BKindLog));"));
        assert!(classifiers.contains("__classifiers::union!(BKind=B(B,BLog)|C(C,CLog));"));
        assert!(classifiers.contains("D{child:__classifiers::OptionLog<AKindLog>,}"));
    }

    #[test]
    fn non_containment_reference_typed_by_concrete_superclass_expands_to_family() {
        let (_classifiers, references) =
            generate_modules_from_file("../examples/concrete_polymorphic_targets.ecore");

        assert!(references.contains("DTargetEdge[0,1]"));
        assert!(references.contains("DToA:DId->AId(DTargetEdge)"));
        assert!(references.contains("DToB:DId->BId(DTargetEdge)"));
        assert!(references.contains("DToC:DId->CId(DTargetEdge)"));
    }
}
