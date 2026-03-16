use ecore_rs::{
    ctx::Ctx,
    repr::{Structural, builtin::Typ as BuiltinTyp, idx, structural},
};
use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::{
    CLASSIFIERS_PATH_MOD,
    codegen::{
        annotation::{DatatypeOverride, datatype_override, uw_map_spec},
        cycles::{BoxingStrategy, CycleAnalysis},
        datatype::{
            crdt::{Crdt, Map, Named, NestedCrdt, Primitive, SimpleCrdt},
            to_crdt::ToCrdt,
        },
        feature::bounds::{BoundKind, normalize_bounds},
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        import::{Import, Log},
        warnings::Warning,
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
        let (bound_kind, mut warnings) =
            normalize_bounds(self.reference.bounds, &self.reference.name);

        if let Some(changeable) = self.reference.changeable
            && !changeable
        {
            warnings.push(Warning::UnsupportedFeatureProperty {
                feature: self.reference.name.clone(),
                property: "changeable".into(),
                value: "false".into(),
            })
        }

        if let Some(transient) = self.reference.transient {
            warnings.push(Warning::UnsupportedFeatureProperty {
                feature: self.reference.name.clone(),
                property: "transient".into(),
                value: transient.to_string().into(),
            })
        }
        if let Some(volatile) = self.reference.volatile {
            warnings.push(Warning::UnsupportedFeatureProperty {
                feature: self.reference.name.clone(),
                property: "volatile".into(),
                value: volatile.to_string().into(),
            })
        }
        if let Some(derived) = self.reference.derived {
            warnings.push(Warning::UnsupportedFeatureProperty {
                feature: self.reference.name.clone(),
                property: "derived".into(),
                value: derived.to_string().into(),
            })
        }
        if let Some(derived) = self.reference.unsettable {
            warnings.push(Warning::UnsupportedFeatureProperty {
                feature: self.reference.name.clone(),
                property: "derived".into(),
                value: derived.to_string().into(),
            })
        }

        let target_class = self
            .ctx
            .classes()
            .get(*self.reference.typ.unwrap())
            .unwrap();

        let snake = self.reference.name.to_snake_case();
        let name = syn::parse_str::<Ident>(&snake)
            .unwrap_or_else(|_| Ident::new_raw(&snake, Span::call_site()));
        let target_type = format_ident!("{}Log", target_class.name().to_upper_camel_case());
        let boxing_strategy = self
            .cycle_analysis
            .boxing_strategy(self.source_class, &self.reference.name);
        let boxed_target_type = quote! { Box<#target_type> };

        if let Some(spec) = uw_map_spec(self.reference) {
            anyhow::ensure!(
                matches!(bound_kind, BoundKind::Many),
                "uw-map reference `{}` must be multi-valued",
                self.reference.name
            );

            let key_feature = target_class
                .structural()
                .iter()
                .find(|feature| feature.name == spec.key_feature)
                .ok_or_else(|| {
                    anyhow::anyhow!("UWMap key feature `{}` not found", spec.key_feature)
                })?;
            let value_feature = target_class
                .structural()
                .iter()
                .find(|feature| feature.name == spec.value_feature)
                .ok_or_else(|| {
                    anyhow::anyhow!("UWMap value feature `{}` not found", spec.value_feature)
                })?;

            anyhow::ensure!(
                key_feature.kind == structural::Typ::EAttribute,
                "UWMap key feature must be an attribute"
            );
            anyhow::ensure!(
                value_feature.kind != structural::Typ::EReference || value_feature.containment,
                "UWMap value feature cannot be a non-containment reference"
            );
            anyhow::ensure!(
                matches!(
                    normalize_bounds(value_feature.bounds, &value_feature.name).0,
                    BoundKind::Single
                ),
                "UWMap value feature must be single-valued"
            );

            let key_class = self.ctx.classes().get(*key_feature.typ.unwrap()).unwrap();
            let key_ty = if key_class.is_enum() {
                let enum_name = format_ident!("{}", key_class.name().to_upper_camel_case());
                quote! { #enum_name }
            } else {
                let typ: BuiltinTyp = key_class.name().parse().map_err(|_| {
                    anyhow::anyhow!("Unsupported UWMap key type `{}`", key_class.name())
                })?;
                typ.to_rust_type().ok_or_else(|| {
                    anyhow::anyhow!("UWMap key type `{}` has no Rust type", key_class.name())
                })?
            };

            let (value_log_ty, mut imports) =
                self.uw_map_value_log_type(target_class.idx, value_feature, &path)?;
            imports.push(Import::Crdt(Crdt::Nested(NestedCrdt::Map(Map::UWMap))));

            let stream = quote! { #name: #path::UWMapLog<#key_ty, #value_log_ty> };
            return Ok(Fragment::new(stream, imports, warnings));
        }

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
                    quote! { #path::NestedListLog<#target_type> }
                } else {
                    quote! { #path::NestedListLog<#boxed_target_type> }
                },
                vec![Import::Crdt(Crdt::Nested(NestedCrdt::List))],
            ),
        };

        let stream = quote! { #name: #field_type };

        Ok(Fragment::new(stream, imports, warnings))
    }
}

impl<'a> ContainmentGenerator<'a> {
    fn uw_map_value_log_type(
        &self,
        entry_class: idx::Class,
        value_feature: &Structural,
        path: &syn::Path,
    ) -> anyhow::Result<(TokenStream, Vec<Import>)> {
        match value_feature.kind {
            structural::Typ::EAttribute => {
                let value_class = self.ctx.classes().get(*value_feature.typ.unwrap()).unwrap();
                let (rust_ty, mut primitive) = if value_class.is_enum() {
                    let enum_name = format_ident!("{}", value_class.name());
                    (
                        Some(quote! { #enum_name }),
                        Primitive::Register(crate::codegen::datatype::crdt::Register::MultiValue),
                    )
                } else {
                    let typ: BuiltinTyp = value_class.name().parse().map_err(|_| {
                        anyhow::anyhow!("Failed to parse type: {}", value_class.name())
                    })?;
                    (typ.to_rust_type(), typ.to_crdt_container())
                };

                if let Some(DatatypeOverride::Primitive(override_primitive)) =
                    datatype_override(value_feature)
                {
                    primitive = override_primitive;
                }

                let (log_ty, imports) = match primitive {
                    Primitive::Counter(_) => {
                        let rust_ty = rust_ty
                            .ok_or_else(|| anyhow::anyhow!("Counter must have a Rust type"))?;
                        (
                            quote! { #path::VecLog<#path::Counter<#rust_ty>> },
                            vec![
                                Import::Log(Log::VecLog),
                                Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(primitive))),
                            ],
                        )
                    }
                    Primitive::Flag(flag) => {
                        let flag_name = format_ident!("{}", flag.name());
                        (
                            quote! { #path::VecLog<#path::#flag_name> },
                            vec![
                                Import::Log(Log::VecLog),
                                Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(Primitive::Flag(
                                    flag,
                                )))),
                            ],
                        )
                    }
                    Primitive::Register(register) => {
                        let rust_ty = rust_ty
                            .ok_or_else(|| anyhow::anyhow!("Register must have a Rust type"))?;
                        let register_name = format_ident!("{}", register.name());
                        (
                            quote! { #path::VecLog<#path::#register_name<#rust_ty>> },
                            vec![
                                Import::Log(Log::VecLog),
                                Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(
                                    Primitive::Register(register),
                                ))),
                            ],
                        )
                    }
                    Primitive::List => (
                        quote! { #path::EventGraph<#path::List<char>> },
                        vec![
                            Import::Log(Log::EventGraph),
                            Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(Primitive::List))),
                        ],
                    ),
                };
                Ok((log_ty, imports))
            }
            structural::Typ::EReference => {
                anyhow::ensure!(
                    value_feature.containment,
                    "UWMap value feature cannot be a non-containment reference"
                );
                let value_class = self.ctx.classes().get(*value_feature.typ.unwrap()).unwrap();
                let value_log = format_ident!("{}Log", value_class.name().to_upper_camel_case());
                let boxing_strategy = self
                    .cycle_analysis
                    .boxing_strategy(entry_class, &value_feature.name);
                let log_ty = if boxing_strategy == BoxingStrategy::NoBox {
                    quote! { #value_log }
                } else {
                    quote! { Box<#value_log> }
                };
                Ok((log_ty, Vec::new()))
            }
        }
    }
}
