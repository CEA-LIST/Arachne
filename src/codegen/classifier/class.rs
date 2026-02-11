use ecore_rs::ctx::Ctx;
use ecore_rs::repr::Class;
use proc_macro2::Span;
use quote::format_ident;
use quote::quote;
use syn::Ident;

use crate::codegen::generate::Fragment;
use crate::codegen::generate::Generate;
use crate::codegen::generator::GEN_MOD;
use crate::codegen::import::{CrdtImport, Import, LogImport, MacrosImport};

const INHERITANCE_SUFFIX: &str = "Feat";

pub struct ClassGenerator<'a> {
    class: &'a Class,
    ctx: &'a Ctx,
}

impl<'a> ClassGenerator<'a> {
    pub fn new(class: &'a Class, ctx: &'a Ctx) -> Self {
        Self { class, ctx }
    }
}

impl Generate for ClassGenerator<'_> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        if self.class.is_interface() {
            // Emit a warning that interfaces are not supported
            let warning = crate::codegen::warnings::Warning::InterfaceNotSupported(
                self.class.name().to_string(),
            );
            warning.emit();
            // Return an empty fragment to skip code generation for this class
            return Ok(Fragment::new(quote! {}, Vec::new(), Vec::new()));
        }

        let mut imports = vec![
            Import::Log(LogImport::VecLog),
            Import::Crdt(CrdtImport::Counter),
        ];

        let name = Ident::new(self.class.name(), Span::call_site());
        let gen_mod: syn::Path = syn::parse_str(GEN_MOD).unwrap();

        if self.class.is_abstract() {
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
            let tokens = quote! {
                #gen_mod::union!(#name = #(#sub_names(#sub_names, #sub_names_log))|*);

                #gen_mod::record!(#feat_name {
                    placeholder: #gen_mod::VecLog<#gen_mod::Counter<i32>>,
                });
            };

            imports.push(Import::Macros(MacrosImport::Record));
            imports.push(Import::Macros(MacrosImport::Union));

            Ok(Fragment::new(tokens, imports, Vec::new()))
        } else {
            let tokens = quote! {
                #gen_mod::record!(#name {
                    placeholder: #gen_mod::VecLog<#gen_mod::Counter<i32>>,
                });
            };

            imports.push(Import::Macros(MacrosImport::Record));
            imports.push(Import::Macros(MacrosImport::Union));

            Ok(Fragment::new(tokens, imports, Vec::new()))
        }
    }
}
