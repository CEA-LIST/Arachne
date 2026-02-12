use std::str::FromStr;

use ecore_rs::{
    ctx::Ctx,
    repr::{Structural, builtin::Typ},
};
use heck::ToSnakeCase;
use proc_macro2::Span;
use quote::quote;
use syn::Ident;

use crate::codegen::{
    datatype::{
        crdt::{Crdt, SimpleCrdt},
        to_crdt::ToCrdt,
    },
    generate::{Fragment, Generate},
    generator::PATH_MOD,
    import::{Import, Log},
    warnings::Warning,
};

#[derive(Clone, Copy, Debug)]
enum BoundKind {
    Optional,
    Single,
    Many,
}

fn normalize_bounds(
    bounds: ecore_rs::repr::bounds::Bounds,
    feature: &str,
) -> (BoundKind, Vec<Warning>) {
    let applied = match (bounds.lbound, bounds.ubound) {
        (0, Some(1)) => (BoundKind::Optional, None),
        (1, Some(1)) => (BoundKind::Single, None),
        (0, None) => (BoundKind::Many, None),
        (0, Some(0)) => (BoundKind::Optional, Some("0..1")),
        (0, Some(_)) => (BoundKind::Many, Some("0..*")),
        (1, None) => (BoundKind::Many, Some("0..*")),
        (lbound, Some(ubound)) if lbound > 1 || ubound > 1 => (BoundKind::Many, Some("0..*")),
        (lbound, Some(ubound)) => {
            let _ = (lbound, ubound);
            (BoundKind::Many, Some("0..*"))
        }
        (_, None) => (BoundKind::Many, Some("0..*")),
    };

    let warnings = applied.1.map_or(Vec::new(), |applied| {
        vec![Warning::UnsupportedBounds {
            feature: feature.to_string(),
            bounds: bounds.to_string(),
            applied: applied.to_string(),
        }]
    });

    (applied.0, warnings)
}
pub struct AttributeGenerator<'a> {
    attribute: &'a Structural,
    ctx: &'a Ctx,
}

impl<'a> AttributeGenerator<'a> {
    pub fn new(attribute: &'a Structural, ctx: &'a Ctx) -> Self {
        assert_eq!(attribute.kind, ecore_rs::repr::structural::Typ::EAttribute);
        Self { attribute, ctx }
    }
}

impl<'a> Generate for AttributeGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let path: syn::Path = syn::parse_str(PATH_MOD).unwrap();

        let (bound_kind, warnings) = normalize_bounds(self.attribute.bounds, &self.attribute.name);

        let name = Ident::new(&self.attribute.name.to_snake_case(), Span::call_site());
        let class_typ = self
            .ctx
            .classes()
            .get(*self.attribute.typ.unwrap())
            .unwrap();
        let typ: Typ = FromStr::from_str(class_typ.name()).unwrap();

        let crdt_type = ToCrdt::to_rust_type(&typ);
        let crdt_container = ToCrdt::to_crdt_container(&typ);

        let (log_type, crdt_inner, log_import) = match &crdt_container {
            Crdt::Simple(SimpleCrdt::Counter(_)) => {
                let rust_type = crdt_type.expect("Counter should have a rust type");
                let type_name = syn::Ident::new(crdt_container.type_name(), Span::call_site());
                (
                    quote! { #path::VecLog },
                    quote! { #path::#type_name<#rust_type> },
                    Import::Log(Log::VecLog),
                )
            }
            Crdt::Simple(SimpleCrdt::Flag(_)) => {
                let type_name = syn::Ident::new(crdt_container.type_name(), Span::call_site());
                (
                    quote! { #path::VecLog },
                    quote! { #path::#type_name },
                    Import::Log(Log::VecLog),
                )
            }
            Crdt::Simple(SimpleCrdt::Register(_)) => {
                let rust_type = crdt_type.expect("Register should have a rust type");
                let type_name = syn::Ident::new(crdt_container.type_name(), Span::call_site());
                (
                    quote! { #path::VecLog },
                    quote! { #path::#type_name<#rust_type> },
                    Import::Log(Log::VecLog),
                )
            }
            Crdt::Simple(SimpleCrdt::List) => {
                // EString -> List<char>, uses EventGraph as log type
                let type_name = syn::Ident::new(crdt_container.type_name(), Span::call_site());
                (
                    quote! { #path::EventGraph },
                    quote! { #path::#type_name<char> },
                    Import::Log(Log::EventGraph),
                )
            }
            Crdt::Simple(SimpleCrdt::Set(_)) => {
                let rust_type = crdt_type.expect("Set should have a rust type");
                let type_name = syn::Ident::new(crdt_container.type_name(), Span::call_site());
                (
                    quote! { #path::VecLog },
                    quote! { #path::#type_name<#rust_type> },
                    Import::Log(Log::VecLog),
                )
            }
            Crdt::Simple(SimpleCrdt::Graph(_)) => {
                let type_name = syn::Ident::new(crdt_container.type_name(), Span::call_site());
                (
                    quote! { #path::VecLog },
                    quote! { #path::#type_name },
                    Import::Log(Log::VecLog),
                )
            }
            Crdt::Nested(_) => unimplemented!("Nested CRDTs not yet supported"),
        };

        let (field_type, bound_log_import, bound_imports) = match bound_kind {
            BoundKind::Single => (
                quote! { #log_type<#crdt_inner> },
                log_import,
                vec![Import::Crdt(crdt_container)],
            ),
            BoundKind::Optional => (
                quote! { Option<#log_type<#crdt_inner>> },
                log_import,
                vec![Import::Crdt(crdt_container)],
            ),
            BoundKind::Many => (
                quote! { #path::EventGraph<#path::List<#crdt_inner>> },
                Import::Log(Log::EventGraph),
                vec![
                    Import::Crdt(crdt_container),
                    Import::Crdt(Crdt::Simple(SimpleCrdt::List)),
                ],
            ),
        };

        let tokens = quote! { #name: #field_type };

        let mut imports = vec![bound_log_import];
        imports.extend(bound_imports);

        Ok(Fragment::new(tokens, imports, warnings))
    }
}
