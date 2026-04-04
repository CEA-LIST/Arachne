use ecore_rs::{ctx::Ctx, repr::idx};
use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::{
    PACKAGE_PATH_MOD,
    codegen::{
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        import::{Import, Log, Protocol},
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
    ref_analysis: &'a ReferenceAnalysis,
}

impl<'a> PackageGenerator<'a> {
    pub fn new(
        ctx: &'a Ctx,
        pack_idx: idx::Pack,
        root_class_indices: Vec<idx::Class>,
        ref_analysis: &'a ReferenceAnalysis,
    ) -> Self {
        Self {
            ctx,
            pack_idx,
            root_class_indices,
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
            Import::Protocol(Protocol::Read),
            Import::Protocol(Protocol::EvalNested),
            Import::Protocol(Protocol::IsLog),
            Import::Protocol(Protocol::Version),
            Import::Protocol(Protocol::Event),
            Import::Protocol(Protocol::QueryOperation),
            Import::Protocol(Protocol::ObjectPath),
            Import::Protocol(Protocol::SinkEffect),
            Import::Protocol(Protocol::Interner),
            Import::Protocol(Protocol::InternalizeOp),
            Import::Log(Log::PartiallyOrdered),
            Import::Custom("crate::classifiers::*"),
        ];

        if self.has_references() {
            imports.extend([
                Import::Protocol(Protocol::FairPolicy),
                Import::Log(Log::Vec),
                Import::Protocol(Protocol::PureCRDT),
                Import::Protocol(Protocol::SinkCollector),
                Import::Custom("crate::references::*"),
                Import::Custom("crate::classifiers::*"),
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
                pub refs: <#path::ReferenceManager<#path::FairPolicy> as #path::PureCRDT>::Value,
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
                reference_manager_log: #path::VecLog<#path::ReferenceManager<#path::FairPolicy>>,
            }
        } else {
            quote! {}
        };
        let reference_getter = if self.has_references() {
            quote! {
                pub fn reference_manager_log(&self) -> &#path::VecLog<#path::ReferenceManager<#path::FairPolicy>> {
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
        let root_variants = self.roots().into_iter().map(|root| {
            let variant = self.root_variant_ident(root);
            let log_field = self.root_field_ident(root);
            let field_stringify = self.root_class_name(root).to_snake_case();
            if self.has_references() {
                quote! { #package_ident::#variant(o) =>
                #path::IsLog::effect(&mut self.#log_field, #path::Event::unfold(event.clone(), o), #path::ObjectPath::new(#package_name).field(#field_stringify), &mut sink),
                }
            } else {
                quote! {
                    #package_ident::#variant(o) => self.#log_field.effect(
                        #path::Event::unfold(event.clone(), o),
                        __package::ObjectPath::new(#package_name),
                        &mut __package::SinkCollector::new(),
                    ),
                }
            }
        });

        let effect = if self.has_references() {
            quote! {
            let mut sink = #path::SinkCollector::new();
                match event.op().clone() {
                    #(#root_variants)*
                    #package_ident::AddReference(o) =>
                        self.reference_manager_log.effect(
                            #path::Event::unfold(event.clone(), #path::ReferenceManager::AddArc(o)),
                            __package::ObjectPath::new(#package_name),
                            &mut __package::SinkCollector::new(),
                        ),
                    #package_ident::RemoveReference(o) =>
                        self.reference_manager_log.effect(
                            #path::Event::unfold(event.clone(), #path::ReferenceManager::RemoveArc(o)),
                            __package::ObjectPath::new(#package_name),
                            &mut __package::SinkCollector::new(),
                        ),
                }
                for sink in sink.into_sinks() {
                    match sink.effect() {
                        #path::SinkEffect::Create | #path::SinkEffect::Update => {
                            let vertex_ops = #path::instance_from_path(sink.path())
                                .map(|instance| #path::ReferenceManager::AddVertex { id: instance });
                            if let Some(o) = vertex_ops {
                                self.reference_manager_log.effect(
                                    #path::Event::unfold(event.clone(), o),
                                    __package::ObjectPath::new(#package_name),
                                    &mut __package::SinkCollector::new(),
                                );
                            }
                        }
                        #path::SinkEffect::Delete => {
                            self.reference_manager_log.effect(__package::Event::unfold(
                                event.clone(),
                                __package::ReferenceManager::DeleteSubtree {
                                    prefix: sink.path().clone(),
                                }),
                                __package::ObjectPath::new(#package_name),
                                &mut __package::SinkCollector::new(),
                            );
                        }
                    }
                }
            }
        } else {
            quote! {
                match event.op().clone() {
                    #(#root_variants)*
                }
            }
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

                fn effect(&mut self, event: #path::Event<Self::Op>, _path: #path::ObjectPath, _sink: &mut #path::SinkCollector) {
                    #effect
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

    fn translate_ids_impl(&self) -> TokenStream {
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_ident = format_ident!("{}", package_name.to_upper_camel_case());
        let translate_root_arms = self.roots().into_iter().map(|root| {
            let variant = self.root_variant_ident(root);
            quote! { #package_ident::#variant(op) => #package_ident::#variant(op.clone()) }
        });
        let translate_ref_arms = if self.has_references() {
            quote! {
                #package_ident::AddReference(op) => {
                    #package_ident::AddReference(op.internalize(interner))
                }
                #package_ident::RemoveReference(op) => {
                    #package_ident::RemoveReference(op.internalize(interner))
                }
            }
        } else {
            quote! {}
        };

        quote! {
            impl __package::InternalizeOp for #package_ident {
                fn internalize(self, interner: &__package::Interner) -> Self {
                    match self {
                        #(#translate_root_arms,)*
                        #translate_ref_arms
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
        let is_log_impl = self.generate_is_log_impl();
        let eval_nested_impl = self.generate_eval_nested_impl();
        let translate_ids = self.translate_ids_impl();

        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let ref_manager_log = if self.has_references() {
            quote! {
                pub type ReferenceManagerLog = #path::POLog<#path::ReferenceManager<#path::FairPolicy>, #path::ReferenceManagerState<#path::FairPolicy>>;
            }
        } else {
            quote! {}
        };

        let tokens = quote! {
            #ref_manager_log

            #package_enum
            #package_value
            #package_log
            #is_log_impl
            #eval_nested_impl
            #translate_ids
        };

        Ok(Fragment::new(tokens, self.imports(), vec![]))
    }
}
