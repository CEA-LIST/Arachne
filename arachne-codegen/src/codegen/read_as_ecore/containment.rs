//! Generates traversal of containment features, including lists and UW maps.

use ecore_rs::repr::{Class, Structural, idx, structural};
use proc_macro2::TokenStream;
use quote::quote;

use super::ReadAsEcoreGenerator;
use crate::codegen::{
    classifier::{inherited_field_ident, is_uninhabited_polymorphic_class},
    cycles::BoxingStrategy,
    feature::{
        bounds::BoundKind,
        plan::{FeaturePlan, MapPlan, Storage},
    },
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
    ) -> anyhow::Result<TokenStream> {
        let module_path = self.path();
        let plan = FeaturePlan::resolve(self.ctx, source_class, feature, self.cycle_analysis)?;
        let path_field = value_ident(&feature.name).to_string();
        let xml_name = feature.name.as_str();
        let child_base_path = if include_field_in_path {
            quote! { #base_path.clone().field(#path_field) }
        } else {
            quote! { #base_path.clone() }
        };

        let target_class = match &plan.storage {
            Storage::Map(map) => {
                return self.generate_uw_map_visit(feature, map, log, child_base_path, writer);
            }
            Storage::Containment { target, .. } => *target,
            Storage::Attribute(_) => {
                anyhow::bail!("Expected a containment plan for `{}`", feature.name)
            }
        };

        Ok(match plan.bounds {
            BoundKind::Single => {
                let child_log = plan.unbox_log(log);
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
                let child_log = plan.unbox_log(quote! { child });
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
                let child_log = plan.unbox_log(quote! { child });
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
        })
    }

    fn generate_uw_map_visit(
        &self,
        feature: &Structural,
        map: &MapPlan<'_>,
        log: TokenStream,
        map_base_path: TokenStream,
        writer: TokenStream,
    ) -> anyhow::Result<TokenStream> {
        let module_path = self.path();
        let key_to_string = self.scalar_to_string(map.key_type, quote! { key });
        let xml_name = feature.name.as_str();
        let key_xml_name = map.key.name.as_str();
        let xmi_type = self.xmi_type(map.entry);

        let (value_attributes, value_children) = match &map.value.storage {
            Storage::Attribute(_) => {
                let collect = self.attribute_collect_from_plan(
                    &map.value,
                    quote! { child },
                    quote! { &mut attrs },
                )?;
                (collect, quote! {})
            }
            Storage::Containment { target, .. } => {
                let value_log = map.value.unbox_log(quote! { child });
                let visit = self.call_target_visitor(
                    target,
                    quote! { writer },
                    Some(&map.value.feature.name),
                    quote! { entry_path.clone() },
                    value_log,
                    false,
                );
                (quote! {}, visit)
            }
            Storage::Map(_) => anyhow::bail!("UWMap values must be single-valued"),
        };

        Ok(quote! {
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
        })
    }

    pub(super) fn generate_child_visits(
        &self,
        class: &Class,
        log: TokenStream,
        base_path: TokenStream,
        writer: TokenStream,
    ) -> anyhow::Result<Vec<TokenStream>> {
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
            )?);
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
                })
                .collect::<anyhow::Result<Vec<_>>>()?,
        );

        Ok(visits)
    }
}
