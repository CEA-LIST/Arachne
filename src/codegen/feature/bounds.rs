use crate::codegen::warnings::Warning;

#[derive(Clone, Copy, Debug)]
pub enum BoundKind {
    Optional,
    Single,
    Many,
}

pub fn normalize_bounds(
    bounds: ecore_rs::repr::bounds::Bounds,
    feature: &str,
) -> (BoundKind, Vec<Warning>) {
    let applied = match (bounds.lbound, bounds.ubound) {
        (0, Some(1)) => (BoundKind::Optional, None),
        (1, Some(1)) => (BoundKind::Single, None),
        (0, None) => (BoundKind::Many, None),
        (0, Some(0)) => (BoundKind::Optional, Some("0..1")),
        (0, Some(_)) => (BoundKind::Many, Some("0..*")),
        (1, None) => (BoundKind::Many, Some("0..*")),
        (lbound, Some(ubound)) if lbound > 1 || ubound > 1 => (BoundKind::Many, Some("0..*")),
        (lbound, Some(ubound)) => {
            let _ = (lbound, ubound);
            (BoundKind::Many, Some("0..*"))
        }
        (_, None) => (BoundKind::Many, Some("0..*")),
    };

    let warnings = applied.1.map_or(Vec::new(), |applied| {
        vec![Warning::UnsupportedBounds {
            feature: feature.to_string(),
            bounds: bounds.to_string(),
            applied: applied.to_string(),
        }]
    });

    (applied.0, warnings)
}
