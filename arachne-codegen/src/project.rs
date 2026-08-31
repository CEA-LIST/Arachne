use std::{
    fs,
    path::{Path, PathBuf},
};

use log::{info, warn};
use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    config::{Config, Formatting, MoiraiPathStyle},
    deployment::{BuildContextLayout, DeploymentCtx, write_deployment_artifacts},
    error::{ArachneError, Result},
};

/// Writes a complete Rust project for the generated code.
pub fn write_project(
    config: &Config,
    project_name: &str,
    package_name: &str,
    classifiers_code: TokenStream,
    references_code: TokenStream,
    package_code: TokenStream,
    metamodel_descriptor: &serde_json::Value,
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

    let moirai = resolve_moirai_paths(&config.moirai_root, root, config.moirai_path_style)?;
    info!(
        "Moirai path dependencies rooted at `{}`",
        to_path_string(&moirai.base)
    );
    let cargo_toml = render_cargo_toml(&project_name, &moirai.base);

    fs::write(root.join("Cargo.toml"), cargo_toml)?;
    // The descriptor `network_node` serves on `GET /api/metamodel` (its
    // `METAMODEL_PATH` defaults to this file, next to the manifest).
    fs::write(
        root.join("metamodel.json"),
        format!("{:#}\n", metamodel_descriptor),
    )?;
    fs::write(src_dir.join("lib.rs"), formatted_lib)?;
    fs::write(src_dir.join("classifiers.rs"), formatted_classifiers)?;
    fs::write(src_dir.join("references.rs"), formatted_references)?;
    fs::write(src_dir.join("package.rs"), formatted_package)?;

    // Generate deployment artifacts (network_node example + Dockerfile)
    let ctx = DeploymentCtx::new(&project_name, package_name, moirai.build_context);
    write_deployment_artifacts(root, &ctx)?;

    Ok(())
}

/// How the generated project addresses the Moirai workspace.
struct MoiraiPaths {
    /// Prefix the `path` dependencies of the generated manifest are built on.
    base: PathBuf,
    /// Layout the generated `Dockerfile` must find in its build context, or
    /// `None` when `base` is absolute and therefore unusable in an image.
    build_context: Option<BuildContextLayout>,
}

/// Resolves how the generated manifest addresses the Moirai workspace.
///
/// With [`MoiraiPathStyle::Relative`] (the default) the prefix is expressed
/// relative to `output_dir`, so the emitted manifest contains no machine
/// specific path and stays valid wherever the pair
/// *(generated project, Moirai workspace)* is checked out — as long as their
/// relative arrangement is preserved. With [`MoiraiPathStyle::Absolute`] the
/// canonical absolute path is emitted instead, which survives moving the
/// generated project alone but pins it to the generating machine.
///
/// Both inputs are canonicalised first, so `..` segments and symlinks in
/// either path cannot leak into the result.
fn resolve_moirai_paths(
    moirai_root: &Path,
    output_dir: &Path,
    style: MoiraiPathStyle,
) -> Result<MoiraiPaths> {
    let moirai_root = fs::canonicalize(moirai_root)
        .map_err(|e| ArachneError::Config(format!("Failed to resolve moirai root: {e}")))?;

    if style == MoiraiPathStyle::Absolute {
        return Ok(MoiraiPaths {
            base: moirai_root,
            build_context: None,
        });
    }

    let output_dir = fs::canonicalize(output_dir)
        .map_err(|e| ArachneError::Config(format!("Failed to resolve output directory: {e}")))?;

    // `diff_paths` only fails when the two paths have incomparable roots
    // (different Windows prefixes); an absolute path is then the only option.
    let Some(base) = pathdiff::diff_paths(&moirai_root, &output_dir) else {
        warn!(
            "Cannot express {} relative to {}; falling back to an absolute path dependency",
            moirai_root.display(),
            output_dir.display()
        );
        return Ok(MoiraiPaths {
            base: moirai_root,
            build_context: None,
        });
    };

    let build_context =
        split_at_common_ancestor(&output_dir, &moirai_root).map(|(project_dir, moirai_dir)| {
            BuildContextLayout {
                project_dir: to_path_string(&project_dir),
                moirai_dir: to_path_string(&moirai_dir),
            }
        });

    Ok(MoiraiPaths {
        base,
        build_context,
    })
}

/// Re-expresses two absolute paths relative to their deepest common ancestor —
/// the directory a Docker build context has to be rooted at for both to be
/// visible.
///
/// Returns `None` when one path contains the other, since there is then no
/// context in which both appear as siblings of a shared root.
fn split_at_common_ancestor(left: &Path, right: &Path) -> Option<(PathBuf, PathBuf)> {
    let left: Vec<_> = left.components().collect();
    let right: Vec<_> = right.components().collect();

    let shared = left
        .iter()
        .zip(right.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let left_rest: PathBuf = left[shared..].iter().collect();
    let right_rest: PathBuf = right[shared..].iter().collect();

    if left_rest.as_os_str().is_empty() || right_rest.as_os_str().is_empty() {
        return None;
    }

    Some((left_rest, right_rest))
}

fn render_cargo_toml(project_name: &str, moirai_root: &Path) -> String {
    let moirai_crdt = moirai_root.join("moirai-crdt");
    let moirai_protocol = moirai_root.join("moirai-protocol");
    let moirai_macros = moirai_root.join("moirai-macros");
    let moirai_fuzz = moirai_root.join("moirai-fuzz");
    let moirai_network = moirai_root.join("moirai-network");

    format!(
        "[package]\nname = \"{project_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nmoirai-crdt = {{ path = \"{}\" }}\nmoirai-protocol = {{ path = \"{}\", features = [\"serde\"] }}\nmoirai-macros = {{ path = \"{}\" }}\nmoirai-fuzz = {{ path = \"{}\" }}\nmoirai-network = {{ path = \"{}\" }}\nserde = {{ version = \"1.0\", features = [\"derive\"] }}\nserde_json = \"1.0\"\npetgraph = \"0.8.3\"\nrand = \"0.10.0\"\n\n[features]\ndefault = [\"fuzz\", \"sink\", \"serde\"]\nfuzz = []\nserde = [\"moirai-protocol/serde\", \"moirai-macros/serde\", \"moirai-crdt/serde\"]\nsink = [\"moirai-protocol/sink\",\"moirai-fuzz/sink\",\"moirai-macros/sink\",\"moirai-crdt/sink\"]\n",
        to_path_string(&moirai_crdt),
        to_path_string(&moirai_protocol),
        to_path_string(&moirai_macros),
        to_path_string(&moirai_fuzz),
        to_path_string(&moirai_network)
    )
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{MoiraiPathStyle, render_cargo_toml, resolve_moirai_paths};

    /// Builds the sibling-repository layout the toolchain assumes:
    /// `<tmp>/moirai` next to `<tmp>/arachne/generated/<name>`.
    struct Layout {
        base: PathBuf,
    }

    impl Layout {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir()
                .join(format!("arachne-project-test-{}-{tag}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            std::fs::create_dir_all(base.join("moirai")).expect("moirai root");
            std::fs::create_dir_all(base.join("arachne/generated/demo")).expect("output dir");
            Self { base }
        }

        fn moirai(&self) -> PathBuf {
            self.base.join("moirai")
        }

        fn output(&self) -> PathBuf {
            self.base.join("arachne/generated/demo")
        }
    }

    impl Drop for Layout {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn relative_style_walks_up_from_the_output_directory() {
        let layout = Layout::new("relative");

        let moirai = resolve_moirai_paths(
            &layout.moirai(),
            &layout.output(),
            MoiraiPathStyle::Relative,
        )
        .expect("resolution should succeed");

        assert_eq!(moirai.base, Path::new("../../../moirai"));
    }

    #[test]
    fn relative_style_describes_the_docker_build_context() {
        let layout = Layout::new("context");

        let moirai = resolve_moirai_paths(
            &layout.moirai(),
            &layout.output(),
            MoiraiPathStyle::Relative,
        )
        .expect("resolution should succeed");

        let context = moirai
            .build_context
            .expect("a relative manifest is buildable in an image");
        assert_eq!(context.project_dir, "arachne/generated/demo");
        assert_eq!(context.moirai_dir, "moirai");
    }

    #[test]
    fn absolute_style_keeps_the_canonical_path_and_has_no_build_context() {
        let layout = Layout::new("absolute");

        let moirai = resolve_moirai_paths(
            &layout.moirai(),
            &layout.output(),
            MoiraiPathStyle::Absolute,
        )
        .expect("resolution should succeed");

        assert!(moirai.base.is_absolute());
        assert!(moirai.base.ends_with("moirai"));
        assert!(moirai.build_context.is_none());
    }

    #[test]
    fn rendered_manifest_contains_no_absolute_path_under_the_relative_style() {
        let layout = Layout::new("manifest");

        let moirai = resolve_moirai_paths(
            &layout.moirai(),
            &layout.output(),
            MoiraiPathStyle::Relative,
        )
        .expect("resolution should succeed");
        let manifest = render_cargo_toml("demo-crdt", &moirai.base);

        assert!(manifest.contains(r#"moirai-crdt = { path = "../../../moirai/moirai-crdt" }"#));
        assert!(
            manifest.contains(r#"moirai-network = { path = "../../../moirai/moirai-network" }"#)
        );
        assert!(
            !manifest.contains("path = \"/"),
            "manifest must not embed absolute paths:\n{manifest}"
        );
    }
}
