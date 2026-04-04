use ecore_rs::{
    ctx::Ctx,
    repr::{Class, Structural, builtin::Typ as BuiltinTyp, structural},
};
use heck::{ToSnakeCase, ToUpperCamelCase};
use log::debug;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::{
    CLASSIFIERS_PATH_MOD,
    codegen::{
        annotation::{DatatypeOverride, datatype_override, transparent_field, uw_map_spec},
        cycles::CycleAnalysis,
        datatype::{
            crdt::{Crdt, Map as CrdtMap, Named, NestedCrdt, Primitive, Register, SimpleCrdt},
            to_crdt::ToCrdt,
        },
        feature::{attribute::AttributeGenerator, containment::ContainmentGenerator},
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        import::{Import, Log, Macros},
        operation::OperationGenerator,
        warnings::Warning,
    },
};

pub const POLYMORPHIC_KIND_SUFFIX: &str = "Kind";
pub const INHERITED_FIELD_SUFFIX: &str = "super";

pub fn has_subclasses(class: &Class) -> bool {
    !class.sub().is_empty()
}

pub fn polymorphic_kind_ident(class: &Class) -> Ident {
    if class.is_abstract() || class.is_interface() || has_subclasses(class) {
        format_ident!(
            "{}{}",
            class.name().to_upper_camel_case(),
            POLYMORPHIC_KIND_SUFFIX
        )
    } else {
        format_ident!("{}", class.name().to_upper_camel_case())
    }
}

pub fn polymorphic_kind_log_ident(class: &Class) -> Ident {
    let kind_name = polymorphic_kind_ident(class);
    format_ident!("{}Log", kind_name)
}

pub fn containment_target_ident(class: &Class) -> Ident {
    polymorphic_kind_ident(class)
}

pub fn containment_target_log_ident(class: &Class) -> Ident {
    polymorphic_kind_log_ident(class)
}

pub fn inherited_field_ident(class: &Class) -> Ident {
    Ident::new(
        &format!(
            "{}_{}",
            class.name().to_snake_case(),
            INHERITED_FIELD_SUFFIX
        ),
        Span::call_site(),
    )
}

pub struct ClassGenerator<'a> {
    class: &'a Class,
    ctx: &'a Ctx,
    cycle_analysis: &'a CycleAnalysis,
}

struct TransparentVariantSpec {
    variant_name: Ident,
    payload_ty: TokenStream,
    log_ty: TokenStream,
    imports: Vec<Import>,
    warnings: Vec<Warning>,
}

impl<'a> ClassGenerator<'a> {
    pub fn new(class: &'a Class, ctx: &'a Ctx, cycle_analysis: &'a CycleAnalysis) -> Self {
        Self {
            class,
            ctx,
            cycle_analysis,
        }
    }

    /// Process all structural features and split them into attributes and references
    fn process_structural_features(&self) -> anyhow::Result<(Vec<Fragment>, Vec<Fragment>)> {
        self.class.structural().iter().try_fold(
            (Vec::new(), Vec::new()),
            |(mut attrs, mut refs), f| {
                match f.kind {
                    ecore_rs::repr::structural::Typ::EAttribute => {
                        attrs.push(AttributeGenerator::new(f, self.ctx).generate()?);
                    }
                    ecore_rs::repr::structural::Typ::EReference if f.containment => {
                        refs.push(
                            ContainmentGenerator::new(
                                f,
                                self.class.idx,
                                self.ctx,
                                self.cycle_analysis,
                            )
                            .generate()?,
                        );
                    }
                    _ => {
                        // Non-containment references are handled through the Typed Graph, so we can skip them here.
                    }
                }
                Ok::<(Vec<Fragment>, Vec<Fragment>), anyhow::Error>((attrs, refs))
            },
        )
    }

    /// Compute inherited field names and types from superclasses
    fn inherited_fields(&self) -> (Vec<Ident>, Vec<TokenStream>) {
        let inherited = self
            .class
            .sup()
            .iter()
            .map(|idx| &self.ctx.classes()[**idx])
            .collect::<Vec<_>>();

        let field_names = inherited
            .iter()
            .map(|class| inherited_field_ident(class))
            .collect::<Vec<_>>();

        let field_types = inherited
            .iter()
            .map(|class| {
                let field_ident = inherited_field_ident(class);
                let log_ident = format_ident!("{}Log", class.name().to_upper_camel_case());
                let base_type = quote! { #log_ident };
                if self
                    .cycle_analysis
                    .boxing_strategy(self.class.idx, &field_ident.to_string())
                    == crate::codegen::cycles::BoxingStrategy::DirectReference
                {
                    quote! { Box<#base_type> }
                } else {
                    base_type
                }
            })
            .collect::<Vec<_>>();

        (field_names, field_types)
    }

    fn is_uw_map_entry_helper(&self) -> bool {
        if !self.class.is_concrete() {
            return false;
        }

        let incoming_features: Vec<&Structural> = self
            .ctx
            .classes()
            .iter()
            .flat_map(|class| class.structural().iter())
            .filter(|feature| feature.typ == Some(self.class.idx))
            .collect();

        !incoming_features.is_empty()
            && incoming_features.iter().all(|feature| {
                feature.kind == structural::Typ::EReference
                    && feature.containment
                    && uw_map_spec(feature).is_some()
            })
    }

    fn generates_concrete_wrapper(&self, class: &Class) -> bool {
        class.is_concrete()
            && transparent_field(class).is_none()
            && !ClassGenerator::new(class, self.ctx, self.cycle_analysis).is_uw_map_entry_helper()
    }

    fn has_wrapper_descendant(&self, class: &Class) -> bool {
        class.sub().iter().any(|idx| {
            let sub = &self.ctx.classes()[**idx];
            self.generates_concrete_wrapper(sub) || self.has_wrapper_descendant(sub)
        })
    }

    fn transparent_variant_spec(
        &self,
        subclass: &Class,
    ) -> anyhow::Result<Option<TransparentVariantSpec>> {
        let Some(field_name) = transparent_field(subclass) else {
            return Ok(None);
        };

        let field = subclass
            .structural()
            .iter()
            .find(|feature| feature.name == field_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Transparent class `{}` refers to unknown field `{}`",
                    subclass.name(),
                    field_name
                )
            })?;

        let variant_name = Ident::new(&subclass.name().to_upper_camel_case(), Span::call_site());
        let (payload_ty, log_ty, imports, warnings) =
            self.transparent_field_types(subclass, field)?;

        Ok(Some(TransparentVariantSpec {
            variant_name,
            payload_ty,
            log_ty,
            imports,
            warnings,
        }))
    }

    fn transparent_field_types(
        &self,
        subclass: &Class,
        field: &Structural,
    ) -> anyhow::Result<(TokenStream, TokenStream, Vec<Import>, Vec<Warning>)> {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD)).unwrap();
        let (bound_kind, warnings) =
            crate::codegen::feature::bounds::normalize_bounds(field.bounds, &field.name);

        match field.kind {
            structural::Typ::EAttribute => {
                let class_typ = self.ctx.classes().get(*field.typ.unwrap()).unwrap();
                let (rust_typ, mut primitive) = if class_typ.is_enum() {
                    let enum_name =
                        Ident::new(&class_typ.name().to_upper_camel_case(), Span::call_site());
                    (
                        Some(quote! { #enum_name }),
                        Primitive::Register(Register::MultiValue),
                    )
                } else {
                    let typ: BuiltinTyp = class_typ.name().parse().map_err(|_| {
                        anyhow::anyhow!("Failed to parse type: {}", class_typ.name())
                    })?;
                    (typ.to_rust_type(), typ.to_crdt_container())
                };

                if let Some(override_typ) = datatype_override(field)
                    && let DatatypeOverride::Primitive(p) = override_typ
                {
                    primitive = p;
                }

                let (payload_ty, log_ty, imports) = match primitive.clone() {
                    Primitive::Counter(_) => {
                        let rust_typ = rust_typ.clone().expect("Counter should have a rust type");
                        (
                            quote! { #path::Counter<#rust_typ> },
                            quote! { #path::VecLog<#path::Counter<#rust_typ>> },
                            vec![
                                Import::Log(Log::Vec),
                                Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(primitive))),
                            ],
                        )
                    }
                    Primitive::Flag(flag) => {
                        let flag_name = format_ident!("{}", flag.name());
                        (
                            quote! { #path::#flag_name },
                            quote! { #path::VecLog<#path::#flag_name> },
                            vec![
                                Import::Log(Log::Vec),
                                Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(Primitive::Flag(
                                    flag,
                                )))),
                            ],
                        )
                    }
                    Primitive::Register(register) => {
                        let rust_typ = rust_typ.clone().expect("Register should have a rust type");
                        let reg_name = format_ident!("{}", register.name());
                        (
                            quote! { #path::#reg_name<#rust_typ> },
                            quote! { #path::VecLog<#path::#reg_name<#rust_typ>> },
                            vec![
                                Import::Log(Log::Vec),
                                Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(
                                    Primitive::Register(register),
                                ))),
                            ],
                        )
                    }
                    Primitive::List => (
                        quote! { #path::List<char> },
                        quote! { #path::EventGraph<#path::List<char>> },
                        vec![
                            Import::Log(Log::EventGraph),
                            Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(Primitive::List))),
                        ],
                    ),
                };

                let (payload_ty, log_ty, mut extra_imports) = match bound_kind {
                    crate::codegen::feature::bounds::BoundKind::Single => {
                        (payload_ty, log_ty, Vec::new())
                    }
                    crate::codegen::feature::bounds::BoundKind::Optional => (
                        quote! { Option<<#log_ty as #path::IsLog>::Op> },
                        quote! { #path::OptionLog<#log_ty> },
                        vec![Import::Crdt(Crdt::Nested(NestedCrdt::Optional))],
                    ),
                    crate::codegen::feature::bounds::BoundKind::Many => (
                        quote! { #path::List<<#log_ty as #path::IsLog>::Op> },
                        quote! { #path::NestedListLog<#log_ty> },
                        vec![Import::Crdt(Crdt::Nested(NestedCrdt::List))],
                    ),
                };
                let mut imports = imports;
                imports.append(&mut extra_imports);
                Ok((payload_ty, log_ty, imports, warnings))
            }
            structural::Typ::EReference => {
                anyhow::ensure!(
                    field.containment,
                    "Transparent field must be a containment reference"
                );
                let target_class = self.ctx.classes().get(*field.typ.unwrap()).unwrap();
                let target_name = containment_target_ident(target_class);
                let target_log = containment_target_log_ident(target_class);
                let boxing_strategy = self
                    .cycle_analysis
                    .boxing_strategy(subclass.idx, &field.name);

                if let Some(spec) = uw_map_spec(field) {
                    anyhow::ensure!(
                        matches!(bound_kind, crate::codegen::feature::bounds::BoundKind::Many),
                        "Transparent uw-map field `{}` must be multi-valued",
                        field.name
                    );
                    let entry_class = target_class;
                    let key_feature = entry_class
                        .structural()
                        .iter()
                        .find(|f| f.name == spec.key_feature)
                        .ok_or_else(|| {
                            anyhow::anyhow!("UWMap key feature `{}` not found", spec.key_feature)
                        })?;
                    let value_feature = entry_class
                        .structural()
                        .iter()
                        .find(|f| f.name == spec.value_feature)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "UWMap value feature `{}` not found",
                                spec.value_feature
                            )
                        })?;
                    anyhow::ensure!(
                        key_feature.kind == structural::Typ::EAttribute,
                        "UWMap key feature must be an attribute"
                    );
                    anyhow::ensure!(
                        value_feature.kind != structural::Typ::EReference
                            || value_feature.containment,
                        "UWMap value feature cannot be a non-containment reference"
                    );

                    let key_class = self.ctx.classes().get(*key_feature.typ.unwrap()).unwrap();
                    let key_ty = if key_class.is_enum() {
                        let enum_name =
                            Ident::new(&key_class.name().to_upper_camel_case(), Span::call_site());
                        quote! { #enum_name }
                    } else {
                        let typ: BuiltinTyp = key_class.name().parse().map_err(|_| {
                            anyhow::anyhow!("Unsupported UWMap key type `{}`", key_class.name())
                        })?;
                        typ.to_rust_type().ok_or_else(|| {
                            anyhow::anyhow!(
                                "UWMap key type `{}` has no Rust type",
                                key_class.name()
                            )
                        })?
                    };

                    let (value_payload, value_log, mut imports, mut field_warnings) =
                        self.transparent_field_types(subclass, value_feature)?;
                    anyhow::ensure!(
                        matches!(value_feature.bounds.ubound, Some(1)),
                        "UWMap value feature must be single-valued"
                    );
                    let payload = quote! { #path::UWMap<#key_ty, Box<#value_payload>> };
                    let log = quote! { #path::UWMapLog<#key_ty, #value_log> };
                    imports.push(Import::Crdt(Crdt::Nested(NestedCrdt::Map(CrdtMap::UWMap))));
                    imports.push(Import::Custom("moirai_crdt::map::uw_map::UWMap"));
                    let mut all_warnings = warnings;
                    all_warnings.append(&mut field_warnings);
                    return Ok((payload, log, imports, all_warnings));
                }

                let (payload_ty, log_ty, imports) = match bound_kind {
                    crate::codegen::feature::bounds::BoundKind::Single => {
                        if boxing_strategy == crate::codegen::cycles::BoxingStrategy::NoBox {
                            (quote! { #target_name }, quote! { #target_log }, vec![])
                        } else {
                            (
                                quote! { Box<#target_name> },
                                quote! { Box<#target_log> },
                                vec![],
                            )
                        }
                    }
                    crate::codegen::feature::bounds::BoundKind::Optional => {
                        let inner_payload =
                            if boxing_strategy == crate::codegen::cycles::BoxingStrategy::NoBox {
                                quote! { #target_name }
                            } else {
                                quote! { Box<#target_name> }
                            };
                        let inner_log =
                            if boxing_strategy == crate::codegen::cycles::BoxingStrategy::NoBox {
                                quote! { #target_log }
                            } else {
                                quote! { Box<#target_log> }
                            };
                        (
                            quote! { Option<#inner_payload> },
                            quote! { #path::OptionLog<#inner_log> },
                            vec![Import::Crdt(Crdt::Nested(NestedCrdt::Optional))],
                        )
                    }
                    crate::codegen::feature::bounds::BoundKind::Many => {
                        let inner_payload =
                            if boxing_strategy == crate::codegen::cycles::BoxingStrategy::NoBox {
                                quote! { #target_name }
                            } else {
                                quote! { Box<#target_name> }
                            };
                        let inner_log =
                            if boxing_strategy == crate::codegen::cycles::BoxingStrategy::NoBox {
                                quote! { #target_log }
                            } else {
                                quote! { Box<#target_log> }
                            };
                        (
                            quote! { #path::NestedList<#inner_payload> },
                            quote! { #path::NestedListLog<#inner_log> },
                            vec![
                                Import::Crdt(Crdt::Nested(NestedCrdt::List)),
                                Import::Custom("moirai_crdt::list::nested_list::NestedList"),
                            ],
                        )
                    }
                };
                Ok((payload_ty, log_ty, imports, warnings))
            }
        }
    }

    fn generate_abstract_class(&self) -> anyhow::Result<Fragment> {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD)).unwrap();
        let kind_name = polymorphic_kind_ident(self.class);
        let name = format_ident!("{}", self.class.name().to_upper_camel_case());

        // Check if the class has a subclass
        let is_inherited = !self.class.sub().is_empty();

        // If no subclass, raise a warning and skip generation
        if !is_inherited {
            let warning = Warning::AbstractWithNoSubclass(self.class.name().to_string());
            return Ok(Fragment::new(quote! {}, vec![], vec![warning]));
        }

        let (_operation_tokens, operation_imports, operation_warnings) = fold_fragments(
            self.class
                .operations()
                .iter()
                .map(|op| OperationGenerator::new(op, self.class, self.ctx).generate())
                .collect::<anyhow::Result<Vec<_>>>()?,
        );
        let (attributes, references) = self.process_structural_features()?;
        let (attribute_tokens, attribute_imports, attribute_warnings) = fold_fragments(attributes);
        let (reference_tokens, reference_imports, reference_warnings) = fold_fragments(references);
        let (inherited_field_names, inherited_field_types) = self.inherited_fields();
        let should_emit_feat = !inherited_field_names.is_empty()
            || !attribute_tokens.is_empty()
            || !reference_tokens.is_empty()
            || self.has_wrapper_descendant(self.class);

        // Collect subclass names for the union type
        let mut union_aliases = Vec::new();
        let mut union_variants = Vec::new();
        let mut union_imports = Vec::new();
        let mut union_warnings = Vec::new();
        for idx in self.class.sub() {
            let subclass = &self.ctx.classes()[**idx];
            if let Some(TransparentVariantSpec {
                variant_name,
                payload_ty,
                log_ty,
                imports,
                warnings,
            }) = self.transparent_variant_spec(subclass)?
            {
                let payload_alias = format_ident!("{}{}", name, variant_name);
                let log_alias = format_ident!("{}{}Log", name, variant_name);
                union_aliases.push(quote! {
                    type #payload_alias = #payload_ty;
                    type #log_alias = #log_ty;
                });
                union_variants.push(quote! { #variant_name(#payload_alias, #log_alias) });
                union_imports.extend(imports);
                union_warnings.extend(warnings);
            } else {
                let variant_name =
                    Ident::new(&subclass.name().to_upper_camel_case(), Span::call_site());
                let payload_name = containment_target_ident(subclass);
                let log_name = containment_target_log_ident(subclass);
                union_variants.push(quote! { #variant_name(#payload_name, #log_name) });
            }
        }

        let record_tokens = if should_emit_feat {
            quote! {
                #path::record!(#name {
                    #(#inherited_field_names: #inherited_field_types,)*
                    #(#attribute_tokens,)*
                    #(#reference_tokens,)*
                });
            }
        } else {
            quote! {}
        };

        let tokens = quote! {
            #(#union_aliases)*
            #path::union!(#kind_name = #(#union_variants)|*);
            #record_tokens
        };

        Ok(Fragment::new(
            tokens,
            [
                vec![
                    Import::Macros(Macros::Record),
                    Import::Macros(Macros::Union),
                ],
                union_imports,
                attribute_imports,
                reference_imports,
                operation_imports,
            ]
            .concat(),
            [
                union_warnings,
                attribute_warnings,
                reference_warnings,
                operation_warnings,
            ]
            .concat(),
        ))
    }

    fn generate_concrete_class(&self) -> anyhow::Result<Fragment> {
        if transparent_field(self.class).is_some() || self.is_uw_map_entry_helper() {
            return Ok(Fragment::new(TokenStream::new(), vec![], vec![]));
        }

        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD)).unwrap();
        let name = Ident::new(&self.class.name().to_upper_camel_case(), Span::call_site());

        let (_operation_tokens, operation_imports, operation_warnings) = fold_fragments(
            self.class
                .operations()
                .iter()
                .map(|op| OperationGenerator::new(op, self.class, self.ctx).generate())
                .collect::<anyhow::Result<Vec<_>>>()?,
        );
        let (attributes, references) = self.process_structural_features()?;
        let (attribute_tokens, attribute_imports, attribute_warnings) = fold_fragments(attributes);
        let (reference_tokens, reference_imports, reference_warnings) = fold_fragments(references);
        let (inherited_field_names, inherited_field_types) = self.inherited_fields();
        let family_name = polymorphic_kind_ident(self.class);
        let family_log = format_ident!("{}Log", name);
        let (family_tokens, family_imports, family_warnings) = if has_subclasses(self.class) {
            let self_variant = quote! { #name(#name, #family_log) };
            let mut union_aliases = Vec::new();
            let mut union_variants = vec![self_variant];
            let mut union_imports = Vec::new();
            let mut union_warnings = Vec::new();

            for idx in self.class.sub() {
                let subclass = &self.ctx.classes()[**idx];
                if let Some(TransparentVariantSpec {
                    variant_name,
                    payload_ty,
                    log_ty,
                    imports,
                    warnings,
                }) = self.transparent_variant_spec(subclass)?
                {
                    let payload_alias = format_ident!("{}{}Value", family_name, variant_name);
                    let log_alias = format_ident!("{}{}Log", family_name, variant_name);
                    union_aliases.push(quote! {
                        type #payload_alias = #payload_ty;
                        type #log_alias = #log_ty;
                    });
                    union_variants.push(quote! { #variant_name(#payload_alias, #log_alias) });
                    union_imports.extend(imports);
                    union_warnings.extend(warnings);
                } else {
                    let variant_name =
                        Ident::new(&subclass.name().to_upper_camel_case(), Span::call_site());
                    let payload_name = containment_target_ident(subclass);
                    let log_name = containment_target_log_ident(subclass);
                    union_variants.push(quote! { #variant_name(#payload_name, #log_name) });
                }
            }

            let tokens = quote! {
                #(#union_aliases)*
                #path::union!(#family_name = #(#union_variants)|*);
            };
            (tokens, union_imports, union_warnings)
        } else {
            (quote! {}, Vec::new(), Vec::new())
        };

        let tokens = quote! {
            #path::record!(#name {
                #(#inherited_field_names: #inherited_field_types,)*
                #(#attribute_tokens,)*
                #(#reference_tokens,)*
            });
            #family_tokens
        };

        Ok(Fragment::new(
            tokens,
            [
                vec![
                    Import::Macros(Macros::Record),
                    Import::Macros(Macros::Union),
                ],
                family_imports,
                attribute_imports,
                reference_imports,
                operation_imports,
            ]
            .concat(),
            [
                family_warnings,
                attribute_warnings,
                reference_warnings,
                operation_warnings,
            ]
            .concat(),
        ))
    }

    fn generate_interface(&self) -> anyhow::Result<Fragment> {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD)).unwrap();
        let name = format_ident!("{}", self.class.name().to_upper_camel_case());

        let (_operation_tokens, operation_imports, operation_warnings) = fold_fragments(
            self.class
                .operations()
                .iter()
                .map(|op| OperationGenerator::new(op, self.class, self.ctx).generate())
                .collect::<anyhow::Result<Vec<_>>>()?,
        );
        let attributes = self
            .class
            .structural()
            .iter()
            .map(|f| AttributeGenerator::new(f, self.ctx).generate())
            .collect::<Result<Vec<_>, _>>()?;

        let (attribute_tokens, attribute_imports, attribute_warnings) =
            attributes.into_iter().fold(
                (Vec::new(), Vec::new(), Vec::new()),
                |(mut toks, mut imps, mut warns), attr| {
                    let (tokens, imports, warnings) = attr.into();
                    toks.push(tokens);
                    imps.extend(imports);
                    warns.extend(warnings);
                    (toks, imps, warns)
                },
            );

        let tokens = quote! {
            #path::record!(#name {
                #(#attribute_tokens,)*
            });
        };

        Ok(Fragment::new(
            tokens,
            [
                vec![Import::Macros(Macros::Record)],
                attribute_imports,
                operation_imports,
            ]
            .concat(),
            [attribute_warnings, operation_warnings].concat(),
        ))
    }

    // TODO: derive Ord from the literal values, and PartialEq/Eq from that
    fn generate_enum(&self) -> anyhow::Result<Fragment> {
        let name = Ident::new(self.class.name(), Span::call_site());

        let variants = self
            .class
            .literals()
            .iter()
            .map(|lit| {
                let camel = lit.name().to_upper_camel_case();
                syn::parse_str::<syn::Ident>(&camel).map_err(|e| {
                    anyhow::anyhow!(
                        "Enum '{}': cannot parse variant '{}' (converted to '{}') as an identifier at {e}",
                        self.class.name(),
                        lit.name(),
                        camel,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let tokens = if let Some((first, rest)) = variants.split_first() {
            quote! {
                #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
                pub enum #name {
                    #[default]
                    #first,
                    #(#rest,)*
                }
            }
        } else {
            quote! {
                #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
                pub enum #name {
                    #(#variants,)*
                }
            }
        };
        Ok(Fragment::new(tokens, vec![], Vec::new()))
    }
}

impl Generate for ClassGenerator<'_> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        if self.class.is_enum() {
            debug!("Generating enum: {}", self.class.name());
            return self.generate_enum();
        }

        if self.class.is_interface() {
            debug!("Generating interface: {}", self.class.name());
            return self.generate_interface();
        }

        if self.class.is_abstract() {
            debug!("Generating abstract class: {}", self.class.name());
            return self.generate_abstract_class();
        }

        if self.class.is_concrete() {
            debug!("Generating concrete class: {}", self.class.name());
            return self.generate_concrete_class();
        }

        Result::Err(anyhow::anyhow!(
            "Class {} is not supported (not abstract, enum, concrete, or interface)",
            self.class.name()
        ))
    }
}

/// Helper function to fold a vector of fragments into separate collections
/// of tokens, imports, and warnings.
fn fold_fragments(
    fragments: Vec<Fragment>,
) -> (Vec<proc_macro2::TokenStream>, Vec<Import>, Vec<Warning>) {
    fragments.into_iter().fold(
        (Vec::new(), Vec::new(), Vec::new()),
        |(mut toks, mut imps, mut warns), fragment| {
            let (tokens, imports, warnings) = fragment.into();
            if !tokens.is_empty() {
                toks.push(tokens);
            }
            imps.extend(imports);
            warns.extend(warnings);
            (toks, imps, warns)
        },
    )
}
