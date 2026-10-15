// Cycle detection algorithm for Ecore models
//
// This module detects cycles in the containment hierarchy and determines
// where Box<T> wrappers are needed to break cycles in generated code.

use std::collections::HashMap;

use ecore_rs::prelude::idx::Class;

use crate::codegen::{
    analysis::ModelIndex,
    classifier::{
        has_codegen_polymorphic_family, inherited_field_ident, is_uninhabited_polymorphic_class,
    },
};

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
    /// Edges that need Box wrapping (feedback arc set)
    pub boxing_requirements: HashMap<(ClassIdx, String), BoxingStrategy>,
}

/// Strategy for applying Box wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxingStrategy {
    /// Box a single reference value: field log uses BoxedLog<T>
    DirectReference,
    /// Box collection element values: field log uses ListLog<BoxedLog<T>>
    CollectionElement,
    /// Do not box (cycle broken elsewhere)
    NoBox,
}

impl CycleAnalysis {
    pub fn analyze(ctx: &ModelIndex<'_>) -> anyhow::Result<Self> {
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
    ctx: &'a ModelIndex<'a>,
}

impl<'a> CycleAnalyzer<'a> {
    fn new(ctx: &'a ModelIndex<'a>) -> Self {
        Self { ctx }
    }

    fn analyze(&self) -> anyhow::Result<CycleAnalysis> {
        let graph = self.build_containment_graph()?;
        let boxing_requirements = select_boxing(&graph, self.ctx.classes().len())?;
        Ok(CycleAnalysis {
            boxing_requirements,
        })
    }

    /// Phase 1: Build the type dependency graph from the Ecore model
    fn build_containment_graph(&self) -> anyhow::Result<Vec<ContainmentEdge>> {
        let mut edges = Vec::new();

        for class in self.ctx.classes().iter() {
            let source = class.idx;

            for structural in self.ctx.supported_features(class.idx) {
                if !structural.containment {
                    continue;
                }

                let target = structural.typ.unwrap();
                let target_class = &self.ctx.classes()[*target];
                let field_name = structural.name.clone();
                let is_many = structural.bounds.ubound != Some(1);

                if is_uninhabited_polymorphic_class(self.ctx, target_class) {
                    continue;
                }

                if has_codegen_polymorphic_family(self.ctx, target_class) {
                    for &member in self.polymorphic_family_members(target) {
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

    fn polymorphic_family_members(&self, root: ClassIdx) -> &[ClassIdx] {
        &self.ctx.info(root).family
    }
}

/// Analyzes the Ecore model and detects cycles requiring Box wrappers
pub fn analyze_cycles(ctx: &ModelIndex<'_>) -> anyhow::Result<CycleAnalysis> {
    CycleAnalysis::analyze(ctx)
}

/// Compact adjacency and field IDs keep all passes linear in graph size. A field
/// can expand to several polymorphic edges; boxing it removes all those edges.
struct Graph<'a> {
    edges: &'a [ContainmentEdge],
    outgoing: Vec<Vec<usize>>,
    incoming: Vec<Vec<usize>>,
    field_ids: Vec<usize>,
    fields: Vec<usize>,
    boxed: Vec<bool>,
}

impl<'a> Graph<'a> {
    fn new(edges: &'a [ContainmentEdge], nodes: usize) -> Self {
        let mut outgoing = vec![Vec::new(); nodes];
        let mut incoming = vec![Vec::new(); nodes];
        let mut ids = HashMap::new();
        let mut fields = Vec::new();
        let mut field_ids = Vec::with_capacity(edges.len());
        for (index, edge) in edges.iter().enumerate() {
            outgoing[*edge.source].push(index);
            incoming[*edge.target].push(index);
            let id = *ids
                .entry((edge.source, edge.field_name.as_str()))
                .or_insert_with(|| {
                    fields.push(index);
                    fields.len() - 1
                });
            field_ids.push(id);
        }
        let boxed = vec![false; fields.len()];
        Self {
            edges,
            outgoing,
            incoming,
            field_ids,
            fields,
            boxed,
        }
    }

    fn active(&self, edge: usize) -> bool {
        !self.boxed[self.field_ids[edge]]
    }

    // Iterative Kosaraju: no call-stack growth, even for deeply nested models.
    fn components(&self) -> Vec<usize> {
        let nodes = self.outgoing.len();
        let mut seen = vec![false; nodes];
        let mut finish = Vec::with_capacity(nodes);
        let mut stack = Vec::new();
        for start in 0..nodes {
            if seen[start] {
                continue;
            }
            seen[start] = true;
            stack.push((start, 0));
            while let Some((node, next)) = stack.last_mut() {
                if *next == self.outgoing[*node].len() {
                    finish.push(*node);
                    stack.pop();
                    continue;
                }
                let edge = self.outgoing[*node][*next];
                *next += 1;
                let target = *self.edges[edge].target;
                if self.active(edge) && !seen[target] {
                    seen[target] = true;
                    stack.push((target, 0));
                }
            }
        }
        let mut component = vec![usize::MAX; nodes];
        let mut pending = Vec::new();
        for start in finish.into_iter().rev() {
            if component[start] != usize::MAX {
                continue;
            }
            component[start] = start;
            pending.push(start);
            while let Some(node) = pending.pop() {
                for &edge in &self.incoming[node] {
                    let source = *self.edges[edge].source;
                    if self.active(edge) && component[source] == usize::MAX {
                        component[source] = start;
                        pending.push(source);
                    }
                }
            }
        }
        component
    }

    fn box_preferred(&mut self, union: bool) {
        let components = self.components();
        for (index, edge) in self.edges.iter().enumerate() {
            let preferred = if union {
                edge.is_union_variant
            } else {
                edge.is_many
            };
            if preferred && components[*edge.source] == components[*edge.target] {
                self.boxed[self.field_ids[index]] = true;
            }
        }
    }

    fn box_back_edges(&mut self) {
        let mut color = vec![0u8; self.outgoing.len()];
        let mut stack = Vec::new();
        // Reverse class order gives stable choices and favors earlier declared
        // fields in ordinary scalar cycles, as the original generator did.
        for start in (0..color.len()).rev() {
            if color[start] != 0 {
                continue;
            }
            color[start] = 1;
            stack.push((start, 0));
            while let Some((node, next)) = stack.last_mut() {
                if *next == self.outgoing[*node].len() {
                    color[*node] = 2;
                    stack.pop();
                    continue;
                }
                let edge = self.outgoing[*node][*next];
                *next += 1;
                if !self.active(edge) {
                    continue;
                }
                let target = *self.edges[edge].target;
                match color[target] {
                    0 => {
                        color[target] = 1;
                        stack.push((target, 0));
                    }
                    1 => self.boxed[self.field_ids[edge]] = true,
                    _ => {}
                }
            }
        }
    }

    fn is_acyclic(&self) -> bool {
        let mut incoming = vec![0usize; self.outgoing.len()];
        for (index, edge) in self.edges.iter().enumerate() {
            if self.active(index) {
                incoming[*edge.target] += 1;
            }
        }
        let mut pending = incoming
            .iter()
            .enumerate()
            .filter_map(|(node, &count)| (count == 0).then_some(node))
            .collect::<Vec<_>>();
        let mut visited = 0;
        while let Some(node) = pending.pop() {
            visited += 1;
            for &index in &self.outgoing[node] {
                if self.active(index) {
                    let target = *self.edges[index].target;
                    incoming[target] -= 1;
                    if incoming[target] == 0 {
                        pending.push(target);
                    }
                }
            }
        }
        visited == incoming.len()
    }
}

fn select_boxing(
    edges: &[ContainmentEdge],
    nodes: usize,
) -> anyhow::Result<HashMap<(ClassIdx, String), BoxingStrategy>> {
    let mut graph = Graph::new(edges, nodes);
    // Preserve family boxing: a union edge is cyclic exactly when source and
    // target share an SCC. Recompute SCCs before preferring collection edges.
    graph.box_preferred(true);
    graph.box_preferred(false);
    graph.box_back_edges();
    anyhow::ensure!(
        graph.is_acyclic(),
        "containment boxing left a recursive type dependency"
    );
    Ok(graph
        .fields
        .iter()
        .enumerate()
        .filter_map(|(id, &index)| {
            if !graph.boxed[id] {
                return None;
            }
            let edge = &edges[index];
            let strategy = if edge.is_many {
                BoxingStrategy::CollectionElement
            } else {
                BoxingStrategy::DirectReference
            };
            Some(((edge.source, edge.field_name.clone()), strategy))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(source: usize, target: usize, many: bool, union: bool) -> ContainmentEdge {
        ContainmentEdge {
            source: source.into(),
            target: target.into(),
            field_name: format!("field_{source}_{target}"),
            is_many: many,
            is_union_variant: union,
        }
    }

    #[test]
    fn dense_dag_needs_no_boxing() {
        let edges = (0..200)
            .flat_map(|s| (s + 1..200).map(move |t| edge(s, t, false, false)))
            .collect::<Vec<_>>();
        assert!(select_boxing(&edges, 200).unwrap().is_empty());
    }

    #[test]
    fn deep_graph_does_not_use_the_call_stack() {
        let mut edges = (0..50_000)
            .map(|s| edge(s, s + 1, false, false))
            .collect::<Vec<_>>();
        edges.push(edge(50_000, 0, false, false));
        assert_eq!(select_boxing(&edges, 50_001).unwrap().len(), 1);
    }

    #[test]
    fn every_small_directed_graph_is_broken() {
        for mask in 0u32..(1 << 16) {
            let edges = (0..16)
                .filter(|bit| mask & (1 << bit) != 0)
                .map(|bit| edge(bit / 4, bit % 4, bit % 3 == 0, bit % 5 == 0))
                .collect::<Vec<_>>();
            let boxed = select_boxing(&edges, 4).unwrap();
            let remaining = edges
                .into_iter()
                .filter(|e| !boxed.contains_key(&(e.source, e.field_name.clone())))
                .collect::<Vec<_>>();
            assert!(Graph::new(&remaining, 4).is_acyclic(), "graph {mask}");
        }
    }

    #[test]
    fn polymorphic_field_removes_all_its_edges() {
        let mut edges = vec![
            edge(0, 1, true, true),
            edge(0, 2, true, true),
            edge(1, 0, false, false),
            edge(2, 0, false, false),
        ];
        edges[1].field_name = edges[0].field_name.clone();
        let boxed = select_boxing(&edges, 3).unwrap();
        assert_eq!(boxed.len(), 1);
        assert_eq!(
            boxed[&(0.into(), edges[0].field_name.clone())],
            BoxingStrategy::CollectionElement
        );
    }
}
