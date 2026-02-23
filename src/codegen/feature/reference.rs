use ecore_rs::{ctx::Ctx, repr::Structural};
use heck::ToSnakeCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::codegen::{
    classifier::class::INHERITANCE_SUFFIX,
    datatype::crdt::{Crdt, NestedCrdt},
    feature::bounds::{BoundKind, normalize_bounds},
    generate::{Fragment, Generate},
    generator::PATH_MOD,
    import::Import,
};

pub struct ReferenceGenerator<'a> {
    reference: &'a Structural,
    ctx: &'a Ctx,
}

impl<'a> ReferenceGenerator<'a> {
    pub fn new(reference: &'a Structural, ctx: &'a Ctx) -> Self {
        assert_eq!(reference.kind, ecore_rs::repr::structural::Typ::EReference);
        Self { reference, ctx }
    }
}

impl<'a> Generate for ReferenceGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        if !self.reference.containment {
            // Non-containment references will be implemented later with a special mechanism
            return Ok(Fragment::new(TokenStream::new(), vec![], vec![]));
        }

        let path: syn::Path = syn::parse_str(PATH_MOD).unwrap();
        let (bound_kind, warnings) = normalize_bounds(self.reference.bounds, &self.reference.name);

        let target_class = self
            .ctx
            .classes()
            .get(*self.reference.typ.unwrap())
            .unwrap();

        let name = Ident::new(&self.reference.name.to_snake_case(), Span::call_site());
        let target_type = if target_class.is_abstract() {
            format_ident!("{}{}Log", target_class.name(), INHERITANCE_SUFFIX)
        } else {
            format_ident!("{}Log", target_class.name())
        };

        let (field_type, imports) = match bound_kind {
            BoundKind::Single => (quote! { #target_type }, vec![]),
            BoundKind::Optional => (
                quote! { #path::OptionLog<#target_type> },
                vec![Import::Crdt(Crdt::Nested(NestedCrdt::Optional))],
            ),
            BoundKind::Many => (
                quote! { #path::ListLog<#target_type> },
                vec![Import::Crdt(Crdt::Nested(NestedCrdt::List))],
            ),
        };

        let stream = quote! { #name: #field_type };

        Ok(Fragment::new(stream, imports, warnings))
    }
}
