use ecore_rs::{
    ctx::Ctx,
    repr::{idx, structural},
};
use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::{
    PACKAGE_PATH_MOD,
    codegen::{
        classifier::INHERITANCE_SUFFIX,
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        import::{CrdtOp, Import, Log, NestedCrdtOp, Protocol},
        reference::analysis::ReferenceAnalysis,
    },
};

#[derive(Clone, Copy)]
struct RootMeta {
    class_idx: idx::Class,
}

pub struct PackageGenerator<'a> {
    ctx: &'a Ctx,
    pack_idx: idx::Pack,
    root_class_indices: Vec<idx::Class>,
    reachable_class_indices: Vec<idx::Class>,
    ref_analysis: &'a ReferenceAnalysis,
}

impl<'a> PackageGenerator<'a> {
    pub fn new(
        ctx: &'a Ctx,
        pack_idx: idx::Pack,
        root_class_indices: Vec<idx::Class>,
        reachable_class_indices: Vec<idx::Class>,
        ref_analysis: &'a ReferenceAnalysis,
    ) -> Self {
        Self {
            ctx,
            pack_idx,
            root_class_indices,
            reachable_class_indices,
            ref_analysis,
        }
    }

    fn roots(&self) -> Vec<RootMeta> {
        self.root_class_indices
            .iter()
            .copied()
            .map(|class_idx| RootMeta { class_idx })
            .collect()
    }

    fn root_class_name(&self, root: RootMeta) -> &str {
        self.ctx.classes()[*root.class_idx].name()
    }

    fn root_variant_ident(&self, root: RootMeta) -> Ident {
        Ident::new(
            &self.root_class_name(root).to_upper_camel_case(),
            Span::call_site(),
        )
    }

    fn root_log_ident(&self, root: RootMeta) -> Ident {
        format_ident!("{}Log", self.root_class_name(root).to_upper_camel_case())
    }

    fn root_value_ident(&self, root: RootMeta) -> Ident {
        format_ident!("{}Value", self.root_class_name(root).to_upper_camel_case())
    }

    fn root_field_ident(&self, root: RootMeta) -> Ident {
        format_ident!("{}_log", self.root_class_name(root).to_snake_case())
    }

    fn has_references(&self) -> bool {
        self.ref_analysis.has_references()
    }

    fn imports(&self) -> Vec<Import> {
        let mut imports = vec![
            Import::CrdtOp(CrdtOp::Nested(NestedCrdtOp::ListOp)),
            Import::CrdtOp(CrdtOp::Nested(NestedCrdtOp::MapOp)),
            Import::Protocol(Protocol::Read),
            Import::Protocol(Protocol::EvalNested),
            Import::Protocol(Protocol::IsLog),
            Import::Protocol(Protocol::Version),
            Import::Protocol(Protocol::Event),
            Import::Protocol(Protocol::QueryOperation),
            Import::Protocol(Protocol::EventId),
            Import::Custom(String::from("crate::classifiers::*")),
            Import::Custom(String::from("moirai_crdt::option::Optional")),
            Import::Custom(String::from("std::fmt::Display")),
        ];

        if self.has_references() {
            imports.extend([
                Import::Protocol(Protocol::LwwPolicy),
                Import::Log(Log::VecLog),
                Import::Protocol(Protocol::PureCRDT),
                Import::Custom(String::from("crate::references::*")),
            ]);
        }

        imports
    }

    fn generate_package_enum(&self) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_ident = format_ident!("{}", package_name.to_upper_camel_case());
        let root_variants = self.roots().into_iter().map(|root| {
            let variant = self.root_variant_ident(root);
            quote! { #variant(#path::#variant) }
        });
        let reference_variants = if self.has_references() {
            quote! { , AddReference(#path::Refs), RemoveReference(#path::Refs) }
        } else {
            quote! {}
        };

        quote! {
            #[derive(Debug, Clone)]
            pub enum #package_ident {
                #(#root_variants),*
                #reference_variants
            }
        }
    }

    fn generate_package_value_struct(&self) -> TokenStream {
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_value_name = format_ident!("{}Value", package_name.to_upper_camel_case());
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let root_fields = self.roots().into_iter().map(|root| {
            let field = Ident::new(
                &self.root_class_name(root).to_snake_case(),
                Span::call_site(),
            );
            let value_ty = self.root_value_ident(root);
            quote! { pub #field: #path::#value_ty }
        });
        let refs_field = if self.has_references() {
            quote! {
                pub refs: <#path::ReferenceManager<#path::LwwPolicy> as #path::PureCRDT>::Value,
            }
        } else {
            quote! {}
        };

        quote! {
            #[derive(Debug, Clone, Default)]
            pub struct #package_value_name {
                #(#root_fields,)*
                #refs_field
            }
        }
    }

    fn generate_package_log_struct(&self) -> TokenStream {
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_log_name = format_ident!("{}Log", package_name.to_upper_camel_case());
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();

        let root_fields = self.roots().into_iter().map(|root| {
            let field = self.root_field_ident(root);
            let log_ty = self.root_log_ident(root);
            quote! { #field: #path::#log_ty }
        });
        let root_getters = self.roots().into_iter().map(|root| {
            let field = self.root_field_ident(root);
            let log_ty = self.root_log_ident(root);
            quote! {
                pub fn #field(&self) -> &#path::#log_ty {
                    &self.#field
                }
            }
        });
        let reference_field = if self.has_references() {
            quote! {
                reference_manager_log: #path::VecLog<#path::ReferenceManager<#path::LwwPolicy>>,
            }
        } else {
            quote! {}
        };
        let reference_getter = if self.has_references() {
            quote! {
                pub fn reference_manager_log(&self) -> &#path::VecLog<#path::ReferenceManager<#path::LwwPolicy>> {
                    &self.reference_manager_log
                }
            }
        } else {
            quote! {}
        };

        quote! {
            #[derive(Debug, Clone, Default)]
            pub struct #package_log_name {
                #(#root_fields,)*
                #reference_field
            }

            impl #package_log_name {
                #(#root_getters)*
                #reference_getter
            }
        }
    }

    fn generate_reference_sync_support(&self) -> TokenStream {
        if !self.has_references() {
            return quote! {};
        }

        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let support_path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let package_ident = format_ident!("{}", package_name.to_upper_camel_case());
        let package_log_name = format_ident!("{}Log", package_name.to_upper_camel_case());
        let root_dispatch = self.roots().into_iter().map(|root| {
            let variant = self.root_variant_ident(root);
            let field = self.root_field_ident(root);
            let root_key = self.root_class_name(root).to_snake_case();
            quote! {
                #package_ident::#variant(op) => {
                    let root_id = #support_path::ObjectId::root(#root_key);
                    VertexSyncLog::collect_vertex_effects(
                        &self.#field,
                        op,
                        event.id(),
                        &root_id,
                        &refs,
                        &mut effects,
                    );
                }
            }
        });
        let support_defs = self.generate_vertex_support_defs();
        let to_instances_impls = self.generate_to_instances_impls();
        let vertex_impls = self.generate_vertex_effect_impls();

        quote! {
            #support_defs
            #to_instances_impls
            #vertex_impls

            impl #package_log_name {
                fn sync_reference_vertices(&mut self, event: &#support_path::Event<#package_ident>) {
                    let refs = #support_path::EvalNested::execute_query(
                        &self.reference_manager_log,
                        #support_path::Read::new(),
                    );
                    let mut effects = std::vec::Vec::new();

                    match event.op() {
                        #(#root_dispatch,)*
                        #package_ident::AddReference(_) | #package_ident::RemoveReference(_) => {}
                    }

                    for effect in effects {
                        let graph_op = match effect {
                            VertexGraphEffect::Add(id) => {
                                #support_path::ReferenceManager::<#support_path::LwwPolicy>::AddVertex { id }
                            }
                            VertexGraphEffect::Remove(id) => {
                                #support_path::ReferenceManager::<#support_path::LwwPolicy>::RemoveVertex { id }
                            }
                        };
                        #support_path::IsLog::effect(
                            &mut self.reference_manager_log,
                            #support_path::Event::unfold(event.clone(), graph_op),
                        );
                    }
                }
            }
        }
    }

    fn generate_is_log_impl(&self) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_log_name = format_ident!("{}Log", package_name.to_upper_camel_case());
        let package_ident = format_ident!("{}", package_name.to_upper_camel_case());
        let package_value_name = format_ident!("{}Value", package_name.to_upper_camel_case());

        let enabled_root_arms = self.roots().into_iter().map(|root| {
            let variant = self.root_variant_ident(root);
            let field = self.root_field_ident(root);
            quote! { #package_ident::#variant(o) => self.#field.is_enabled(o) }
        });
        let effect_root_arms = self.roots().into_iter().map(|root| {
            let variant = self.root_variant_ident(root);
            let field = self.root_field_ident(root);
            quote! { #package_ident::#variant(root) => self.#field.effect(#path::Event::unfold(event, root)) }
        });
        let stabilize_roots = self.roots().into_iter().map(|root| {
            let field = self.root_field_ident(root);
            quote! { self.#field.stabilize(version); }
        });
        let redundant_roots = self.roots().into_iter().map(|root| {
            let field = self.root_field_ident(root);
            quote! { self.#field.redundant_by_parent(version, conservative); }
        });
        let default_checks = self.roots().into_iter().map(|root| {
            let field = self.root_field_ident(root);
            quote! { self.#field.is_default() }
        });

        let reference_is_enabled = if self.has_references() {
            quote! {
                #package_ident::AddReference(o) => self
                    .reference_manager_log
                    .is_enabled(&#path::ReferenceManager::AddArc(o.clone())),
                #package_ident::RemoveReference(o) => self
                    .reference_manager_log
                    .is_enabled(&#path::ReferenceManager::RemoveArc(o.clone())),
            }
        } else {
            quote! {}
        };
        let pre_effect = if self.has_references() {
            quote! { self.sync_reference_vertices(&event); }
        } else {
            quote! {}
        };
        let reference_effect = if self.has_references() {
            quote! {
                #package_ident::AddReference(refs) => self
                    .reference_manager_log
                    .effect(#path::Event::unfold(event, #path::ReferenceManager::AddArc(refs))),
                #package_ident::RemoveReference(refs) => self
                    .reference_manager_log
                    .effect(#path::Event::unfold(event, #path::ReferenceManager::RemoveArc(refs))),
            }
        } else {
            quote! {}
        };
        let stabilize_refs = if self.has_references() {
            quote! { self.reference_manager_log.stabilize(version); }
        } else {
            quote! {}
        };
        let redundant_refs = if self.has_references() {
            quote! { self.reference_manager_log.redundant_by_parent(version, conservative); }
        } else {
            quote! {}
        };

        quote! {
            impl #path::IsLog for #package_log_name {
                type Value = #package_value_name;
                type Op = #package_ident;

                fn is_enabled(&self, op: &Self::Op) -> bool {
                    match op {
                        #(#enabled_root_arms,)*
                        #reference_is_enabled
                    }
                }

                fn effect(&mut self, event: #path::Event<Self::Op>) {
                    #pre_effect

                    match event.op().clone() {
                        #(#effect_root_arms,)*
                        #reference_effect
                    }
                }

                fn stabilize(&mut self, version: &#path::Version) {
                    #(#stabilize_roots)*
                    #stabilize_refs
                }

                fn redundant_by_parent(&mut self, version: &#path::Version, conservative: bool) {
                    #(#redundant_roots)*
                    #redundant_refs
                }

                fn is_default(&self) -> bool {
                    true #(&& #default_checks)*
                }
            }
        }
    }

    fn referenceable_class_chain(&self, class_idx: idx::Class) -> Vec<idx::Class> {
        fn collect(
            ctx: &Ctx,
            class_idx: idx::Class,
            referenceable: &[idx::Class],
            acc: &mut Vec<idx::Class>,
        ) {
            if referenceable.contains(&class_idx) && !acc.contains(&class_idx) {
                acc.push(class_idx);
            }
            for super_idx in ctx.classes()[*class_idx].sup() {
                collect(ctx, *super_idx, referenceable, acc);
            }
        }

        let mut out = Vec::new();
        collect(
            self.ctx,
            class_idx,
            &self.ref_analysis.referenceable_classes,
            &mut out,
        );
        out
    }

    fn generate_vertex_support_defs(&self) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let refs_value = quote! {
            <#path::ReferenceManager<#path::LwwPolicy> as #path::PureCRDT>::Value
        };
        let instance_matches = self
            .ref_analysis
            .referenceable_classes
            .iter()
            .map(|class_idx| {
                let class_name = self.ctx.classes()[**class_idx].name();
                let variant = format_ident!("{}Id", class_name);
                quote! { #path::Instance::#variant(id) => &id.0 }
            });

        quote! {
            #[derive(Debug, Clone)]
            enum VertexGraphEffect {
                Add(#path::Instance),
                Remove(#path::Instance),
            }

            trait ToInstances {
                fn to_instances(&self, object_id: #path::ObjectId) -> std::vec::Vec<#path::Instance>;
            }

            trait VertexSyncLog {
                type Op;

                fn collect_vertex_effects(
                    &self,
                    op: &Self::Op,
                    event_id: &#path::EventId,
                    object_id: &#path::ObjectId,
                    refs: &#refs_value,
                    out: &mut std::vec::Vec<VertexGraphEffect>,
                );
            }

            fn instance_object_id(instance: &#path::Instance) -> &#path::ObjectId {
                match instance {
                    #(#instance_matches,)*
                }
            }

            fn has_object_prefix(object_id: &#path::ObjectId, prefix: &#path::ObjectId) -> bool {
                object_id.root == prefix.root
                    && object_id.path.len() >= prefix.path.len()
                    && object_id.path.iter().zip(prefix.path.iter()).all(|(left, right)| left == right)
            }

            fn remove_instances_for_prefix(
                refs: &#refs_value,
                prefix: &#path::ObjectId,
                out: &mut std::vec::Vec<VertexGraphEffect>,
            ) {
                for instance in refs.node_weights() {
                    if has_object_prefix(instance_object_id(instance), prefix) {
                        out.push(VertexGraphEffect::Remove(instance.clone()));
                    }
                }
            }

            impl<T> ToInstances for std::boxed::Box<T>
            where
                T: ToInstances,
            {
                fn to_instances(&self, object_id: #path::ObjectId) -> std::vec::Vec<#path::Instance> {
                    self.as_ref().to_instances(object_id)
                }
            }

            impl<L> VertexSyncLog for std::boxed::Box<L>
            where
                L: VertexSyncLog,
            {
                type Op = std::boxed::Box<L::Op>;

                fn collect_vertex_effects(
                    &self,
                    op: &Self::Op,
                    event_id: &#path::EventId,
                    object_id: &#path::ObjectId,
                    refs: &#refs_value,
                    out: &mut std::vec::Vec<VertexGraphEffect>,
                ) {
                    self.as_ref().collect_vertex_effects(op.as_ref(), event_id, object_id, refs, out);
                }
            }

            impl<L> VertexSyncLog for moirai_crdt::list::nested_list::NestedListLog<L>
            where
                L: #path::IsLog + VertexSyncLog<Op = <L as #path::IsLog>::Op> + Default,
            {
                type Op = #path::NestedList<<L as #path::IsLog>::Op>;

                fn collect_vertex_effects(
                    &self,
                    op: &Self::Op,
                    event_id: &#path::EventId,
                    object_id: &#path::ObjectId,
                    refs: &#refs_value,
                    out: &mut std::vec::Vec<VertexGraphEffect>,
                ) {
                    match op {
                        #path::NestedList::Insert { value, .. } => {
                            let child_id = object_id.clone().list_element(event_id.clone());
                            VertexSyncLog::collect_vertex_effects(&L::default(), value, event_id, &child_id, refs, out);
                        }
                        #path::NestedList::Update { pos, value } => {
                            let positions = #path::EvalNested::execute_query(self.positions(), #path::Read::new());
                            let target_id = positions[*pos].clone();
                            let child_id = object_id.clone().list_element(target_id.clone());
                            if let Some(child) = self.children().get(&target_id) {
                                VertexSyncLog::collect_vertex_effects(child, value, event_id, &child_id, refs, out);
                            }
                        }
                        #path::NestedList::Delete { pos } => {
                            let positions = #path::EvalNested::execute_query(self.positions(), #path::Read::new());
                            let target_id = positions[*pos].clone();
                            let child_id = object_id.clone().list_element(target_id);
                            remove_instances_for_prefix(refs, &child_id, out);
                        }
                    }
                }
            }

            impl<K, L> VertexSyncLog for moirai_crdt::map::uw_map::UWMapLog<K, L>
            where
                K: Clone + std::fmt::Debug + std::hash::Hash + Eq + ToString,
                L: #path::IsLog + VertexSyncLog<Op = <L as #path::IsLog>::Op> + Default,
            {
                type Op = #path::UWMap<K, <L as #path::IsLog>::Op>;

                fn collect_vertex_effects(
                    &self,
                    op: &Self::Op,
                    event_id: &#path::EventId,
                    object_id: &#path::ObjectId,
                    refs: &#refs_value,
                    out: &mut std::vec::Vec<VertexGraphEffect>,
                ) {
                    match op {
                        #path::UWMap::Update(key, value) => {
                            let child_id = object_id.clone().map_entry(key.to_string());
                            if let Some(child) = self.children().get(key) {
                                VertexSyncLog::collect_vertex_effects(child, value, event_id, &child_id, refs, out);
                            } else {
                                VertexSyncLog::collect_vertex_effects(&L::default(), value, event_id, &child_id, refs, out);
                            }
                        }
                        #path::UWMap::Remove(key) => {
                            let child_id = object_id.clone().map_entry(key.to_string());
                            remove_instances_for_prefix(refs, &child_id, out);
                        }
                        #path::UWMap::Clear => {
                            remove_instances_for_prefix(refs, object_id, out);
                        }
                    }
                }
            }

            impl<L> VertexSyncLog for moirai_crdt::option::OptionLog<L>
            where
                L: #path::IsLog + VertexSyncLog<Op = <L as #path::IsLog>::Op> + Default,
            {
                type Op = #path::Optional<<L as #path::IsLog>::Op>;

                fn collect_vertex_effects(
                    &self,
                    op: &Self::Op,
                    event_id: &#path::EventId,
                    object_id: &#path::ObjectId,
                    refs: &#refs_value,
                    out: &mut std::vec::Vec<VertexGraphEffect>,
                ) {
                    match op {
                        #path::Optional::Set(value) => {
                            if let Some(child) = self.child() {
                                VertexSyncLog::collect_vertex_effects(child, value, event_id, object_id, refs, out);
                            } else {
                                VertexSyncLog::collect_vertex_effects(&L::default(), value, event_id, object_id, refs, out);
                            }
                        }
                        #path::Optional::Unset => {
                            remove_instances_for_prefix(refs, object_id, out);
                        }
                    }
                }
            }
        }
    }

    fn generate_to_instances_impls(&self) -> TokenStream {
        let support_path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let impls = self.reachable_class_indices.iter().map(|class_idx| {
            let class = &self.ctx.classes()[**class_idx];
            if class.is_abstract() {
                let union_name = format_ident!("{}", class.name().to_upper_camel_case());
                let arms = class.sub().iter().map(|sub_idx| {
                    let sub = &self.ctx.classes()[**sub_idx];
                    let variant = format_ident!("{}", sub.name().to_upper_camel_case());
                    quote! { #support_path::#union_name::#variant(inner) => ToInstances::to_instances(inner, object_id) }
                });
                quote! {
                    impl ToInstances for #support_path::#union_name {
                        fn to_instances(&self, object_id: #support_path::ObjectId) -> std::vec::Vec<#support_path::Instance> {
                            match self {
                                #(#arms,)*
                            }
                        }
                    }
                }
            } else {
                let op_name = format_ident!("{}", class.name().to_upper_camel_case());
                let instances = self.referenceable_class_chain(*class_idx).into_iter().map(|ref_idx| {
                    let ref_name = self.ctx.classes()[*ref_idx].name();
                    let id_ty = format_ident!("{}Id", ref_name);
                    let variant = format_ident!("{}Id", ref_name);
                    quote! {
                        #support_path::Instance::#variant(#support_path::#id_ty(object_id.clone()))
                    }
                });
                quote! {
                    impl ToInstances for #support_path::#op_name {
                        fn to_instances(&self, object_id: #support_path::ObjectId) -> std::vec::Vec<#support_path::Instance> {
                            std::vec![#(#instances),*]
                        }
                    }
                }
            }
        });

        quote! { #(#impls)* }
    }

    fn generate_vertex_effect_impls(&self) -> TokenStream {
        let support_path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let impls = self.reachable_class_indices.iter().map(|class_idx| {
            let class = &self.ctx.classes()[**class_idx];
            if class.is_abstract() {
                let union_name = format_ident!("{}", class.name().to_upper_camel_case());
                let log_name = format_ident!("{}Log", class.name().to_upper_camel_case());
                let child_enum = format_ident!("{}Child", class.name().to_upper_camel_case());
                let container_enum = format_ident!("{}Container", class.name().to_upper_camel_case());
                let feat_name =
                    format_ident!("{}{}", class.name().to_upper_camel_case(), INHERITANCE_SUFFIX);
                let feat_log_name = format_ident!(
                    "{}{}Log",
                    class.name().to_upper_camel_case(),
                    INHERITANCE_SUFFIX
                );
                let feat_inherited_arms = class.sup().iter().map(|super_idx| {
                    let super_class = &self.ctx.classes()[**super_idx];
                    let field_name_str =
                        format!("{}{}", super_class.name(), INHERITANCE_SUFFIX).to_snake_case();
                    let field_name = format_ident!("{}", field_name_str);
                    let variant = format_ident!("{}", field_name_str.to_upper_camel_case());
                    quote! {
                        #support_path::#feat_name::#variant(inner_op) => {
                            let child_id = object_id.clone().field(#field_name_str);
                            VertexSyncLog::collect_vertex_effects(self.#field_name(), inner_op, event_id, &child_id, refs, out);
                        }
                    }
                });
                let feat_containment_arms = class.structural().iter().filter(|feature| {
                    feature.kind == structural::Typ::EReference && feature.containment
                }).map(|feature| {
                    let field_name_str = feature.name.to_snake_case();
                    let field_name = format_ident!("{}", field_name_str);
                    let variant = format_ident!("{}", field_name_str.to_upper_camel_case());
                    quote! {
                        #support_path::#feat_name::#variant(inner_op) => {
                            let child_id = object_id.clone().field(#field_name_str);
                            VertexSyncLog::collect_vertex_effects(self.#field_name(), inner_op, event_id, &child_id, refs, out);
                        }
                    }
                });
                let arms = class.sub().iter().map(|sub_idx| {
                    let sub = &self.ctx.classes()[**sub_idx];
                    let variant = format_ident!("{}", sub.name().to_upper_camel_case());
                    let log_ty = format_ident!("{}Log", sub.name().to_upper_camel_case());
                    quote! {
                        (#support_path::#union_name::#variant(inner_op), #support_path::#container_enum::Unset) => {
                            let variant_id = object_id.clone().variant(stringify!(#variant));
                            VertexSyncLog::collect_vertex_effects(&#support_path::#log_ty::default(), inner_op, event_id, &variant_id, refs, out);
                        }
                        (#support_path::#union_name::#variant(inner_op), #support_path::#container_enum::Value(child)) => {
                            match child.as_ref() {
                                #support_path::#child_enum::#variant(log) => {
                                    let variant_id = object_id.clone().variant(stringify!(#variant));
                                    VertexSyncLog::collect_vertex_effects(log, inner_op, event_id, &variant_id, refs, out);
                                }
                                _ => {
                                    let variant_id = object_id.clone().variant(stringify!(#variant));
                                    VertexSyncLog::collect_vertex_effects(&#support_path::#log_ty::default(), inner_op, event_id, &variant_id, refs, out);
                                }
                            }
                        }
                        (#support_path::#union_name::#variant(inner_op), #support_path::#container_enum::Conflicts(children)) => {
                            let variant_id = object_id.clone().variant(stringify!(#variant));
                            if let Some(#support_path::#child_enum::#variant(log)) = children.iter().find(|child| matches!(child, #support_path::#child_enum::#variant(_))) {
                                VertexSyncLog::collect_vertex_effects(log, inner_op, event_id, &variant_id, refs, out);
                            } else {
                                VertexSyncLog::collect_vertex_effects(&#support_path::#log_ty::default(), inner_op, event_id, &variant_id, refs, out);
                            }
                        }
                    }
                });
                quote! {
                    impl VertexSyncLog for #support_path::#log_name {
                        type Op = #support_path::#union_name;

                        fn collect_vertex_effects(
                            &self,
                            op: &Self::Op,
                            event_id: &#support_path::EventId,
                            object_id: &#support_path::ObjectId,
                            refs: &<#support_path::ReferenceManager<#support_path::LwwPolicy> as #support_path::PureCRDT>::Value,
                            out: &mut std::vec::Vec<VertexGraphEffect>,
                        ) {
                            match (op, &self.child) {
                                #(#arms,)*
                            }
                        }
                    }

                    impl VertexSyncLog for #support_path::#feat_log_name {
                        type Op = #support_path::#feat_name;

                        fn collect_vertex_effects(
                            &self,
                            op: &Self::Op,
                            event_id: &#support_path::EventId,
                            object_id: &#support_path::ObjectId,
                            refs: &<#support_path::ReferenceManager<#support_path::LwwPolicy> as #support_path::PureCRDT>::Value,
                            out: &mut std::vec::Vec<VertexGraphEffect>,
                        ) {
                            match op {
                                #(#feat_inherited_arms,)*
                                #(#feat_containment_arms,)*
                                _ => {}
                            }
                        }
                    }
                }
            } else {
                let op_name = format_ident!("{}", class.name().to_upper_camel_case());
                let log_name = format_ident!("{}Log", class.name().to_upper_camel_case());

                let inherited_arms = class.sup().iter().map(|super_idx| {
                    let super_class = &self.ctx.classes()[**super_idx];
                    let field_name_str = format!("{}{}", super_class.name(), INHERITANCE_SUFFIX).to_snake_case();
                    let field_name = format_ident!("{}", field_name_str);
                    let variant = format_ident!("{}", field_name_str.to_upper_camel_case());
                    quote! {
                            #support_path::#op_name::#variant(inner_op) => {
                                let child_id = object_id.clone().field(#field_name_str);
                                VertexSyncLog::collect_vertex_effects(self.#field_name(), inner_op, event_id, &child_id, refs, out);
                            }
                        }
                    });

                let containment_arms = class.structural().iter().filter(|feature| {
                    feature.kind == structural::Typ::EReference && feature.containment
                }).map(|feature| {
                    let field_name_str = feature.name.to_snake_case();
                    let field_name = format_ident!("{}", field_name_str);
                    let variant = format_ident!("{}", field_name_str.to_upper_camel_case());
                    quote! {
                        #support_path::#op_name::#variant(inner_op) => {
                            let child_id = object_id.clone().field(#field_name_str);
                            VertexSyncLog::collect_vertex_effects(self.#field_name(), inner_op, event_id, &child_id, refs, out);
                        }
                    }
                });

                quote! {
                    impl VertexSyncLog for #support_path::#log_name {
                        type Op = #support_path::#op_name;

                        fn collect_vertex_effects(
                            &self,
                            op: &Self::Op,
                            event_id: &#support_path::EventId,
                            object_id: &#support_path::ObjectId,
                            refs: &<#support_path::ReferenceManager<#support_path::LwwPolicy> as #support_path::PureCRDT>::Value,
                            out: &mut std::vec::Vec<VertexGraphEffect>,
                        ) {
                            if #support_path::IsLog::is_default(self) {
                                out.extend(ToInstances::to_instances(op, object_id.clone()).into_iter().map(VertexGraphEffect::Add));
                            }

                            match op {
                                #support_path::#op_name::New => {}
                                #(#inherited_arms,)*
                                #(#containment_arms,)*
                                _ => {}
                            }
                        }
                    }
                }
            }
        });

        quote! { #(#impls)* }
    }

    fn generate_eval_nested_impl(&self) -> TokenStream {
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_log_name = format_ident!("{}Log", package_name.to_upper_camel_case());
        let package_value_name = format_ident!("{}Value", package_name.to_upper_camel_case());
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();

        let root_reads = self.roots().into_iter().map(|root| {
            let field_name = Ident::new(
                &self.root_class_name(root).to_snake_case(),
                Span::call_site(),
            );
            let log_field = self.root_field_ident(root);
            quote! { #field_name: self.#log_field.execute_query(#path::Read::new()) }
        });
        let refs_field = if self.has_references() {
            quote! { refs: self.reference_manager_log.execute_query(#path::Read::new()), }
        } else {
            quote! {}
        };

        quote! {
            impl #path::EvalNested<#path::Read<<Self as #path::IsLog>::Value>> for #package_log_name {
                fn execute_query(
                    &self,
                    _q: #path::Read<<Self as #path::IsLog>::Value>,
                ) -> <#path::Read<<Self as #path::IsLog>::Value> as #path::QueryOperation>::Response {
                    #package_value_name {
                        #(#root_reads,)*
                        #refs_field
                    }
                }
            }
        }
    }
}

impl<'a> Generate for PackageGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let package_enum = self.generate_package_enum();
        let package_value = self.generate_package_value_struct();
        let package_log = self.generate_package_log_struct();
        let reference_sync_support = self.generate_reference_sync_support();
        let is_log_impl = self.generate_is_log_impl();
        let eval_nested_impl = self.generate_eval_nested_impl();

        let tokens = quote! {
            #package_enum
            #package_value
            #package_log
            #reference_sync_support
            #is_log_impl
            #eval_nested_impl
        };

        Ok(Fragment::new(tokens, self.imports(), vec![]))
    }
}
