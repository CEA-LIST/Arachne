pub mod codegen;
pub mod config;
pub mod error;
pub mod parser;
mod project;
mod utils;

use std::path::PathBuf;

pub use config::Config;
use ecore_rs::repr::{Class, Pack, idx};
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

    info!("Generating rust tokens");
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

    // Find root class (first class in sorted order that has no superclass)
    let root_class = sorted_classes
        .iter()
        .find(|c| c.sup().is_empty() && !c.is_enum() && !c.is_interface())
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
