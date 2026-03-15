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
    /// Path to the Moirai workspace root
    pub moirai_root: PathBuf,
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
            moirai_root: PathBuf::from("../moirai"),
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

    /// Sets the path to the Moirai workspace root
    pub fn with_moirai_root(mut self, moirai_root: impl Into<PathBuf>) -> Self {
        self.moirai_root = moirai_root.into();
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

        if !self.moirai_root.exists() {
            return Err(crate::error::ArachneError::Config(format!(
                "Moirai root does not exist: {:?}",
                self.moirai_root
            )));
        }

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new("./examples/bt.ecore")
    }
}
