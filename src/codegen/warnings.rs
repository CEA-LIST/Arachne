/// Represents unsupported features encountered during code generation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Warning {
    /// EClass is an interface, not a concrete class
    InterfaceNotSupported(String),
}

impl Warning {
    /// Return a human-readable warning message
    pub fn message(&self) -> String {
        match self {
            Warning::InterfaceNotSupported(name) => {
                format!(
                    "Warning: EClass '{}' is an interface and is not supported in v1. It will be skipped.",
                    name
                )
            }
        }
    }

    /// Emit the warning to stderr
    pub fn emit(&self) {
        eprintln!("{}", self.message());
    }
}
