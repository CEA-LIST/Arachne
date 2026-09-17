use log::warn;

/// Represents unsupported features encountered during code generation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Warning {
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
    /// A containment typed by Ecore's `EObject`, such as `EAnnotation.contents`, is not generated
    AnyObjectContainmentNotSupported {
        /// `Class.feature`
        feature: String,
    },
    /// A non-containment reference typed by Ecore's `EObject`, such as `EAnnotation.references`,
    /// is not generated: it would need an arc whose target is a vertex of any kind
    AnyObjectReferenceNotSupported {
        /// `Class.feature`
        feature: String,
    },
    /// A transient reference of Ecore's own classes, `EAnnotation.eModelElement`, is not generated
    TransientEcoreReferenceNotGenerated {
        /// `Class.feature`
        feature: String,
    },
}

impl Warning {
    /// Return a human-readable warning message
    pub fn message(&self) -> String {
        match self {
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
                    "Abstract class `{}` has no concrete subclasses. It will be skipped.",
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
            Warning::AnyObjectContainmentNotSupported { feature } => {
                format!(
                    "Containment `{}` holds objects of any class (it is typed by Ecore's `EObject`), which is not supported: it is not generated, and nothing it contains is replicated.",
                    feature
                )
            }
            Warning::AnyObjectReferenceNotSupported { feature } => {
                format!(
                    "Reference `{}` refers to an object of any class (it is typed by Ecore's `EObject`), which is not supported: it is not generated. It would need an arc whose target is a vertex of any kind, which Moirai's typed graph does not offer.",
                    feature
                )
            }
            Warning::TransientEcoreReferenceNotGenerated { feature } => {
                format!(
                    "Reference `{}` of Ecore's own classes is transient, the back-pointer of a containment: it is not generated.",
                    feature
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
