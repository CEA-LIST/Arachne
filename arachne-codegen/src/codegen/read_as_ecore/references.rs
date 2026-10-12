//! Resolves typed-graph arcs into XMI links between serialized objects.

use ecore_rs::repr::structural;
use proc_macro2::TokenStream;
use quote::quote;

use super::ReadAsEcoreGenerator;
use crate::codegen::ident::classifier_type_ident_with_suffix;

impl ReadAsEcoreGenerator<'_> {
    pub(super) fn generate_reference_resolver(&self) -> TokenStream {
        if !self.has_references() {
            return quote! {};
        }

        let path = self.path();
        let connection_names = &self.ref_analysis.connection_names;
        let mut arms = Vec::new();

        for class_idx in self.reachable_classes {
            let class = self.class(*class_idx);
            for feature in self.ctx.supported_features(class.idx).filter(|feature| {
                feature.kind == structural::Typ::EReference
                    && !feature.containment
                    && feature.transient != Some(true)
            }) {
                let feature_key = self.reference_feature_key(class, feature);
                let source_family = self.concrete_family(class.idx);

                arms.extend(
                    self.ref_analysis
                        .refs
                        .iter()
                        .zip(connection_names.iter())
                        .filter(|(reference, _)| {
                            reference.reference_name == feature.name
                                && source_family.contains(&reference.source_class)
                                && self.reachable_classes.contains(&reference.target_class)
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
