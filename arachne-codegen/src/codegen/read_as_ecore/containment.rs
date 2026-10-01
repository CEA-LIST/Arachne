//! Generates traversal of containment features, including lists and UW maps.

use ecore_rs::repr::{Class, Structural, idx, structural};
use proc_macro2::TokenStream;
use quote::quote;

use super::ReadAsEcoreGenerator;
use crate::codegen::{
    annotation::{transparent_field, uw_map_spec},
    classifier::{inherited_field_ident, is_uninhabited_polymorphic_class},
    cycles::BoxingStrategy,
    feature::bounds::{BoundKind, normalize_bounds},
    ident::value_ident,
};

impl ReadAsEcoreGenerator<'_> {
    pub(super) fn unbox_log_expr(
        &self,
        source_class: idx::Class,
        field_name: &str,
        log: TokenStream,
    ) -> TokenStream {
        if self
            .cycle_analysis
            .boxing_strategy(source_class, field_name)
            == BoxingStrategy::NoBox
        {
            log
        } else {
            quote! { (#log).inner() }
        }
    }

    fn call_target_visitor(
        &self,
        target_class: &Class,
        writer: TokenStream,
        element_name: Option<&str>,
        path_expr: TokenStream,
        log_expr: TokenStream,
        root: bool,
    ) -> TokenStream {
        let refs_arg = self.refs_arg();

        if self.union_is_generated(target_class) {
            let visitor = self.union_visitor_ident(target_class);
            let element = match element_name {
                Some(name) => quote! { Some(#name) },
                None => quote! { None },
            };
            quote! {
                #visitor(#writer, #element, #path_expr, #log_expr, #refs_arg);
            }
        } else {
            let visitor = self.class_visitor_ident(target_class);
            let element = element_name
                .map(str::to_string)
                .unwrap_or_else(|| self.root_element(target_class));
            let xmi_type = if root {
                quote! { None }
            } else {
                let typ = self.xmi_type(target_class);
                quote! { Some(#typ) }
            };
            quote! {
                #visitor(#writer, #element, #xmi_type, #path_expr, #log_expr, #refs_arg);
            }
        }
    }

    pub(super) fn generate_containment_visit(
        &self,
        source_class: idx::Class,
        feature: &Structural,
        log: TokenStream,
        base_path: TokenStream,
        include_field_in_path: bool,
        writer: TokenStream,
    ) -> TokenStream {
        let module_path = self.path();
        if let Some(spec) = uw_map_spec(feature) {
            return self.generate_uw_map_visit(
                source_class,
                feature,
                spec.key_feature,
                spec.value_feature,
                log,
                base_path,
                include_field_in_path,
                writer,
            );
        }

        let target_class = self.class(feature.typ.expect("containment should have a type"));
        let path_field = value_ident(&feature.name).to_string();
        let xml_name = feature.name.as_str();
        let (bound_kind, _) = normalize_bounds(feature.bounds, &feature.name);
        let unbox = |log| self.unbox_log_expr(source_class, &feature.name, log);
        let child_base_path = if include_field_in_path {
            quote! { #base_path.clone().field(#path_field) }
        } else {
            quote! { #base_path.clone() }
        };

        match bound_kind {
            BoundKind::Single => {
                let child_log = unbox(log);
                self.call_target_visitor(
                    target_class,
                    writer,
                    Some(xml_name),
                    child_base_path,
                    child_log,
                    false,
                )
            }
            BoundKind::Optional => {
                let child_log = unbox(quote! { child });
                let call = self.call_target_visitor(
                    target_class,
                    writer,
                    Some(xml_name),
                    quote! { child_path },
                    child_log,
                    false,
                );
                quote! {
                    if let Some(child) = (#log).child() {
                        let child_path = #child_base_path;
                        #call
                    }
                }
            }
            BoundKind::Many => {
                let child_log = unbox(quote! { child });
                let call = self.call_target_visitor(
                    target_class,
                    writer,
                    Some(xml_name),
                    quote! { child_path },
                    child_log,
                    false,
                );
                quote! {
                    {
                        let list_base_path = #child_base_path;
                        let positions = xmi_read::<_, Vec<#module_path::EventId>>(
                            (#log).positions()
                        );
                        for event_id in &positions {
                            if let Some(child) = (#log).children().get_child(event_id) {
                                let child_path =
                                    list_base_path.clone().list_element(event_id.clone());
                                #call
                            }
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_uw_map_visit(
        &self,
        source_class: idx::Class,
        feature: &Structural,
        key_feature_name: String,
        value_feature_name: String,
        log: TokenStream,
        base_path: TokenStream,
        include_field_in_path: bool,
        writer: TokenStream,
    ) -> TokenStream {
        let module_path = self.path();
        let target_class = self.class(feature.typ.expect("uw-map should have a target type"));
        let key_feature = target_class
            .structural()
            .iter()
            .find(|candidate| candidate.name == key_feature_name)
            .expect("uw-map key feature should exist");
        let value_feature = target_class
            .structural()
            .iter()
            .find(|candidate| candidate.name == value_feature_name)
            .expect("uw-map value feature should exist");
        let key_class = self.class(key_feature.typ.expect("uw-map key should have a type"));
        let key_to_string = self.scalar_to_string(key_class, quote! { key });
        let path_field = value_ident(&feature.name).to_string();
        let xml_name = feature.name.as_str();
        let key_xml_name = key_feature.name.as_str();
        let xmi_type = self.xmi_type(target_class);
        let map_base_path = if include_field_in_path {
            quote! { #base_path.clone().field(#path_field) }
        } else {
            quote! { #base_path.clone() }
        };

        let (value_attributes, value_children) = match value_feature.kind {
            structural::Typ::EAttribute => {
                let collect = self.attribute_collect_from_log(
                    value_feature,
                    quote! { child },
                    quote! { &mut attrs },
                );
                (quote! { #collect }, quote! {})
            }
            structural::Typ::EReference => {
                let value_target =
                    self.class(value_feature.typ.expect("uw-map value should have a type"));
                let value_xml_name = value_feature.name.as_str();
                let value_log_source = if transparent_field(self.class(source_class)).is_some() {
                    source_class
                } else {
                    target_class.idx
                };
                let value_log =
                    self.unbox_log_expr(value_log_source, &value_feature.name, quote! { child });
                let visit = self.call_target_visitor(
                    value_target,
                    quote! { writer },
                    Some(value_xml_name),
                    quote! { entry_path.clone() },
                    value_log,
                    false,
                );
                (quote! {}, visit)
            }
        };

        quote! {
            {
                let map_base_path = #map_base_path;
                let mut entries = (#log).children().iter().collect::<Vec<_>>();
                entries.sort_by_key(|(key, _)| format!("{:?}", key));

                for (key, child) in entries {
                    let entry_path = map_base_path.clone().map_entry(format!("{:?}", key));
                    let mut attrs = #module_path::XmiAttributes::for_object(&entry_path);
                    attrs.push_value("xsi:type", #xmi_type);
                    attrs.push_values(#key_xml_name, vec![#key_to_string], true);
                    #value_attributes
                    (#writer).element(#xml_name, &attrs, |writer| {
                        #value_children
                    });
                }
            }
        }
    }

    pub(super) fn generate_child_visits(
        &self,
        class: &Class,
        log: TokenStream,
        base_path: TokenStream,
        writer: TokenStream,
    ) -> Vec<TokenStream> {
        let mut visits = Vec::new();

        for super_idx in class.sup() {
            let super_class = self.class(*super_idx);
            let accessor = inherited_field_ident(super_class);
            let inherited_field = accessor.to_string();
            let inherited_log =
                self.unbox_log_expr(class.idx, &inherited_field, quote! { (#log).#accessor() });
            visits.extend(self.generate_child_visits(
                super_class,
                inherited_log,
                quote! { (#base_path).clone().field(#inherited_field) },
                writer.clone(),
            ));
        }

        visits.extend(
            class
                .structural()
                .iter()
                .filter(|feature| {
                    feature.kind == structural::Typ::EReference
                        && feature.containment
                        && feature.transient != Some(true)
                        && feature.typ.is_some_and(|target| {
                            !is_uninhabited_polymorphic_class(self.ctx, self.class(target))
                        })
                })
                .map(|feature| {
                    let accessor = value_ident(&feature.name);
                    self.generate_containment_visit(
                        class.idx,
                        feature,
                        quote! { (#log).#accessor() },
                        base_path.clone(),
                        true,
                        writer.clone(),
                    )
                }),
        );

        visits
    }
}
