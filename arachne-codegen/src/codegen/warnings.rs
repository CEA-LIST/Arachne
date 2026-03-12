/// Represents unsupported features encountered during code generation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Warning {
    /// EClass is an interface, not a concrete class
    InterfaceNotSupported(String),
    /// Unsupported bounds were normalized to the nearest supported mapping
    UnsupportedBounds {
        feature: String,
        bounds: String,
        applied: String,
    },
    UnsupportedPropertyCombination {
        feature: String,
        properties: Vec<String>,
        applied: Vec<String>,
    },
}

impl Warning {
    /// Return a human-readable warning message
    pub fn message(&self) -> String {
        match self {
            Warning::InterfaceNotSupported(name) => {
                format!(
                    "Warning: `EClass` '{}' is an interface and is not supported in v1. It will be skipped.",
                    name
                )
            }
            Warning::UnsupportedBounds {
                feature,
                bounds,
                applied,
            } => {
                format!(
                    "Warning: feature '{}' has unsupported bounds `{}`. Applied nearest supported bounds {} instead.",
                    feature, bounds, applied
                )
            }
            Warning::UnsupportedPropertyCombination {
                feature,
                properties,
                applied,
            } => {
                format!(
                    "Warning: typed element '{}' has unsupported property combination: `{}`. Applied best-effort mapping instead: `{}`.",
                    feature,
                    properties.join(", "),
                    applied.join(", ")
                )
            }
        }
    }

    /// Emit the warning to stderr
    pub fn emit(&self) {
        eprintln!("{}", self.message());
    }
}
