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

    /// Two classifiers would be listed in the descriptor under one key, so one of the two
    /// would silently take the other's features. Ecore's own classes are keyed
    /// `ecore::<Name>`, which no well-formed Ecore name can be, so this is a metamodel EMF
    /// would itself refuse; it is checked because descriptors are emitted for metamodels EMF
    /// has never seen.
    #[error("two classifiers would be described under the one key `{0}`")]
    DuplicateDescriptorClass(String),
}

/// Specialized Result type for Arachne
pub type Result<T> = std::result::Result<T, ArachneError>;

impl From<ecore_rs::prelude::res::Error> for ArachneError {
    fn from(err: ecore_rs::prelude::res::Error) -> Self {
        ArachneError::EcoreParse(err.to_string())
    }
}
