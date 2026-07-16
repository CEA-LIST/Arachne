pub mod analysis;
pub mod containment;

use ecore_rs::{ctx::Ctx, prelude::idx};
use log::debug;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::{
    REFERENCES_PATH_MOD,
    codegen::{
        cycles::CycleAnalysis,
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        ident::{classifier_type_ident, classifier_type_ident_with_suffix, rust_ident, type_ident},
        import::{Import, Macros, Protocol},
        reference::analysis::{ReferenceAnalysis, analyze_references},
    },
    utils::hash::HashMap,
};

/// Top-level generator for non-containment reference support.
pub struct ReferenceGenerator<'a> {
    ctx: &'a Ctx,
    pack_classes: Vec<idx::Class>,
}

impl<'a> ReferenceGenerator<'a> {
    pub fn new(
        ctx: &'a Ctx,
        pack_classes: Vec<idx::Class>,
        _root_class_indices: Vec<idx::Class>,
        _cycle_analysis: &'a CycleAnalysis,
    ) -> Self {
        Self { ctx, pack_classes }
    }
}

impl<'a> Generate for ReferenceGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        debug!("Analyzing references...");
        let analysis = analyze_references(self.ctx, &self.pack_classes);

        if !analysis.has_references() {
            return Ok(Fragment::new(TokenStream::new(), vec![], vec![]));
        }

        debug!("Generating instance_from_sink_kind...");
        let instance_from_sink_kind = self.generate_instance_from_sink_kind(&analysis);
        debug!("Generating edge structs...");
        let edge_structs = self.generate_edge_structs(&analysis);
        debug!("Generating typed graph...");
        let typed_graph = self.generate_typed_graph(&analysis);
        let path =
            syn::parse_str::<syn::Path>(&format!("{}{}", PRIVATE_MOD_PREFIX, REFERENCES_PATH_MOD))
                .unwrap();
        let instance_variants = analysis
            .referenceable_classes
            .iter()
            .map(|&class_idx| {
                let class = &self.ctx.classes()[*class_idx];
                classifier_type_ident_with_suffix(self.ctx, class, "Id")
            })
            .collect::<Vec<_>>();

        let instance_path = quote! {
            pub fn instance_path(instance: &Instance) -> &#path::ObjectPath {
                match instance {
                    #(Instance::#instance_variants(id) => &id.0,)*
                }
            }
        };

        let tokens = quote! {
            #instance_from_sink_kind
            #instance_path

            #edge_structs

            #typed_graph
        };

        let imports = vec![
            Import::Macros(Macros::TypedGraph),
            Import::Protocol(Protocol::ObjectPath),
        ];

        Ok(Fragment::new(tokens, imports, vec![]))
    }
}

impl<'a> ReferenceGenerator<'a> {
    fn generate_instance_from_sink_kind(&self, analysis: &ReferenceAnalysis) -> TokenStream {
        let path =
            syn::parse_str::<syn::Path>(&format!("{}{}", PRIVATE_MOD_PREFIX, REFERENCES_PATH_MOD))
                .unwrap();

        let mut arms = Vec::new();

        for &vertex_class in &analysis.referenceable_classes {
            let vertex_class = &self.ctx.classes()[*vertex_class];
            let kind = classifier_type_ident(self.ctx, vertex_class).to_string();
            let id_ty = classifier_type_ident_with_suffix(self.ctx, vertex_class, "Id");
            let variant = classifier_type_ident_with_suffix(self.ctx, vertex_class, "Id");

            arms.push(quote! {
                #kind => Some(Instance::#variant(#id_ty(path.clone())))
            });
        }

        quote! {
            pub fn instance_from_sink_kind(
                kind: &str,
                path: &#path::ObjectPath,
            ) -> Option<Instance> {
                match kind {
                    #(#arms,)*
                    _ => None,
                }
            }
        }
    }

    fn edge_type_names(&self, analysis: &ReferenceAnalysis) -> Vec<Ident> {
        analysis
            .refs
            .iter()
            .map(|r| {
                let source_class = &self.ctx.classes()[*r.source_class];
                let source_name = classifier_type_ident(self.ctx, source_class);
                let reference_name = type_ident(&r.reference_name);
                rust_ident(format!("{source_name}{reference_name}Edge"))
            })
            .collect()
    }

    fn connection_names(&self, analysis: &ReferenceAnalysis) -> Vec<Ident> {
        let mut counts: HashMap<String, usize> = HashMap::default();

        analysis
            .refs
            .iter()
            .map(|r| {
                let source_class = &self.ctx.classes()[*r.source_class];
                let target_class = &self.ctx.classes()[*r.target_class];
                let source_name = classifier_type_ident(self.ctx, source_class);
                let target_name = classifier_type_ident(self.ctx, target_class);
                let base_name = format!("{source_name}To{target_name}");
                let suffix = counts.entry(base_name.clone()).or_insert(0);
                let unique_name = if *suffix == 0 {
                    base_name
                } else {
                    format!("{base_name}{}", *suffix + 1)
                };
                *suffix += 1;

                rust_ident(unique_name)
            })
            .collect()
    }

    /// Generate `#[derive(Debug, Clone, PartialEq, Eq, Hash)] pub struct {RefName}Edge;`
    /// for each non-containment reference.
    fn generate_edge_structs(&self, analysis: &ReferenceAnalysis) -> TokenStream {
        let mut seen = std::collections::HashSet::new();
        let edge_names = self.edge_type_names(analysis);
        let structs: Vec<TokenStream> = analysis
            .refs
            .iter()
            .zip(edge_names.iter())
            .filter_map(|(_, edge_name)| {
                if !seen.insert(edge_name.to_string()) {
                    return None;
                }

                Some(quote! {
                    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
                    pub struct #edge_name;
                })
            })
            .collect();

        quote! { #(#structs)* }
    }

    /// Generate the `typed_graph!` macro invocation.
    fn generate_typed_graph(&self, analysis: &ReferenceAnalysis) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, REFERENCES_PATH_MOD)).unwrap();
        let edge_names = self.edge_type_names(analysis);
        let connection_names = self.connection_names(analysis);

        // Vertices: one per referenceable class
        let vertices: Vec<Ident> = analysis
            .referenceable_classes
            .iter()
            .map(|&class_idx| {
                let class = &self.ctx.classes()[*class_idx];
                classifier_type_ident_with_suffix(self.ctx, class, "Id")
            })
            .collect();

        let mut seen_edge_types = std::collections::HashSet::new();
        let edge_types: Vec<TokenStream> = analysis
            .refs
            .iter()
            .zip(edge_names.iter())
            .filter_map(|(r, edge_name)| {
                if !seen_edge_types.insert(edge_name.to_string()) {
                    return None;
                }

                let lower = proc_macro2::Literal::usize_unsuffixed(r.lower_bound);
                let upper_token: TokenStream = match r.upper_bound {
                    Some(u) => {
                        let lit = proc_macro2::Literal::usize_unsuffixed(u);
                        quote! { #lit }
                    }
                    None => quote! { * },
                };

                Some(quote! {
                    #edge_name [#lower, #upper_token]
                })
            })
            .collect();

        // Connections: one per non-containment reference
        let connections: Vec<TokenStream> = analysis
            .refs
            .iter()
            .zip(edge_names.iter().zip(connection_names.iter()))
            .map(|(r, (edge_name, conn_name))| {
                let source_class = &self.ctx.classes()[*r.source_class];
                let target_class = &self.ctx.classes()[*r.target_class];
                let source_id = classifier_type_ident_with_suffix(self.ctx, source_class, "Id");
                let target_id = classifier_type_ident_with_suffix(self.ctx, target_class, "Id");

                quote! {
                    #conn_name: #source_id -> #target_id (#edge_name)
                }
            })
            .collect();

        quote! {
            #path::typed_graph! {
                types {
                    graph = ReferenceManager,
                    vertex_kind = Instance,
                    edge_kind = Ref,
                    arc_kind = Refs,
                },

                vertices {
                    #(#vertices),*
                },

                edges {
                    #(#edge_types),*
                },

                arcs {
                    #(#connections),*
                }
            }
        }
    }
}
