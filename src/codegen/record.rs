use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use crate::error::Result;

/// Builder for generating record! macro invocations
pub struct RecordBuilder {
    name: Ident,
    fields: Vec<(Ident, TokenStream)>,
}

impl RecordBuilder {
    /// Creates a new record builder
    pub fn new(name: impl AsRef<str>) -> Self {
        let name = Ident::new(name.as_ref(), Span::call_site());
        Self {
            name,
            fields: Vec::new(),
        }
    }

    pub fn add_field(&mut self, name: impl AsRef<str>, crdt_type: TokenStream) -> &mut Self {
        let field_name = Ident::new(name.as_ref(), Span::call_site());
        self.fields.push((field_name, crdt_type));
        self
    }

    /// Builds the record! macro invocation
    pub fn build(self) -> Result<TokenStream> {
        let name = self.name;
        let fields = self.fields.into_iter().map(|(field_name, field_type)| {
            quote! {
                #field_name: #field_type
            }
        });

        Ok(quote! {
            record!(#name {
                #(#fields),*
            });
        })
    }
}
