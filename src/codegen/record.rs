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

    /// Adds a field to the record
    ///
    /// # Arguments
    /// * `name` - Field name
    /// * `crdt_type` - The CRDT type for this field
    pub fn add_field(&mut self, name: impl AsRef<str>, crdt_type: TokenStream) -> &mut Self {
        let field_name = Ident::new(name.as_ref(), Span::call_site());
        self.fields.push((field_name, crdt_type));
        self
    }

    /// Adds a Counter field
    pub fn add_counter_field(&mut self, name: impl AsRef<str>) -> &mut Self {
        let crdt_type = quote! {
            POLog::<Counter<i32>, Vec<TaggedOp<Counter<i32>>>>
        };
        self.add_field(name, crdt_type)
    }

    /// Adds an LWWRegister field
    pub fn add_lww_register_field(&mut self, name: impl AsRef<str>, value_type: &str) -> &mut Self {
        let value_ident = Ident::new(value_type, Span::call_site());
        let crdt_type = quote! {
            POLog::<LWWRegister<#value_ident>, Vec<TaggedOp<LWWRegister<#value_ident>>>>
        };
        self.add_field(name, crdt_type)
    }

    /// Adds an ORSet field
    pub fn add_orset_field(&mut self, name: impl AsRef<str>, element_type: &str) -> &mut Self {
        let elem_ident = Ident::new(element_type, Span::call_site());
        let crdt_type = quote! {
            POLog::<ORSet<#elem_ident>, Vec<TaggedOp<ORSet<#elem_ident>>>>
        };
        self.add_field(name, crdt_type)
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
