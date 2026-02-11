pub mod codegen;
pub mod config;
pub mod error;
pub mod parser;
pub mod project;

use proc_macro2::TokenStream;
use std::collections::{HashMap, HashSet};

pub use config::Config;
pub use error::{AtraktosError, Result};
pub use parser::EcoreParser;

use crate::codegen::{classifier::class::ClassGenerator, generate::Generate, generator::Generator};
use ecore_rs::repr::Class;

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

/// Sorts classes in topological order of the inheritance hierarchy.
/// Parent classes come before child classes, allowing inheritance links to be generated.
fn topological_sort<'a>(ctx: &ecore_rs::ctx::Ctx, classes: &[&'a Class]) -> Vec<&'a Class> {
    let mut sorted = Vec::new();
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();

    // Build a map for quick lookup
    let class_map: HashMap<ecore_rs::repr::idx::Class, &'a Class> =
        classes.iter().map(|c| (c.idx, *c)).collect();

    fn visit<'a>(
        class: &'a Class,
        ctx: &ecore_rs::ctx::Ctx,
        class_map: &HashMap<ecore_rs::repr::idx::Class, &'a Class>,
        sorted: &mut Vec<&'a Class>,
        visited: &mut HashSet<ecore_rs::repr::idx::Class>,
        visiting: &mut HashSet<ecore_rs::repr::idx::Class>,
    ) {
        if visited.contains(&class.idx) {
            return;
        }

        if visiting.contains(&class.idx) {
            // Cycle detected, skip to avoid infinite loop
            return;
        }

        visiting.insert(class.idx);

        // Visit all superclasses first
        for super_idx in class.sup() {
            if let Some(super_class) = class_map.get(super_idx) {
                visit(super_class, ctx, class_map, sorted, visited, visiting);
            }
        }

        visiting.remove(&class.idx);
        visited.insert(class.idx);
        sorted.push(class);
    }

    for &class in classes {
        visit(
            class,
            ctx,
            &class_map,
            &mut sorted,
            &mut visited,
            &mut visiting,
        );
    }

    sorted
}

/// Formats generated code using prettyplease
pub fn format_code(tokens: TokenStream) -> Result<String> {
    let syntax_tree = syn::parse2(tokens)?;
    Ok(prettyplease::unparse(&syntax_tree))
}
