//! Generates the query type and its package-level evaluation implementation.

use proc_macro2::TokenStream;
use quote::quote;

use super::ReadAsEcoreGenerator;
use crate::codegen::ident::value_ident;

impl ReadAsEcoreGenerator<'_> {
    pub(super) fn generate_helpers(&self) -> TokenStream {
        let path = self.path();

        quote! {
            /// Serializes the current replicated model as XMI conforming to the
            /// source Ecore package.
            #[derive(Debug, Clone, Copy, Default)]
            pub struct ReadAsEcore;

            impl #path::QueryOperation for ReadAsEcore {
                type Response = Vec<u8>;
            }

            impl ReadAsEcore {
                pub fn new() -> Self {
                    Self
                }
            }

            fn xmi_read<L, V>(log: &L) -> V
            where
                L: #path::IsLog
                    + #path::EvalNested<#path::Read<V>>,
            {
                log.execute_query(&#path::Read::<V>::new())
            }
        }
    }

    pub(super) fn generate_query_impl(&self) -> TokenStream {
        let path = self.path();
        let package_log_name =
            crate::codegen::ident::type_ident_with_suffix(self.package_name(), "Log");
        let package_name = self.package_name();
        let ns_prefix = self.ns_prefix();
        let ns_uri = self.ns_uri();
        let refs_read = if self.has_references() {
            quote! {
                let refs = build_xmi_references(&xmi_read(self.reference_manager_log()));
            }
        } else {
            quote! {}
        };
        let refs_call_arg = self.refs_call_arg();

        let roots = self.root_class_indices.iter().map(|root_idx| {
            let root_class = self.class(*root_idx);
            let root_field = value_ident(root_class.name());
            let root_log_field =
                crate::codegen::ident::value_ident_with_suffix(root_class.name(), "log");
            let root_path_field = root_field.to_string();
            let root_path = quote! {
                #path::ObjectPath::new(#package_name).field(#root_path_field)
            };

            if self.union_is_generated(root_class) {
                let visitor = self.union_visitor_ident(root_class);
                quote! {
                    #visitor(
                        &mut writer,
                        None,
                        #root_path,
                        self.#root_log_field(),
                        #refs_call_arg
                    );
                }
            } else {
                let visitor = self.class_visitor_ident(root_class);
                let element_name = self.root_element(root_class);
                quote! {
                    #visitor(
                        &mut writer,
                        #element_name,
                        None,
                        #root_path,
                        self.#root_log_field(),
                        #refs_call_arg
                    );
                }
            }
        });

        quote! {
            impl #path::EvalNested<ReadAsEcore> for #path::#package_log_name {
                fn execute_query(
                    &self,
                    _q: &ReadAsEcore,
                ) -> <ReadAsEcore as #path::QueryOperation>::Response {
                    #refs_read

                    let mut writer = #path::XmiWriter::new(#ns_prefix, #ns_uri);
                    #(#roots)*
                    writer.finish()
                }
            }
        }
    }
}
