use ecore_rs::{ctx::Ctx, prelude::idx, repr::structural};

use crate::utils::hash::HashSet;

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
    pub fn analyze(ctx: &Ctx, package_classes: &[idx::Class]) -> Self {
        let package_set: HashSet<idx::Class> = package_classes.iter().copied().collect();
        let mut refs = Vec::new();
        let mut seen_refs = HashSet::default();
        let mut referenceable_set = HashSet::default();

        for &class_idx in package_classes {
            let class = &ctx.classes()[*class_idx];

            for feature in class.structural() {
                if feature.kind != structural::Typ::EReference {
                    continue;
                }
                if feature.containment {
                    continue;
                }

                let target_idx = match feature.typ {
                    Some(t) => t,
                    None => continue,
                };

                if !package_set.contains(&target_idx) {
                    continue;
                }

                let concrete_sources =
                    Self::concrete_classes_in_package(ctx, class_idx, &package_set);
                let concrete_targets =
                    Self::concrete_classes_in_package(ctx, target_idx, &package_set);

                for source_class in &concrete_sources {
                    for target_class in &concrete_targets {
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

        Self {
            refs,
            referenceable_classes,
        }
    }

    fn concrete_classes_in_package(
        ctx: &Ctx,
        class_idx: idx::Class,
        package_set: &HashSet<idx::Class>,
    ) -> Vec<idx::Class> {
        let mut classes = if ctx.classes()[*class_idx].is_concrete() {
            vec![class_idx]
        } else {
            let mut visited = HashSet::default();
            Self::find_concrete_descendants(ctx, class_idx, &mut visited)
        };

        classes.retain(|class_idx| package_set.contains(class_idx));
        classes.sort_by_key(|class_idx| *class_idx);
        classes.dedup();
        classes
    }

    /// Find all concrete subclasses of a class (recursively).
    /// If the class itself is concrete, it is included.
    fn find_concrete_descendants(
        ctx: &Ctx,
        class_idx: idx::Class,
        visited: &mut HashSet<idx::Class>,
    ) -> Vec<idx::Class> {
        if !visited.insert(class_idx) {
            return Vec::new();
        }

        let class = &ctx.classes()[*class_idx];

        if class.is_concrete() {
            return vec![class_idx];
        }

        let mut result = Vec::new();
        for sub_idx in class.sub() {
            result.extend(Self::find_concrete_descendants(ctx, *sub_idx, visited));
        }
        result
    }
}

pub fn analyze_references(ctx: &Ctx, package_classes: &[idx::Class]) -> ReferenceAnalysis {
    ReferenceAnalysis::analyze(ctx, package_classes)
}
