use std::fs;

use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    config::{Config, Formatting},
    error::{ArachneError, Result},
};

const ARACHNE_XMI_RUNTIME: &str = arachne_xmi::RUNTIME_SOURCE;

/// Writes a complete Rust project for the generated code.
pub fn write_project(
    config: &Config,
    project_name: &str,
    classifiers_code: TokenStream,
    references_code: TokenStream,
    package_code: TokenStream,
    read_as_ecore_code: TokenStream,
    requires_sink: bool,
) -> Result<()> {
    let project_name = sanitize_package_name(project_name);
    let root = &config.output_dir;
    let src_dir = root.join("src");
    let xmi_runtime_dir = root.join("runtime/arachne-xmi");
    let xmi_runtime_src_dir = xmi_runtime_dir.join("src");

    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&xmi_runtime_src_dir)?;

    let lib_rs = render_lib_rs();

    let (
        formatted_classifiers,
        formatted_references,
        formatted_package,
        formatted_read_as_ecore,
        formatted_lib,
    ) = match config.format_code {
        Formatting::None => {
            // Do not format the code
            (
                classifiers_code.to_string(),
                references_code.to_string(),
                package_code.to_string(),
                read_as_ecore_code.to_string(),
                lib_rs.to_string(),
            )
        }
        Formatting::Rustfmt => (
            format_with_rustfmt(classifiers_code)?,
            format_with_rustfmt(references_code)?,
            format_with_rustfmt(package_code)?,
            format_with_rustfmt(read_as_ecore_code)?,
            format_with_rustfmt(lib_rs)?,
        ),
        Formatting::Prettyplease => (
            format_with_prettyplease(classifiers_code)?,
            format_with_prettyplease(references_code)?,
            format_with_prettyplease(package_code)?,
            format_with_prettyplease(read_as_ecore_code)?,
            format_with_prettyplease(lib_rs)?,
        ),
    };

    let cargo_toml = render_cargo_toml(&project_name, config, requires_sink)?;

    fs::write(root.join("Cargo.toml"), cargo_toml)?;
    fs::write(
        xmi_runtime_dir.join("Cargo.toml"),
        render_arachne_xmi_cargo_toml(),
    )?;
    fs::write(xmi_runtime_src_dir.join("lib.rs"), ARACHNE_XMI_RUNTIME)?;
    fs::write(src_dir.join("lib.rs"), formatted_lib)?;
    fs::write(src_dir.join("classifiers.rs"), formatted_classifiers)?;
    fs::write(src_dir.join("references.rs"), formatted_references)?;
    fs::write(src_dir.join("package.rs"), formatted_package)?;
    fs::write(src_dir.join("read_as_ecore.rs"), formatted_read_as_ecore)?;

    Ok(())
}

fn render_cargo_toml(project_name: &str, config: &Config, requires_sink: bool) -> Result<String> {
    let moirai = |name: &str| -> Result<String> {
        let options = match name {
            "moirai-crdt" => ", default-features = false",
            // Reference membership depends on sinks even in a minimal build.
            "moirai-protocol" if requires_sink => ", features = [\"sink\"]",
            _ => "",
        };
        if let Some(path) = &config.moirai_path {
            // Absolute paths also work when the generated project is outside Arachne.
            let path = path.canonicalize()?.join(name);
            let path = path.to_str().ok_or_else(|| {
                ArachneError::Config("Moirai path must be valid UTF-8".to_string())
            })?;
            Ok(format!("{{ path = {}{options} }}", toml_string(path)))
        } else {
            Ok(
                "{ git = \"https://github.com/CEA-LIST/Moirai.git\", \"tag\" = \"v0.8\" }"
                    .to_string(),
            )
        }
    };
    let protocol = moirai("moirai-protocol")?;
    let crdt = moirai("moirai-crdt")?;
    let macros = moirai("moirai-macros")?;
    Ok(format!(
        "[package]\n\
        name = \"{project_name}\"\n\
        version = \"0.1.0\"\n\
        edition = \"2024\"\n\n\
        [workspace]\n\
        resolver = \"3\"\n\n\
        [dependencies]\n\
        moirai-protocol = {protocol}\n\
        moirai-crdt = {crdt}\n\
        moirai-macros = {macros}\n\
        petgraph = \"0.8.3\"\n\
        rustc-hash = \"1.1.0\"\n\
        arachne-xmi = {{ path = \"runtime/arachne-xmi\" }}\n\
        deepsize = {{ git = \"https://github.com/leo-olivier/deepsize.git\", optional = true, features = [\"elsa\"] }}\n\n\
        [features]\n\
        default = [\"fuzz\", \"sink\"]\n\
        fuzz = [\"moirai-crdt/fuzz\", \"moirai-macros/fuzz\", \"test_utils\"]\n\
        sink = [\"moirai-protocol/sink\",\"moirai-macros/sink\",\"moirai-crdt/sink\"]\n\
        test_utils = [\"dep:deepsize\",\"moirai-protocol/test_utils\",\"moirai-macros/test_utils\",\"moirai-crdt/test_utils\"]\n",
    ))
}

fn toml_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04X}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

fn render_arachne_xmi_cargo_toml() -> &'static str {
    "[package]\n\
    name = \"arachne-xmi\"\n\
    version = \"0.1.0\"\n\
    edition = \"2024\"\n\
    license = \"Apache-2.0\"\n\n\
    [dependencies]\n\
    quick-xml = \"0.42.0\"\n"
}

fn render_lib_rs() -> TokenStream {
    quote! {
        pub mod package;
        pub mod classifiers;
        pub mod references;
        pub mod read_as_ecore;
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

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use quote::quote;

    use super::{toml_string, write_project};
    use crate::config::{Config, Formatting};

    #[test]
    fn manifest_paths_escape_toml_special_characters() {
        assert_eq!(
            toml_string("C:\\Moirai\"é\n\u{7}"),
            "\"C:\\\\Moirai\\\"é\\n\\u0007\""
        );
    }

    #[test]
    fn writes_read_as_ecore_to_a_separate_generated_module() {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        let output_dir = std::env::temp_dir().join(format!(
            "arachne-codegen-project-{}-{nonce}",
            std::process::id()
        ));
        let config = Config::new("unused.ecore")
            .with_output_dir(&output_dir)
            .with_formatting(Formatting::None);

        write_project(
            &config,
            "test-project",
            quote! { pub struct ClassifierMarker; },
            quote! { pub struct ReferenceMarker; },
            quote! { pub struct PackageMarker; },
            quote! { pub struct ReadAsEcoreMarker; },
            false,
        )
        .expect("project generation should succeed");

        let package = fs::read_to_string(output_dir.join("src/package.rs"))
            .expect("package.rs should be generated");
        let read_as_ecore = fs::read_to_string(output_dir.join("src/read_as_ecore.rs"))
            .expect("read_as_ecore.rs should be generated");
        let lib =
            fs::read_to_string(output_dir.join("src/lib.rs")).expect("lib.rs should be generated");
        let cargo = fs::read_to_string(output_dir.join("Cargo.toml"))
            .expect("Cargo.toml should be generated");
        let xmi_runtime_cargo =
            fs::read_to_string(output_dir.join("runtime/arachne-xmi/Cargo.toml"))
                .expect("the shared XMI runtime manifest should be generated");
        let xmi_runtime = fs::read_to_string(output_dir.join("runtime/arachne-xmi/src/lib.rs"))
            .expect("the shared XMI runtime source should be generated");

        assert!(package.contains("PackageMarker"));
        assert!(!package.contains("ReadAsEcoreMarker"));
        assert!(read_as_ecore.contains("ReadAsEcoreMarker"));
        assert!(lib.contains("pub mod read_as_ecore"));
        assert!(!output_dir.join("runtime/arachne-moirai").exists());
        assert!(cargo.contains("arachne-xmi = { path = \"runtime/arachne-xmi\" }"));
        assert!(cargo.contains("[workspace]"));
        assert!(cargo.contains("rustc-hash = \"1.1.0\""));
        assert!(xmi_runtime_cargo.contains("quick-xml = \"0.42.0\""));
        assert!(xmi_runtime.contains("pub struct Writer"));
        assert!(xmi_runtime.contains("quick_xml"));

        fs::remove_dir_all(output_dir).expect("generated test project should be removable");
    }
}
