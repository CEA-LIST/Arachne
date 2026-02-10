use ecore_rs::repr::Class;
use proc_macro2::TokenStream;
use quote::quote;

use crate::codegen::import::{CrdtImport, Import, LogImport, MacrosImport};
use crate::{codegen::GEN_MOD, error::Result};

pub struct ClassGenerator<'a> {
    class: &'a Class,
}

impl<'a> ClassGenerator<'a> {
    pub fn new(class: &'a Class) -> Self {
        Self { class }
    }

    pub fn generate(
        &self,
        generator: &mut crate::codegen::Generator,
    ) -> Result<Option<TokenStream>> {
        let name = proc_macro2::Ident::new(self.class.name(), proc_macro2::Span::call_site());
        let gen_mod: syn::Path = syn::parse_str(GEN_MOD).unwrap();

        // Register imports on first use
        generator.register_import(Import::Crdt(CrdtImport::Counter));
        generator.register_import(Import::Log(LogImport::VecLog));
        generator.register_import(Import::Macros(MacrosImport::Record));

        let code = quote! {
            #gen_mod::record!(#name {
                placeholder: #gen_mod::VecLog<#gen_mod::Counter<i32>>,
            });
        };

        Ok(Some(code))
    }
}
