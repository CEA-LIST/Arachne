use ecore_rs::{ctx::Ctx, repr::Class};
use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::Ident;

use crate::codegen::{
    cycles::CycleAnalysis,
    feature::{attribute::AttributeGenerator, containment::ContainmentGenerator},
    generate::{Fragment, Generate},
    generator::PATH_MOD_PRIVATE,
    import::{Import, Macros},
    operation::OperationGenerator,
    warnings::Warning,
};

pub const INHERITANCE_SUFFIX: &str = "Feat";

pub struct ClassGenerator<'a> {
    class: &'a Class,
    ctx: &'a Ctx,
    cycle_analysis: &'a CycleAnalysis,
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
    fn inherited_fields(&self) -> (Vec<Ident>, Vec<Ident>) {
        let sup = self
            .class
            .sup()
            .iter()
            .map(|idx| self.ctx.classes()[**idx].name())
            .collect::<Vec<_>>();

        let field_names_str = sup
            .iter()
            .map(|name| format!("{}{}", name, INHERITANCE_SUFFIX).to_snake_case())
            .collect::<Vec<_>>();

        let field_names = field_names_str
            .iter()
            .map(|name| Ident::new(name, Span::call_site()))
            .collect::<Vec<_>>();

        let field_types = sup
            .iter()
            .map(|name| format_ident!("{}{}Log", name, INHERITANCE_SUFFIX))
            .collect::<Vec<_>>();

        (field_names, field_types)
    }

    fn generate_abstract_class(&self) -> anyhow::Result<Fragment> {
        let path: syn::Path = syn::parse_str(PATH_MOD_PRIVATE).unwrap();
        let name = Ident::new(self.class.name(), Span::call_site());
        let feat_name = format_ident!("{}{}", self.class.name(), INHERITANCE_SUFFIX);

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

        // Collect subclass names for the union type
        let sub_names = self
            .class
            .sub()
            .iter()
            .map(|idx| Ident::new(self.ctx.classes()[**idx].name(), Span::call_site()))
            .collect::<Vec<_>>();
        let sub_names_log = sub_names
            .iter()
            .map(|name| format_ident!("{}Log", name))
            .collect::<Vec<_>>();

        let tokens = quote! {
            #path::union!(#name = #(#sub_names(#sub_names, #sub_names_log))|*);

            #path::record!(#feat_name {
                #(#inherited_field_names: #inherited_field_types,)*
                #(#attribute_tokens,)*
                #(#reference_tokens,)*
            });
        };

        Ok(Fragment::new(
            tokens,
            [
                vec![
                    Import::Macros(Macros::Record),
                    Import::Macros(Macros::Union),
                ],
                attribute_imports,
                reference_imports,
                operation_imports,
            ]
            .concat(),
            [attribute_warnings, reference_warnings, operation_warnings].concat(),
        ))
    }

    fn generate_concrete_class(&self) -> anyhow::Result<Fragment> {
        let path: syn::Path = syn::parse_str(PATH_MOD_PRIVATE).unwrap();
        let name = Ident::new(self.class.name(), Span::call_site());

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

        let tokens = quote! {
            #path::record!(#name {
                #(#inherited_field_names: #inherited_field_types,)*
                #(#attribute_tokens,)*
                #(#reference_tokens,)*
            });
        };

        Ok(Fragment::new(
            tokens,
            [
                vec![
                    Import::Macros(Macros::Record),
                    Import::Macros(Macros::Union),
                ],
                attribute_imports,
                reference_imports,
                operation_imports,
            ]
            .concat(),
            [attribute_warnings, reference_warnings, operation_warnings].concat(),
        ))
    }

    fn generate_interface(&self) -> anyhow::Result<Fragment> {
        let path: syn::Path = syn::parse_str(PATH_MOD_PRIVATE).unwrap();
        let feat_name = format_ident!("{}{}", self.class.name(), INHERITANCE_SUFFIX);

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
            #path::record!(#feat_name {
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
                #[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
                pub enum #name {
                    #[default]
                    #first,
                    #(#rest,)*
                }
            }
        } else {
            quote! {
                #[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
            return self.generate_enum();
        }

        if self.class.is_interface() {
            return self.generate_interface();
        }

        if self.class.is_abstract() {
            return self.generate_abstract_class();
        }

        if self.class.is_concrete() {
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
