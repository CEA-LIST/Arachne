use std::collections::BTreeSet;

use ecore_rs::{prelude::idx, repr::structural};
use log::warn;
use syn::Ident;

use crate::{
    codegen::{
        analysis::ModelIndex,
        ident::{rust_ident, type_ident},
    },
    utils::hash::{HashMap, HashSet},
};

/// A non-containment reference in the Ecore model.
#[derive(Debug, Clone)]
pub struct NonContainmentRef {
    /// The class that owns (defines) the reference.
    pub source_class: idx::Class,
    /// The class that is referenced.
    pub target_class: idx::Class,
    /// The name of the reference (e.g., "entry").
    pub reference_name: String,
    /// Lower bound of the reference cardinality.
    pub lower_bound: usize,
    /// Upper bound of the reference cardinality (None = unbounded).
    pub upper_bound: Option<usize>,
}

/// Result of analyzing non-containment references in an Ecore model.
#[derive(Debug)]
pub struct ReferenceAnalysis {
    /// All non-containment references found.
    pub refs: Vec<NonContainmentRef>,
    /// Classes that need vertex ID types (sources ∪ targets of non-containment refs).
    /// Ordered deterministically for stable code generation.
    pub referenceable_classes: Vec<idx::Class>,
    /// Edge type names corresponding to `refs`; repeated targets may share an edge type.
    pub edge_names: Vec<Ident>,
    /// Unique arc names corresponding to `refs`, shared by the graph and XMI generators.
    pub connection_names: Vec<Ident>,
}

impl ReferenceAnalysis {
    pub fn has_references(&self) -> bool {
        !self.refs.is_empty()
    }

    /// Analyze the Ecore model to find all non-containment references.
    ///
    /// References are projected onto the concrete classes that can actually appear
    /// in the generated model. For example, a reference `(absract) StructuralFeature -> (abstract) Classifier`
    /// becomes:
    /// - `Attribute -> Class`
    /// - `Attribute -> DataType`
    /// - `Reference -> Class`
    /// - `Reference -> DataType`
    ///
    /// restricted to the classes that are part of the generated package slice.
    pub fn analyze(
        ctx: &ModelIndex<'_>,
        reachable_classes: &BTreeSet<idx::Class>,
        families: &[Vec<idx::Class>],
    ) -> Self {
        let mut refs = Vec::new();
        let mut seen_refs = HashSet::default();
        let mut referenceable_set = HashSet::default();

        for &class_idx in reachable_classes {
            let class = &ctx.classes()[*class_idx];

            for feature in class.structural() {
                if feature.kind != structural::Typ::EReference {
                    continue;
                }
                if feature.containment {
                    continue;
                }

                let target_idx: idx::Class = match feature.typ {
                    Some(t) => t,
                    None => continue,
                };

                if !reachable_classes.contains(&target_idx) {
                    warn!(
                        "Reference `{}` in class `{}` points to a target class `{}` outside the package slice. Skipping.",
                        feature.name,
                        class.name(),
                        ctx.class(target_idx).name()
                    );
                    continue;
                }

                let concrete_sources = &families[*class_idx];
                let concrete_targets = &families[*target_idx];

                for source_class in concrete_sources {
                    for target_class in concrete_targets {
                        let key = (
                            *source_class,
                            *target_class,
                            feature.name.clone(),
                            feature.bounds.lbound,
                            feature.bounds.ubound,
                        );

                        if !seen_refs.insert(key) {
                            continue;
                        }

                        refs.push(NonContainmentRef {
                            source_class: *source_class,
                            target_class: *target_class,
                            reference_name: feature.name.clone(),
                            lower_bound: feature.bounds.lbound,
                            upper_bound: feature.bounds.ubound,
                        });

                        referenceable_set.insert(*source_class);
                        referenceable_set.insert(*target_class);
                    }
                }
            }
        }

        refs.sort_by_key(|r| {
            (
                r.source_class,
                r.target_class,
                r.reference_name.clone(),
                r.lower_bound,
                r.upper_bound,
            )
        });

        let mut referenceable_classes: Vec<idx::Class> = referenceable_set.into_iter().collect();
        referenceable_classes.sort_by_key(|c| *c);

        let mut edge_names = Vec::with_capacity(refs.len());
        let mut connection_names = Vec::with_capacity(refs.len());
        let mut counts: HashMap<String, usize> = HashMap::default();
        for reference in &refs {
            let source = ctx.classifier_name(reference.source_class);
            let target = ctx.classifier_name(reference.target_class);
            let feature = type_ident(&reference.reference_name);
            edge_names.push(rust_ident(format!("{source}{feature}Edge")));
            let base = format!("{source}To{target}");
            let suffix = counts.entry(base.clone()).or_insert(0);
            let name = if *suffix == 0 {
                base
            } else {
                format!("{base}{}", *suffix + 1)
            };
            *suffix += 1;
            connection_names.push(rust_ident(name));
        }
        Self {
            refs,
            referenceable_classes,
            edge_names,
            connection_names,
        }
    }
}
