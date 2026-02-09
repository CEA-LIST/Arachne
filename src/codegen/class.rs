use ecore_rs::repr::Class;
use proc_macro2::TokenStream;
use quote::quote;

use crate::error::Result;

/// Generates CRDT code for an EClass
pub struct ClassGenerator<'a> {
    class: &'a Class,
}

const LOG_PATH: &'static str =
    "moirai_protocol::state::po_log::VecLog::<moirai_crdt::flag::ew_flag::EWFlag>";

impl<'a> ClassGenerator<'a> {
    /// Creates a new class generator
    pub fn new(class: &'a Class) -> Self {
        Self { class }
    }

    /// Generates the CRDT type for this class
    pub fn generate(&self) -> Result<Option<TokenStream>> {
        let class_name = self.class.name();
        let name = proc_macro2::Ident::new(class_name, proc_macro2::Span::call_site());

        let ty: syn::Type = syn::parse_str(LOG_PATH)?;

        let code = quote! {
            moirai_macros::record!(#name {
                placeholder: #ty,
            });
        };

        Ok(Some(code))
    }
}
