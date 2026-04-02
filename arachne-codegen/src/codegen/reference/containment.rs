use ecore_rs::{ctx::Ctx, prelude::idx, repr::structural};
use heck::{ToSnakeCase, ToUpperCamelCase};

use crate::{
    codegen::{
        annotation::uw_map_spec,
        classifier::{containment_target_ident, has_subclasses, inherited_field_ident},
        cycles::{BoxingStrategy, CycleAnalysis},
        reference::analysis::ReferenceAnalysis,
    },
    utils::hash::HashSet,
};

/// A single step in the containment path from root to a referenceable class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathStep {
    /// A record field access emitted as `path.field("...")`.
    Field {
        class_name: String,
        variant_name: String,
        is_boxed: bool,
    },
    /// A union variant emitted as `path.variant("...")`.
    Variant {
        union_name: String,
        variant_name: String,
    },
    /// A nested-list child emitted as `path.list_element(...)`.
    ListElement,
    /// A map child emitted as `path.map_entry(...)`.
    MapEntry,
}

/// A complete path from root to a creation/deletion point for a referenceable class.
#[derive(Debug, Clone)]
pub struct ContainmentPath {
    /// The referenceable class that this path leads to.
    pub vertex_class: idx::Class,
    /// Steps from the root field (exclusive) to the concrete object path emitted by sinks.
    pub steps: Vec<PathStep>,
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
    visited_polymorphic_families: HashSet<idx::Class>,
    result: Vec<ContainmentPath>,
}

/// Find all sink paths from a package root class to each concrete referenceable class.
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
        visited_polymorphic_families: HashSet::default(),
        result: Vec::new(),
    };

    if is_polymorphic_class(&ctx.classes()[*root_class])
        && !ctx.classes()[*root_class].is_concrete()
    {
        explore_polymorphic_class(&env, root_class, &mut state, false);
    } else {
        find_paths_recursive(&env, root_class, &mut state, false);
    }

    state.result
}

fn find_paths_recursive(
    env: &TraversalEnv,
    current_class: idx::Class,
    state: &mut TraversalState,
    passed_through_box: bool,
) {
    if !state.visited.insert(current_class) {
        return; // Avoid infinite loops in cyclic containment
    }

    if env.referenceable_set.contains(&current_class) {
        push_unique_path(
            &mut state.result,
            current_class,
            state.current_steps.clone(),
            state.current_log_path.clone(),
        );
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
            == BoxingStrategy::DirectReference;

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
            if uw_map_spec(feature).is_some() {
                state.current_steps.push(PathStep::MapEntry);
            } else {
                state.current_steps.push(PathStep::ListElement);
            }

            if is_polymorphic_class(target_class) {
                explore_polymorphic_class(env, target_idx, state, passed_through_box || is_boxed);
            } else {
                find_paths_recursive(env, target_idx, state, passed_through_box || is_boxed);
            }

            state.current_steps.pop();
        } else {
            let new_passed = passed_through_box || is_boxed;
            if is_polymorphic_class(target_class) {
                explore_polymorphic_class(env, target_idx, state, new_passed);
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
        let field_snake = inherited_field_ident(super_class).to_string();
        let field_variant_name = field_snake.to_upper_camel_case();

        state.current_steps.push(PathStep::Field {
            class_name: class.name().to_string(),
            variant_name: field_variant_name,
            is_boxed: false,
        });
        state.current_log_path.push(field_snake);

        if super_class.is_abstract() || super_class.is_interface() {
            find_feat_paths_recursive(env, *super_idx, state, passed_through_box);
        } else {
            find_paths_recursive(env, *super_idx, state, passed_through_box);
        }

        state.current_steps.pop();
        state.current_log_path.pop();
    }

    state.visited.remove(&current_class);
}

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

        let feat_class_name = class.name().to_string();
        let is_boxed = env.cycle_analysis.boxing_strategy(class_idx, &feature.name)
            == BoxingStrategy::DirectReference;

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
            if uw_map_spec(feature).is_some() {
                state.current_steps.push(PathStep::MapEntry);
            } else {
                state.current_steps.push(PathStep::ListElement);
            }

            let new_passed = passed_through_box || is_boxed;
            if is_polymorphic_class(target_class) {
                explore_polymorphic_class(env, target_idx, state, new_passed);
            } else {
                find_paths_recursive(env, target_idx, state, new_passed);
            }

            state.current_steps.pop();
        } else {
            let new_passed = passed_through_box || is_boxed;
            if is_polymorphic_class(target_class) {
                explore_polymorphic_class(env, target_idx, state, new_passed);
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
        let (field_snake, field_variant_name, field_class_name) = {
            let feat_field_snake = inherited_field_ident(super_class).to_string();
            (
                feat_field_snake.clone(),
                feat_field_snake.to_upper_camel_case(),
                class.name().to_string(),
            )
        };

        state.current_steps.push(PathStep::Field {
            class_name: field_class_name,
            variant_name: field_variant_name,
            is_boxed: false,
        });
        state.current_log_path.push(field_snake);

        if super_class.is_abstract() || super_class.is_interface() {
            find_feat_paths_recursive(env, *super_idx, state, passed_through_box);
        } else {
            find_paths_recursive(env, *super_idx, state, passed_through_box);
        }

        state.current_steps.pop();
        state.current_log_path.pop();
    }
}

fn push_unique_path(
    result: &mut Vec<ContainmentPath>,
    vertex_class: idx::Class,
    steps: Vec<PathStep>,
    log_field_path: Vec<String>,
) {
    if result
        .iter()
        .any(|p| p.vertex_class == vertex_class && p.steps == steps)
    {
        return;
    }

    result.push(ContainmentPath {
        vertex_class,
        steps,
        log_field_path,
    });
}

fn explore_polymorphic_class(
    env: &TraversalEnv,
    class_idx: idx::Class,
    state: &mut TraversalState,
    passed_through_box: bool,
) {
    if !state.visited_polymorphic_families.insert(class_idx) {
        return;
    }

    let class = &env.ctx.classes()[*class_idx];
    let union_name = containment_target_ident(class).to_string();

    if class.is_concrete() && has_subclasses(class) {
        state.current_steps.push(PathStep::Variant {
            union_name: union_name.clone(),
            variant_name: class.name().to_string(),
        });
        find_paths_recursive(env, class_idx, state, passed_through_box);
        state.current_steps.pop();
    }

    for sub_idx in class.sub() {
        let sub_class = &env.ctx.classes()[**sub_idx];

        state.current_steps.push(PathStep::Variant {
            union_name: union_name.clone(),
            variant_name: sub_class.name().to_string(),
        });

        if is_polymorphic_class(sub_class) {
            explore_polymorphic_class(env, *sub_idx, state, passed_through_box);
        } else {
            find_paths_recursive(env, *sub_idx, state, passed_through_box);
        }

        state.current_steps.pop();
    }

    state.visited_polymorphic_families.remove(&class_idx);
}

fn is_polymorphic_class(class: &ecore_rs::repr::Class) -> bool {
    class.is_abstract() || class.is_interface() || has_subclasses(class)
}
