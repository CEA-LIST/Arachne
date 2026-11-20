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

    /// A package with no EClass has no root class either; the second line says why, as the
    /// first is the line every package without a root class gets.
    #[error(
        "Failed to find a root class in `{0}` package\ncaused by: the package declares no EClass"
    )]
    NoEClass(String),

    #[error("No user-defined package found in the Ecore model")]
    NoValidPackageFound,

    /// Two classifiers would be listed in the descriptor under one key, so one of the two
    /// would silently take the other's features. Ecore's own classes are keyed
    /// `ecore::<Name>`, which no well-formed Ecore name can be, so this is a metamodel EMF
    /// would itself refuse; it is checked because descriptors are emitted for metamodels EMF
    /// has never seen.
    #[error("two classifiers would be described under the one key `{0}`")]
    DuplicateDescriptorClass(String),

    /// A class the descriptor writes names a class or enum the descriptor does not write, by
    /// identity: a loader or a reader of the descriptor could not resolve the name, or would
    /// resolve it to another class of the same name.
    #[error(
        "descriptor not closed: class `{class}` names `{missing}`, which the descriptor does not write"
    )]
    DescriptorNotClosed { class: String, missing: String },

    /// A descriptor that writes a subpackage classifier cannot name the subpackage's namespace
    /// or key its classifiers without clashing; the text says which.
    #[error("subpackage refused: {0}")]
    Subpackage(String),

    /// A map keyed by an enum would be written with the enum's `Ctx` index, which names
    /// nothing a reader of the descriptor can resolve; refused until it is written by name.
    #[error(
        "`{class}.{feature}` is a map with an enum key, which the descriptor cannot write by name"
    )]
    EnumKeyedMap { class: String, feature: String },
}

/// Specialized Result type for Arachne
pub type Result<T> = std::result::Result<T, ArachneError>;

impl From<ecore_rs::prelude::res::Error> for ArachneError {
    fn from(err: ecore_rs::prelude::res::Error) -> Self {
        ArachneError::EcoreParse(err.to_string())
    }
}
