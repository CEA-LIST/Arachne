pub mod analysis;
pub mod containment;

use ecore_rs::{ctx::Ctx, prelude::idx};
use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::{
    REFERENCES_PATH_MOD,
    codegen::{
        cycles::CycleAnalysis,
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        import::{Import, Macros, Protocol},
        reference::analysis::{ReferenceAnalysis, analyze_references},
        reference::containment::{PathStep, find_creation_paths},
    },
    utils::hash::HashMap,
};

/// Top-level generator for non-containment reference support.
pub struct ReferenceGenerator<'a> {
    ctx: &'a Ctx,
    pack_classes: Vec<idx::Class>,
    root_class_indices: Vec<idx::Class>,
    cycle_analysis: &'a CycleAnalysis,
}

impl<'a> ReferenceGenerator<'a> {
    pub fn new(
        ctx: &'a Ctx,
        pack_classes: Vec<idx::Class>,
        root_class_indices: Vec<idx::Class>,
        cycle_analysis: &'a CycleAnalysis,
    ) -> Self {
        Self {
            ctx,
            pack_classes,
            root_class_indices,
            cycle_analysis,
        }
    }
}

impl<'a> Generate for ReferenceGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let analysis = analyze_references(self.ctx, &self.pack_classes);

        if !analysis.has_references() {
            return Ok(Fragment::new(TokenStream::new(), vec![], vec![]));
        }

        let instance_from_path = self.generate_instance_from_path(&analysis);
        let id_structs = self.generate_id_structs(&analysis);
        let edge_structs = self.generate_edge_structs(&analysis);
        let typed_graph = self.generate_typed_graph(&analysis);

        let path =
            syn::parse_str::<syn::Path>(&format!("{}{}", PRIVATE_MOD_PREFIX, REFERENCES_PATH_MOD))
                .unwrap();

        let vertex_ops_from_sink = if analysis.has_references() {
            quote! {
                pub fn vertex_ops_from_sink<P: #path::Policy>(sink: &#path::Sink) -> Option<ReferenceManager<P>> {
                    let instance = instance_from_path(sink.object_path())?;

                    match sink.effect() {
                        #path::SinkEffect::Create | #path::SinkEffect::Update => {
                            Some(#path::ReferenceManager::AddVertex { id: instance })
                        }
                        #path::SinkEffect::Delete => Some(#path::ReferenceManager::RemoveVertex { id: instance }),
                    }
                }
            }
        } else {
            quote! {}
        };

        let tokens = quote! {
            #instance_from_path

            #vertex_ops_from_sink

            #id_structs
            #edge_structs
            #typed_graph
        };

        let imports = vec![
            Import::Macros(Macros::TypedGraph),
            Import::Protocol(Protocol::Sink),
            Import::Protocol(Protocol::ObjectPath),
            Import::Protocol(Protocol::PathSegment),
            Import::Protocol(Protocol::SinkEffect),
            Import::Protocol(Protocol::Policy),
            Import::Custom("crate::references::*"),
        ];

        Ok(Fragment::new(tokens, imports, vec![]))
    }
}

impl<'a> ReferenceGenerator<'a> {
    fn referenceable_class_chain(
        &self,
        class_idx: idx::Class,
        analysis: &ReferenceAnalysis,
    ) -> Vec<idx::Class> {
        fn collect(
            ctx: &Ctx,
            class_idx: idx::Class,
            referenceable: &[idx::Class],
            acc: &mut Vec<idx::Class>,
        ) {
            if referenceable.contains(&class_idx) && !acc.contains(&class_idx) {
                acc.push(class_idx);
            }
            for super_idx in ctx.classes()[*class_idx].sup() {
                collect(ctx, *super_idx, referenceable, acc);
            }
        }

        let mut out = Vec::new();
        collect(
            self.ctx,
            class_idx,
            &analysis.referenceable_classes,
            &mut out,
        );
        out
    }

    fn generate_instance_from_path(&self, analysis: &ReferenceAnalysis) -> TokenStream {
        let path =
            syn::parse_str::<syn::Path>(&format!("{}{}", PRIVATE_MOD_PREFIX, REFERENCES_PATH_MOD))
                .unwrap();

        let mut seen = std::collections::HashSet::<String>::new();
        let mut arms = Vec::new();

        for &root_idx in &self.root_class_indices {
            let root_class = &self.ctx.classes()[*root_idx];
            if root_class.is_concrete() {
                for ref_class_idx in self.referenceable_class_chain(root_idx, analysis) {
                    let ref_class = &self.ctx.classes()[*ref_class_idx];
                    let id_ty = format_ident!("{}Id", ref_class.name());
                    let variant = format_ident!("{}Id", ref_class.name());
                    let key = format!("root:{}:{}", root_class.name(), ref_class.name());
                    if seen.insert(key) {
                        arms.push(quote! {
                            [] => Some(Instance::#variant(#id_ty(path.clone())))
                        });
                    }
                }
            }

            for containment_path in
                find_creation_paths(self.ctx, root_idx, analysis, self.cycle_analysis)
            {
                let vertex_class = &self.ctx.classes()[*containment_path.vertex_class];
                let id_ty = format_ident!("{}Id", vertex_class.name());
                let variant = format_ident!("{}Id", vertex_class.name());

                let seg_patterns: Vec<TokenStream> = containment_path
                    .steps
                    .iter()
                    .filter_map(|step| match step {
                        PathStep::Field { variant_name, .. } => {
                            let field_name = variant_name.to_snake_case();
                            Some(quote! { Field(#field_name) })
                        }
                        PathStep::Variant { variant_name, .. } => {
                            Some(quote! { Variant(#variant_name) })
                        }
                        PathStep::ListInsert | PathStep::ListDelete => {
                            Some(quote! { ListElement(_) })
                        }
                    })
                    .collect();

                let pattern_key = seg_patterns
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("|");
                let key = format!("{}:{}", vertex_class.name(), pattern_key);

                if seen.insert(key) {
                    arms.push(quote! {
                        [.., #(#seg_patterns),*] => {
                            Some(Instance::#variant(#id_ty(path.clone())))
                        }
                    });
                }
            }
        }

        quote! {
            fn instance_from_path(path: &#path::ObjectPath) -> Option<Instance> {
                use #path::PathSegment::*;

                let segs = path.segments();

                match segs {
                    #(#arms,)*
                    _ => None,
                }
            }
        }
    }

    fn reference_type_names(&self, analysis: &ReferenceAnalysis) -> Vec<(Ident, Ident)> {
        let mut counts: HashMap<String, usize> = HashMap::default();

        analysis
            .refs
            .iter()
            .map(|r| {
                let source_class = &self.ctx.classes()[*r.source_class];
                let base_name = format!(
                    "{}{}",
                    source_class.name().to_upper_camel_case(),
                    r.reference_name.to_upper_camel_case()
                );
                let suffix = counts.entry(base_name.clone()).or_insert(0);
                let unique_name = if *suffix == 0 {
                    base_name
                } else {
                    format!("{base_name}{}", *suffix + 1)
                };
                *suffix += 1;

                (
                    Ident::new(&format!("{unique_name}Edge"), Span::call_site()),
                    Ident::new(&unique_name, Span::call_site()),
                )
            })
            .collect()
    }

    pub fn generate_id_structs(&self, analysis: &ReferenceAnalysis) -> TokenStream {
        let path =
            syn::parse_str::<syn::Path>(&format!("{}{}", PRIVATE_MOD_PREFIX, REFERENCES_PATH_MOD))
                .unwrap();
        let structs: Vec<TokenStream> = analysis
            .referenceable_classes
            .iter()
            .map(|&class_idx| {
                let class = &self.ctx.classes()[*class_idx];
                let id_name = format_ident!("{}Id", class.name());
                quote! {
                    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
                    pub struct #id_name(pub #path::ObjectPath);
                }
            })
            .collect();

        quote! { #(#structs)* }
    }

    /// Generate `#[derive(Debug, Clone, PartialEq, Eq, Hash)] pub struct {RefName}Edge;`
    /// for each non-containment reference.
    fn generate_edge_structs(&self, analysis: &ReferenceAnalysis) -> TokenStream {
        let reference_names = self.reference_type_names(analysis);
        let structs: Vec<TokenStream> = analysis
            .refs
            .iter()
            .zip(reference_names.iter())
            .map(|(_, (edge_name, _))| {
                quote! {
                    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
                    pub struct #edge_name;
                }
            })
            .collect();

        quote! { #(#structs)* }
    }

    /// Generate the `typed_graph!` macro invocation.
    fn generate_typed_graph(&self, analysis: &ReferenceAnalysis) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, REFERENCES_PATH_MOD)).unwrap();
        let reference_names = self.reference_type_names(analysis);

        // Vertices: one per referenceable class
        let vertices: Vec<Ident> = analysis
            .referenceable_classes
            .iter()
            .map(|&class_idx| {
                let class = &self.ctx.classes()[*class_idx];
                format_ident!("{}Id", class.name())
            })
            .collect();

        // Connections: one per non-containment reference
        let connections: Vec<TokenStream> = analysis
            .refs
            .iter()
            .zip(reference_names.iter())
            .map(|(r, (edge_name, conn_name))| {
                let source_class = &self.ctx.classes()[*r.source_class];
                let target_class = &self.ctx.classes()[*r.target_class];
                let source_id = format_ident!("{}Id", source_class.name());
                let target_id = format_ident!("{}Id", target_class.name());
                let lower = proc_macro2::Literal::usize_unsuffixed(r.lower_bound);
                let upper_token: TokenStream = match r.upper_bound {
                    Some(u) => {
                        let lit = proc_macro2::Literal::usize_unsuffixed(u);
                        quote! { #lit }
                    }
                    None => quote! { * },
                };

                quote! {
                    #conn_name: #source_id -> #target_id (#edge_name) [#lower, #upper_token]
                }
            })
            .collect();

        quote! {
            #path::typed_graph! {
                graph: ReferenceManager,
                vertex: Instance,
                edge: Ref,
                arcs_type: Refs,
                vertices { #(#vertices),* },
                connections {
                    #(#connections),*
                }
            }
        }
    }
}
