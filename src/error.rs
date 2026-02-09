use thiserror::Error;

/// Errors that can occur during code generation
#[derive(Debug, Error)]
pub enum AtraktosError {
    #[error("Failed to read Ecore file: {0}")]
    FileRead(#[from] std::io::Error),

    #[error("Failed to parse Ecore metamodel: {0}")]
    EcoreParse(String),

    #[error("Failed to generate code: {0}")]
    CodeGeneration(String),

    #[error("Failed to parse generated code: {0}")]
    SynParse(#[from] syn::Error),

    #[error("Invalid Ecore model: {0}")]
    InvalidModel(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

/// Specialized Result type for Atraktos
pub type Result<T> = std::result::Result<T, AtraktosError>;

impl From<ecore_rs::prelude::res::Error> for AtraktosError {
    fn from(err: ecore_rs::prelude::res::Error) -> Self {
        AtraktosError::EcoreParse(err.to_string())
    }
}
