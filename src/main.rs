use atraktos::{Config, generate};
use std::error::Error;
use std::process;

fn main() {
    // Configuration: path to the Ecore metamodel
    let config = Config::new("./examples/bt.ecore")
        .with_output_dir(".output")
        .with_project_name("bt-crdt")
        .with_debug(true);

    // Run the code generator
    if let Err(err) = generate(config) {
        eprintln!("Error: {}", err);
        eprintln!("\nDetails:");

        // Print error chain if available
        let mut source = Error::source(&err);
        while let Some(cause) = source {
            eprintln!("  Caused by: {}", cause);
            source = cause.source();
        }

        process::exit(1);
    }

    println!("Code generation completed successfully.");
}
