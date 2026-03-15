use std::str::FromStr;

use ecore_rs::{
    ctx::Ctx,
    repr::{Structural, builtin::Typ},
};
use heck::ToSnakeCase;
use proc_macro2::Span;
use quote::quote;
use syn::Ident;

use crate::{
    CLASSIFIERS_PATH_MOD,
    codegen::{
        datatype::{
            crdt::{Bag, Collection, Crdt, Named, NestedCrdt, Primitive, Set, SimpleCrdt},
            to_crdt::ToCrdt,
        },
        feature::bounds::{BoundKind, normalize_bounds},
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        import::{Import, Log},
        warnings::Warning,
    },
};

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
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD)).unwrap();

        let (bound_kind, mut warnings) =
            normalize_bounds(self.attribute.bounds, &self.attribute.name);

        let snake = self.attribute.name.to_snake_case();
        let name = syn::parse_str::<Ident>(&snake)
            .unwrap_or_else(|_| Ident::new_raw(&snake, Span::call_site()));
        let class_typ = self
            .ctx
            .classes()
            .get(*self.attribute.typ.unwrap())
            .unwrap();

        let (rust_typ, crdt) = if class_typ.is_enum() {
            let enum_name = Ident::new(class_typ.name(), Span::call_site());
            (
                Some(quote! { #enum_name }),
                Primitive::Register(Default::default()),
            )
        } else {
            let typ: Typ = FromStr::from_str(class_typ.name())
                .unwrap_or_else(|_| panic!("Failed to parse type: {}", class_typ.name()));
            (ToCrdt::to_rust_type(&typ), ToCrdt::to_crdt_container(&typ))
        };

        let (log_type, crdt_inner, log_import) = match &crdt {
            Primitive::Counter(_) => {
                let rust_typ = rust_typ.clone().expect("Counter should have a rust type");
                let type_name = syn::Ident::new(crdt.name(), Span::call_site());
                (
                    quote! { #path::VecLog },
                    quote! { #path::#type_name<#rust_typ> },
                    Import::Log(Log::VecLog),
                )
            }
            Primitive::Flag(_) => {
                let type_name = syn::Ident::new(crdt.name(), Span::call_site());
                (
                    quote! { #path::VecLog },
                    quote! { #path::#type_name },
                    Import::Log(Log::VecLog),
                )
            }
            Primitive::Register(_) => {
                let rust_typ = rust_typ.clone().expect("Register should have a rust type");
                let type_name = syn::Ident::new(crdt.name(), Span::call_site());
                (
                    quote! { #path::VecLog },
                    quote! { #path::#type_name<#rust_typ> },
                    Import::Log(Log::VecLog),
                )
            }
            Primitive::List => {
                // EString -> List<char>, uses EventGraph as log type
                let type_name = syn::Ident::new(crdt.name(), Span::call_site());
                (
                    quote! { #path::EventGraph },
                    quote! { #path::#type_name<char> },
                    Import::Log(Log::EventGraph),
                )
            }
        };

        let (field_type, imports) = match (
            bound_kind,
            self.attribute.unique.unwrap_or(false),
            self.attribute.ordered.unwrap_or(true),
        ) {
            (BoundKind::Single, _, _) => (
                quote! { #log_type<#crdt_inner> },
                vec![
                    log_import,
                    Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(crdt))),
                ],
            ),
            (BoundKind::Optional, _, _) => (
                quote! { #path::OptionLog<#log_type<#crdt_inner>> },
                vec![
                    log_import,
                    Import::Crdt(Crdt::Nested(NestedCrdt::Optional)),
                    Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(crdt))),
                ],
            ),
            (BoundKind::Many, false, true) => (
                quote! { #path::ListLog<#log_type<#crdt_inner>> },
                vec![
                    Import::Log(Log::EventGraph),
                    Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(crdt))),
                    Import::Crdt(Crdt::Nested(NestedCrdt::List)),
                ],
            ),
            (BoundKind::Many, true, true) => {
                // TODO: Unique list case
                warnings.push(Warning::UnsupportedPropertyCombination {
                    feature: self.attribute.name.clone(),
                    properties: vec!["unique".into(), "ordered".into()],
                    applied: vec!["ordered".into()],
                });
                (
                    quote! { #path::ListLog<#log_type<#crdt_inner>> },
                    vec![
                        Import::Log(Log::EventGraph),
                        Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(crdt))),
                        Import::Crdt(Crdt::Nested(NestedCrdt::List)),
                    ],
                )
            }
            (BoundKind::Many, false, false) => (
                quote! { #path::AWBagLog<#rust_typ> },
                vec![Import::Crdt(Crdt::Simple(SimpleCrdt::Collection(
                    Collection::Bag(Bag::AWBag),
                )))],
            ),
            (BoundKind::Many, true, false) => (
                quote! { #path::VecLog<#path::AWSet<#rust_typ>> },
                vec![Import::Crdt(Crdt::Simple(SimpleCrdt::Collection(
                    Collection::Set(Set::AWSet),
                )))],
            ),
        };

        let tokens = quote! { #name: #field_type };

        Ok(Fragment::new(tokens, imports, warnings))
    }
}
