//! Restores object identities erased by transparent and map-entry projections.

use std::collections::BTreeSet;

use ecore_rs::repr::{Class, Structural};
use proc_macro2::TokenStream;
use quote::quote;

use super::ReferenceGenerator;
use crate::codegen::{
    cycles::CycleAnalysis,
    feature::plan::{FeaturePlan, Storage},
    ident::{classifier_type_ident, value_ident},
};

fn projection_key(class: &Class, field: &Structural) -> (usize, usize) {
    (
        *class.idx,
        class
            .structural()
            .iter()
            .position(|candidate| candidate.name == field.name)
            .expect("projected field belongs to its class"),
    )
}

pub(crate) fn projected_log_type(
    class: &Class,
    field: &Structural,
    log: TokenStream,
    path: &syn::Path,
) -> TokenStream {
    let (class, field) = projection_key(class, field);
    quote! { #path::ProjectedLog<#log, #class, #field> }
}

impl ReferenceGenerator<'_> {
    pub(super) fn generate_projection_support(&self) -> anyhow::Result<TokenStream> {
        let mut projections = BTreeSet::new();
        // Only representation matters here; boxing was already chosen by the
        // classifier generator and does not affect a projected object's identity.
        let unboxed = CycleAnalysis::default();
        for &class_idx in self.reachable_classes {
            let class = self.ctx.class(class_idx);
            let info = self.ctx.info(class_idx);
            for field in self.ctx.supported_features(class_idx) {
                if info.referenceable
                    && info.transparent_field.as_deref() == Some(field.name.as_str())
                {
                    projections.insert(projection_key(class, field));
                }
                if field.containment
                    && !field
                        .typ
                        .is_some_and(|target| self.ctx.info(target).uninhabited)
                {
                    let plan = FeaturePlan::resolve(self.ctx, class_idx, field, &unboxed)?;
                    if let Storage::Map(map) = plan.storage
                        && map.tracks_identity(self.ctx)
                    {
                        projections.insert(projection_key(map.entry, map.value.feature));
                    }
                }
            }
        }
        if projections.is_empty() {
            return Ok(quote! {});
        }

        let arms = projections.into_iter().map(|(class_idx, field_idx)| {
            let class = self.ctx.class(class_idx.into());
            let kind = classifier_type_ident(self.ctx, class).to_string();
            let field = value_ident(&class.structural()[field_idx].name).to_string();
            quote! { (#class_idx, #field_idx) => (#kind, #field), }
        });
        let runtime: syn::File = syn::parse_str(include_str!("projection_runtime.rs"))?;
        Ok(quote! {
            // Const IDs identify declarations, not runtime instances. ObjectPath
            // supplies instance identity, including map keys and list event IDs.
            fn projection_metadata(class: usize, field: usize) -> (&'static str, &'static str) {
                match (class, field) {
                    #(#arms)*
                    _ => unreachable!("unknown generated projection"),
                }
            }
            mod projection {
                #runtime
            }
            pub use projection::ProjectedLog;
        })
    }
}
