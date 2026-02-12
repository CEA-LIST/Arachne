use ecore_rs::ctx::Ctx;
use ecore_rs::repr::Class;
use heck::ToSnakeCase;
use heck::ToUpperCamelCase;
use proc_macro2::Span;
use quote::format_ident;
use quote::quote;
use syn::Ident;

use crate::codegen::feature::attribute::AttributeGenerator;
use crate::codegen::generate::Fragment;
use crate::codegen::generate::Generate;
use crate::codegen::generator::PATH_MOD;
use crate::codegen::import::Import;
use crate::codegen::import::Macros;

const INHERITANCE_SUFFIX: &str = "Feat";

pub struct ClassGenerator<'a> {
    class: &'a Class,
    ctx: &'a Ctx,
}

impl<'a> ClassGenerator<'a> {
    pub fn new(class: &'a Class, ctx: &'a Ctx) -> Self {
        Self { class, ctx }
    }

    fn generate_abstract_class(&self) -> anyhow::Result<Fragment> {
        let path: syn::Path = syn::parse_str(PATH_MOD).unwrap();
        let name = Ident::new(self.class.name(), Span::call_site());
        let feat_name = format_ident!("{}{}", self.class.name(), INHERITANCE_SUFFIX);

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

        let attributes = self
            .class
            .structural()
            .iter()
            .filter_map(|f| match f.kind {
                ecore_rs::repr::structural::Typ::EAttribute => {
                    Some(AttributeGenerator::new(f, self.ctx).generate())
                }
                _ => None,
            })
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
            #path::union!(#name = #(#sub_names(#sub_names, #sub_names_log))|*);

            #path::record!(#feat_name {
                #(#attribute_tokens,)*
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
            ]
            .concat(),
            attribute_warnings,
        ))
    }

    fn generate_concrete_class(&self) -> anyhow::Result<Fragment> {
        let path: syn::Path = syn::parse_str(PATH_MOD).unwrap();
        let name = Ident::new(self.class.name(), Span::call_site());

        let (inherited_field_names, inherited_field_types) = {
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
        };

        let attributes = self
            .class
            .structural()
            .iter()
            .filter_map(|f| match f.kind {
                ecore_rs::repr::structural::Typ::EAttribute => {
                    Some(AttributeGenerator::new(f, self.ctx).generate())
                }
                _ => None,
            })
            .collect::<Result<Vec<_>, _>>()?;

        let (attribute_fields, attribute_imports, attribute_warnings) =
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
                #(#inherited_field_names: #inherited_field_types,)*
                #(#attribute_fields,)*
            });
        };

        Ok(Fragment::new(
            tokens,
            [vec![Import::Macros(Macros::Record)], attribute_imports].concat(),
            attribute_warnings,
        ))
    }

    fn generate_interface(&self) -> anyhow::Result<Fragment> {
        let path: syn::Path = syn::parse_str(PATH_MOD).unwrap();
        let feat_name = format_ident!("{}{}", self.class.name(), INHERITANCE_SUFFIX);

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
            [vec![Import::Macros(Macros::Record)], attribute_imports].concat(),
            attribute_warnings,
        ))
    }

    // TODO: derive Ord from the literal values, and PartialEq/Eq from that
    fn generate_enum(&self) -> anyhow::Result<Fragment> {
        let name = Ident::new(self.class.name(), Span::call_site());

        let variants = self
            .class
            .literals()
            .iter()
            .map(|lit| Ident::new(&lit.name().to_upper_camel_case(), Span::call_site()))
            .collect::<Vec<_>>();
        let tokens = quote! {
            pub enum #name {
                #(#variants,)*
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
