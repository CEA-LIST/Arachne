use std::fs;

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

    let lib_rs = render_lib_rs();

    let (formatted_classifiers, formatted_references, formatted_package, formatted_lib) =
        match config.format_code {
            Formatting::None => {
                // Do not format the code
                (
                    classifiers_code.to_string(),
                    references_code.to_string(),
                    package_code.to_string(),
                    lib_rs.to_string(),
                )
            }
            Formatting::Rustfmt => (
                format_with_rustfmt(classifiers_code)?,
                format_with_rustfmt(references_code)?,
                format_with_rustfmt(package_code)?,
                format_with_rustfmt(lib_rs)?,
            ),
            Formatting::Prettyplease => (
                format_with_prettyplease(classifiers_code)?,
                format_with_prettyplease(references_code)?,
                format_with_prettyplease(package_code)?,
                format_with_prettyplease(lib_rs)?,
            ),
        };

    let cargo_toml = render_cargo_toml(&project_name)?;

    fs::write(root.join("Cargo.toml"), cargo_toml)?;
    fs::write(src_dir.join("lib.rs"), formatted_lib)?;
    fs::write(src_dir.join("classifiers.rs"), formatted_classifiers)?;
    fs::write(src_dir.join("references.rs"), formatted_references)?;
    fs::write(src_dir.join("package.rs"), formatted_package)?;

    Ok(())
}

fn render_cargo_toml(project_name: &str) -> Result<String> {
    Ok(format!(
        "[package]\n\
        name = \"{project_name}\"\n\
        version = \"0.1.0\"\n\
        edition = \"2024\"\n\n\
        [dependencies]\n\
        moirai-protocol = {{ git = \"https://github.com/CEA-LIST/Moirai.git\", tag = \"v0.5\" }}\n\
        moirai-fuzz = {{ git = \"https://github.com/CEA-LIST/Moirai.git\", tag = \"v0.5\" }}\n\
        moirai-crdt = {{ git = \"https://github.com/CEA-LIST/Moirai.git\", tag = \"v0.5\" }}\n\
        moirai-macros = {{ git = \"https://github.com/CEA-LIST/Moirai.git\", tag = \"v0.5\" }}\n\
        petgraph = \"0.8.3\"\n\
        rand = \"0.10.0\"\n\
        deepsize = {{ git = \"https://github.com/leo-olivier/deepsize.git\", optional = true, features = [\"elsa\"] }}\n\n\
        [features]\n\
        default = [\"fuzz\", \"sink\"]\n\
        fuzz = []\n\
        sink = [\"moirai-protocol/sink\",\"moirai-fuzz/sink\",\"moirai-macros/sink\",\"moirai-crdt/sink\"]\n\
        test_utils = [\"dep:deepsize\",\"moirai-protocol/test_utils\",\"moirai-macros/test_utils\",\"moirai-crdt/test_utils\"]\n",
    ))
}

fn render_lib_rs() -> TokenStream {
    quote! {
        pub mod package;
        pub mod classifiers;
        pub mod references;
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
