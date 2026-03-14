pub mod codegen;
pub mod config;
pub mod error;
pub mod parser;
mod project;
mod utils;

use std::path::PathBuf;

pub use config::Config;
use ecore_rs::repr::{Class, Pack};
pub use error::{ArachneError, Result};
use log::{debug, info, warn};
pub use parser::EcoreParser;
use proc_macro2::TokenStream;

use crate::{
    codegen::{
        classifier::class::ClassGenerator,
        cycles::analyze_cycles,
        generate::Generate,
        generator::Generator,
        reference::{PackageGenerator, analysis::analyze_references},
    },
    utils::topo::topological_sort,
};

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
    // Validate configuration
    config.validate()?;

    info!("Parsing ecore metamodel: {:?}", config.input_path);
    // Parse the Ecore metamodel
    let parser = EcoreParser::from_file(&config.input_path)?;

    let packs = parser
        .ctx
        .packs()
        .iter()
        .filter(|p| p.name() != "[root]" && p.name() != "[builtin]")
        .collect::<Vec<&Pack>>();
    if packs.is_empty() {
        return Err(anyhow::anyhow!("No EPackage found in metamodel"));
    }
    if packs.len() > 1 {
        warn!(
            "Multiple packages found in metamodel, using first non-root package: `{}`. Other packages will be ignored.",
            packs[0].name()
        );
    }
    let pack = packs[0];

    let class_count = pack.classes().len();
    debug!(
        "Found package `{}` with {} classes",
        pack.name(),
        class_count
    );

    info!("Generating rust tokens");
    // Generate code
    let (generator, model_tokens) = generate_from_parser(&parser)?;

    // Emit any warnings collected during generation
    generator.emit_warnings();

    // Build the final TokenStream
    let generated = generator.build();

    // Format the generated code
    let formatted = format_code(generated)?;

    // Format the model code (if any)
    let formatted_model = model_tokens.map(format_code).transpose()?;

    // Choose a project name
    let project_name = config
        .project_name
        .clone()
        .or_else(|| Some(parser.ctx.packs()[parser.ctx.top_pack()].name().to_string()))
        .unwrap_or_else(|| "generated_crdt".to_string());

    info!("Writing generated project '{}'", project_name);
    // Write a full Rust project
    project::write_project(
        &config,
        &project_name,
        &formatted,
        formatted_model.as_deref(),
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
/// Returns the CRDT generator and optionally the model TokenStream (if non-containment refs exist).
pub fn generate_from_parser(
    parser: &EcoreParser,
) -> anyhow::Result<(Generator, Option<TokenStream>)> {
    let mut generator = Generator::new();
    let cycle_analysis = analyze_cycles(&parser.ctx)?;

    let pack = parser
        .ctx
        .packs()
        .iter()
        .find(|p| p.name() != "[root]" && p.name() != "[builtin]")
        .unwrap();

    let class_indices = pack.classes();
    let package_classes: Vec<_> = class_indices.iter().copied().collect();

    // Get all classes in the package
    let classes: Vec<&Class> = parser
        .ctx
        .classes()
        .iter()
        .filter(|c| class_indices.contains(&c.idx))
        .collect();

    // Sort classes topologically by inheritance hierarchy
    let sorted_classes = topological_sort(&parser.ctx, &classes);

    for class in &sorted_classes {
        let class_gen = ClassGenerator::new(class, &parser.ctx, &cycle_analysis);
        let fragment = class_gen.generate()?;
        generator.register(fragment);
    }

    // Prefer a top-level class that defines non-containment references when possible.
    // This gives more useful generated package operations in simple models.
    let reference_analysis = analyze_references(&parser.ctx, &package_classes);

    // Find root class (first class in sorted order that has no superclass)
    let root_class = sorted_classes
        .iter()
        .find(|c| {
            c.sup().is_empty()
                && !c.is_enum()
                && !c.is_interface()
                && reference_analysis
                    .refs
                    .iter()
                    .any(|r| r.source_class == c.idx)
        })
        .or_else(|| {
            sorted_classes
                .iter()
                .find(|c| c.sup().is_empty() && !c.is_enum() && !c.is_interface())
        })
        .expect("No root class found in the model");

    // Generate reference management code (model.rs)
    let package_gen = PackageGenerator::new(
        &parser.ctx,
        root_class.idx,
        package_classes,
        pack.name(),
        &cycle_analysis,
    );

    let model_tokens = package_gen.generate().map(|fragment| {
        let (tokens, _, _) = fragment.into();
        tokens
    });

    Ok((generator, model_tokens))
}

/// Formats generated code using prettyplease
pub fn format_code(tokens: TokenStream) -> Result<String> {
    let syntax_tree = syn::parse2(tokens)?;
    Ok(prettyplease::unparse(&syntax_tree))
}
