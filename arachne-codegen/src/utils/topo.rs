use ecore_rs::repr::Class;

use crate::utils::hash::{HashMap, HashSet};

/// Sorts classes in topological order of the inheritance hierarchy.
/// Parent classes come before child classes, allowing inheritance links to be generated.
pub fn topological_sort<'a>(ctx: &ecore_rs::ctx::Ctx, classes: &[&'a Class]) -> Vec<&'a Class> {
    let mut sorted = Vec::new();
    let mut visited = HashSet::default();
    let mut visiting = HashSet::default();

    // Build a map for quick lookup
    let class_map: HashMap<ecore_rs::repr::idx::Class, &'a Class> =
        classes.iter().map(|c| (c.idx, *c)).collect();

    fn visit<'a>(
        class: &'a Class,
        _ctx: &ecore_rs::ctx::Ctx,
        class_map: &HashMap<ecore_rs::repr::idx::Class, &'a Class>,
        sorted: &mut Vec<&'a Class>,
        visited: &mut HashSet<ecore_rs::repr::idx::Class>,
        visiting: &mut HashSet<ecore_rs::repr::idx::Class>,
    ) {
        if visited.contains(&class.idx) {
            return;
        }

        if visiting.contains(&class.idx) {
            // Cycle detected, skip to avoid infinite loop
            return;
        }

        visiting.insert(class.idx);

        // Visit all superclasses first
        for super_idx in class.sup() {
            if let Some(super_class) = class_map.get(super_idx) {
                visit(super_class, _ctx, class_map, sorted, visited, visiting);
            }
        }

        visiting.remove(&class.idx);
        visited.insert(class.idx);
        sorted.push(class);
    }

    for &class in classes {
        visit(
            class,
            ctx,
            &class_map,
            &mut sorted,
            &mut visited,
            &mut visiting,
        );
    }

    sorted
}
