use log::warn;

/// Represents unsupported features encountered during code generation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Warning {
    /// EClass is an interface, not a concrete class
    InterfaceNotSupported(String),
    /// Unsupported bounds were normalized to the nearest supported mapping
    UnsupportedAttributeBounds {
        attribute: String,
        bounds: String,
        applied: String,
    },
    UnsupportedFeatureProperty {
        feature: String,
        property: String,
        value: String,
    },
    UnsupportedPropertyCombination {
        feature: String,
        properties: Vec<String>,
        applied: Vec<String>,
    },
    /// Abstract class has no subclasses
    AbstractWithNoSubclass(String),
    /// Unsupported operation encountered during code generation
    OperationNotSupported(String),
    UnsupportedAnnotation {
        feature: String,
        annotation: String,
        reason: String,
    },
}

impl Warning {
    /// Return a human-readable warning message
    pub fn message(&self) -> String {
        match self {
            Warning::InterfaceNotSupported(name) => {
                format!(
                    "`EClass` `{}` is an interface and is not supported in v1. It will be skipped.",
                    name
                )
            }
            Warning::UnsupportedAttributeBounds {
                attribute,
                bounds,
                applied,
            } => {
                format!(
                    "Attribute `{}` has unsupported bounds `{}`. Applied nearest supported bounds {} instead.",
                    attribute, bounds, applied
                )
            }
            Warning::UnsupportedPropertyCombination {
                feature,
                properties,
                applied,
            } => {
                format!(
                    "Typed element `{}` has unsupported property combination: `{}`. Applied best-effort mapping instead: `{}`.",
                    feature,
                    properties.join(", "),
                    applied.join(", ")
                )
            }
            Warning::AbstractWithNoSubclass(name) => {
                format!(
                    "Abstract class `{}` has no subclasses. It will be skipped.",
                    name
                )
            }
            Warning::OperationNotSupported(name) => {
                format!(
                    "Operation `{}` is not supported in v1 and will be skipped.",
                    name
                )
            }
            Warning::UnsupportedAnnotation {
                feature,
                annotation,
                reason,
            } => {
                format!(
                    "Feature `{}` has unsupported annotation `{}`: {}.",
                    feature, annotation, reason
                )
            }
            Warning::UnsupportedFeatureProperty {
                feature,
                property,
                value,
            } => {
                format!(
                    "Feature `{}` has unsupported property `{}` with value `{}`.",
                    feature, property, value
                )
            }
        }
    }

    /// Emit the warning to stderr
    pub fn emit(&self) {
        warn!("{}", self.message());
    }
}
