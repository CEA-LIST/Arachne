use std::process;

use atraktos::{Config, generate};

fn main() {
    // Configuration: path to the Ecore metamodel
    let config = Config::new("./examples/bt.ecore")
        .with_output_dir(".output")
        .with_project_name("kitchen_sink")
        .with_debug(true);

    // Generate code based on the provided configuration
    if let Err(e) = generate(config) {
        eprintln!("Error during code generation: {}", e);
        process::exit(1);
    }

    println!("Code generation completed successfully.");
}
