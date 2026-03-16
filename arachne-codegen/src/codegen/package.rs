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

pub struct PackageGenerator<'a> {
    ctx: &'a Ctx,
    pack_idx: idx::Pack,
    root_class_idx: idx::Class,
    ref_analysis: &'a ReferenceAnalysis,
    cycle_analysis: &'a CycleAnalysis,
}

impl<'a> PackageGenerator<'a> {
    pub fn new(
        ctx: &'a Ctx,
        pack_idx: idx::Pack,
        root_class_idx: idx::Class,
        ref_analysis: &'a ReferenceAnalysis,
        cycle_analysis: &'a CycleAnalysis,
    ) -> Self {
        Self {
            ctx,
            pack_idx,
            root_class_idx,
            ref_analysis,
            cycle_analysis,
        }
    }
}

impl<'a> Generate for PackageGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        // Find creation paths
        let creation_paths = find_creation_paths(
            self.ctx,
            self.root_class_idx,
            self.ref_analysis,
            self.cycle_analysis,
        );

        let package_enum = self.generate_package_enum();
        let package_log = self.generate_package_log(&creation_paths);

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

    /// Generate the top-level package enum.
    fn generate_package_enum(&self) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let root_class_name = self.ctx.classes()[*self.root_class_idx].name();
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let root_ident = Ident::new(&root_class_name.to_upper_camel_case(), Span::call_site());
        let package_ident = format_ident!("{}", package_name.to_upper_camel_case());
        let reference_variant = if self.has_references() {
            quote! { , Reference(#path::Refs) }
        } else {
            quote! {}
        };

        quote! {
            #[derive(Debug, Clone)]
            pub enum #package_ident {
                #root_ident(#path::#root_ident)
                #reference_variant
            }
        }
    }

    fn generate_package_log(&self, creation_paths: &[ContainmentPath]) -> TokenStream {
        let package_log_struct = self.generate_package_log_struct();
        let reference_sync_support = self.generate_reference_sync_support(creation_paths);

        let is_log_impl = self.generate_is_log_impl(creation_paths);
        let eval_nested_impl = self.generate_eval_nested_impl();

        quote! {
            #package_log_struct
            #reference_sync_support

            #is_log_impl

            #eval_nested_impl
        }
    }

    /// Generate the package log struct.
    fn generate_package_log_struct(&self) -> TokenStream {
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let root_class_name = self.ctx.classes()[*self.root_class_idx].name();

        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let class_log = format_ident!("{}Log", root_class_name);
        let class_log_name = format_ident!("{}_log", root_class_name.to_snake_case());
        let package_log_name = format_ident!("{}Log", package_name.to_upper_camel_case());
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
                #class_log_name: #path::#class_log,
                #reference_field
            }

            impl #package_log_name {
                pub fn #class_log_name(&self) -> &#path::#class_log {
                    &self.#class_log_name
                }

                #reference_getter
            }
        }
    }

    fn generate_is_log_impl(&self, _creation_paths: &[ContainmentPath]) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let root_class_name = self.ctx.classes()[*self.root_class_idx].name();
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();

        let root_value = format_ident!("{}Value", root_class_name);
        let class_log_name = format_ident!("{}_log", root_class_name.to_snake_case());
        let class_name = format_ident!("{}", root_class_name.to_upper_camel_case());
        let package_log_name = format_ident!("{}Log", package_name.to_upper_camel_case());
        let package_ident = format_ident!("{}", package_name.to_upper_camel_case());
        let value_ty = if self.has_references() {
            quote! {
                (#path::#root_value, <#path::ReferenceManager<#path::LwwPolicy> as #path::PureCRDT>::Value)
            }
        } else {
            quote! { #path::#root_value }
        };
        let reference_is_enabled = if self.has_references() {
            quote! {
                #package_ident::Reference(o) => self
                    .reference_manager_log
                    .is_enabled(&#path::ReferenceManager::AddArc(o.clone())),
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
                    .effect(#path::Event::unfold(event, #path::ReferenceManager::AddArc(refs))),
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
            quote! {
                self.reference_manager_log
                    .redundant_by_parent(version, conservative);
            }
        } else {
            quote! {}
        };

        quote! {
            impl #path::IsLog for #package_log_name {
                type Value = #value_ty;
                type Op = #package_ident;

                fn is_enabled(&self, op: &Self::Op) -> bool {
                    match op {
                        #package_ident::#class_name(o) => self.#class_log_name.is_enabled(o),
                        #reference_is_enabled
                    }
                }

                fn effect(&mut self, event: #path::Event<Self::Op>) {
                    #pre_effect

                    match event.op().clone() {
                        #package_ident::#class_name(root) => self.#class_log_name.effect(#path::Event::unfold(event, root)),
                        #reference_effect
                    }
                }

                fn stabilize(&mut self, version: &#path::Version) {
                    self.#class_log_name.stabilize(version);
                    #stabilize_refs
                }

                fn redundant_by_parent(&mut self, version: &#path::Version, conservative: bool) {
                    self.#class_log_name.redundant_by_parent(version, conservative);
                    #redundant_refs
                }

                fn is_default(&self) -> bool {
                    self.#class_log_name.is_default()
                }
            }
        }
    }

    fn generate_reference_sync_support(&self, creation_paths: &[ContainmentPath]) -> TokenStream {
        if !self.has_references() {
            return quote! {};
        }

        let root_class_name = self.ctx.classes()[*self.root_class_idx].name();
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let support_path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();

        let class_name = format_ident!("{}", root_class_name.to_upper_camel_case());
        let package_ident = format_ident!("{}", package_name.to_upper_camel_case());
        let package_log_name = format_ident!("{}Log", package_name.to_upper_camel_case());
        let descriptor_name =
            format_ident!("{}VertexSyncDescriptor", package_name.to_upper_camel_case());
        let class_log_name = format_ident!("{}_log", root_class_name.to_snake_case());

        let root_is_referenceable = self
            .ref_analysis
            .referenceable_classes
            .contains(&self.root_class_idx);
        let root_bootstrap = if root_is_referenceable {
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
        };

        let descriptor_entries: Vec<TokenStream> = creation_paths
            .iter()
            .enumerate()
            .map(|(index, _path)| {
                let matches_create_fn = format_ident!("matches_create_{}", index);
                let should_create_fn = format_ident!("should_create_{}", index);
                let make_instance_fn = format_ident!("make_instance_{}", index);
                let lookup_deleted_id_fn = format_ident!("lookup_deleted_id_{}", index);

                quote! {
                    #descriptor_name {
                        matches_create: Self::#matches_create_fn,
                        should_create: Self::#should_create_fn,
                        make_instance: Self::#make_instance_fn,
                        lookup_deleted_id: Self::#lookup_deleted_id_fn,
                    }
                }
            })
            .collect();

        let helper_fns: Vec<TokenStream> = creation_paths
            .iter()
            .enumerate()
            .flat_map(|(index, path)| self.generate_descriptor_helpers(index, path))
            .collect();

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
                    #root_bootstrap

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
                            let remove_vertex =
                                #support_path::ReferenceManager::<#support_path::LwwPolicy>::RemoveVertex {
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
        index: usize,
        containment_path: &ContainmentPath,
    ) -> Vec<TokenStream> {
        let root_class_name = self.ctx.classes()[*self.root_class_idx].name();
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_ident = format_ident!("{}", package_name.to_upper_camel_case());
        let root_class_ident = format_ident!("{}", root_class_name.to_upper_camel_case());
        let class_log_name = format_ident!("{}_log", root_class_name.to_snake_case());
        let support_path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();

        let matches_create_fn = format_ident!("matches_create_{}", index);
        let should_create_fn = format_ident!("should_create_{}", index);
        let make_instance_fn = format_ident!("make_instance_{}", index);
        let lookup_deleted_id_fn = format_ident!("lookup_deleted_id_{}", index);

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
                                self
                                    .#class_log_name()
                                    #log_path
                                    .positions(),
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

    /// Build a fully nested match pattern from path steps.
    /// The innermost binding is `..` for Insert or `{ pos }` for Delete.
    fn build_nested_pattern(&self, steps: &[PathStep]) -> TokenStream {
        if steps.is_empty() {
            return quote! { _ };
        }

        // Build from the inside out (right to left)
        let mut pattern = self.build_leaf_pattern(steps.last().unwrap());

        for step in steps.iter().rev().skip(1) {
            pattern = self.wrap_step_pattern(step, pattern);
        }

        pattern
    }

    /// Build a nested match pattern, but capture a variable at the last step
    /// instead of continuing the nesting.
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

        // Last step captures the variable
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

        // Wrap remaining steps from inside out
        for step in steps.iter().rev().skip(1) {
            pattern = self.wrap_step_pattern(step, pattern);
        }

        pattern
    }

    /// Build the leaf pattern for the innermost step.
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

    /// Wrap an inner pattern in an outer step.
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

    /// Build the log field path for accessing position data during deletion.
    fn build_log_field_path(&self, path: &[String]) -> TokenStream {
        let fields: Vec<Ident> = path
            .iter()
            .map(|f| Ident::new(f, Span::call_site()))
            .collect();

        quote! { #(.#fields())* }
    }

    fn generate_eval_nested_impl(&self) -> TokenStream {
        let class_name = self.ctx.classes()[*self.root_class_idx].name();
        let class_log_name = format_ident!("{}_log", class_name.to_snake_case());
        let class_name = format_ident!("{}", class_name.to_snake_case());
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_log_name = format_ident!("{}Log", package_name.to_upper_camel_case());

        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let query_body = if self.has_references() {
            quote! {
                let #class_name = self.#class_log_name.execute_query(#path::Read::new());
                let refs = self.reference_manager_log.execute_query(#path::Read::new());
                (#class_name, refs)
            }
        } else {
            quote! { self.#class_log_name.execute_query(#path::Read::new()) }
        };

        quote! {
            impl #path::EvalNested<#path::Read<<Self as #path::IsLog>::Value>> for #package_log_name {
                fn execute_query(
                    &self,
                    _q: #path::Read<<Self as #path::IsLog>::Value>,
                ) -> <#path::Read<<Self as #path::IsLog>::Value> as #path::QueryOperation>::Response {
                    #query_body
                }
            }
        }
    }
}
