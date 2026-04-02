pub mod analysis;
pub mod containment;

use ecore_rs::{ctx::Ctx, prelude::idx};
use heck::{ToSnakeCase, ToUpperCamelCase};
use log::debug;
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
        debug!("Analyzing references...");
        let analysis = analyze_references(self.ctx, &self.pack_classes);

        if !analysis.has_references() {
            return Ok(Fragment::new(TokenStream::new(), vec![], vec![]));
        }

        debug!("Generating instance_from_path...");
        let instance_from_path = self.generate_instance_from_path(&analysis);
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
                let variant_name = format_ident!("{}Id", class.name());
                variant_name
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
            #instance_from_path
            #instance_path

            #edge_structs

            #typed_graph
        };

        let imports = vec![
            Import::Macros(Macros::TypedGraph),
            Import::Protocol(Protocol::ObjectPath),
            Import::Protocol(Protocol::PathSegment),
        ];

        Ok(Fragment::new(tokens, imports, vec![]))
    }
}

impl<'a> ReferenceGenerator<'a> {
    fn shortest_discriminating_suffixes(
        &self,
        full_paths: &[(idx::Class, Vec<PathPatternSegment>)],
    ) -> Vec<(idx::Class, Vec<PathPatternSegment>)> {
        let mut result = Vec::new();

        for (i, (vertex_class, full_path)) in full_paths.iter().enumerate() {
            let mut chosen = full_path.clone();

            for suffix_len in 1..=full_path.len() {
                let suffix = full_path[full_path.len() - suffix_len..].to_vec();
                let ambiguous =
                    full_paths
                        .iter()
                        .enumerate()
                        .any(|(j, (other_class, other_path))| {
                            i != j
                                && other_class != vertex_class
                                && other_path.len() >= suffix_len
                                && other_path[other_path.len() - suffix_len..] == suffix
                        });

                if !ambiguous {
                    chosen = suffix;
                    break;
                }
            }

            if !result.iter().any(|(existing_class, existing_suffix)| {
                existing_class == vertex_class && existing_suffix == &chosen
            }) {
                result.push((*vertex_class, chosen));
            }
        }

        result
    }

    fn generate_instance_from_path(&self, analysis: &ReferenceAnalysis) -> TokenStream {
        let path =
            syn::parse_str::<syn::Path>(&format!("{}{}", PRIVATE_MOD_PREFIX, REFERENCES_PATH_MOD))
                .unwrap();

        let mut full_paths = Vec::<(idx::Class, Vec<PathPatternSegment>)>::new();
        let mut arms = Vec::new();

        for &root_idx in &self.root_class_indices {
            let root_field = self.ctx.classes()[*root_idx].name().to_snake_case();

            for containment_path in
                find_creation_paths(self.ctx, root_idx, analysis, self.cycle_analysis)
            {
                let vertex_class = &self.ctx.classes()[*containment_path.vertex_class];
                let mut seg_patterns = vec![PathPatternSegment::Field(root_field.clone())];
                seg_patterns.extend(containment_path.steps.iter().map(PathPatternSegment::from));
                full_paths.push((vertex_class.idx, seg_patterns));
            }
        }

        for (vertex_class, seg_patterns) in self.shortest_discriminating_suffixes(&full_paths) {
            let vertex_class = &self.ctx.classes()[*vertex_class];
            let id_ty = format_ident!("{}Id", vertex_class.name());
            let variant = format_ident!("{}Id", vertex_class.name());
            let seg_patterns = seg_patterns.iter().map(|segment| segment.to_tokens(&path));

            arms.push(quote! {
                [.., #(#seg_patterns),*] => {
                    Some(Instance::#variant(#id_ty(path.clone())))
                }
            });
        }

        quote! {
            pub fn instance_from_path(path: &#path::ObjectPath) -> Option<Instance> {
                let segs = path.segments();

                match segs {
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
                let base_name = format!(
                    "{}{}Edge",
                    source_class.name().to_upper_camel_case(),
                    r.reference_name.to_upper_camel_case()
                );
                Ident::new(&base_name, Span::call_site())
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
                let base_name = format!(
                    "{}To{}",
                    source_class.name().to_upper_camel_case(),
                    target_class.name().to_upper_camel_case()
                );
                let suffix = counts.entry(base_name.clone()).or_insert(0);
                let unique_name = if *suffix == 0 {
                    base_name
                } else {
                    format!("{base_name}{}", *suffix + 1)
                };
                *suffix += 1;

                Ident::new(&unique_name, Span::call_site())
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
                format_ident!("{}Id", class.name())
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
                let source_id = format_ident!("{}Id", source_class.name());
                let target_id = format_ident!("{}Id", target_class.name());

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

#[derive(Clone, Debug, PartialEq, Eq)]
enum PathPatternSegment {
    Field(String),
    Variant(String),
    ListElement,
    MapEntry,
}

impl PathPatternSegment {
    fn to_tokens(&self, path: &syn::Path) -> TokenStream {
        match self {
            Self::Field(name) => quote! { #path::Field(#name) },
            Self::Variant(name) => quote! { #path::Variant(#name) },
            Self::ListElement => quote! { #path::ListElement(_) },
            Self::MapEntry => quote! { #path::MapEntry(_) },
        }
    }
}

impl From<&PathStep> for PathPatternSegment {
    fn from(step: &PathStep) -> Self {
        match step {
            PathStep::Field { variant_name, .. } => Self::Field(variant_name.to_lowercase()),
            PathStep::Variant { variant_name, .. } => Self::Variant(variant_name.to_lowercase()),
            PathStep::ListElement => Self::ListElement,
            PathStep::MapEntry => Self::MapEntry,
        }
    }
}
