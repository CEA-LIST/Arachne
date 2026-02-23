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
    codegen::{classifier::class::ClassGenerator, generate::Generate, generator::Generator},
    utils::topo::topological_sort,
};

/// Main entry point for code generation
pub fn generate(config: Config) -> anyhow::Result<()> {
    // Validate configuration
    config.validate()?;

    // Parse the Ecore metamodel
    let parser = EcoreParser::from_file(&config.input_path)?;

    // Generate code
    let generator = generate_from_parser(&parser)?;

    // Emit any warnings collected during generation
    generator.emit_warnings();

    // Build the final TokenStream
    let generated = generator.build();

    // Format the generated code
    let formatted = format_code(generated)?;

    // Choose a project name
    let project_name = config
        .project_name
        .clone()
        .or_else(|| Some(parser.ctx.packs()[parser.ctx.top_pack()].name().to_string()))
        .unwrap_or_else(|| "generated_crdt".to_string());

    // Write a full Rust project
    project::write_project(&config, &project_name, &formatted)?;

    if config.debug {
        println!("Generated project written to: {:?}", config.output_dir);
    }

    Ok(())
}

/// Generates code from a parsed Ecore context
pub fn generate_from_parser(parser: &EcoreParser) -> anyhow::Result<Generator> {
    let mut generator = Generator::new();

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

    for class in sorted_classes {
        let class_gen = ClassGenerator::new(class, &parser.ctx);
        let fragment = class_gen.generate()?;
        generator.register(fragment);
    }

    Ok(generator)
}

/// Formats generated code using prettyplease
pub fn format_code(tokens: TokenStream) -> Result<String> {
    let syntax_tree = syn::parse2(tokens)?;
    Ok(prettyplease::unparse(&syntax_tree))
}
