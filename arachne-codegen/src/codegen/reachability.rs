//! The classes a descriptor lists, and its roots: which classes a package reaches from its
//! top-level roots. Describe's own copy of what code generation computes for itself, deciding
//! what a classifier is by its `xsi:type` and not by the shared `is_enum` and
//! `is_instantiable_class`, which take a datatype with no literal for a class.

use std::collections::HashSet;

use ecore_rs::{
    ctx::Ctx,
    repr::{Class, idx, structural},
};
use log::debug;

use crate::codegen::semantics::is_eclass;

/// A concrete, non-interface EClass: what a model can hold an instance of.
fn is_instantiable_class(class: &Class) -> bool {
    class.is_concrete() && !class.is_interface() && is_eclass(class)
}

fn has_instantiable_descendant(ctx: &Ctx, class: &Class) -> bool {
    class.sub().iter().any(|idx| {
        let subclass = &ctx.classes()[**idx];
        is_instantiable_class(subclass) || has_instantiable_descendant(ctx, subclass)
    })
}

pub(crate) fn is_uninhabited_polymorphic_class(ctx: &Ctx, class: &Class) -> bool {
    (class.is_abstract() || class.is_interface()) && !has_instantiable_descendant(ctx, class)
}

/// Computes the top-level root classes of a package: the instantiable,
/// non-enum, non-interface classes no other instantiable class contains —
/// falling back to abstract classes and interfaces with concrete descendants
/// and no external container when there is no such instantiable class.
///
/// `package_classes` are the package's own classes, the only candidates for a
/// root. `package_classes_and_ecore` adds the built-in Ecore classes the
/// package reaches, which contain and are contained like the package's own and
/// so must be searched when asking whether a candidate has a container.
///
/// Empty when the package has no viable root; callers decide whether that is an error.
pub(crate) fn compute_top_level_roots(
    ctx: &Ctx,
    package_classes: &[idx::Class],
    package_classes_and_ecore: &[idx::Class],
    package_class_set: &HashSet<idx::Class>,
) -> Vec<idx::Class> {
    let concrete_containment_incoming =
        compute_concrete_containment_incoming(ctx, package_classes_and_ecore, package_class_set);

    let mut top_level_roots: Vec<idx::Class> = package_classes
        .iter()
        .copied()
        .filter(|class_idx| {
            let class = &ctx.classes()[**class_idx];
            is_instantiable_class(class) && !concrete_containment_incoming.contains(class_idx)
        })
        .collect();

    if top_level_roots.is_empty() {
        debug!(
            "No top-level roots found based on concrete classes. Falling back to abstract/interface classes with concrete descendants and no external containers."
        );
        top_level_roots = package_classes
            .iter()
            .copied()
            .filter(|class_idx| {
                let class = &ctx.classes()[**class_idx];
                is_eclass(class)
                    && (class.is_interface() || !class.is_concrete())
                    && has_concrete_descendant(ctx, *class_idx, package_class_set)
                    && abstract_family_has_no_external_container(
                        ctx,
                        *class_idx,
                        package_classes_and_ecore,
                        package_class_set,
                    )
            })
            .collect();
    }

    top_level_roots
}

pub(crate) fn collect_reachable_classes(
    ctx: &Ctx,
    root_class: idx::Class,
    package_classes: &HashSet<idx::Class>,
) -> HashSet<idx::Class> {
    let mut reachable = HashSet::new();
    let mut stack = vec![root_class];

    while let Some(class_idx) = stack.pop() {
        if !package_classes.contains(&class_idx) || !reachable.insert(class_idx) {
            continue;
        }

        let class = &ctx.classes()[*class_idx];

        for parent in class.sup() {
            stack.push(*parent);
        }

        if !class.sub().is_empty() {
            for sub in class.sub() {
                stack.push(*sub);
            }
        }

        for feature in class.structural() {
            if feature.kind == structural::Typ::EReference
                && feature.containment
                && let Some(target) = feature.typ
            {
                stack.push(target);
            }
        }
    }

    reachable
}

/// Adds to `reachable` what its classes name: `EObject` alone, when a listed class has it as a
/// supertype or a reference target, so that listing it pulls in none of its subclasses; and, for
/// a plain reference target in `package_classes`, that class with what `collect_reachable_classes`
/// reaches from it, never through `EObject`. Repeats until nothing is added.
pub(crate) fn follow_reference_targets(
    ctx: &Ctx,
    reachable: &mut HashSet<idx::Class>,
    package_classes: &HashSet<idx::Class>,
    described: &HashSet<idx::Class>,
) {
    let eobject = crate::codegen::ecore::eobject(ctx);
    let walkable: HashSet<idx::Class> = described
        .iter()
        .copied()
        .filter(|class| Some(*class) != eobject)
        .collect();
    loop {
        let before = reachable.len();
        if let Some(eobject) = eobject
            && reachable.iter().any(|class| {
                ctx[*class].sup().contains(&eobject)
                    || ctx[*class].structural().iter().any(|feature| {
                        feature.kind == structural::Typ::EReference && feature.typ == Some(eobject)
                    })
            })
        {
            reachable.insert(eobject);
        }
        let targets: Vec<idx::Class> = reachable
            .iter()
            .flat_map(|class| ctx[*class].structural())
            .filter(|feature| feature.kind == structural::Typ::EReference && !feature.containment)
            .filter_map(|feature| feature.typ)
            .filter(|target| !reachable.contains(target))
            .collect();
        for target in targets {
            if package_classes.contains(&target) && is_eclass(&ctx[target]) {
                reachable.extend(collect_reachable_classes(ctx, target, &walkable));
            }
        }
        if reachable.len() == before {
            return;
        }
    }
}

fn has_concrete_descendant(
    ctx: &Ctx,
    class_idx: idx::Class,
    package_classes: &HashSet<idx::Class>,
) -> bool {
    let mut stack: Vec<idx::Class> = ctx.classes()[*class_idx].sub().iter().copied().collect();

    while let Some(candidate) = stack.pop() {
        if !package_classes.contains(&candidate) {
            continue;
        }

        let class = &ctx.classes()[*candidate];
        if is_instantiable_class(class) {
            return true;
        }

        stack.extend(class.sub().iter().copied());
    }

    false
}

fn concrete_descendants_in_package(
    ctx: &Ctx,
    class_idx: idx::Class,
    package_classes: &HashSet<idx::Class>,
) -> HashSet<idx::Class> {
    let mut result = HashSet::new();
    let mut stack = vec![class_idx];

    while let Some(candidate) = stack.pop() {
        if !package_classes.contains(&candidate) {
            continue;
        }

        let class = &ctx.classes()[*candidate];
        if is_instantiable_class(class) {
            result.insert(candidate);
        }

        stack.extend(class.sub().iter().copied());
    }

    result
}

fn compute_concrete_containment_incoming(
    ctx: &Ctx,
    package_classes: &[idx::Class],
    package_class_set: &HashSet<idx::Class>,
) -> HashSet<idx::Class> {
    let mut incoming = HashSet::new();

    for &source_class_idx in package_classes {
        let source_concretes =
            concrete_descendants_in_package(ctx, source_class_idx, package_class_set);
        if source_concretes.is_empty() {
            continue;
        }

        for feature in ctx.classes()[*source_class_idx].structural() {
            if feature.kind != structural::Typ::EReference || !feature.containment {
                continue;
            }

            let Some(target_class_idx) = feature.typ else {
                continue;
            };
            if !package_class_set.contains(&target_class_idx) {
                continue;
            }

            let target_concretes =
                concrete_descendants_in_package(ctx, target_class_idx, package_class_set);
            for target in target_concretes {
                if source_concretes.iter().any(|source| *source != target) {
                    incoming.insert(target);
                }
            }
        }
    }

    incoming
}

fn abstract_family_has_no_external_container(
    ctx: &Ctx,
    class_idx: idx::Class,
    package_classes: &[idx::Class],
    package_class_set: &HashSet<idx::Class>,
) -> bool {
    let family = concrete_descendants_in_package(ctx, class_idx, package_class_set);
    if family.is_empty() {
        return false;
    }
    let family_context = collect_reachable_classes(ctx, class_idx, package_class_set);

    for &source_class_idx in package_classes {
        let source_concretes =
            concrete_descendants_in_package(ctx, source_class_idx, package_class_set);
        if source_concretes.is_empty() {
            continue;
        }

        for feature in ctx.classes()[*source_class_idx].structural() {
            if feature.kind != structural::Typ::EReference || !feature.containment {
                continue;
            }

            let Some(target_class_idx) = feature.typ else {
                continue;
            };
            if !package_class_set.contains(&target_class_idx) {
                continue;
            }

            let target_concretes =
                concrete_descendants_in_package(ctx, target_class_idx, package_class_set);
            if target_concretes
                .iter()
                .any(|target| family.contains(target))
                && source_concretes
                    .iter()
                    .any(|source| !family_context.contains(source))
            {
                return false;
            }
        }
    }

    true
}
