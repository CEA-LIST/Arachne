//! Generates the read-only projection from a package CRDT to Ecore XMI.

use ecore_rs::{
    ctx::Ctx,
    repr::{Class, Structural, idx, structural},
};
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
        annotation::{transparent_field, uw_map_spec},
        classifier::{
            classifier_ident, has_codegen_polymorphic_family, is_instantiable_class,
            is_uninhabited_polymorphic_class, polymorphic_kind_ident,
        },
        cycles::CycleAnalysis,
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        ident::{rust_ident, value_ident},
        import::{Import, Protocol},
        reference::analysis::ReferenceAnalysis,
    },
};

pub struct ReadAsEcoreGenerator<'a> {
    ctx: &'a Ctx,
    pack_idx: idx::Pack,
    package_classes: Vec<idx::Class>,
    root_class_indices: Vec<idx::Class>,
    ref_analysis: &'a ReferenceAnalysis,
    cycle_analysis: &'a CycleAnalysis,
}

impl<'a> ReadAsEcoreGenerator<'a> {
    pub fn new(
        ctx: &'a Ctx,
        pack_idx: idx::Pack,
        package_classes: Vec<idx::Class>,
        root_class_indices: Vec<idx::Class>,
        ref_analysis: &'a ReferenceAnalysis,
        cycle_analysis: &'a CycleAnalysis,
    ) -> Self {
        Self {
            ctx,
            pack_idx,
            package_classes,
            root_class_indices,
            ref_analysis,
            cycle_analysis,
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

    fn package_class_set(&self) -> std::collections::HashSet<idx::Class> {
        self.package_classes.iter().copied().collect()
    }

    fn is_uw_map_entry_helper(&self, class: &Class) -> bool {
        if !is_instantiable_class(class) {
            return false;
        }

        let incoming_features = self
            .ctx
            .classes()
            .iter()
            .flat_map(|source| source.structural().iter())
            .filter(|feature| feature.typ == Some(class.idx))
            .collect::<Vec<_>>();

        !incoming_features.is_empty()
            && incoming_features.iter().all(|feature| {
                feature.kind == structural::Typ::EReference
                    && feature.containment
                    && uw_map_spec(feature).is_some()
            })
    }

    fn generates_concrete_wrapper(&self, class: &Class) -> bool {
        is_instantiable_class(class)
            && transparent_field(class).is_none()
            && !self.is_uw_map_entry_helper(class)
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

    fn concrete_family(&self, class_idx: idx::Class) -> Vec<idx::Class> {
        let package_set = self.package_class_set();
        let mut result = Vec::new();
        let mut stack = vec![class_idx];

        while let Some(candidate) = stack.pop() {
            if !package_set.contains(&candidate) {
                continue;
            }

            let class = self.class(candidate);
            if is_instantiable_class(class) {
                result.push(candidate);
            }
            stack.extend(class.sub().iter().copied());
        }

        result.sort_by_key(|idx| **idx);
        result.dedup();
        result
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
}

impl Generate for ReadAsEcoreGenerator<'_> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let helpers = self.generate_helpers();
        let reference_resolver = self.generate_reference_resolver();

        let class_visitors = self
            .package_classes
            .iter()
            .map(|class_idx| self.generate_class_visitor(self.class(*class_idx)));
        let union_visitors = self
            .package_classes
            .iter()
            .map(|class_idx| self.generate_union_visitor(self.class(*class_idx)));
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
