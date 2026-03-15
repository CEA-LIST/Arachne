use std::{fs, path::Path};

use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    config::{Config, Formatting},
    error::{ArachneError, Result},
};

/// Writes a complete Rust project for the generated code.
pub fn write_project(
    config: &Config,
    project_name: &str,
    classifiers_code: TokenStream,
    references_code: TokenStream,
    package_code: TokenStream,
) -> Result<()> {
    let project_name = sanitize_package_name(project_name);
    let root = &config.output_dir;
    let src_dir = root.join("src");

    fs::create_dir_all(&src_dir)?;

    let main_rs = render_main_rs();

    let (formatted_classifiers, formatted_references, formatted_package, formatted_main) =
        match config.format_code {
            Formatting::None => {
                // Do not format the code
                (
                    classifiers_code.to_string(),
                    references_code.to_string(),
                    package_code.to_string(),
                    main_rs.to_string(),
                )
            }
            Formatting::Rustfmt => (
                format_with_rustfmt(classifiers_code)?,
                format_with_rustfmt(references_code)?,
                format_with_rustfmt(package_code)?,
                format_with_rustfmt(main_rs)?,
            ),
            Formatting::Prettyplease => (
                format_with_prettyplease(classifiers_code)?,
                format_with_prettyplease(references_code)?,
                format_with_prettyplease(package_code)?,
                format_with_prettyplease(main_rs)?,
            ),
        };

    let cargo_toml = render_cargo_toml(&project_name, &config.moirai_root)?;

    fs::write(root.join("Cargo.toml"), cargo_toml)?;
    fs::write(src_dir.join("main.rs"), formatted_main)?;
    fs::write(src_dir.join("classifiers.rs"), formatted_classifiers)?;
    fs::write(src_dir.join("references.rs"), formatted_references)?;
    fs::write(src_dir.join("package.rs"), formatted_package)?;

    Ok(())
}

fn render_cargo_toml(project_name: &str, moirai_root: &Path) -> Result<String> {
    let moirai_root = fs::canonicalize(moirai_root)
        .map_err(|e| ArachneError::Config(format!("Failed to resolve moirai root: {e}")))?;

    let moirai_crdt = moirai_root.join("moirai-crdt");
    let moirai_protocol = moirai_root.join("moirai-protocol");
    let moirai_macros = moirai_root.join("moirai-macros");

    Ok(format!(
        "[package]\nname = \"{project_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nmoirai-crdt = {{ path = \"{}\" }}\nmoirai-protocol = {{ path = \"{}\" }}\nmoirai-macros = {{ path = \"{}\" }}\npetgraph = \"0.8.3\"\nxml-builder = \"0.5.4\" \n",
        to_path_string(&moirai_crdt),
        to_path_string(&moirai_protocol),
        to_path_string(&moirai_macros)
    ))
}

fn render_main_rs() -> TokenStream {
    quote! {
        mod package;
        mod classifiers;
        mod references;

        fn main() {}
    }
}

fn sanitize_package_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;

    for ch in name.chars() {
        let lower = ch.to_ascii_lowercase();
        let is_valid = lower.is_ascii_alphanumeric();

        if is_valid {
            out.push(lower);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }

    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "generated-crdt".to_string()
    } else {
        trimmed
    }
}

fn to_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn format_with_prettyplease(tokens: TokenStream) -> Result<String> {
    let syntax_tree = syn::parse2(tokens)?;
    Ok(prettyplease::unparse(&syntax_tree))
}

fn format_with_rustfmt(tokens: TokenStream) -> Result<String> {
    let mut rustfmt = std::process::Command::new("rustfmt")
        .arg("--emit")
        .arg("stdout")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| ArachneError::Config(format!("Failed to spawn rustfmt: {e}")))?;

    {
        let stdin = rustfmt
            .stdin
            .as_mut()
            .ok_or_else(|| ArachneError::Config("Failed to open rustfmt stdin".to_string()))?;
        use std::io::Write;
        stdin.write_all(tokens.to_string().as_bytes())?;
    }

    let output = rustfmt
        .wait_with_output()
        .map_err(|e| ArachneError::Config(format!("Failed to read rustfmt output: {e}")))?;

    String::from_utf8(output.stdout).map_err(|e| {
        ArachneError::Config(format!("Failed to convert rustfmt output to string: {e}"))
    })
}
