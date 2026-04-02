// Cycle detection algorithm for Ecore models
//
// This module detects cycles in the containment hierarchy and determines
// where Box<T> wrappers are needed to break cycles in generated code.

use std::collections::{HashMap, HashSet};

use ecore_rs::{ctx::Ctx, prelude::idx::Class};

use crate::codegen::classifier::{has_subclasses, inherited_field_ident};

type ClassIdx = Class;

/// Represents a containment relationship in the type hierarchy
#[derive(Clone, Debug)]
struct ContainmentEdge {
    /// Source class
    source: ClassIdx,
    /// Target class (what is contained)
    target: ClassIdx,
    /// The reference/field name
    field_name: String,
    /// Is this a many-cardinality reference (collection)
    is_many: bool,
    /// Is this edge through a union type (abstract class variant)
    is_union_variant: bool,
}

/// Result of cycle analysis
#[derive(Debug)]
pub struct CycleAnalysis {
    /// Set of edges that form cycles
    #[allow(dead_code)]
    cycles: Vec<Vec<ContainmentEdge>>,
    /// Edges that need Box wrapping (feedback arc set)
    pub boxing_requirements: HashMap<(ClassIdx, String), BoxingStrategy>,
}

/// Strategy for applying Box wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxingStrategy {
    /// Box a single reference: field: Box<T>
    DirectReference,
    /// Box elements in a collection: field: ListLog<Box<T>>
    CollectionElement,
    /// Do not box (cycle broken elsewhere)
    NoBox,
}

impl CycleAnalysis {
    pub fn analyze(ctx: &Ctx) -> anyhow::Result<Self> {
        CycleAnalyzer::new(ctx).analyze()
    }

    /// Check if a specific field needs boxing
    pub fn needs_boxing(&self, source: ClassIdx, field_name: &str) -> bool {
        matches!(
            self.boxing_requirements.get(&(source, field_name.to_string())),
            Some(strategy) if *strategy != BoxingStrategy::NoBox
        )
    }

    /// Get the boxing strategy for a field
    pub fn boxing_strategy(&self, source: ClassIdx, field_name: &str) -> BoxingStrategy {
        self.boxing_requirements
            .get(&(source, field_name.to_string()))
            .copied()
            .unwrap_or(BoxingStrategy::NoBox)
    }
}

struct CycleAnalyzer<'a> {
    ctx: &'a Ctx,
}

impl<'a> CycleAnalyzer<'a> {
    fn new(ctx: &'a Ctx) -> Self {
        Self { ctx }
    }

    fn analyze(&self) -> anyhow::Result<CycleAnalysis> {
        let graph = self.build_containment_graph()?;
        let cycles = self.find_all_cycles(&graph);
        let mut boxing_requirements = Self::determine_boxing_strategy(&cycles);
        boxing_requirements.extend(self.determine_polymorphic_family_boxing(&graph));

        Ok(CycleAnalysis {
            cycles,
            boxing_requirements,
        })
    }

    /// Phase 1: Build the type dependency graph from the Ecore model
    fn build_containment_graph(&self) -> anyhow::Result<Vec<ContainmentEdge>> {
        let mut edges = Vec::new();

        for class in self.ctx.classes().iter() {
            let source = class.idx;

            for structural in class.structural() {
                if !structural.containment {
                    continue;
                }

                let target = structural.typ.unwrap();
                let target_class = &self.ctx.classes()[*target];
                let field_name = structural.name.clone();
                let is_many = structural.bounds.ubound != Some(1);

                if Self::is_polymorphic_class(target_class) {
                    for member in self.polymorphic_family_members(target) {
                        edges.push(ContainmentEdge {
                            source,
                            target: member,
                            field_name: field_name.clone(),
                            is_many,
                            is_union_variant: true,
                        });
                    }
                } else {
                    edges.push(ContainmentEdge {
                        source,
                        target,
                        field_name: field_name.clone(),
                        is_many,
                        is_union_variant: false,
                    });
                }
            }

            for superclass_idx in class.sup() {
                let superclass = &self.ctx.classes()[**superclass_idx];
                edges.push(ContainmentEdge {
                    source,
                    target: superclass.idx,
                    field_name: inherited_field_ident(superclass).to_string(),
                    is_many: false,
                    is_union_variant: false,
                });
            }
        }

        Ok(edges)
    }

    /// Phase 2: Find all elementary cycles in the containment graph
    fn find_all_cycles(&self, edges: &[ContainmentEdge]) -> Vec<Vec<ContainmentEdge>> {
        let mut cycles = Vec::new();
        let num_classes = self.ctx.classes().len();

        let mut adj_list: HashMap<ClassIdx, Vec<&ContainmentEdge>> = HashMap::new();
        for edge in edges {
            adj_list.entry(edge.source).or_default().push(edge);
        }

        let mut visited = HashSet::new();
        let mut rec_stack = Vec::new();
        let mut rec_stack_set = HashSet::new();

        for start_idx in 0..num_classes {
            let start = ClassIdx::from(start_idx);

            if !visited.contains(&start) {
                Self::dfs_find_cycles(
                    start,
                    &adj_list,
                    &mut visited,
                    &mut rec_stack,
                    &mut rec_stack_set,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    /// Depth-first search to find cycles
    fn dfs_find_cycles(
        node: ClassIdx,
        adj_list: &HashMap<ClassIdx, Vec<&ContainmentEdge>>,
        visited: &mut HashSet<ClassIdx>,
        rec_stack: &mut Vec<ClassIdx>,
        rec_stack_set: &mut HashSet<ClassIdx>,
        cycles: &mut Vec<Vec<ContainmentEdge>>,
    ) {
        visited.insert(node);
        rec_stack.push(node);
        rec_stack_set.insert(node);

        if let Some(outgoing_edges) = adj_list.get(&node) {
            for edge in outgoing_edges {
                let target = edge.target;

                if !visited.contains(&target) {
                    Self::dfs_find_cycles(
                        target,
                        adj_list,
                        visited,
                        rec_stack,
                        rec_stack_set,
                        cycles,
                    );
                } else if rec_stack_set.contains(&target)
                    && let Some(pos) = rec_stack.iter().position(|&n| n == target)
                {
                    let cycle_path: Vec<ClassIdx> = rec_stack[pos..].to_vec();

                    if !cycles.is_empty() {
                        let cycle_set: HashSet<_> = cycle_path.iter().cloned().collect();
                        let is_duplicate = cycles.iter().any(|existing_cycle| {
                            let existing_set: HashSet<_> =
                                existing_cycle.iter().map(|e| e.source).collect();
                            cycle_set == existing_set
                        });

                        if !is_duplicate {
                            let mut cycle_edges = Vec::new();
                            for i in 0..cycle_path.len() {
                                let from = cycle_path[i];
                                let to = cycle_path[(i + 1) % cycle_path.len()];

                                if let Some(edges) = adj_list.get(&from) {
                                    for e in edges {
                                        if e.target == to {
                                            cycle_edges.push((*e).clone());
                                            break;
                                        }
                                    }
                                }
                            }

                            if !cycle_edges.is_empty() {
                                cycles.push(cycle_edges);
                            }
                        }
                    } else {
                        let mut cycle_edges = Vec::new();
                        for i in 0..cycle_path.len() {
                            let from = cycle_path[i];
                            let to = cycle_path[(i + 1) % cycle_path.len()];

                            if let Some(edges) = adj_list.get(&from) {
                                for e in edges {
                                    if e.target == to {
                                        cycle_edges.push((*e).clone());
                                        break;
                                    }
                                }
                            }
                        }

                        if !cycle_edges.is_empty() {
                            cycles.push(cycle_edges);
                        }
                    }
                }
            }
        }

        rec_stack.pop();
        rec_stack_set.remove(&node);
    }

    /// Phase 3: Determine which edges need Box wrapping using heuristics
    fn determine_boxing_strategy(
        cycles: &[Vec<ContainmentEdge>],
    ) -> HashMap<(ClassIdx, String), BoxingStrategy> {
        let mut boxing_requirements: HashMap<(ClassIdx, String), BoxingStrategy> = HashMap::new();

        for cycle in cycles {
            if let Some(edge_to_box) = Self::select_edge_to_box(cycle) {
                let strategy = if edge_to_box.is_many {
                    BoxingStrategy::CollectionElement
                } else {
                    BoxingStrategy::DirectReference
                };

                boxing_requirements.insert(
                    (edge_to_box.source, edge_to_box.field_name.clone()),
                    strategy,
                );
            }
        }

        boxing_requirements
    }

    fn determine_polymorphic_family_boxing(
        &self,
        edges: &[ContainmentEdge],
    ) -> HashMap<(ClassIdx, String), BoxingStrategy> {
        let mut boxing_requirements = HashMap::new();
        let mut adj_list: HashMap<ClassIdx, Vec<ClassIdx>> = HashMap::new();

        for edge in edges {
            adj_list.entry(edge.source).or_default().push(edge.target);
        }

        for class in self.ctx.classes().iter() {
            for structural in class.structural() {
                if !structural.containment {
                    continue;
                }

                let Some(target_idx) = structural.typ else {
                    continue;
                };
                let target_class = &self.ctx.classes()[*target_idx];

                if !Self::is_polymorphic_class(target_class) {
                    continue;
                }

                let family_members = self.polymorphic_family_members(target_idx);
                let closes_family_cycle = family_members.into_iter().any(|member| {
                    let mut visited = HashSet::new();
                    Self::path_exists(&adj_list, member, class.idx, &mut visited)
                });

                if !closes_family_cycle {
                    continue;
                }

                let strategy = if structural.bounds.ubound != Some(1) {
                    BoxingStrategy::CollectionElement
                } else {
                    BoxingStrategy::DirectReference
                };
                boxing_requirements.insert((class.idx, structural.name.clone()), strategy);
            }
        }

        boxing_requirements
    }

    fn polymorphic_family_members(&self, root: ClassIdx) -> HashSet<ClassIdx> {
        let mut members = HashSet::new();
        let mut stack = vec![root];

        while let Some(class_idx) = stack.pop() {
            if !members.insert(class_idx) {
                continue;
            }

            let class = &self.ctx.classes()[*class_idx];
            stack.extend(class.sub().iter().copied());
        }

        members
    }

    fn path_exists(
        adj_list: &HashMap<ClassIdx, Vec<ClassIdx>>,
        start: ClassIdx,
        goal: ClassIdx,
        visited: &mut HashSet<ClassIdx>,
    ) -> bool {
        if start == goal {
            return true;
        }

        if !visited.insert(start) {
            return false;
        }

        adj_list.get(&start).is_some_and(|targets| {
            targets
                .iter()
                .copied()
                .any(|target| Self::path_exists(adj_list, target, goal, visited))
        })
    }

    fn is_polymorphic_class(class: &ecore_rs::repr::Class) -> bool {
        class.is_abstract() || class.is_interface() || has_subclasses(class)
    }

    /// Select the best edge to break in a cycle using heuristics
    fn select_edge_to_box(cycle: &[ContainmentEdge]) -> Option<ContainmentEdge> {
        if cycle.is_empty() {
            return None;
        }

        let union_edge = cycle.iter().find(|e| e.is_union_variant);
        if let Some(edge) = union_edge {
            return Some(edge.clone());
        }

        let collection_edge = cycle.iter().find(|e| e.is_many);
        if let Some(edge) = collection_edge {
            return Some(edge.clone());
        }

        let edge = cycle.iter().min_by_key(|_e| 0);

        edge.cloned()
    }
}

/// Analyzes the Ecore model and detects cycles requiring Box wrappers
pub fn analyze_cycles(ctx: &Ctx) -> anyhow::Result<CycleAnalysis> {
    CycleAnalysis::analyze(ctx)
}
