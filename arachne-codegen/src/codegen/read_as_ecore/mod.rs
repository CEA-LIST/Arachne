//! Generates the read-only projection from a package CRDT to Ecore XMI.

use std::collections::BTreeSet;

use ecore_rs::repr::{Class, Structural, idx};
use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

mod attributes;
mod containment;
mod query;
mod references;
mod visitors;

use crate::{
    READ_AS_ECORE_PATH_MOD,
    codegen::{
        analysis::{GenerationPlan, ModelIndex},
        classifier::{classifier_ident, has_codegen_polymorphic_family, polymorphic_kind_ident},
        cycles::CycleAnalysis,
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        ident::{rust_ident, value_ident},
        import::{Import, Protocol},
        reference::analysis::ReferenceAnalysis,
    },
};

pub struct ReadAsEcoreGenerator<'a> {
    ctx: &'a ModelIndex<'a>,
    pack_idx: idx::Pack,
    /// Shared package membership and class-ID order for visitors and reference filtering.
    reachable_classes: &'a BTreeSet<idx::Class>,
    concrete_families: &'a [Vec<idx::Class>],
    root_class_indices: &'a [idx::Class],
    ref_analysis: &'a ReferenceAnalysis,
    cycle_analysis: &'a CycleAnalysis,
}

impl<'a> ReadAsEcoreGenerator<'a> {
    pub fn new(plan: &'a GenerationPlan<'a>) -> Self {
        Self {
            ctx: &plan.model,
            pack_idx: plan.pack_idx,
            reachable_classes: &plan.reachable_classes,
            concrete_families: &plan.concrete_families,
            root_class_indices: &plan.roots,
            ref_analysis: &plan.references,
            cycle_analysis: &plan.cycles,
        }
    }

    fn path(&self) -> syn::Path {
        syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, READ_AS_ECORE_PATH_MOD)).unwrap()
    }

    fn package_name(&self) -> &str {
        self.ctx.packs().get(self.pack_idx).unwrap().name()
    }

    fn ns_prefix(&self) -> String {
        self.ctx
            .packs()
            .get(self.pack_idx)
            .unwrap()
            .ns_prefix()
            .filter(|prefix| !prefix.is_empty())
            .unwrap_or_else(|| self.package_name())
            .to_string()
    }

    fn ns_uri(&self) -> String {
        self.ctx
            .packs()
            .get(self.pack_idx)
            .unwrap()
            .ns_uri()
            .filter(|uri| !uri.is_empty())
            .unwrap_or_else(|| self.package_name())
            .to_string()
    }

    fn has_references(&self) -> bool {
        self.ref_analysis.has_references()
    }

    fn refs_param(&self) -> TokenStream {
        if self.has_references() {
            quote! { refs: &XmiReferences, }
        } else {
            quote! {}
        }
    }

    fn refs_arg(&self) -> TokenStream {
        if self.has_references() {
            quote! { refs, }
        } else {
            quote! {}
        }
    }

    fn refs_call_arg(&self) -> TokenStream {
        if self.has_references() {
            quote! { &refs, }
        } else {
            quote! {}
        }
    }

    fn class(&self, class_idx: idx::Class) -> &Class {
        &self.ctx.classes()[*class_idx]
    }

    fn generates_concrete_wrapper(&self, class: &Class) -> bool {
        self.ctx.info(class.idx).wrapper
    }

    fn union_is_generated(&self, class: &Class) -> bool {
        !class.is_enum() && has_codegen_polymorphic_family(self.ctx, class)
    }

    fn class_visitor_ident(&self, class: &Class) -> Ident {
        rust_ident(format!(
            "visit_{}",
            classifier_ident(self.ctx, class)
                .to_string()
                .to_snake_case()
        ))
    }

    fn union_visitor_ident(&self, class: &Class) -> Ident {
        rust_ident(format!(
            "visit_{}",
            polymorphic_kind_ident(self.ctx, class)
                .to_string()
                .to_snake_case()
        ))
    }

    fn reference_feature_key(&self, class: &Class, feature: &Structural) -> String {
        format!(
            "{}::{}",
            classifier_ident(self.ctx, class)
                .to_string()
                .to_snake_case(),
            value_ident(&feature.name)
        )
    }

    fn xmi_type(&self, class: &Class) -> String {
        format!("{}:{}", self.ns_prefix(), class.name())
    }

    fn root_element(&self, class: &Class) -> String {
        self.xmi_type(class)
    }

    fn concrete_family(&self, class_idx: idx::Class) -> &[idx::Class] {
        &self.concrete_families[*class_idx]
    }

    fn union_variants(&self, class: &Class) -> Vec<&Class> {
        self.ctx
            .info(class.idx)
            .variants
            .iter()
            .map(|idx| self.class(*idx))
            .collect()
    }
}

impl Generate for ReadAsEcoreGenerator<'_> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let helpers = self.generate_helpers();
        let reference_resolver = self.generate_reference_resolver();

        let class_visitors = self
            .reachable_classes
            .iter()
            .map(|class_idx| self.generate_class_visitor(self.class(*class_idx)))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let union_visitors = self
            .reachable_classes
            .iter()
            .map(|class_idx| self.generate_union_visitor(self.class(*class_idx)))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let query_impl = self.generate_query_impl();

        let tokens = quote! {
            #helpers
            #reference_resolver
            #(#class_visitors)*
            #(#union_visitors)*
            #query_impl
        };

        let mut imports = vec![
            Import::Custom("crate::package::*"),
            Import::Custom("crate::classifiers::*"),
            Import::Protocol(Protocol::ObjectPath),
            Import::Protocol(Protocol::EventId),
            Import::Protocol(Protocol::IsLog),
            Import::Protocol(Protocol::Read),
            Import::Protocol(Protocol::EvalNested),
            Import::Protocol(Protocol::QueryOperation),
            Import::Custom("arachne_xmi::{Attributes as XmiAttributes, Writer as XmiWriter}"),
        ];
        if self.has_references() {
            imports.push(Import::Custom("crate::references::*"));
            imports.push(Import::Custom(
                "arachne_xmi::ReferenceIndex as XmiReferenceIndex",
            ));
        }

        Ok(Fragment::new(tokens, imports, vec![]))
    }
}
