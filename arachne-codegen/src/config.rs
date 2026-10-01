use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Formatting {
    None,
    Rustfmt,
    Prettyplease,
}

/// Configuration for the Arachne code generator
#[derive(Debug, Clone)]
pub struct Config {
    /// Path to the input Ecore metamodel file
    pub input_path: PathBuf,
    /// Directory where the generated Rust project will be written
    pub output_dir: PathBuf,
    /// Optional generated project name (Cargo package name)
    pub project_name: Option<String>,
    /// Local Moirai workspace to use instead of the pinned Git revision.
    pub moirai_path: Option<PathBuf>,
    /// Format output code
    pub format_code: Formatting,
}

impl Config {
    /// Creates a new configuration with default values
    pub fn new(input_path: impl Into<PathBuf>) -> Self {
        let input_path = input_path.into();
        Self {
            input_path,
            output_dir: PathBuf::from(".output/generated_project"),
            project_name: None,
            moirai_path: None,
            format_code: Formatting::Prettyplease,
        }
    }

    /// Sets the output directory for the generated project
    pub fn with_output_dir(mut self, output_dir: impl Into<PathBuf>) -> Self {
        self.output_dir = output_dir.into();
        self
    }

    /// Sets the generated project name (Cargo package name)
    pub fn with_project_name(mut self, project_name: impl Into<String>) -> Self {
        self.project_name = Some(project_name.into());
        self
    }

    pub fn with_moirai_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.moirai_path = Some(path.into());
        self
    }

    pub fn with_formatting(mut self, formatting: Formatting) -> Self {
        self.format_code = formatting;
        self
    }

    /// Validates the configuration
    pub fn validate(&self) -> crate::error::Result<()> {
        if !self.input_path.exists() {
            return Err(crate::error::ArachneError::Config(format!(
                "Input file does not exist: {:?}",
                self.input_path
            )));
        }

        if let Some(path) = &self.moirai_path {
            for name in [
                "moirai-protocol",
                "moirai-crdt",
                "moirai-macros",
                "moirai-fuzz",
            ] {
                if !path.join(name).join("Cargo.toml").is_file() {
                    return Err(crate::error::ArachneError::Config(format!(
                        "Moirai workspace is missing {name}/Cargo.toml: {}",
                        path.display()
                    )));
                }
            }
        }

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new("./examples/bt.ecore")
    }
}
