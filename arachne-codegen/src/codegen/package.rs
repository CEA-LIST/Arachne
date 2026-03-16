use ecore_rs::{ctx::Ctx, repr::idx};
use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::{
    PACKAGE_PATH_MOD,
    codegen::{
        cycles::CycleAnalysis,
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        import::{CrdtOp, Import, Log, NestedCrdtOp, Protocol},
        reference::{
            analysis::ReferenceAnalysis,
            containment::{ContainmentPath, PathStep, find_creation_paths},
        },
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
    ref_analysis: &'a ReferenceAnalysis,
    cycle_analysis: &'a CycleAnalysis,
}

impl<'a> PackageGenerator<'a> {
    pub fn new(
        ctx: &'a Ctx,
        pack_idx: idx::Pack,
        root_class_indices: Vec<idx::Class>,
        ref_analysis: &'a ReferenceAnalysis,
        cycle_analysis: &'a CycleAnalysis,
    ) -> Self {
        Self {
            ctx,
            pack_idx,
            root_class_indices,
            ref_analysis,
            cycle_analysis,
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
}

impl<'a> Generate for PackageGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let creation_specs = self
            .roots()
            .into_iter()
            .map(|root| {
                (
                    root,
                    find_creation_paths(
                        self.ctx,
                        root.class_idx,
                        self.ref_analysis,
                        self.cycle_analysis,
                    ),
                )
            })
            .collect::<Vec<_>>();

        let package_enum = self.generate_package_enum();
        let package_log = self.generate_package_log(&creation_specs);

        let tokens = quote! {
            #package_enum
            #package_log
        };

        Ok(Fragment::new(tokens, self.imports(), vec![]))
    }
}

impl<'a> PackageGenerator<'a> {
    fn has_references(&self) -> bool {
        self.ref_analysis.has_references()
    }

    fn imports(&self) -> Vec<Import> {
        let mut imports = vec![
            Import::CrdtOp(CrdtOp::Nested(NestedCrdtOp::ListOp)),
            Import::Protocol(Protocol::Read),
            Import::Protocol(Protocol::EvalNested),
            Import::Protocol(Protocol::IsLog),
            Import::Protocol(Protocol::Version),
            Import::Protocol(Protocol::Event),
            Import::Protocol(Protocol::QueryOperation),
            Import::Custom(String::from("crate::classifiers::*")),
        ];

        if self.has_references() {
            imports.extend([
                Import::Protocol(Protocol::LwwPolicy),
                Import::Protocol(Protocol::EventId),
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
        let reference_variant = if self.has_references() {
            quote! { , Reference(#path::Refs) }
        } else {
            quote! {}
        };

        quote! {
            #[derive(Debug, Clone)]
            pub enum #package_ident {
                #(#root_variants),*
                #reference_variant
            }
        }
    }

    fn generate_package_log(
        &self,
        creation_specs: &[(RootMeta, Vec<ContainmentPath>)],
    ) -> TokenStream {
        let package_value = self.generate_package_value_struct();
        let package_log_struct = self.generate_package_log_struct();
        let reference_sync_support = self.generate_reference_sync_support(creation_specs);
        let is_log_impl = self.generate_is_log_impl();
        let eval_nested_impl = self.generate_eval_nested_impl();

        quote! {
            #package_value
            #package_log_struct
            #reference_sync_support
            #is_log_impl
            #eval_nested_impl
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
                #package_ident::Reference(o) => self
                    .reference_manager_log
                    .is_enabled(&#path::ReferenceManager::AddArc(o.clone()))
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
                #package_ident::Reference(refs) => self
                    .reference_manager_log
                    .effect(#path::Event::unfold(event, #path::ReferenceManager::AddArc(refs)))
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

    fn generate_reference_sync_support(
        &self,
        creation_specs: &[(RootMeta, Vec<ContainmentPath>)],
    ) -> TokenStream {
        if !self.has_references() {
            return quote! {};
        }

        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let support_path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let package_ident = format_ident!("{}", package_name.to_upper_camel_case());
        let package_log_name = format_ident!("{}Log", package_name.to_upper_camel_case());
        let descriptor_name =
            format_ident!("{}VertexSyncDescriptor", package_name.to_upper_camel_case());

        let root_bootstraps = creation_specs.iter().map(|(root, _)| {
            let root_class_name = self.root_class_name(*root);
            let class_name = self.root_variant_ident(*root);
            let class_log_name = self.root_field_ident(*root);
            let root_is_referenceable = self.ref_analysis.referenceable_classes.contains(&root.class_idx);
            if root_is_referenceable {
                let root_id_struct = format_ident!("{}Id", root_class_name);
                let root_instance_variant = format_ident!("{}Id", root_class_name);
                quote! {
                    if #support_path::IsLog::is_default(&self.#class_log_name)
                        && matches!(event.op(), #package_ident::#class_name(_))
                    {
                        let id = #support_path::#root_id_struct(event.id().clone());
                        let new_vertex = #support_path::ReferenceManager::<#support_path::LwwPolicy>::AddVertex {
                            id: #support_path::Instance::#root_instance_variant(id),
                        };
                        #support_path::IsLog::effect(
                            &mut self.reference_manager_log,
                            #support_path::Event::unfold(event.clone(), new_vertex)
                        );
                    }
                }
            } else {
                quote! {}
            }
        });

        let mut descriptor_entries = Vec::new();
        let mut helper_fns = Vec::new();
        for (root_ord, (root, paths)) in creation_specs.iter().enumerate() {
            for (path_ord, path) in paths.iter().enumerate() {
                let matches_create_fn = format_ident!("matches_create_{}_{}", root_ord, path_ord);
                let should_create_fn = format_ident!("should_create_{}_{}", root_ord, path_ord);
                let make_instance_fn = format_ident!("make_instance_{}_{}", root_ord, path_ord);
                let lookup_deleted_id_fn =
                    format_ident!("lookup_deleted_id_{}_{}", root_ord, path_ord);

                descriptor_entries.push(quote! {
                    #descriptor_name {
                        matches_create: Self::#matches_create_fn,
                        should_create: Self::#should_create_fn,
                        make_instance: Self::#make_instance_fn,
                        lookup_deleted_id: Self::#lookup_deleted_id_fn,
                    }
                });

                helper_fns.extend(
                    self.generate_descriptor_helpers(root_ord, path_ord, *root, path)
                        .into_iter(),
                );
            }
        }

        quote! {
            #[derive(Clone, Copy)]
            struct #descriptor_name {
                matches_create: fn(&#package_ident) -> bool,
                should_create: fn(&#package_log_name, &#package_ident) -> bool,
                make_instance: fn(#support_path::EventId) -> #support_path::Instance,
                lookup_deleted_id: fn(&#package_log_name, &#package_ident) -> Option<#support_path::EventId>,
            }

            impl #package_log_name {
                fn vertex_sync_descriptors() -> &'static [#descriptor_name] {
                    &[#(#descriptor_entries),*]
                }

                fn sync_reference_vertices(&mut self, event: &#support_path::Event<#package_ident>) {
                    #(#root_bootstraps)*

                    for descriptor in Self::vertex_sync_descriptors() {
                        if (descriptor.matches_create)(event.op())
                            && (descriptor.should_create)(self, event.op())
                        {
                            let new_vertex = #support_path::ReferenceManager::<#support_path::LwwPolicy>::AddVertex {
                                id: (descriptor.make_instance)(event.id().clone()),
                            };
                            #support_path::IsLog::effect(&mut self.reference_manager_log, #support_path::Event::unfold(event.clone(), new_vertex));
                        }

                        if let Some(event_id) = (descriptor.lookup_deleted_id)(self, event.op()) {
                            let remove_vertex = #support_path::ReferenceManager::<#support_path::LwwPolicy>::RemoveVertex {
                                id: (descriptor.make_instance)(event_id),
                            };
                            #support_path::IsLog::effect(&mut self.reference_manager_log, #support_path::Event::unfold(event.clone(), remove_vertex));
                        }
                    }
                }

                #(#helper_fns)*
            }
        }
    }

    fn generate_descriptor_helpers(
        &self,
        root_ord: usize,
        path_ord: usize,
        root: RootMeta,
        containment_path: &ContainmentPath,
    ) -> Vec<TokenStream> {
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_ident = format_ident!("{}", package_name.to_upper_camel_case());
        let root_class_ident = self.root_variant_ident(root);
        let class_log_name = self.root_field_ident(root);
        let support_path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();

        let matches_create_fn = format_ident!("matches_create_{}_{}", root_ord, path_ord);
        let should_create_fn = format_ident!("should_create_{}_{}", root_ord, path_ord);
        let make_instance_fn = format_ident!("make_instance_{}_{}", root_ord, path_ord);
        let lookup_deleted_id_fn = format_ident!("lookup_deleted_id_{}_{}", root_ord, path_ord);

        let vertex_class = &self.ctx.classes()[*containment_path.vertex_class];
        let id_struct = format_ident!("{}Id", vertex_class.name());
        let instance_variant = format_ident!("{}Id", vertex_class.name());

        let create_matcher = if let Some(box_idx) = containment_path
            .steps
            .iter()
            .position(|step| matches!(step, PathStep::Field { is_boxed: true, .. }))
        {
            let outer_steps = containment_path.steps[..=box_idx].to_vec();
            let inner_steps = &containment_path.steps[box_idx + 1..];
            let outer_pattern = self.build_nested_pattern_with_capture(&outer_steps, "inner_val");
            let inner_pattern = self.build_nested_pattern(inner_steps);
            let inner_var = format_ident!("inner_val");
            let captured_value = match outer_steps.last() {
                Some(PathStep::Field { is_boxed: true, .. }) => quote! { #inner_var.as_ref() },
                _ => quote! { #inner_var },
            };

            quote! {
                fn #matches_create_fn(op: &#package_ident) -> bool {
                    match op {
                        #package_ident::#root_class_ident(#outer_pattern) => {
                            matches!(#captured_value, #inner_pattern)
                        }
                        _ => false,
                    }
                }
            }
        } else {
            let pattern = self.build_nested_pattern(&containment_path.steps);
            quote! {
                fn #matches_create_fn(op: &#package_ident) -> bool {
                    matches!(op, #package_ident::#root_class_ident(#pattern))
                }
            }
        };

        let make_instance = quote! {
            fn #make_instance_fn(event_id: #support_path::EventId) -> #support_path::Instance {
                #support_path::Instance::#instance_variant(#support_path::#id_struct(event_id))
            }
        };

        let log_path = self.build_log_field_path(&containment_path.log_field_path);
        let should_create = if matches!(containment_path.steps.last(), Some(PathStep::ListInsert)) {
            quote! {
                fn #should_create_fn(&self, _op: &#package_ident) -> bool {
                    true
                }
            }
        } else {
            quote! {
                fn #should_create_fn(&self, _op: &#package_ident) -> bool {
                    self
                        .#class_log_name()
                        #log_path
                        .__id()
                        .is_none()
                }
            }
        };

        let is_list_path = matches!(containment_path.steps.last(), Some(PathStep::ListInsert));
        let lookup_deleted_id = if !is_list_path
            || containment_path
                .steps
                .iter()
                .any(|step| matches!(step, PathStep::Field { is_boxed: true, .. }))
        {
            quote! {
                fn #lookup_deleted_id_fn(&self, _op: &#package_ident) -> Option<#support_path::EventId> {
                    None
                }
            }
        } else {
            let delete_steps: Vec<PathStep> = containment_path
                .steps
                .iter()
                .map(|step| match step {
                    PathStep::ListInsert => PathStep::ListDelete,
                    other => other.clone(),
                })
                .collect();
            let pattern = self.build_nested_pattern(&delete_steps);
            quote! {
                fn #lookup_deleted_id_fn(&self, op: &#package_ident) -> Option<#support_path::EventId> {
                    match op {
                        #package_ident::#root_class_ident(#pattern) => {
                            let positions = #support_path::EvalNested::execute_query(
                                self.#class_log_name() #log_path .positions(),
                                #support_path::Read::new(),
                            );
                            Some(positions[*pos].clone())
                        }
                        _ => None,
                    }
                }
            }
        };

        vec![
            create_matcher,
            should_create,
            make_instance,
            lookup_deleted_id,
        ]
    }

    fn build_nested_pattern(&self, steps: &[PathStep]) -> TokenStream {
        if steps.is_empty() {
            return quote! { _ };
        }

        let mut pattern = self.build_leaf_pattern(steps.last().unwrap());
        for step in steps.iter().rev().skip(1) {
            pattern = self.wrap_step_pattern(step, pattern);
        }
        pattern
    }

    fn build_nested_pattern_with_capture(
        &self,
        steps: &[PathStep],
        capture_name: &str,
    ) -> TokenStream {
        if steps.is_empty() {
            let var = Ident::new(capture_name, Span::call_site());
            return quote! { #var };
        }

        let var = Ident::new(capture_name, Span::call_site());
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();

        let mut pattern = match steps.last().unwrap() {
            PathStep::Field {
                class_name,
                variant_name,
                ..
            } => {
                let class = Ident::new(class_name, Span::call_site());
                let variant = Ident::new(variant_name, Span::call_site());
                quote! { #path::#class::#variant(#var) }
            }
            PathStep::Variant {
                union_name,
                variant_name,
            } => {
                let union_n = Ident::new(union_name, Span::call_site());
                let variant = Ident::new(variant_name, Span::call_site());
                quote! { #path::#union_n::#variant(#var) }
            }
            _ => quote! { #var },
        };

        for step in steps.iter().rev().skip(1) {
            pattern = self.wrap_step_pattern(step, pattern);
        }

        pattern
    }

    fn build_leaf_pattern(&self, step: &PathStep) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        match step {
            PathStep::ListInsert => quote! { #path::List::Insert { .. } },
            PathStep::ListDelete => quote! { #path::List::Delete { pos } },
            PathStep::Field {
                class_name,
                variant_name,
                ..
            } => {
                let class = Ident::new(class_name, Span::call_site());
                let variant = Ident::new(variant_name, Span::call_site());
                quote! { #path::#class::#variant(..) }
            }
            PathStep::Variant {
                union_name,
                variant_name,
            } => {
                let union_n = Ident::new(union_name, Span::call_site());
                let variant = Ident::new(variant_name, Span::call_site());
                quote! { #path::#union_n::#variant(..) }
            }
        }
    }

    fn wrap_step_pattern(&self, step: &PathStep, inner: TokenStream) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        match step {
            PathStep::Field {
                class_name,
                variant_name,
                ..
            } => {
                let class = Ident::new(class_name, Span::call_site());
                let variant = Ident::new(variant_name, Span::call_site());
                quote! { #path::#class::#variant(#inner) }
            }
            PathStep::Variant {
                union_name,
                variant_name,
            } => {
                let union_n = Ident::new(union_name, Span::call_site());
                let variant = Ident::new(variant_name, Span::call_site());
                quote! { #path::#union_n::#variant(#inner) }
            }
            PathStep::ListInsert => quote! { #path::List::Insert { .. } },
            PathStep::ListDelete => quote! { #path::List::Delete { pos } },
        }
    }

    fn build_log_field_path(&self, path: &[String]) -> TokenStream {
        let fields: Vec<Ident> = path
            .iter()
            .map(|f| Ident::new(f, Span::call_site()))
            .collect();

        quote! { #(.#fields())* }
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
