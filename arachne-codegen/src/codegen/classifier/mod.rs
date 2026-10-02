use ecore_rs::{
    ctx::Ctx,
    repr::{Class, Structural, structural},
};
use log::debug;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::{
    CLASSIFIERS_PATH_MOD,
    codegen::{
        annotation::{transparent_field, uw_map_spec},
        cycles::CycleAnalysis,
        feature::{
            attribute::AttributeGenerator,
            containment::ContainmentGenerator,
            plan::{FeaturePlan, FeatureTypes},
        },
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        ident::{
            classifier_type_ident, classifier_type_ident_with_suffix, rust_ident, type_ident,
            value_ident_with_suffix,
        },
        import::{Import, Macros, Protocol},
        operation::OperationGenerator,
        warnings::Warning,
    },
};

pub const POLYMORPHIC_KIND_SUFFIX: &str = "Kind";
pub const INHERITED_FIELD_SUFFIX: &str = "super";

pub fn has_subclasses(class: &Class) -> bool {
    !class.sub().is_empty()
}

pub fn is_instantiable_class(class: &Class) -> bool {
    class.is_concrete() && !class.is_interface() && !class.is_enum()
}

pub fn has_instantiable_descendant(ctx: &Ctx, class: &Class) -> bool {
    class.sub().iter().any(|idx| {
        let subclass = &ctx.classes()[**idx];
        is_instantiable_class(subclass) || has_instantiable_descendant(ctx, subclass)
    })
}

pub fn is_uninhabited_polymorphic_class(ctx: &Ctx, class: &Class) -> bool {
    (class.is_abstract() || class.is_interface()) && !has_instantiable_descendant(ctx, class)
}

pub fn has_codegen_subclasses(ctx: &Ctx, class: &Class) -> bool {
    class.sub().iter().any(|idx| {
        let subclass = &ctx.classes()[**idx];
        !is_uninhabited_polymorphic_class(ctx, subclass)
    })
}

pub fn has_codegen_polymorphic_family(ctx: &Ctx, class: &Class) -> bool {
    if class.is_abstract() || class.is_interface() {
        !is_uninhabited_polymorphic_class(ctx, class)
    } else {
        has_codegen_subclasses(ctx, class)
    }
}

pub fn classifier_ident(ctx: &Ctx, class: &Class) -> Ident {
    classifier_type_ident(ctx, class)
}

pub fn classifier_log_ident(ctx: &Ctx, class: &Class) -> Ident {
    classifier_type_ident_with_suffix(ctx, class, "Log")
}

pub fn classifier_value_ident(ctx: &Ctx, class: &Class) -> Ident {
    classifier_type_ident_with_suffix(ctx, class, "Value")
}

pub fn polymorphic_kind_ident(ctx: &Ctx, class: &Class) -> Ident {
    if has_codegen_polymorphic_family(ctx, class) {
        classifier_type_ident_with_suffix(ctx, class, POLYMORPHIC_KIND_SUFFIX)
    } else {
        classifier_ident(ctx, class)
    }
}

pub fn polymorphic_kind_log_ident(ctx: &Ctx, class: &Class) -> Ident {
    if has_codegen_polymorphic_family(ctx, class) {
        classifier_type_ident_with_suffix(ctx, class, &format!("{POLYMORPHIC_KIND_SUFFIX}Log"))
    } else {
        classifier_log_ident(ctx, class)
    }
}

pub fn polymorphic_kind_value_ident(ctx: &Ctx, class: &Class) -> Ident {
    if has_codegen_polymorphic_family(ctx, class) {
        classifier_type_ident_with_suffix(ctx, class, &format!("{POLYMORPHIC_KIND_SUFFIX}Value"))
    } else {
        classifier_value_ident(ctx, class)
    }
}

pub fn containment_target_ident(ctx: &Ctx, class: &Class) -> Ident {
    polymorphic_kind_ident(ctx, class)
}

pub fn containment_target_log_ident(ctx: &Ctx, class: &Class) -> Ident {
    polymorphic_kind_log_ident(ctx, class)
}

pub fn containment_target_value_ident(ctx: &Ctx, class: &Class) -> Ident {
    polymorphic_kind_value_ident(ctx, class)
}

pub fn inherited_field_ident(class: &Class) -> Ident {
    value_ident_with_suffix(class.name(), INHERITED_FIELD_SUFFIX)
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
    value_ty: TokenStream,
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
                        let target_is_uninhabited = if let Some(target_idx) = f.typ {
                            let target = &self.ctx.classes()[*target_idx];
                            is_uninhabited_polymorphic_class(self.ctx, target)
                        } else {
                            false
                        };

                        if !target_is_uninhabited {
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
    fn inherited_fields(&self) -> (Vec<Ident>, Vec<TokenStream>, Vec<TokenStream>, Vec<Import>) {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD)).unwrap();
        let inherited = self
            .class
            .sup()
            .iter()
            .map(|idx| &self.ctx.classes()[**idx])
            .collect::<Vec<_>>();

        let mut field_names = Vec::new();
        let mut field_types = Vec::new();
        let mut value_types = Vec::new();
        let mut imports = Vec::new();
        for class in inherited {
            let field_ident = inherited_field_ident(class);
            let log_ident = classifier_log_ident(self.ctx, class);
            let value_ident = classifier_value_ident(self.ctx, class);
            let is_boxed = self
                .cycle_analysis
                .boxing_strategy(self.class.idx, &field_ident.to_string())
                == crate::codegen::cycles::BoxingStrategy::DirectReference;

            field_names.push(field_ident);
            if is_boxed {
                imports.push(Import::Protocol(Protocol::BoxedLog));
                field_types.push(quote! { #path::BoxedLog<#log_ident> });
                value_types.push(quote! { Box<#value_ident> });
            } else {
                field_types.push(quote! { #log_ident });
                value_types.push(quote! { #value_ident });
            }
        }

        (field_names, field_types, value_types, imports)
    }

    fn is_uw_map_entry_helper(&self) -> bool {
        if !is_instantiable_class(self.class) {
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
        is_instantiable_class(class)
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

        let variant_name = classifier_ident(self.ctx, subclass);
        let plan = FeaturePlan::resolve(self.ctx, subclass.idx, field, self.cycle_analysis)?;
        let path = syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD))?;
        let FeatureTypes {
            payload: payload_ty,
            log: log_ty,
            value: value_ty,
            imports,
        } = plan.types(self.ctx, &path, None)?;
        let warnings = plan.warnings;

        Ok(Some(TransparentVariantSpec {
            variant_name,
            payload_ty,
            log_ty,
            value_ty,
            imports,
            warnings,
        }))
    }

    fn generate_abstract_class(&self) -> anyhow::Result<Fragment> {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD)).unwrap();
        let kind_name = polymorphic_kind_ident(self.ctx, self.class);
        let name = classifier_ident(self.ctx, self.class);

        // Check if the class has a subclass
        if is_uninhabited_polymorphic_class(self.ctx, self.class) {
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
        let (
            inherited_field_names,
            inherited_field_types,
            inherited_value_types,
            inherited_imports,
        ) = self.inherited_fields();
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
            if is_uninhabited_polymorphic_class(self.ctx, subclass) {
                continue;
            }

            if let Some(TransparentVariantSpec {
                variant_name,
                payload_ty,
                log_ty,
                value_ty,
                imports,
                warnings,
            }) = self.transparent_variant_spec(subclass)?
            {
                let payload_alias = rust_ident(format!("{}{}", name, variant_name));
                let log_alias = rust_ident(format!("{}{}Log", name, variant_name));
                union_aliases.push(quote! {
                    type #payload_alias = #payload_ty;
                    type #log_alias = #log_ty;
                });
                union_variants
                    .push(quote! { #variant_name(#payload_alias, #log_alias => #value_ty) });
                union_imports.extend(imports);
                union_warnings.extend(warnings);
            } else {
                let variant_name = classifier_ident(self.ctx, subclass);
                let payload_name = containment_target_ident(self.ctx, subclass);
                let log_name = containment_target_log_ident(self.ctx, subclass);
                let value_name = containment_target_value_ident(self.ctx, subclass);
                union_variants
                    .push(quote! { #variant_name(#payload_name, #log_name => #value_name) });
            }
        }

        let record_tokens = if should_emit_feat {
            quote! {
                #path::record!(#name {
                    #(#inherited_field_names: #inherited_field_types => #inherited_value_types,)*
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
        let macro_imports = if should_emit_feat {
            vec![
                Import::Macros(Macros::Record),
                Import::Macros(Macros::Union),
            ]
        } else {
            vec![Import::Macros(Macros::Union)]
        };

        Ok(Fragment::new(
            tokens,
            [
                macro_imports,
                union_imports,
                inherited_imports,
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
        let name = classifier_ident(self.ctx, self.class);

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
        let (
            inherited_field_names,
            inherited_field_types,
            inherited_value_types,
            inherited_imports,
        ) = self.inherited_fields();
        let family_name = polymorphic_kind_ident(self.ctx, self.class);
        let family_log = classifier_log_ident(self.ctx, self.class);
        let (family_tokens, family_imports, family_warnings) =
            if has_codegen_subclasses(self.ctx, self.class) {
                let self_value = classifier_value_ident(self.ctx, self.class);
                let self_variant = quote! { #name(#name, #family_log => #self_value) };
                let mut union_aliases = Vec::new();
                let mut union_variants = vec![self_variant];
                let mut union_imports = Vec::new();
                let mut union_warnings = Vec::new();

                for idx in self.class.sub() {
                    let subclass = &self.ctx.classes()[**idx];
                    if is_uninhabited_polymorphic_class(self.ctx, subclass) {
                        continue;
                    }

                    if let Some(TransparentVariantSpec {
                        variant_name,
                        payload_ty,
                        log_ty,
                        value_ty,
                        imports,
                        warnings,
                    }) = self.transparent_variant_spec(subclass)?
                    {
                        let payload_alias =
                            rust_ident(format!("{}{}Value", family_name, variant_name));
                        let log_alias = rust_ident(format!("{}{}Log", family_name, variant_name));
                        union_aliases.push(quote! {
                            type #payload_alias = #payload_ty;
                            type #log_alias = #log_ty;
                        });
                        union_variants.push(
                            quote! { #variant_name(#payload_alias, #log_alias => #value_ty) },
                        );
                        union_imports.extend(imports);
                        union_warnings.extend(warnings);
                    } else {
                        let variant_name = classifier_ident(self.ctx, subclass);
                        let payload_name = containment_target_ident(self.ctx, subclass);
                        let log_name = containment_target_log_ident(self.ctx, subclass);
                        let value_name = containment_target_value_ident(self.ctx, subclass);
                        union_variants.push(
                            quote! { #variant_name(#payload_name, #log_name => #value_name) },
                        );
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
                #(#inherited_field_names: #inherited_field_types => #inherited_value_types,)*
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
                    // TODO: Only include the union macro if there are subclasses
                    Import::Macros(Macros::Union),
                ],
                family_imports,
                inherited_imports,
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

    // TODO: derive Ord from the literal values, and PartialEq/Eq from that
    fn generate_enum(&self) -> anyhow::Result<Fragment> {
        let name = classifier_ident(self.ctx, self.class);

        let variants = self
            .class
            .literals()
            .iter()
            .map(|lit| type_ident(lit.name()))
            .collect::<Vec<_>>();
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

        if self.class.is_abstract() || self.class.is_interface() {
            debug!("Generating abstract/interface class: {}", self.class.name());
            return self.generate_abstract_class();
        }

        if is_instantiable_class(self.class) {
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
