pub mod codegen;
pub mod config;
pub mod error;
pub mod parser;
mod project;
mod utils;

pub use config::Config;
use ecore_rs::repr::Class;
pub use error::{AtraktosError, Result};
pub use parser::EcoreParser;
use proc_macro2::TokenStream;

use crate::{
    codegen::{
        classifier::class::ClassGenerator, cycles::analyze_cycles, generate::Generate,
        generator::Generator, reference::ModelGenerator,
    },
    utils::topo::topological_sort,
};

/// Main entry point for code generation
pub fn generate(config: Config) -> anyhow::Result<()> {
    // Validate configuration
    config.validate()?;

    // Parse the Ecore metamodel
    let parser = EcoreParser::from_file(&config.input_path)?;

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

    // Write a full Rust project
    project::write_project(
        &config,
        &project_name,
        &formatted,
        formatted_model.as_deref(),
    )?;

    if config.debug {
        println!("Generated project written to: {:?}", config.output_dir);
    }

    Ok(())
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

    // Find root class (first class in sorted order that has no superclass,
    // or explicitly the class named "Root" if it exists)
    let root_class = sorted_classes
        .iter()
        .find(|c| c.name() == "Root")
        .or_else(|| {
            sorted_classes
                .iter()
                .find(|c| c.sup().is_empty() && !c.is_enum())
        })
        .expect("No root class found in the model");

    // Generate reference management code (model.rs)
    let model_gen = ModelGenerator::new(
        &parser.ctx,
        root_class.idx,
        class_indices.iter().copied().collect(),
        &cycle_analysis,
    );

    let model_tokens = model_gen.generate().map(|fragment| {
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
