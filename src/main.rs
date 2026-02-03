use ecore_rs::ctx::Ctx;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: atraktos <ecore_file>");
        std::process::exit(1);
    }

    let ecore_path = &args[1];

    let ecore_content = std::fs::read_to_string(ecore_path).unwrap_or_else(|err| {
        eprintln!("Failed to read file {}: {}", ecore_path, err);
        std::process::exit(1);
    });

    match generate_crdts_from_ecore(&ecore_content) {
        Ok(output) => println!("{}", output),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Generates CRDT code from an Ecore metamodel
///
/// # Arguments
/// * `ecore_path` - Path to the Ecore metamodel file (.ecore)
///
/// # Returns
/// * `Result<String, String>` - Generated Rust code or error message
fn generate_crdts_from_ecore(content: &str) -> Result<String, String> {
    let ctx = Ctx::parse(content).unwrap_or_else(|err| {
        println!("an error occurred:");
        for line in err.to_string().lines() {
            println!("- {}", line)
        }
        panic!("run failed")
    });
    ctx.classes();
    todo!()
}
