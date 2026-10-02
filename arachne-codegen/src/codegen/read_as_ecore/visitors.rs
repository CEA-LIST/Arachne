//! Generates visitors for EClasses and their polymorphic Union CRDTs.

use ecore_rs::repr::{Class, Structural, structural};
use proc_macro2::TokenStream;
use quote::quote;

use super::ReadAsEcoreGenerator;
use crate::codegen::{
    annotation::transparent_field,
    classifier::{classifier_ident, classifier_log_ident, polymorphic_kind_log_ident},
    ident::classifier_type_ident_with_suffix,
};

impl ReadAsEcoreGenerator<'_> {
    pub(super) fn generate_class_visitor(&self, class: &Class) -> anyhow::Result<TokenStream> {
        if !self.generates_concrete_wrapper(class) {
            return Ok(quote! {});
        }

        let path = self.path();
        let fn_ident = self.class_visitor_ident(class);
        let log_ty = classifier_log_ident(self.ctx, class);
        let refs_param = self.refs_param();
        let attribute_visits = self.generate_attribute_visits(
            class,
            quote! { log },
            quote! { &path },
            quote! { &mut attrs },
        )?;
        let child_visits =
            self.generate_child_visits(class, quote! { log }, quote! { path }, quote! { writer })?;

        Ok(quote! {
            // The visitor signature is uniform; leaf classes may not use every argument.
            #[allow(dead_code, unused_variables)]
            fn #fn_ident(
                writer: &mut #path::XmiWriter,
                element_name: &str,
                xmi_type: Option<&str>,
                path: #path::ObjectPath,
                log: &#path::#log_ty,
                #refs_param
            ) {
                let mut attrs = #path::XmiAttributes::for_object(&path);
                if let Some(xmi_type) = xmi_type {
                    attrs.push_value("xsi:type", xmi_type);
                }
                #(#attribute_visits)*
                writer.element(element_name, &attrs, |writer| {
                    #(#child_visits)*
                });
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_transparent_variant_visit(
        &self,
        subclass: &Class,
        field: &Structural,
        writer: TokenStream,
        element_name: TokenStream,
        xmi_type: TokenStream,
        path: TokenStream,
        log: TokenStream,
    ) -> anyhow::Result<TokenStream> {
        let module_path = self.path();
        let (attribute_visit, child_visit) = if field.transient == Some(true) {
            (quote! {}, quote! {})
        } else {
            match field.kind {
                structural::Typ::EAttribute => {
                    let collect =
                        self.attribute_collect_from_log(field, log.clone(), quote! { &mut attrs })?;
                    (quote! { #collect }, quote! {})
                }
                structural::Typ::EReference => (
                    quote! {},
                    self.generate_containment_visit(
                        subclass.idx,
                        field,
                        log,
                        quote! { path },
                        false,
                        quote! { writer },
                    )?,
                ),
            }
        };

        let subclass_type = self.xmi_type(subclass);
        Ok(quote! {
            {
                let path = #path;
                let mut attrs = #module_path::XmiAttributes::for_object(&path);
                if #xmi_type {
                    attrs.push_value("xsi:type", #subclass_type);
                }
                #attribute_visit
                (#writer).element(#element_name, &attrs, |writer| {
                    #child_visit
                });
            }
        })
    }

    pub(super) fn generate_union_visitor(&self, class: &Class) -> anyhow::Result<TokenStream> {
        if !self.union_is_generated(class) {
            return Ok(quote! {});
        }

        let path = self.path();
        let fn_ident = self.union_visitor_ident(class);
        let log_ty = polymorphic_kind_log_ident(self.ctx, class);
        let container_ty = classifier_type_ident_with_suffix(self.ctx, class, "KindContainer");
        let child_ty = classifier_type_ident_with_suffix(self.ctx, class, "KindChild");
        let refs_param = self.refs_param();
        let refs_arg = self.refs_arg();
        let variants = self.union_variants(class);
        let child_rank_arms = variants
            .iter()
            .enumerate()
            .map(|(rank, subclass)| {
                let variant = classifier_ident(self.ctx, subclass);
                quote! { #path::#child_ty::#variant(_) => #rank }
            })
            .collect::<Vec<_>>();

        let child_arms = variants
            .into_iter()
            .map(|subclass| {
                let variant = classifier_ident(self.ctx, subclass);
                let variant_path = variant.to_string().to_lowercase();
                let subclass_type = self.xmi_type(subclass);
                let root_element = self.root_element(subclass);

                Ok(if let Some(field_name) = transparent_field(subclass) {
                    let field = subclass
                        .structural()
                        .iter()
                        .find(|feature| feature.name == field_name)
                        .expect("transparent field should exist");
                    let body = self.generate_transparent_variant_visit(
                        subclass,
                        field,
                        quote! { writer },
                        quote! { element_name },
                        quote! { emit_xmi_type },
                        quote! { path.clone().variant(#variant_path) },
                        quote! { child_log },
                    )?;
                    quote! {
                        #path::#child_ty::#variant(child_log) => {
                            let (element_name, emit_xmi_type) = match element_name {
                                Some(name) => (name, true),
                                None => (#root_element, false),
                            };
                            #body
                        }
                    }
                } else if subclass.idx != class.idx && self.union_is_generated(subclass) {
                    let visitor = self.union_visitor_ident(subclass);
                    quote! {
                        #path::#child_ty::#variant(child_log) => {
                            #visitor(
                                writer,
                                element_name,
                                path.clone().variant(#variant_path),
                                child_log,
                                #refs_arg
                            );
                        }
                    }
                } else {
                    let visitor = self.class_visitor_ident(subclass);
                    quote! {
                        #path::#child_ty::#variant(child_log) => {
                            let (element_name, xmi_type) = match element_name {
                                Some(name) => (name, Some(#subclass_type)),
                                None => (#root_element, None),
                            };
                            #visitor(
                                writer,
                                element_name,
                                xmi_type,
                                path.clone().variant(#variant_path),
                                child_log,
                                #refs_arg
                            );
                        }
                    }
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(quote! {
            // Some family unions are updated directly but never occur in containment traversal.
            #[allow(dead_code, unused_variables)]
            fn #fn_ident(
                writer: &mut #path::XmiWriter,
                element_name: Option<&str>,
                path: #path::ObjectPath,
                log: &#path::#log_ty,
                #refs_param
            ) {
                let visit_child = |
                    child: &#path::#child_ty,
                    writer: &mut #path::XmiWriter,
                | {
                    match child {
                        #(#child_arms,)*
                    }
                };
                let child_rank = |child: &#path::#child_ty| match child {
                    #(#child_rank_arms,)*
                };

                match &log.child {
                    #path::#container_ty::Unset => {}
                    #path::#container_ty::Value(child) => {
                        visit_child(child.as_ref(), writer);
                    }
                    #path::#container_ty::Conflicts(children) => {
                        let mut children = children.iter().collect::<Vec<_>>();
                        children.sort_by_key(|child| child_rank(child));
                        for child in children {
                            visit_child(child, writer);
                        }
                    }
                }
            }
        })
    }
}
