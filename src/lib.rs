//! Atraktos - A code generator for CRDTs from Ecore metamodels
//!
//! This library provides functionality to parse Ecore metamodels and generate
//! Conflict-free Replicated Data Types (CRDTs) using the Moirai library.
//!
//! # Example
//!
//! ```no_run
//! use atraktos::{Config, generate};
//!
//! let config = Config::new("./examples/bt.ecore")
//!     .with_output_dir(".output/generated_project");
//!
//! generate(config).expect("Failed to generate code");
//! ```

pub mod codegen;
pub mod config;
pub mod error;
pub mod parser;
pub mod project;

use proc_macro2::TokenStream;

pub use config::Config;
pub use error::{AtraktosError, Result};
pub use parser::EcoreParser;

/// Main entry point for code generation
pub fn generate(config: Config) -> Result<()> {
    // Validate configuration
    config.validate()?;

    // Parse the Ecore metamodel
    let parser = EcoreParser::from_file(&config.input_path)?;

    // Generate code
    let generated = generate_from_parser(&parser)?;

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
pub fn generate_from_parser(parser: &EcoreParser) -> Result<TokenStream> {
    let mut generator = codegen::Generator::new();

    // Generate code for each concrete EClass
    for class in parser
        .ctx
        .classes()
        .iter()
        .filter(|c| c.is_concrete() && c.typ() == "ecore:EClass")
    {
        let class_gen = codegen::class::ClassGenerator::new(class);

        if let Some(tokens) = class_gen.generate()? {
            generator.concat(tokens);
        }
    }

    Ok(generator.build())
}

/// Formats generated code using prettyplease
pub fn format_code(tokens: TokenStream) -> Result<String> {
    let syntax_tree = syn::parse2(tokens)?;
    Ok(prettyplease::unparse(&syntax_tree))
}
