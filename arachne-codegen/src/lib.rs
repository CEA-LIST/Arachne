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
        annotation::transparent_field,
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
    let (classifiers, references, package) = generate_from_parser(&parser, pack)?;

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
        class_count,
    })
}

/// Generates code from a parsed Ecore context.
/// Returns the generated classifiers CRDT objects and the generated reference management code
pub fn generate_from_parser<'a>(
    parser: &'a EcoreParser,
    pack: &'a Pack,
) -> anyhow::Result<(Generator<'a>, Generator<'a>, Generator<'a>)> {
    let mut classifiers = Generator::new(CLASSIFIERS_PATH_MOD);
    let mut references = Generator::new(REFERENCES_PATH_MOD);
    let mut package = Generator::new(PACKAGE_PATH_MOD);

    let cycle_analysis = analyze_cycles(&parser.ctx)?;

    let package_classes: Vec<idx::Class> = pack.classes().iter().copied().collect();

    // Get all classes in the package
    let classes: Vec<&Class> = parser
        .ctx
        .classes()
        .iter()
        .filter(|c| package_classes.contains(&c.idx))
        .collect();

    // Sort classes topologically by inheritance hierarchy
    let sorted_classes = topological_sort(&parser.ctx, &classes);
    let reference_analysis = analyze_references(&parser.ctx, &package_classes);

    // Derive the package root from top-level concrete containment roots.
    let contained_classes: std::collections::HashSet<idx::Class> = sorted_classes
        .iter()
        .flat_map(|class| {
            class.structural().iter().filter_map(|feature| {
                (feature.kind == structural::Typ::EReference
                    && feature.containment
                    && feature
                        .typ
                        .is_some_and(|target| package_classes.contains(&target)))
                .then_some(feature.typ.unwrap())
            })
        })
        .collect();

    let top_level_concrete_roots: Vec<&Class> = sorted_classes
        .iter()
        .copied()
        .filter(|c| {
            !c.is_enum()
                && !c.is_interface()
                && c.is_concrete()
                && !contained_classes.contains(&c.idx)
        })
        .collect();

    let transparent_union_candidates = top_level_concrete_roots.len() > 1
        && top_level_concrete_roots
            .iter()
            .all(|class| transparent_field(class).is_some());

    let root_class = if top_level_concrete_roots.len() == 1 {
        Some(top_level_concrete_roots[0])
    } else if transparent_union_candidates {
        find_common_ancestor(&parser.ctx, &top_level_concrete_roots)
            .or_else(|| top_level_concrete_roots.first().copied())
    } else if !top_level_concrete_roots.is_empty() {
        top_level_concrete_roots.first().copied()
    } else {
        sorted_classes
            .iter()
            .copied()
            .find(|c| c.sup().is_empty() && !c.is_enum() && !c.is_interface())
    }
    .ok_or_else(|| ArachneError::RootClassNotFound(pack.name().to_string()))?;

    debug!(
        "Identified root class `{}` for package `{}`",
        root_class.name(),
        pack.name()
    );

    for class in &sorted_classes {
        let class_gen = ClassGenerator::new(class, &parser.ctx, &cycle_analysis);
        let fragment = class_gen.generate()?;
        classifiers.register(fragment);
    }

    let refs = ReferenceGenerator::new(&parser.ctx, package_classes);
    let fragment = refs.generate()?;
    references.register(fragment);

    let package_gen = PackageGenerator::new(
        &parser.ctx,
        pack.idx,
        root_class.idx,
        &reference_analysis,
        &cycle_analysis,
    );
    let fragment = package_gen.generate()?;
    package.register(fragment);

    Ok((classifiers, references, package))
}

fn find_common_ancestor<'a>(
    ctx: &'a ecore_rs::ctx::Ctx,
    classes: &[&'a Class],
) -> Option<&'a Class> {
    let first = *classes.first()?;
    let mut candidates = ancestor_chain(ctx, first.idx);
    candidates.reverse();

    candidates
        .into_iter()
        .find(|candidate_idx| {
            classes.iter().all(|class| {
                let ancestors = ancestor_chain(ctx, class.idx);
                ancestors.contains(candidate_idx)
            })
        })
        .map(|idx| &ctx.classes()[*idx])
}

fn ancestor_chain(ctx: &ecore_rs::ctx::Ctx, class_idx: idx::Class) -> Vec<idx::Class> {
    let mut chain = vec![class_idx];
    let mut current = class_idx;

    while let Some(parent) = ctx.classes()[*current].sup().first().copied() {
        chain.push(parent);
        current = parent;
    }

    chain
}
