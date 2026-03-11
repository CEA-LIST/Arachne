use ecore_rs::ctx::Ctx;
use heck::ToUpperCamelCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::codegen::reference::analysis::ReferenceAnalysis;

/// Generate `#[derive(Debug, Clone, PartialEq, Eq, Hash)] pub struct {ClassName}Id(pub EventId);`
/// for each class that participates in a non-containment reference.
pub fn generate_id_structs(ctx: &Ctx, analysis: &ReferenceAnalysis) -> TokenStream {
    let structs: Vec<TokenStream> = analysis
        .referenceable_classes
        .iter()
        .map(|&class_idx| {
            let class = &ctx.classes()[*class_idx];
            let id_name = format_ident!("{}Id", class.name());
            quote! {
                #[derive(Debug, Clone, PartialEq, Eq, Hash)]
                pub struct #id_name(pub EventId);
            }
        })
        .collect();

    quote! { #(#structs)* }
}

/// Generate `#[derive(Debug, Clone, PartialEq, Eq, Hash)] pub struct {RefName}Edge;`
/// for each non-containment reference.
pub fn generate_edge_structs(analysis: &ReferenceAnalysis) -> TokenStream {
    let structs: Vec<TokenStream> = analysis
        .refs
        .iter()
        .map(|r| {
            let edge_name = format_ident!("{}Edge", r.reference_name.to_upper_camel_case());
            quote! {
                #[derive(Debug, Clone, PartialEq, Eq, Hash)]
                pub struct #edge_name;
            }
        })
        .collect();

    quote! { #(#structs)* }
}

/// Generate the `typed_graph!` macro invocation.
pub fn generate_typed_graph(ctx: &Ctx, analysis: &ReferenceAnalysis) -> TokenStream {
    // Vertices: one per referenceable class
    let vertices: Vec<Ident> = analysis
        .referenceable_classes
        .iter()
        .map(|&class_idx| {
            let class = &ctx.classes()[*class_idx];
            format_ident!("{}Id", class.name())
        })
        .collect();

    // Connections: one per non-containment reference
    let connections: Vec<TokenStream> = analysis
        .refs
        .iter()
        .map(|r| {
            let conn_name = Ident::new(&r.reference_name.to_upper_camel_case(), Span::call_site());
            let source_class = &ctx.classes()[*r.source_class];
            let target_class = &ctx.classes()[*r.target_class];
            let source_id = format_ident!("{}Id", source_class.name());
            let target_id = format_ident!("{}Id", target_class.name());
            let edge_name = format_ident!("{}Edge", r.reference_name.to_upper_camel_case());
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
        typed_graph! {
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

/// Generate the top-level `Model` enum.
pub fn generate_model_enum(root_class_name: &str) -> TokenStream {
    let root_ident = Ident::new(root_class_name, Span::call_site());

    quote! {
        #[derive(Debug, Clone)]
        pub enum Model {
            Root(#root_ident),
            Reference(Refs),
        }
    }
}
