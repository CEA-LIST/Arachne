use ecore_rs::{ctx::Ctx, repr::idx};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::{
    CLASSIFIERS_PATH_MOD, PACKAGE_PATH_MOD,
    codegen::{
        classifier::{
            has_codegen_polymorphic_family, polymorphic_kind_ident, polymorphic_kind_log_ident,
        },
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        ident::{
            classifier_type_ident_with_suffix, type_ident, type_ident_with_suffix, value_ident,
            value_ident_with_suffix,
        },
        import::{Import, Log, Protocol},
        reference::analysis::ReferenceAnalysis,
    },
};

#[derive(Clone, Copy)]
struct RootMeta {
    class_idx: idx::Class,
}

fn classifiers_path() -> syn::Path {
    syn::parse_str(&format!("crate::{CLASSIFIERS_PATH_MOD}"))
        .expect("generated classifiers module path should be valid")
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

    fn root_class(&self, root: RootMeta) -> &ecore_rs::repr::Class {
        &self.ctx.classes()[*root.class_idx]
    }

    fn root_variant_ident(&self, root: RootMeta) -> Ident {
        polymorphic_kind_ident(self.ctx, self.root_class(root))
    }

    fn root_log_ident(&self, root: RootMeta) -> Ident {
        polymorphic_kind_log_ident(self.ctx, self.root_class(root))
    }

    fn root_value_ident(&self, root: RootMeta) -> Ident {
        let class = self.root_class(root);
        if has_codegen_polymorphic_family(self.ctx, class) {
            classifier_type_ident_with_suffix(self.ctx, class, "KindValue")
        } else {
            classifier_type_ident_with_suffix(self.ctx, class, "Value")
        }
    }

    fn root_field_ident(&self, root: RootMeta) -> Ident {
        value_ident_with_suffix(self.root_class_name(root), "log")
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
            Import::Custom("moirai_protocol::event::Event as ProtocolEvent"),
            Import::Protocol(Protocol::QueryOperation),
            Import::Protocol(Protocol::SinkEffect),
            Import::Protocol(Protocol::EffectContext),
            Import::Protocol(Protocol::SinkCollector),
        ];

        if self.has_references() {
            imports.extend([
                Import::Protocol(Protocol::FairPolicy),
                Import::Log(Log::Vec),
                Import::Custom("petgraph::graph::DiGraph"),
                Import::Custom("crate::references::*"),
            ]);
        }

        imports
    }

    fn generate_package_enum(&self) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_ident = type_ident(package_name);
        let classifiers = classifiers_path();
        let root_variants = self.roots().into_iter().map(|root| {
            let variant = self.root_variant_ident(root);
            quote! { #variant(#classifiers::#variant) }
        });
        let reference_variants = if self.has_references() {
            quote! { , AddReference(#path::Refs), RemoveReference(#path::Refs) }
        } else {
            quote! {}
        };

        quote! {
            #[derive(Debug, Clone)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
            pub enum #package_ident {
                #(#root_variants),*
                #reference_variants
            }
        }
    }

    fn generate_package_rejection_enum(&self) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_rejection_name = type_ident_with_suffix(package_name, "Rejection");
        let reference_log_ty = quote! { #path::VecLog<#path::ReferenceManager<#path::FairPolicy>> };
        let classifiers = classifiers_path();

        let root_variants = self.roots().into_iter().map(|root| {
            let variant = self.root_variant_ident(root);
            let log_ty = self.root_log_ident(root);
            quote! {
                #variant(<#classifiers::#log_ty as #path::IsLog>::Rejection)
            }
        });
        let root_display_arms = self.roots().into_iter().map(|root| {
            let variant = self.root_variant_ident(root);
            let label = variant.to_string();
            quote! {
                Self::#variant(error) => write!(f, "{}: {}", #label, error)
            }
        });
        let reference_variants = if self.has_references() {
            quote! {
                AddReference(<#reference_log_ty as #path::IsLog>::Rejection),
                RemoveReference(<#reference_log_ty as #path::IsLog>::Rejection),
            }
        } else {
            quote! {}
        };
        let reference_display_arms = if self.has_references() {
            quote! {
                Self::AddReference(error) => write!(f, "AddReference: {}", error),
                Self::RemoveReference(error) => write!(f, "RemoveReference: {}", error),
            }
        } else {
            quote! {}
        };

        quote! {
            #[derive(Debug)]
            pub enum #package_rejection_name {
                #(#root_variants,)*
                #reference_variants
            }

            impl std::fmt::Display for #package_rejection_name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match self {
                        #(#root_display_arms,)*
                        #reference_display_arms
                    }
                }
            }
        }
    }

    fn generate_package_value_struct(&self) -> TokenStream {
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_value_name = type_ident_with_suffix(package_name, "Value");
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let classifiers = classifiers_path();
        let root_fields = self.roots().into_iter().map(|root| {
            let field = value_ident(self.root_class_name(root));
            let value_ty = self.root_value_ident(root);
            quote! { pub #field: #classifiers::#value_ty }
        });
        let refs_field = if self.has_references() {
            // petgraph::Graph doesn't implement Serialize/Deserialize,
            // so we skip the refs field during serde and default it on deserialization.
            // v0.7 took the `Value` associated type off the CRDT trait, so the
            // reference manager's read-out is named outright: the typed graph
            // macro reads a `VecLog<ReferenceManager<P>>` as a
            // `DiGraph<Instance, Ref>` (`moirai-macros/src/typed_graph.rs`),
            // over the vertex and edge kinds `reference/mod.rs` always names.
            quote! {
                #[cfg_attr(feature = "serde", serde(skip))]
                pub refs: #path::DiGraph<#path::Instance, #path::Ref>,
            }
        } else {
            quote! {}
        };

        quote! {
            #[derive(Debug, Clone, Default)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
            pub struct #package_value_name {
                #(#root_fields,)*
                #refs_field
            }
        }
    }

    fn generate_package_log_struct(&self) -> TokenStream {
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_log_name = type_ident_with_suffix(package_name, "Log");
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();
        let classifiers = classifiers_path();

        let root_fields = self.roots().into_iter().map(|root| {
            let field = self.root_field_ident(root);
            let log_ty = self.root_log_ident(root);
            quote! { #field: #classifiers::#log_ty }
        });
        let root_getters = self.roots().into_iter().map(|root| {
            let field = self.root_field_ident(root);
            let log_ty = self.root_log_ident(root);
            quote! {
                pub fn #field(&self) -> &#classifiers::#log_ty {
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
            #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
        let package_log_name = type_ident_with_suffix(package_name, "Log");
        let package_ident = type_ident(package_name);
        let package_rejection_name = type_ident_with_suffix(package_name, "Rejection");

        let enabled_root_arms = self.roots().into_iter().map(|root| {
            let variant = self.root_variant_ident(root);
            let field = self.root_field_ident(root);
            quote! {
                #package_ident::#variant(o) => self
                    .#field
                    .is_enabled(o)
                    .map_err(#package_rejection_name::#variant)
            }
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
        let reference_default_check = if self.has_references() {
            quote! { self.reference_manager_log.is_default() }
        } else {
            quote! { true }
        };

        let reference_is_enabled = if self.has_references() {
            quote! {
                #package_ident::AddReference(o) => self
                    .reference_manager_log
                    .is_enabled(&#path::ReferenceManager::AddArc(o.clone()))
                    .map_err(#package_rejection_name::AddReference),
                #package_ident::RemoveReference(o) => self
                    .reference_manager_log
                    .is_enabled(&#path::ReferenceManager::RemoveArc(o.clone()))
                    .map_err(#package_rejection_name::RemoveReference),
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
            let field_stringify = value_ident(self.root_class_name(root)).to_string();
            quote! {
                #package_ident::#variant(o) => {
                    let child_event = #path::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(#field_stringify, |ctx| {
                        self.#log_field.effect(child_event, ctx);
                    });
                }
            }
        });

        // Package generation ignores the parent EffectContext and always creates a new root context
        // This is because the package log is the top-level log and should not be nested within another context.

        let effect = if self.has_references() {
            quote! {
                let mut sink = #path::SinkCollector::new();
                {
                    let mut ctx = #path::EffectContext::root(#package_name, Some(&mut sink));
                    match event.op().clone() {
                        #(#root_variants)*
                        #package_ident::AddReference(o) => {
                            let mut ctx = #path::EffectContext::silent();
                            self.reference_manager_log.effect(
                                #path::ProtocolEvent::unfold(event.clone(), #path::ReferenceManager::AddArc(o)),
                                &mut ctx
                            );
                        }
                        #package_ident::RemoveReference(o) => {
                            let mut ctx = #path::EffectContext::silent();
                            self.reference_manager_log.effect(
                                #path::ProtocolEvent::unfold(event.clone(), #path::ReferenceManager::RemoveArc(o)),
                                &mut ctx
                            );
                        }
                    }
                }
                let mut reference_effect_disambiguator = 0u32;
                for sink in sink.into_sinks() {
                    match sink.effect() {
                        #path::SinkEffect::Create | #path::SinkEffect::Update => {
                            let vertex_ops = sink.kind()
                                .and_then(|kind| #path::instance_from_sink_kind(kind, sink.path()))
                                .map(|instance| #path::ReferenceManager::AddVertex { id: instance });
                            if let Some(o) = vertex_ops {
                                reference_effect_disambiguator += 1;
                                let mut ctx = #path::EffectContext::silent();
                                self.reference_manager_log.effect(
                                    #path::ProtocolEvent::unfold_with_disambiguator(
                                        event.clone(),
                                        reference_effect_disambiguator,
                                        o,
                                    ),
                                    &mut ctx
                                );
                            }
                        }
                        #path::SinkEffect::Delete => {
                            reference_effect_disambiguator += 1;
                            let mut ctx = #path::EffectContext::silent();
                            self.reference_manager_log.effect(
                                #path::ProtocolEvent::unfold_with_disambiguator(
                                    event.clone(),
                                    reference_effect_disambiguator,
                                    #path::ReferenceManager::DeleteSubtree {
                                        prefix: sink.path().clone(),
                                    }),
                                &mut ctx
                            );
                        }
                    }
                }
            }
        } else {
            quote! {
                let mut ctx = #path::EffectContext::root(#package_name, None);
                match event.op().clone() {
                    #(#root_variants)*
                }
            }
        };

        quote! {
            impl #path::IsLog for #package_log_name {
                type Command = #package_ident;
                type Op = #package_ident;
                type Rejection = #package_rejection_name;

                fn prepare(&self, command: Self::Command) -> Self::Op {
                    command
                }

                fn is_enabled(&self, op: &Self::Op) -> Result<(), Self::Rejection> {
                    match op {
                        #(#enabled_root_arms,)*
                        #reference_is_enabled
                    }
                }

                fn effect(&mut self, event: #path::ProtocolEvent<Self::Op>, _ctx: &mut #path::EffectContext<'_>) {
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
                    #reference_default_check #(&& #default_checks)*
                }
            }
        }
    }

    fn generate_eval_nested_impl(&self) -> TokenStream {
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_log_name = type_ident_with_suffix(package_name, "Log");
        let package_value_name = type_ident_with_suffix(package_name, "Value");
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();

        let root_reads = self.roots().into_iter().map(|root| {
            let field_name = value_ident(self.root_class_name(root));
            let log_field = self.root_field_ident(root);
            quote! { #field_name: self.#log_field.execute_query(&#path::Read::new()) }
        });
        let refs_field = if self.has_references() {
            quote! { refs: self.reference_manager_log.execute_query(&#path::Read::new()), }
        } else {
            quote! {}
        };

        // The package's own value type, named outright: v0.7's `IsLog` has no
        // `Value` associated type to project it through.
        quote! {
            impl #path::EvalNested<#path::Read<#package_value_name>> for #package_log_name {
                fn execute_query(
                    &self,
                    _q: &#path::Read<#package_value_name>,
                ) -> <#path::Read<#package_value_name> as #path::QueryOperation>::Response {
                    #package_value_name {
                        #(#root_reads,)*
                        #refs_field
                    }
                }
            }
        }
    }

    /// Generate the `QueryableLog` impl for the package log.
    ///
    /// This enables the `GET /api/state` endpoint in `GenericNode` by providing
    /// a concrete function that calls `replica.query(Read::new())` and serializes
    /// the resulting `Value` to JSON.
    ///
    /// Each generated package gets this automatically — downstream code just needs
    /// to call `node.enable_state_query()` before `node.run()`.
    fn generate_queryable_log_impl(&self) -> TokenStream {
        let package_name = self.ctx.packs().get(self.pack_idx).unwrap().name();
        let package_log_name = type_ident_with_suffix(package_name, "Log");
        let package_ident = type_ident(package_name);
        let package_value_name = type_ident_with_suffix(package_name, "Value");
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap();

        quote! {
            /// Auto-generated [`QueryableLog`] impl — enables `GET /api/state` in `GenericNode`.
            ///
            /// Calls `replica.query(Read::new())` to evaluate the full CRDT state via
            /// the `EvalNested<Read<Value>>` chain, then serializes the result to JSON.
            impl moirai_network::query::QueryableLog for #package_log_name {
                fn query_state_json(
                    replica: &moirai_protocol::replica::Replica<
                        Self,
                        moirai_protocol::broadcast::tcsb::Tcsb<#package_ident>,
                    >,
                ) -> serde_json::Value {
                    use moirai_protocol::replica::IsReplica;
                    let value: #package_value_name = replica.query(&#path::Read::new());
                    serde_json::to_value(&value)
                        .unwrap_or_else(|e| serde_json::json!({ "error": format!("serialize: {}", e) }))
                }
            }
        }
    }

}

impl<'a> Generate for PackageGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let package_enum = self.generate_package_enum();
        let package_rejection = self.generate_package_rejection_enum();
        let package_value = self.generate_package_value_struct();
        let package_log = self.generate_package_log_struct();
        let is_log_impl = self.generate_is_log_impl();
        let eval_nested_impl = self.generate_eval_nested_impl();
        let queryable_log_impl = self.generate_queryable_log_impl();

        let tokens = quote! {
            #package_enum
            #package_rejection
            #package_value
            #package_log
            #is_log_impl
            #eval_nested_impl
            #queryable_log_impl
        };

        let imports = self.imports();

        Ok(Fragment::new(tokens, imports, vec![]))
    }
}
