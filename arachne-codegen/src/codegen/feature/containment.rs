use ecore_rs::{
    ctx::Ctx,
    repr::{Structural, idx},
};
use heck::ToSnakeCase;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::Ident;

use crate::{
    CLASSIFIERS_PATH_MOD,
    codegen::{
        cycles::{BoxingStrategy, CycleAnalysis},
        datatype::crdt::{Crdt, NestedCrdt},
        feature::bounds::{BoundKind, normalize_bounds},
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        import::Import,
    },
};

pub struct ContainmentGenerator<'a> {
    reference: &'a Structural,
    source_class: idx::Class,
    ctx: &'a Ctx,
    cycle_analysis: &'a CycleAnalysis,
}

impl<'a> ContainmentGenerator<'a> {
    pub fn new(
        reference: &'a Structural,
        source_class: idx::Class,
        ctx: &'a Ctx,
        cycle_analysis: &'a CycleAnalysis,
    ) -> Self {
        assert_eq!(reference.kind, ecore_rs::repr::structural::Typ::EReference);
        Self {
            reference,
            source_class,
            ctx,
            cycle_analysis,
        }
    }
}

impl<'a> Generate for ContainmentGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD)).unwrap();
        let (bound_kind, warnings) = normalize_bounds(self.reference.bounds, &self.reference.name);

        let target_class = self
            .ctx
            .classes()
            .get(*self.reference.typ.unwrap())
            .unwrap();

        let snake = self.reference.name.to_snake_case();
        let name = syn::parse_str::<Ident>(&snake)
            .unwrap_or_else(|_| Ident::new_raw(&snake, Span::call_site()));
        let target_type = format_ident!("{}Log", target_class.name());
        let boxing_strategy = self
            .cycle_analysis
            .boxing_strategy(self.source_class, &self.reference.name);
        let boxed_target_type = quote! { Box<#target_type> };

        let (field_type, imports) = match bound_kind {
            BoundKind::Single => {
                if boxing_strategy == BoxingStrategy::NoBox {
                    (quote! { #target_type }, vec![])
                } else {
                    (boxed_target_type.clone(), vec![])
                }
            }
            BoundKind::Optional => (
                if boxing_strategy == BoxingStrategy::NoBox {
                    quote! { #path::OptionLog<#target_type> }
                } else {
                    quote! { #path::OptionLog<#boxed_target_type> }
                },
                vec![Import::Crdt(Crdt::Nested(NestedCrdt::Optional))],
            ),
            BoundKind::Many => (
                if boxing_strategy == BoxingStrategy::NoBox {
                    quote! { #path::ListLog<#target_type> }
                } else {
                    quote! { #path::ListLog<#boxed_target_type> }
                },
                vec![Import::Crdt(Crdt::Nested(NestedCrdt::List))],
            ),
        };

        let stream = quote! { #name: #field_type };

        Ok(Fragment::new(stream, imports, warnings))
    }
}
