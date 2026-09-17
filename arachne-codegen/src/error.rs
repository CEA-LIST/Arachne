use thiserror::Error;

/// Errors that can occur during code generation
#[derive(Debug, Error)]
pub enum ArachneError {
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

    #[error("Failed to find a root class in `{0}` package")]
    RootClassNotFound(String),

    #[error("No user-defined package found in the Ecore model")]
    NoValidPackageFound,

    /// The generator writes a crate for a package whose classes extend or are typed by Ecore's
    /// own classes, but the descriptor does not describe them yet, so nothing on the interpreted
    /// path could read one. Refused here rather than published half-described.
    #[error(
        "`{0}` uses Ecore's own classes, which the metamodel descriptor does not describe yet"
    )]
    EcoreBuiltinsNotDescribed(String),
}

/// Specialized Result type for Arachne
pub type Result<T> = std::result::Result<T, ArachneError>;

impl From<ecore_rs::prelude::res::Error> for ArachneError {
    fn from(err: ecore_rs::prelude::res::Error) -> Self {
        ArachneError::EcoreParse(err.to_string())
    }
}
