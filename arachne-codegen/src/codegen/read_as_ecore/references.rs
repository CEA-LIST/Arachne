//! Resolves typed-graph arcs into XMI links between serialized objects.

use ecore_rs::repr::structural;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use super::ReadAsEcoreGenerator;
use crate::codegen::{
    classifier::classifier_ident,
    ident::{classifier_type_ident_with_suffix, rust_ident},
};

impl ReadAsEcoreGenerator<'_> {
    fn reference_connection_names(&self) -> Vec<Ident> {
        let mut counts = std::collections::HashMap::<String, usize>::new();

        self.ref_analysis
            .refs
            .iter()
            .map(|reference| {
                let source_class = self.class(reference.source_class);
                let target_class = self.class(reference.target_class);
                let source_name = classifier_ident(self.ctx, source_class);
                let target_name = classifier_ident(self.ctx, target_class);
                let base_name = format!("{source_name}To{target_name}");
                let suffix = counts.entry(base_name.clone()).or_insert(0);
                let unique_name = if *suffix == 0 {
                    base_name
                } else {
                    format!("{base_name}{}", *suffix + 1)
                };
                *suffix += 1;
                rust_ident(unique_name)
            })
            .collect()
    }

    pub(super) fn generate_reference_resolver(&self) -> TokenStream {
        if !self.has_references() {
            return quote! {};
        }

        let path = self.path();
        let connection_names = self.reference_connection_names();
        let package_set = self.package_class_set();
        let mut arms = Vec::new();

        for class_idx in &self.package_classes {
            let class = self.class(*class_idx);
            for feature in class.structural().iter().filter(|feature| {
                feature.kind == structural::Typ::EReference
                    && !feature.containment
                    && feature.transient != Some(true)
            }) {
                let feature_key = self.reference_feature_key(class, feature);
                let source_family = self
                    .concrete_family(class.idx)
                    .into_iter()
                    .collect::<std::collections::HashSet<_>>();

                arms.extend(
                    self.ref_analysis
                        .refs
                        .iter()
                        .zip(connection_names.iter())
                        .filter(|(reference, _)| {
                            reference.reference_name == feature.name
                                && source_family.contains(&reference.source_class)
                                && package_set.contains(&reference.target_class)
                        })
                        .map(|(reference, connection)| {
                            let source_class = self.class(reference.source_class);
                            let target_class = self.class(reference.target_class);
                            let source_id =
                                classifier_type_ident_with_suffix(self.ctx, source_class, "Id");
                            let target_id =
                                classifier_type_ident_with_suffix(self.ctx, target_class, "Id");
                            quote! {
                                (
                                    #path::Instance::#source_id(source),
                                    #path::Instance::#target_id(target),
                                    #path::Ref::#connection(_),
                                ) => {
                                    values.insert(
                                        source.0.clone(),
                                        #feature_key,
                                        &target.0,
                                    );
                                }
                            }
                        }),
                );
            }
        }

        quote! {
            type XmiReferences = #path::XmiReferenceIndex<#path::ObjectPath>;

            fn build_xmi_references(
                refs: &petgraph::graph::DiGraph<#path::Instance, #path::Ref>,
            ) -> XmiReferences {
                let mut values = XmiReferences::default();

                use petgraph::visit::EdgeRef as _;
                for edge in refs.edge_references() {
                    let source = &refs[edge.source()];
                    let target = &refs[edge.target()];
                    match (source, target, edge.weight()) {
                        #(#arms,)*
                        _ => {}
                    }
                }
                values
            }
        }
    }
}
