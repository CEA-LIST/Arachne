use ecore_rs::ctx::Ctx;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::codegen::reference::{
    analysis::ReferenceAnalysis,
    containment::{ContainmentPath, PathStep},
};

/// Generate the `ModelLog` struct, `IsLog` impl, and `EvalNested` impl.
pub fn generate_model_log(
    ctx: &Ctx,
    analysis: &ReferenceAnalysis,
    root_class_name: &str,
    creation_paths: &[ContainmentPath],
) -> TokenStream {
    let model_log_struct = generate_model_log_struct(root_class_name);
    let is_log_impl = generate_is_log_impl(ctx, analysis, root_class_name, creation_paths);
    let eval_nested_impl = generate_eval_nested_impl(root_class_name);

    quote! {
        #model_log_struct
        #is_log_impl
        #eval_nested_impl
    }
}

/// Generate the `ModelLog` struct.
fn generate_model_log_struct(root_class_name: &str) -> TokenStream {
    let root_log = format_ident!("{}Log", root_class_name);

    quote! {
        #[derive(Debug, Clone, Default)]
        pub struct ModelLog {
            pub root_log: #root_log,
            pub reference_manager_log: VecLog<ReferenceManager<LwwPolicy>>,
        }
    }
}

/// Generate the `IsLog` implementation for `ModelLog`.
fn generate_is_log_impl(
    ctx: &Ctx,
    _analysis: &ReferenceAnalysis,
    root_class_name: &str,
    creation_paths: &[ContainmentPath],
) -> TokenStream {
    let root_value = format_ident!("{}Value", root_class_name);

    // Generate vertex creation match arms
    let creation_arms = generate_creation_arms(ctx, creation_paths);

    // Generate vertex deletion match arms (only for direct list paths without Box)
    let deletion_arms = generate_deletion_arms(ctx, creation_paths);

    quote! {
        impl IsLog for ModelLog {
            type Value = (#root_value, <ReferenceManager<LwwPolicy> as PureCRDT>::Value);
            type Op = Model;

            fn is_enabled(&self, op: &Self::Op) -> bool {
                match op {
                    Model::Root(o) => self.root_log.is_enabled(o),
                    Model::Reference(o) => self
                        .reference_manager_log
                        .is_enabled(&ReferenceManager::AddArc(o.clone())),
                }
            }

            fn effect(&mut self, event: Event<Self::Op>) {
                match &event.op() {
                    #creation_arms
                    #deletion_arms
                    _ => {}
                }

                match event.op().clone() {
                    Model::Root(root) => self.root_log.effect(Event::unfold(event, root)),
                    Model::Reference(refs) => self
                        .reference_manager_log
                        .effect(Event::unfold(event, ReferenceManager::AddArc(refs))),
                }
            }

            fn stabilize(&mut self, version: &Version) {
                self.root_log.stabilize(version);
                self.reference_manager_log.stabilize(version);
            }

            fn redundant_by_parent(&mut self, version: &Version, conservative: bool) {
                self.root_log.redundant_by_parent(version, conservative);
                self.reference_manager_log
                    .redundant_by_parent(version, conservative);
            }

            fn is_default(&self) -> bool {
                self.root_log.is_default()
            }
        }
    }
}

/// Generate match arms for vertex creation (List::Insert).
///
/// Paths that share the same outer match pattern (up to a Box boundary) and the
/// same vertex class are grouped into a single match arm with OR-combined inner
/// patterns. This avoids duplicate outer arms that Rust would flag as unreachable.
fn generate_creation_arms(ctx: &Ctx, creation_paths: &[ContainmentPath]) -> TokenStream {
    use std::collections::BTreeMap;

    let mut non_boxed_arms: Vec<TokenStream> = Vec::new();

    // Group boxed paths by (serialized outer steps, vertex class name).
    // Value: (outer_steps, vertex_name, inner_patterns)
    let mut boxed_groups: BTreeMap<String, (Vec<PathStep>, String, Vec<TokenStream>)> =
        BTreeMap::new();

    for path in creation_paths {
        let box_index = path
            .steps
            .iter()
            .position(|step| matches!(step, PathStep::Field { is_boxed: true, .. }));

        if let Some(box_idx) = box_index {
            let outer_steps = path.steps[..=box_idx].to_vec();
            let inner_steps = &path.steps[box_idx + 1..];
            let vertex_name = ctx.classes()[*path.vertex_class].name().to_string();
            let key = format!("{}_{}", outer_steps_key(&outer_steps), vertex_name);

            let inner_pattern = build_nested_pattern(inner_steps);

            boxed_groups
                .entry(key)
                .or_insert_with(|| (outer_steps, vertex_name, Vec::new()))
                .2
                .push(inner_pattern);
        } else {
            let vertex_class = &ctx.classes()[*path.vertex_class];
            let id_struct = format_ident!("{}Id", vertex_class.name());
            let instance_variant = format_ident!("{}Id", vertex_class.name());
            let pattern = build_nested_pattern(&path.steps);

            non_boxed_arms.push(quote! {
                Model::Root(#pattern) => {
                    let id = #id_struct(event.id().clone());
                    let new_vertex = ReferenceManager::<LwwPolicy>::AddVertex {
                        id: Instance::#instance_variant(id),
                    };
                    self.reference_manager_log
                        .effect(Event::unfold(event.clone(), new_vertex));
                }
            });
        }
    }

    // Generate grouped boxed arms
    let mut boxed_arms: Vec<TokenStream> = Vec::new();
    for (outer_steps, vertex_name, inner_patterns) in boxed_groups.values() {
        let outer_pattern = build_nested_pattern_with_capture(outer_steps, "inner_val");
        let inner_var = format_ident!("inner_val");

        let id_struct = format_ident!("{}Id", vertex_name);
        let instance_variant = format_ident!("{}Id", vertex_name);

        boxed_arms.push(quote! {
            Model::Root(#outer_pattern) => {
                if let #(#inner_patterns)|* = #inner_var.as_ref() {
                    let id = #id_struct(event.id().clone());
                    let new_vertex = ReferenceManager::<LwwPolicy>::AddVertex {
                        id: Instance::#instance_variant(id),
                    };
                    self.reference_manager_log
                        .effect(Event::unfold(event.clone(), new_vertex));
                }
            }
        });
    }

    let all_arms = boxed_arms.into_iter().chain(non_boxed_arms);
    quote! { #(#all_arms)* }
}

/// Generate a string key from path steps for grouping.
fn outer_steps_key(steps: &[PathStep]) -> String {
    steps
        .iter()
        .map(|s| match s {
            PathStep::Field {
                class_name,
                variant_name,
                ..
            } => format!("F:{}/{}", class_name, variant_name),
            PathStep::Variant {
                union_name,
                variant_name,
            } => format!("V:{}/{}", union_name, variant_name),
            PathStep::ListInsert => "LI".to_string(),
            PathStep::ListDelete => "LD".to_string(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Generate match arms for vertex deletion (List::Delete).
/// Only generates for simple paths (no Box in the path).
fn generate_deletion_arms(ctx: &Ctx, creation_paths: &[ContainmentPath]) -> TokenStream {
    let arms: Vec<TokenStream> = creation_paths
        .iter()
        .filter(|path| {
            // Only handle deletion for paths without Box
            !path
                .steps
                .iter()
                .any(|step| matches!(step, PathStep::Field { is_boxed: true, .. }))
        })
        .map(|path| {
            let vertex_class = &ctx.classes()[*path.vertex_class];
            let id_struct = format_ident!("{}Id", vertex_class.name());
            let instance_variant = format_ident!("{}Id", vertex_class.name());

            // Build the deletion pattern (same as creation but with ListDelete)
            let delete_steps: Vec<PathStep> = path
                .steps
                .iter()
                .map(|s| match s {
                    PathStep::ListInsert => PathStep::ListDelete,
                    other => other.clone(),
                })
                .collect();

            let pattern = build_nested_pattern(&delete_steps);

            // Build the log field path for position lookup
            let log_path = build_log_field_path(&path.log_field_path);

            quote! {
                Model::Root(#pattern) => {
                    let positions = self
                        .root_log
                        #log_path
                        .position
                        .execute_query(Read::new());
                    let event_id = positions[*pos].clone();
                    let remove_vertex = ReferenceManager::<LwwPolicy>::RemoveVertex {
                        id: Instance::#instance_variant(#id_struct(event_id)),
                    };
                    self.reference_manager_log
                        .effect(Event::unfold(event.clone(), remove_vertex));
                }
            }
        })
        .collect();

    quote! { #(#arms)* }
}

/// Build a fully nested match pattern from path steps.
/// The innermost binding is `..` for Insert or `{ pos }` for Delete.
fn build_nested_pattern(steps: &[PathStep]) -> TokenStream {
    if steps.is_empty() {
        return quote! { _ };
    }

    // Build from the inside out (right to left)
    let mut pattern = build_leaf_pattern(steps.last().unwrap());

    for step in steps.iter().rev().skip(1) {
        pattern = wrap_step_pattern(step, pattern);
    }

    pattern
}

/// Build a nested match pattern, but capture a variable at the last step
/// instead of continuing the nesting.
fn build_nested_pattern_with_capture(steps: &[PathStep], capture_name: &str) -> TokenStream {
    if steps.is_empty() {
        let var = Ident::new(capture_name, Span::call_site());
        return quote! { #var };
    }

    let var = Ident::new(capture_name, Span::call_site());

    // Last step captures the variable
    let mut pattern = match steps.last().unwrap() {
        PathStep::Field {
            class_name,
            variant_name,
            ..
        } => {
            let class = Ident::new(class_name, Span::call_site());
            let variant = Ident::new(variant_name, Span::call_site());
            quote! { #class::#variant(#var) }
        }
        PathStep::Variant {
            union_name,
            variant_name,
        } => {
            let union_n = Ident::new(union_name, Span::call_site());
            let variant = Ident::new(variant_name, Span::call_site());
            quote! { #union_n::#variant(#var) }
        }
        _ => quote! { #var },
    };

    // Wrap remaining steps from inside out
    for step in steps.iter().rev().skip(1) {
        pattern = wrap_step_pattern(step, pattern);
    }

    pattern
}

/// Build the leaf pattern for the innermost step.
fn build_leaf_pattern(step: &PathStep) -> TokenStream {
    match step {
        PathStep::ListInsert => quote! { List::Insert { .. } },
        PathStep::ListDelete => quote! { List::Delete { pos } },
        PathStep::Field {
            class_name,
            variant_name,
            ..
        } => {
            let class = Ident::new(class_name, Span::call_site());
            let variant = Ident::new(variant_name, Span::call_site());
            quote! { #class::#variant(..) }
        }
        PathStep::Variant {
            union_name,
            variant_name,
        } => {
            let union_n = Ident::new(union_name, Span::call_site());
            let variant = Ident::new(variant_name, Span::call_site());
            quote! { #union_n::#variant(..) }
        }
    }
}

/// Wrap an inner pattern in an outer step.
fn wrap_step_pattern(step: &PathStep, inner: TokenStream) -> TokenStream {
    match step {
        PathStep::Field {
            class_name,
            variant_name,
            ..
        } => {
            let class = Ident::new(class_name, Span::call_site());
            let variant = Ident::new(variant_name, Span::call_site());
            quote! { #class::#variant(#inner) }
        }
        PathStep::Variant {
            union_name,
            variant_name,
        } => {
            let union_n = Ident::new(union_name, Span::call_site());
            let variant = Ident::new(variant_name, Span::call_site());
            quote! { #union_n::#variant(#inner) }
        }
        PathStep::ListInsert => quote! { List::Insert { .. } },
        PathStep::ListDelete => quote! { List::Delete { pos } },
    }
}

/// Build the log field path for accessing position data during deletion.
/// e.g., `["main", "blackboard", "entries"]` → `.main.blackboard.entries`
fn build_log_field_path(path: &[String]) -> TokenStream {
    let fields: Vec<Ident> = path
        .iter()
        .map(|f| Ident::new(f, Span::call_site()))
        .collect();

    quote! { #(.#fields)* }
}

/// Generate the `EvalNested` implementation for `ModelLog`.
fn generate_eval_nested_impl(_root_class_name: &str) -> TokenStream {
    quote! {
        impl EvalNested<Read<<Self as IsLog>::Value>> for ModelLog {
            fn execute_query(
                &self,
                _q: Read<<Self as IsLog>::Value>,
            ) -> <Read<<Self as IsLog>::Value> as QueryOperation>::Response {
                let root = self.root_log.execute_query(Read::new());
                let reference_manager = self.reference_manager_log.execute_query(Read::new());
                (root, reference_manager)
            }
        }
    }
}
