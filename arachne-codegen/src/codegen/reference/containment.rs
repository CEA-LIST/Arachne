use ecore_rs::{ctx::Ctx, prelude::idx, repr::structural};
use heck::{ToSnakeCase, ToUpperCamelCase};

use crate::{
    codegen::{
        classifier::class::INHERITANCE_SUFFIX,
        cycles::{BoxingStrategy, CycleAnalysis},
        reference::analysis::{ReferenceAnalysis, find_concrete_descendants},
    },
    utils::hash::HashSet,
};

/// A single step in the containment path from root to a referenceable class.
#[derive(Debug, Clone)]
pub enum PathStep {
    /// A record field access: `ClassName::VariantName(inner)`.
    Field {
        class_name: String,
        variant_name: String,
        is_boxed: bool,
    },
    /// A union variant: `UnionName::VariantName(inner)`.
    Variant {
        union_name: String,
        variant_name: String,
    },
    /// `List::Insert { .. }` — creation of a new element in a list.
    ListInsert,
    /// `List::Delete { pos }` — deletion of an element in a list.
    ListDelete,
}

/// A complete path from root to a creation/deletion point for a referenceable class.
#[derive(Debug, Clone)]
pub struct ContainmentPath {
    /// The referenceable class that this path leads to.
    /// This is the class at the "reference definition level" — if the class is abstract,
    /// the concrete subclass(es) are the ones actually created, but we use the abstract
    /// class's ID type for the vertex.
    pub vertex_class: idx::Class,
    /// Steps from the root (exclusive) to the creation/deletion point.
    pub steps: Vec<PathStep>,
    /// For deletion: the field path on the log struct to access the list's position data.
    pub log_field_path: Vec<String>,
}

struct TraversalEnv<'a> {
    ctx: &'a Ctx,
    referenceable_set: &'a HashSet<idx::Class>,
    cycle_analysis: &'a CycleAnalysis,
}

struct TraversalState {
    current_steps: Vec<PathStep>,
    current_log_path: Vec<String>,
    visited: HashSet<idx::Class>,
    result: Vec<ContainmentPath>,
}

/// Find all creation paths from the root class to each referenceable class.
///
/// A creation path exists for every List-type containment feature that can contain
/// (directly or through concrete subclasses) a referenceable class.
pub fn find_creation_paths(
    ctx: &Ctx,
    root_class: idx::Class,
    analysis: &ReferenceAnalysis,
    cycle_analysis: &CycleAnalysis,
) -> Vec<ContainmentPath> {
    let referenceable_set: HashSet<idx::Class> =
        analysis.referenceable_classes.iter().copied().collect();

    let env = TraversalEnv {
        ctx,
        referenceable_set: &referenceable_set,
        cycle_analysis,
    };
    let mut state = TraversalState {
        current_steps: Vec::new(),
        current_log_path: Vec::new(),
        visited: HashSet::default(),
        result: Vec::new(),
    };

    find_paths_recursive(&env, root_class, &mut state, false);

    state.result
}

/// Recursively explore the containment tree to find paths to referenceable classes.
fn find_paths_recursive(
    env: &TraversalEnv,
    current_class: idx::Class,
    state: &mut TraversalState,
    passed_through_box: bool,
) {
    if !state.visited.insert(current_class) {
        return; // Avoid infinite loops in cyclic containment
    }

    let class = &env.ctx.classes()[*current_class];

    // Process containment references defined on this class
    for feature in class.structural() {
        if feature.kind != structural::Typ::EReference || !feature.containment {
            continue;
        }

        let target_idx = match feature.typ {
            Some(t) => t,
            None => continue,
        };

        let target_class = &env.ctx.classes()[*target_idx];
        let field_snake = feature.name.to_snake_case();
        let variant_name = field_snake.to_upper_camel_case();
        let is_many = feature.bounds.ubound != Some(1);
        let is_boxed = env
            .cycle_analysis
            .boxing_strategy(current_class, &feature.name)
            != BoxingStrategy::NoBox;

        // Don't recurse through a second boxed containment reference.
        // Operations behind a second Box cannot be matched with simple
        // pattern nesting (would need nested .as_ref() calls).
        if is_boxed && passed_through_box {
            continue;
        }

        // Push the field step
        state.current_steps.push(PathStep::Field {
            class_name: class.name().to_string(),
            variant_name: variant_name.clone(),
            is_boxed,
        });
        state.current_log_path.push(field_snake.clone());

        if is_many {
            // This is a List containment: check if the target is referenceable
            // and generate List::Insert / List::Delete patterns.
            // We do NOT recurse into the target's sub-features because
            // operations on existing list elements are wrapped in the List type
            // and cannot be matched with direct nesting.
            check_list_target(
                env.ctx,
                target_idx,
                env.referenceable_set,
                &state.current_steps,
                &state.current_log_path,
                &mut state.result,
            );
        } else {
            // Single containment: recurse into the target class
            let new_passed = passed_through_box || is_boxed;
            if target_class.is_abstract() {
                explore_abstract_class(env, target_idx, state, new_passed);
            } else {
                find_paths_recursive(env, target_idx, state, new_passed);
            }
        }

        state.current_steps.pop();
        state.current_log_path.pop();
    }

    // Process inherited features: for each superclass, recurse through its Feat type
    for super_idx in class.sup() {
        let super_class = &env.ctx.classes()[**super_idx];
        let feat_field_snake =
            format!("{}{}", super_class.name(), INHERITANCE_SUFFIX).to_snake_case();
        let feat_variant_name = feat_field_snake.to_upper_camel_case();

        state.current_steps.push(PathStep::Field {
            class_name: class.name().to_string(),
            variant_name: feat_variant_name,
            is_boxed: false,
        });
        state.current_log_path.push(feat_field_snake);

        // Recurse into the superclass to find its features
        // (but treat it as a Feat record, not a union)
        find_feat_paths_recursive(env, *super_idx, state, passed_through_box);

        state.current_steps.pop();
        state.current_log_path.pop();
    }

    state.visited.remove(&current_class);
}

/// Explore features defined on an abstract class (through its *Feat record).
/// This handles the case where inherited features contain referenceable classes.
fn find_feat_paths_recursive(
    env: &TraversalEnv,
    class_idx: idx::Class,
    state: &mut TraversalState,
    passed_through_box: bool,
) {
    let class = &env.ctx.classes()[*class_idx];

    // Process this class's own structural features
    for feature in class.structural() {
        if feature.kind != structural::Typ::EReference || !feature.containment {
            continue;
        }

        let target_idx = match feature.typ {
            Some(t) => t,
            None => continue,
        };

        let target_class = &env.ctx.classes()[*target_idx];
        let field_snake = feature.name.to_snake_case();
        let variant_name = field_snake.to_upper_camel_case();
        let is_many = feature.bounds.ubound != Some(1);

        // Use the feat class name (e.g., "ExecutionNodeFeat") for the field step
        let feat_class_name = format!("{}{}", class.name(), INHERITANCE_SUFFIX);
        let is_boxed =
            env.cycle_analysis.boxing_strategy(class_idx, &feature.name) != BoxingStrategy::NoBox;

        // Don't recurse through a second boxed containment reference.
        if is_boxed && passed_through_box {
            continue;
        }

        state.current_steps.push(PathStep::Field {
            class_name: feat_class_name,
            variant_name: variant_name.clone(),
            is_boxed,
        });
        state.current_log_path.push(field_snake.clone());

        if is_many {
            check_list_target(
                env.ctx,
                target_idx,
                env.referenceable_set,
                &state.current_steps,
                &state.current_log_path,
                &mut state.result,
            );
        } else {
            let new_passed = passed_through_box || is_boxed;
            if target_class.is_abstract() {
                explore_abstract_class(env, target_idx, state, new_passed);
            } else {
                find_paths_recursive(env, target_idx, state, new_passed);
            }
        }

        state.current_steps.pop();
        state.current_log_path.pop();
    }

    // Continue up the inheritance chain
    for super_idx in class.sup() {
        let super_class = &env.ctx.classes()[**super_idx];
        let feat_field_snake =
            format!("{}{}", super_class.name(), INHERITANCE_SUFFIX).to_snake_case();
        let feat_variant_name = feat_field_snake.to_upper_camel_case();

        state.current_steps.push(PathStep::Field {
            class_name: format!("{}{}", class.name(), INHERITANCE_SUFFIX),
            variant_name: feat_variant_name,
            is_boxed: false,
        });
        state.current_log_path.push(feat_field_snake);

        find_feat_paths_recursive(env, *super_idx, state, passed_through_box);

        state.current_steps.pop();
        state.current_log_path.pop();
    }
}

/// Check if a list-contained target class (or any of its concrete descendants)
/// is referenceable, and if so, record creation paths.
fn check_list_target(
    ctx: &Ctx,
    target_idx: idx::Class,
    referenceable_set: &HashSet<idx::Class>,
    current_steps: &[PathStep],
    current_log_path: &[String],
    result: &mut Vec<ContainmentPath>,
) {
    // Check if the target class itself is referenceable
    if referenceable_set.contains(&target_idx) {
        // Direct match: the list contains instances of a referenceable class
        let mut insert_steps = current_steps.to_owned();
        insert_steps.push(PathStep::ListInsert);

        result.push(ContainmentPath {
            vertex_class: target_idx,
            steps: insert_steps,
            log_field_path: current_log_path.to_vec(),
        });
    }

    // Check if the target (or any of its concrete descendants) is a subtype
    // of a referenceable class. This covers:
    // - Abstract target with concrete descendants that inherit from a referenceable class
    // - Concrete target that itself inherits from a referenceable class
    let target_class = &ctx.classes()[*target_idx];
    let classes_to_check = if target_class.is_abstract() {
        find_concrete_descendants(ctx, target_idx)
    } else {
        vec![target_idx]
    };

    for concrete_idx in &classes_to_check {
        for &ref_class in referenceable_set {
            if ref_class != target_idx && is_subtype_of(ctx, *concrete_idx, ref_class) {
                let mut insert_steps = current_steps.to_owned();
                insert_steps.push(PathStep::ListInsert);

                if !result.iter().any(|p| {
                    p.vertex_class == ref_class && steps_prefix_match(&p.steps, &insert_steps)
                }) {
                    result.push(ContainmentPath {
                        vertex_class: ref_class,
                        steps: insert_steps,
                        log_field_path: current_log_path.to_vec(),
                    });
                }
            }
        }
    }
}

/// Explore an abstract class by following all its concrete subclass variants.
fn explore_abstract_class(
    env: &TraversalEnv,
    abstract_class: idx::Class,
    state: &mut TraversalState,
    passed_through_box: bool,
) {
    let class = &env.ctx.classes()[*abstract_class];

    for sub_idx in class.sub() {
        let sub_class = &env.ctx.classes()[**sub_idx];

        state.current_steps.push(PathStep::Variant {
            union_name: class.name().to_string(),
            variant_name: sub_class.name().to_string(),
        });

        if sub_class.is_abstract() {
            // Continue through sub-abstract classes
            explore_abstract_class(env, *sub_idx, state, passed_through_box);
        } else {
            find_paths_recursive(env, *sub_idx, state, passed_through_box);
        }

        state.current_steps.pop();
    }
}

/// Check if class `a` is a subtype of class `b` (transitively through inheritance).
fn is_subtype_of(ctx: &Ctx, a: idx::Class, b: idx::Class) -> bool {
    if a == b {
        return true;
    }
    let a_class = &ctx.classes()[*a];
    for super_idx in a_class.sup() {
        if is_subtype_of(ctx, *super_idx, b) {
            return true;
        }
    }
    false
}

/// Check if two step sequences share the same prefix (for deduplication).
fn steps_prefix_match(a: &[PathStep], b: &[PathStep]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| match (x, y) {
        (
            PathStep::Field {
                class_name: a1,
                variant_name: a2,
                ..
            },
            PathStep::Field {
                class_name: b1,
                variant_name: b2,
                ..
            },
        ) => a1 == b1 && a2 == b2,
        (
            PathStep::Variant {
                union_name: a1,
                variant_name: a2,
            },
            PathStep::Variant {
                union_name: b1,
                variant_name: b2,
            },
        ) => a1 == b1 && a2 == b2,
        (PathStep::ListInsert, PathStep::ListInsert) => true,
        (PathStep::ListDelete, PathStep::ListDelete) => true,
        _ => false,
    })
}
