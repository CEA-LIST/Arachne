use ecore_rs::repr::Structural;

use crate::codegen::warnings::Warning;

/// Checks for unsupported feature properties.
/// Returns `false` if the feature is not supported and records its warnings.
/// False flags require no additional semantics; `changeable=true` is supported.
pub fn unsupported_feature_properties(feature: &Structural, warnings: &mut Vec<Warning>) -> bool {
    let warning_len = warnings.len();

    if let Some(changeable) = feature.changeable
        && !changeable
    {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "changeable".into(),
            value: "false".into(),
        })
    }

    if let Some(transient @ true) = feature.transient {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "transient".into(),
            value: transient.to_string(),
        })
    }

    if let Some(volatile @ true) = feature.volatile {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "volatile".into(),
            value: volatile.to_string(),
        })
    }

    if let Some(derived @ true) = feature.derived {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "derived".into(),
            value: derived.to_string(),
        })
    }

    if let Some(unsettable @ true) = feature.unsettable {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "unsettable".into(),
            value: unsettable.to_string(),
        })
    }

    if let Some(default_value) = &feature.default_value {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "defaultValue".into(),
            value: default_value.clone(),
        })
    }

    if let Some(default_value_literal) = &feature.default_value_literal {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "defaultValueLiteral".into(),
            value: default_value_literal.clone(),
        })
    }

    if let Some(resolve_proxies @ true) = feature.resolve_proxies {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "resolveProxies".into(),
            value: resolve_proxies.to_string(),
        })
    }

    warning_len == warnings.len()
}
