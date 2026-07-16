use ecore_rs::{
    ctx::Ctx,
    repr::{Class, idx},
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::{
    CLASSIFIERS_PATH_MOD, PACKAGE_PATH_MOD,
    codegen::{
        classifier::{
            classifier_ident, has_codegen_polymorphic_family, is_instantiable_class,
            is_uninhabited_polymorphic_class,
        },
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        ident::{
            classifier_type_ident_with_suffix, type_ident_with_suffix, value_ident_with_suffix,
        },
        import::{Import, Protocol},
    },
};

pub struct ReadAsEcoreGenerator<'a> {
    ctx: &'a Ctx,
    pack_idx: idx::Pack,
    root_class_indices: Vec<idx::Class>,
}

impl<'a> ReadAsEcoreGenerator<'a> {
    pub fn new(ctx: &'a Ctx, pack_idx: idx::Pack, root_class_indices: Vec<idx::Class>) -> Self {
        Self {
            ctx,
            pack_idx,
            root_class_indices,
        }
    }

    fn package(&self) -> &ecore_rs::repr::Pack {
        self.ctx
            .packs()
            .get(self.pack_idx)
            .expect("package index should resolve")
    }

    fn package_log_ident(&self) -> Ident {
        type_ident_with_suffix(self.package().name(), "Log")
    }

    fn class(&self, class_idx: idx::Class) -> &Class {
        &self.ctx.classes()[*class_idx]
    }

    fn namespace_prefix(&self) -> String {
        self.package()
            .ns_prefix()
            .filter(|prefix| !prefix.is_empty())
            .unwrap_or_else(|| self.package().name())
            .to_string()
    }

    fn namespace_uri(&self) -> String {
        self.package()
            .ns_uri()
            .filter(|uri| !uri.is_empty())
            .unwrap_or_else(|| self.package().name())
            .to_string()
    }

    fn package_module_path(&self) -> syn::Path {
        syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD))
            .expect("generated package module path should be valid")
    }

    fn classifiers_module_path(&self) -> syn::Path {
        syn::parse_str(&format!("crate::{CLASSIFIERS_PATH_MOD}"))
            .expect("generated classifiers module path should be valid")
    }

    fn qualified_element_name(&self, class: &Class) -> String {
        format!("{}:{}", self.namespace_prefix(), class.name())
    }

    fn generate_add_root_element(&self, class: &Class, document_root: &TokenStream) -> TokenStream {
        let element_name = self.qualified_element_name(class);

        quote! {
            #document_root
                .add_child(xml_builder::XMLElement::new(#element_name))
                .expect("adding a root object to the XMI document should not fail");
        }
    }

    fn union_variants<'b>(&'b self, class: &'b Class) -> Vec<&'b Class> {
        let mut variants = Vec::new();

        if is_instantiable_class(class) {
            variants.push(class);
        }

        variants.extend(
            class
                .sub()
                .iter()
                .map(|idx| self.class(*idx))
                .filter(|subclass| !is_uninhabited_polymorphic_class(self.ctx, subclass)),
        );

        variants
    }

    fn generate_add_root_for_class(
        &self,
        class: &Class,
        log: TokenStream,
        document_root: &TokenStream,
    ) -> TokenStream {
        if !has_codegen_polymorphic_family(self.ctx, class) {
            return self.generate_add_root_element(class, document_root);
        }

        let classifiers = self.classifiers_module_path();
        let container_ty = classifier_type_ident_with_suffix(self.ctx, class, "KindContainer");
        let child_ty = classifier_type_ident_with_suffix(self.ctx, class, "KindChild");
        let child_arms = self
            .union_variants(class)
            .into_iter()
            .map(|variant_class| {
                let variant = classifier_ident(self.ctx, variant_class);
                let is_leaf = variant_class.idx == class.idx
                    || !has_codegen_polymorphic_family(self.ctx, variant_class);
                let (child_log, body) = if is_leaf {
                    (
                        quote! { _ },
                        self.generate_add_root_element(variant_class, document_root),
                    )
                } else {
                    (
                        quote! { __child_log },
                        self.generate_add_root_for_class(
                            variant_class,
                            quote! { __child_log },
                            document_root,
                        ),
                    )
                };

                quote! {
                    #classifiers::#child_ty::#variant(#child_log) => {
                        #body
                    }
                }
            })
            .collect::<Vec<_>>();
        let conflict_child_arms = child_arms.clone();

        quote! {
            match &#log.child {
                #classifiers::#container_ty::Unset => {}
                #classifiers::#container_ty::Value(__child) => {
                    match __child.as_ref() {
                        #(#child_arms,)*
                    }
                }
                #classifiers::#container_ty::Conflicts(__children) => {
                    for __child in __children {
                        match __child {
                            #(#conflict_child_arms,)*
                        }
                    }
                }
            }
        }
    }

    fn generate_root_objects(&self) -> Vec<TokenStream> {
        let document_root = quote! { document_root };

        self.root_class_indices
            .iter()
            .map(|class_idx| {
                let class = self.class(*class_idx);
                let log_field = value_ident_with_suffix(class.name(), "log");
                self.generate_add_root_for_class(class, quote! { self.#log_field }, &document_root)
            })
            .collect()
    }

    fn generate_read_as_ecore(&self) -> TokenStream {
        let path = self.package_module_path();

        quote! {
            /// Serializes the current model state as XMI conforming to the source Ecore metamodel.
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
        }
    }

    fn generate_eval_nested(&self) -> TokenStream {
        let path = self.package_module_path();
        let package_log = self.package_log_ident();
        let namespace_prefix = self.namespace_prefix();
        let namespace_uri = self.namespace_uri();
        let namespace_attribute = format!("xmlns:{namespace_prefix}");
        let root_objects = self.generate_root_objects();

        quote! {
            impl #path::EvalNested<ReadAsEcore> for #package_log {
                fn execute_query(
                    &self,
                    _q: ReadAsEcore,
                ) -> <ReadAsEcore as #path::QueryOperation>::Response {
                    let mut document_root = xml_builder::XMLElement::new("xmi:XMI");
                    document_root.add_attribute("xmi:version", "2.0");
                    document_root.add_attribute("xmlns:xmi", "http://www.omg.org/XMI");
                    document_root.add_attribute(
                        "xmlns:xsi",
                        "http://www.w3.org/2001/XMLSchema-instance",
                    );
                    document_root.add_attribute(#namespace_attribute, #namespace_uri);
                    #(#root_objects)*

                    let mut xml = xml_builder::XMLBuilder::new()
                        .version(xml_builder::XMLVersion::XML1_0)
                        .encoding("UTF-8".into())
                        .build();
                    xml.set_root_element(document_root);

                    let mut writer = Vec::new();
                    xml.generate(&mut writer)
                        .expect("writing model XMI to an in-memory buffer should not fail");
                    writer
                }
            }
        }
    }
}

impl Generate for ReadAsEcoreGenerator<'_> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let read_as_ecore = self.generate_read_as_ecore();
        let eval_nested = self.generate_eval_nested();

        Ok(Fragment::new(
            quote! {
                #read_as_ecore
                #eval_nested
            },
            vec![
                Import::Protocol(Protocol::EvalNested),
                Import::Protocol(Protocol::QueryOperation),
            ],
            vec![],
        ))
    }
}
