use crate::codegen::warnings::Warning;
use ecore_rs::repr::Structural;

pub fn unsupported_feature_properties(feature: &Structural, warnings: &mut Vec<Warning>) {
    if let Some(changeable) = feature.changeable
        && !changeable
    {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "changeable".into(),
            value: "false".into(),
        })
    }

    if let Some(transient) = feature.transient {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "transient".into(),
            value: transient.to_string(),
        })
    }

    if let Some(volatile) = feature.volatile {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "volatile".into(),
            value: volatile.to_string(),
        })
    }

    if let Some(derived) = feature.derived {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "derived".into(),
            value: derived.to_string(),
        })
    }

    if let Some(unsettable) = feature.unsettable {
        warnings.push(Warning::UnsupportedFeatureProperty {
            feature: feature.name.clone(),
            property: "unsettable".into(),
            value: unsettable.to_string(),
        })
    }
}
