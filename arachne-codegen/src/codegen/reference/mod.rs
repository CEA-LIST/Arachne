pub mod analysis;
pub mod containment;

use ecore_rs::{ctx::Ctx, prelude::idx};
use heck::ToUpperCamelCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::{
    REFERENCES_PATH_MOD,
    codegen::{
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
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
    pub fn new(ctx: &'a Ctx, pack_classes: Vec<idx::Class>) -> Self {
        Self { ctx, pack_classes }
    }
}

impl<'a> Generate for ReferenceGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let analysis = analyze_references(self.ctx, &self.pack_classes);

        if !analysis.has_references() {
            return Ok(Fragment::new(TokenStream::new(), vec![], vec![]));
        }

        let object_id = self.generate_object_id();
        let id_structs = self.generate_id_structs(&analysis);
        let edge_structs = self.generate_edge_structs(&analysis);
        let typed_graph = self.generate_typed_graph(&analysis);

        let tokens = quote! {
            #object_id
            #id_structs
            #edge_structs
            #typed_graph
        };

        let imports = vec![
            Import::Macros(Macros::TypedGraph),
            Import::Protocol(Protocol::EventId),
        ];

        Ok(Fragment::new(tokens, imports, vec![]))
    }
}

impl<'a> ReferenceGenerator<'a> {
    fn generate_object_id(&self) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, REFERENCES_PATH_MOD)).unwrap();
        quote! {
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub struct ObjectId {
                pub root: RootId,
                pub path: std::vec::Vec<PathSegment>,
            }

            impl #path::Display for ObjectId {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", self.root)?;
                    for segment in &self.path {
                        match segment {
                            PathSegment::Field(name) => write!(f, "/{}", name)?,
                            PathSegment::ListElement(id) => write!(f, "/{}", id)?,
                            PathSegment::MapEntry(key) => write!(f, "/{}", key)?,
                            PathSegment::Variant(name) => write!(f, "/{}", name)?,
                        }
                    }
                    Ok(())
                }
            }

            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub enum RootId {
                Package(&'static str),
            }

            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub enum PathSegment {
                Field(&'static str),
                ListElement(#path::EventId),
                MapEntry(std::string::String),
                Variant(&'static str),
            }

            impl ObjectId {
                pub fn root(name: &'static str) -> Self {
                    Self {
                        root: RootId::Package(name),
                        path: std::vec::Vec::new(),
                    }
                }

                pub fn field(mut self, name: &'static str) -> Self {
                    self.path.push(PathSegment::Field(name));
                    self
                }

                pub fn list_element(mut self, id: #path::EventId) -> Self {
                    self.path.push(PathSegment::ListElement(id));
                    self
                }

                pub fn map_entry(mut self, key: impl Into<std::string::String>) -> Self {
                    self.path.push(PathSegment::MapEntry(key.into()));
                    self
                }

                pub fn variant(mut self, name: &'static str) -> Self {
                    self.path.push(PathSegment::Variant(name));
                    self
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

    /// Generate `#[derive(Debug, Clone, PartialEq, Eq, Hash)] pub struct {ClassName}Id(pub ObjectId);`
    /// for each class that participates in a non-containment reference.
    pub fn generate_id_structs(&self, analysis: &ReferenceAnalysis) -> TokenStream {
        let structs: Vec<TokenStream> = analysis
            .referenceable_classes
            .iter()
            .map(|&class_idx| {
                let class = &self.ctx.classes()[*class_idx];
                let id_name = format_ident!("{}Id", class.name());
                quote! {
                    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
                    pub struct #id_name(pub ObjectId);
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
