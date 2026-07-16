use std::str::FromStr;

use ecore_rs::{
    ctx::Ctx,
    repr::{Structural, builtin::Typ},
};
use quote::quote;

use crate::{
    CLASSIFIERS_PATH_MOD,
    codegen::{
        annotation::{DatatypeOverride, datatype_override},
        datatype::{
            crdt::{Bag, Collection, Crdt, Named, NestedCrdt, Primitive, Set, SimpleCrdt},
            to_crdt::ToCrdt,
        },
        feature::{
            bounds::{BoundKind, normalize_bounds},
            typed_element::unsupported_feature_properties,
        },
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        ident::{classifier_type_ident, rust_ident, value_ident},
        import::{Import, Log},
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

        unsupported_feature_properties(self.attribute, &mut warnings);

        let name = value_ident(&self.attribute.name);
        let class_typ = self
            .ctx
            .classes()
            .get(*self.attribute.typ.unwrap())
            .unwrap();

        let (rust_typ, mut crdt) = if class_typ.is_enum() {
            let enum_name = classifier_type_ident(self.ctx, class_typ);
            (
                Some(quote! { #enum_name }),
                Primitive::Register(Default::default()),
            )
        } else {
            let typ: Typ = FromStr::from_str(class_typ.name())
                .unwrap_or_else(|_| panic!("Failed to parse type: {}", class_typ.name()));
            (ToCrdt::to_rust_type(&typ), ToCrdt::to_crdt_container(&typ))
        };

        if let Some(override_typ) = datatype_override(self.attribute) {
            match override_typ {
                DatatypeOverride::Primitive(primitive) => {
                    crdt = primitive;
                }
                DatatypeOverride::Set(_) => {}
            }
        }

        let (log_type, crdt_inner, log_import) = match &crdt {
            Primitive::Counter(_) => {
                let rust_typ = rust_typ.clone().expect("Counter should have a rust type");
                let type_name = rust_ident(crdt.name());
                (
                    quote! { #path::VecLog },
                    quote! { #path::#type_name<#rust_typ> },
                    Import::Log(Log::Vec),
                )
            }
            Primitive::Flag(_) => {
                let type_name = rust_ident(crdt.name());
                (
                    quote! { #path::VecLog },
                    quote! { #path::#type_name },
                    Import::Log(Log::Vec),
                )
            }
            Primitive::Register(_) => {
                let rust_typ = rust_typ.clone().expect("Register should have a rust type");
                let type_name = rust_ident(crdt.name());
                (
                    quote! { #path::VecLog },
                    quote! { #path::#type_name<#rust_typ> },
                    Import::Log(Log::Vec),
                )
            }
            Primitive::List => {
                // EString -> List<char>, uses GraphLog as log type
                let type_name = rust_ident(crdt.name());
                (
                    quote! { #path::GraphLog },
                    quote! { #path::#type_name<char> },
                    Import::Log(Log::Graph),
                )
            }
        };

        let (field_type, imports) = match (
            bound_kind,
            // Default to true if not specified, as per Ecore spec
            self.attribute.unique.unwrap_or(true),
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
                quote! { #path::NestedListLog<#log_type<#crdt_inner>> },
                vec![
                    log_import,
                    Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(crdt))),
                    Import::Crdt(Crdt::Nested(NestedCrdt::List)),
                ],
            ),
            (BoundKind::Many, true, true) => {
                let element_type = rust_typ
                    .clone()
                    .expect("Unique ordered attributes should have a Rust element type");
                (
                    quote! { #path::GraphLog<#path::List<#element_type>> },
                    vec![
                        Import::Log(Log::Graph),
                        Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(Primitive::List))),
                    ],
                )
            }
            (BoundKind::Many, false, false) => (
                quote! { #path::AWBagLog<#rust_typ> },
                vec![Import::Crdt(Crdt::Simple(SimpleCrdt::Collection(
                    Collection::Bag(Bag::AWBag),
                )))],
            ),
            (BoundKind::Many, true, false) => {
                let set_typ = match datatype_override(self.attribute) {
                    Some(DatatypeOverride::Set(set)) => set,
                    _ => Set::AWSet,
                };
                let set_name = rust_ident(set_typ.name());
                (
                    quote! { #path::VecLog<#path::#set_name<#rust_typ>> },
                    vec![
                        Import::Log(Log::Vec),
                        Import::Crdt(Crdt::Simple(SimpleCrdt::Collection(Collection::Set(
                            set_typ,
                        )))),
                    ],
                )
            }
        };

        let tokens = quote! { #name: #field_type };

        Ok(Fragment::new(tokens, imports, warnings))
    }
}
