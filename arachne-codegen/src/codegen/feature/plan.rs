//! Shared, validated storage decisions for record fields, transparent variants and XMI.

use anyhow::{Context, Result, ensure};
use ecore_rs::{
    ctx::Ctx,
    repr::{Class, Structural, builtin::Typ, idx, structural},
};
use proc_macro2::TokenStream;
use quote::quote;

use crate::codegen::{
    annotation::{DatatypeOverride, datatype_override, transparent_field, uw_map_spec},
    classifier::{
        containment_target_ident, containment_target_log_ident, containment_target_value_ident,
        is_uninhabited_polymorphic_class,
    },
    cycles::{BoxingStrategy, CycleAnalysis},
    datatype::{
        crdt::{
            Bag, Collection, Crdt, Map, Named, NestedCrdt, Primitive, Register, Set, SimpleCrdt,
        },
        to_crdt::ToCrdt,
    },
    generate::Fragment,
    ident::{classifier_type_ident, rust_ident, value_ident},
    import::{Import, Log, Protocol},
    warnings::Warning,
};

use super::{
    bounds::{BoundKind, normalize_bounds},
    typed_element::unsupported_feature_properties,
};

pub(crate) struct FeaturePlan<'a> {
    pub feature: &'a Structural,
    pub bounds: BoundKind,
    pub storage: Storage<'a>,
    pub warnings: Vec<Warning>,
}

pub(crate) enum Storage<'a> {
    Attribute(AttributePlan<'a>),
    Containment {
        target: &'a Class,
        boxing: BoxingStrategy,
    },
    Map(MapPlan<'a>),
}

pub(crate) struct AttributePlan<'a> {
    pub typ: &'a Class,
    pub primitive: Primitive,
    pub collection: AttributeCollection,
}

pub(crate) enum AttributeCollection {
    Scalar,
    NestedList,
    Sequence,
    Bag,
    Set(Set),
}

pub(crate) struct MapPlan<'a> {
    pub entry: &'a Class,
    pub key: &'a Structural,
    pub key_type: &'a Class,
    pub value: Box<FeaturePlan<'a>>,
}

pub(crate) struct FeatureTypes {
    pub payload: TokenStream,
    pub log: TokenStream,
    pub value: TokenStream,
    pub imports: Vec<Import>,
}

fn feature_type<'a>(ctx: &'a Ctx, feature: &Structural) -> Result<&'a Class> {
    feature
        .typ
        .and_then(|idx| ctx.classes().get(*idx))
        .with_context(|| format!("Feature `{}` has no resolved type", feature.name))
}

fn builtin_type(class: &Class) -> Result<Typ> {
    let typ: Typ = class
        .name()
        .parse()
        .map_err(|_| anyhow::anyhow!("Unsupported attribute type `{}`", class.name()))?;
    ensure!(
        typ != Typ::Object,
        "Unsupported attribute type `{}`",
        class.name()
    );
    Ok(typ)
}

fn rust_type(ctx: &Ctx, class: &Class, namespace: Option<&syn::Path>) -> Result<TokenStream> {
    if class.is_enum() {
        let ident = classifier_type_ident(ctx, class);
        Ok(qualify(quote! { #ident }, namespace))
    } else {
        builtin_type(class)?
            .to_rust_type()
            .with_context(|| format!("Type `{}` has no Rust value type", class.name()))
    }
}

fn qualify(ident: TokenStream, namespace: Option<&syn::Path>) -> TokenStream {
    match namespace {
        Some(path) => quote! { #path::#ident },
        None => ident,
    }
}

impl<'a> FeaturePlan<'a> {
    fn new(feature: &'a Structural, storage: Storage<'a>) -> Self {
        let (bounds, mut warnings) = normalize_bounds(feature.bounds, &feature.name);
        unsupported_feature_properties(feature, &mut warnings);
        Self {
            feature,
            bounds,
            storage,
            warnings,
        }
    }

    pub fn attribute(ctx: &'a Ctx, feature: &'a Structural) -> Result<Self> {
        ensure!(
            feature.kind == structural::Typ::EAttribute,
            "Expected an attribute"
        );
        let typ = feature_type(ctx, feature)?;
        let override_type = datatype_override(feature);
        let primitive = match &override_type {
            Some(DatatypeOverride::Primitive(primitive)) => primitive.clone(),
            _ if typ.is_enum() => Primitive::Register(Register::MultiValue),
            _ => builtin_type(typ)?.to_crdt_container(),
        };
        // Even an override requires a supported underlying value type.
        rust_type(ctx, typ, None)?;
        let mut plan = Self::new(
            feature,
            Storage::Attribute(AttributePlan {
                typ,
                primitive,
                collection: AttributeCollection::Scalar,
            }),
        );
        if let Storage::Attribute(attribute) = &mut plan.storage
            && matches!(plan.bounds, BoundKind::Many)
        {
            attribute.collection = match (
                feature.unique.unwrap_or(true),
                feature.ordered.unwrap_or(true),
            ) {
                (false, true) => AttributeCollection::NestedList,
                (true, true) => AttributeCollection::Sequence,
                (false, false) => AttributeCollection::Bag,
                (true, false) => AttributeCollection::Set(match override_type {
                    Some(DatatypeOverride::Set(set)) => set,
                    _ => Set::AWSet,
                }),
            };
        }
        Ok(plan)
    }

    pub fn resolve(
        ctx: &'a Ctx,
        source: idx::Class,
        feature: &'a Structural,
        cycles: &CycleAnalysis,
    ) -> Result<Self> {
        if feature.kind == structural::Typ::EAttribute {
            return Self::attribute(ctx, feature);
        }
        ensure!(
            feature.containment,
            "Feature `{}` must be a containment reference",
            feature.name
        );
        let target = feature_type(ctx, feature)?;
        ensure!(
            !is_uninhabited_polymorphic_class(ctx, target),
            "Containment field `{}` targets abstract class `{}` with no concrete subclasses",
            feature.name,
            target.name()
        );
        let mut plan = Self::new(
            feature,
            Storage::Containment {
                target,
                boxing: cycles.boxing_strategy(source, &feature.name),
            },
        );

        if let Some(spec) = uw_map_spec(feature) {
            ensure!(
                matches!(plan.bounds, BoundKind::Many),
                "uw-map reference `{}` must be multi-valued",
                feature.name
            );
            let find = |name: &str| {
                target
                    .structural()
                    .iter()
                    .find(|field| field.name == name)
                    .with_context(|| {
                        format!("UWMap feature `{name}` not found in `{}`", target.name())
                    })
            };
            let key = find(&spec.key_feature)?;
            let value = find(&spec.value_feature)?;
            ensure!(
                key.kind == structural::Typ::EAttribute,
                "UWMap key feature must be an attribute"
            );
            ensure!(
                value.kind != structural::Typ::EReference || value.containment,
                "UWMap value feature cannot be a non-containment reference"
            );
            ensure!(
                matches!((value.bounds.lbound, value.bounds.ubound), (1, Some(1))),
                "UWMap value feature must be required and single-valued (1..1)"
            );
            let key_type = feature_type(ctx, key)?;
            rust_type(ctx, key_type, None)?;
            // Transparent variants project the entry value directly into their union.
            // Preserve that projection's boxing owner in the plan so XMI uses it too.
            let owner = if transparent_field(&ctx.classes()[*source]).is_some() {
                source
            } else {
                target.idx
            };
            let value = Box::new(Self::resolve(ctx, owner, value, cycles)?);
            plan.warnings.extend(value.warnings.iter().cloned());
            plan.storage = Storage::Map(MapPlan {
                entry: target,
                key,
                key_type,
                value,
            });
        }
        Ok(plan)
    }

    /// Derives all three generated types together. `namespace` qualifies classifier types
    /// when the consumer is outside classifiers.rs (notably the XMI query module).
    pub fn types(
        &self,
        ctx: &Ctx,
        path: &syn::Path,
        namespace: Option<&syn::Path>,
    ) -> Result<FeatureTypes> {
        let mut types = match &self.storage {
            Storage::Attribute(attribute) => {
                let rust_type = rust_type(ctx, attribute.typ, namespace)?;
                match &attribute.collection {
                    AttributeCollection::Scalar | AttributeCollection::NestedList => {
                        primitive_types(&attribute.primitive, &rust_type, path)
                    }
                    AttributeCollection::Sequence => FeatureTypes {
                        payload: quote! { #path::List<#rust_type> },
                        log: quote! { #path::GraphLog<#path::List<#rust_type>> },
                        value: quote! { Vec<#rust_type> },
                        imports: vec![
                            Import::Log(Log::Graph),
                            Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(Primitive::List))),
                        ],
                    },
                    AttributeCollection::Bag => FeatureTypes {
                        payload: quote! { moirai_crdt::bag::aw_bag::AWBag<#rust_type> },
                        log: quote! { #path::AWBagLog<#rust_type> },
                        value: quote! { rustc_hash::FxHashMap<#rust_type, usize> },
                        imports: vec![Import::Crdt(Crdt::Simple(SimpleCrdt::Collection(
                            Collection::Bag(Bag::AWBag),
                        )))],
                    },
                    AttributeCollection::Set(set) => {
                        let name = rust_ident(set.name());
                        FeatureTypes {
                            payload: quote! { #path::#name<#rust_type> },
                            log: quote! { #path::VecLog<#path::#name<#rust_type>> },
                            value: quote! { rustc_hash::FxHashSet<#rust_type> },
                            imports: vec![
                                Import::Log(Log::Vec),
                                Import::Crdt(Crdt::Simple(SimpleCrdt::Collection(
                                    Collection::Set(set.clone()),
                                ))),
                            ],
                        }
                    }
                }
            }
            Storage::Containment { target, boxing } => {
                let payload = containment_target_ident(ctx, target);
                let log = containment_target_log_ident(ctx, target);
                let value = containment_target_value_ident(ctx, target);
                let mut types = FeatureTypes {
                    payload: qualify(quote! { #payload }, namespace),
                    log: qualify(quote! { #log }, namespace),
                    value: qualify(quote! { #value }, namespace),
                    imports: Vec::new(),
                };
                if *boxing != BoxingStrategy::NoBox {
                    let FeatureTypes {
                        payload,
                        log,
                        value,
                        mut imports,
                    } = types;
                    imports.push(Import::Protocol(Protocol::BoxedLog));
                    types = FeatureTypes {
                        payload: quote! { Box<#payload> },
                        log: quote! { #path::BoxedLog<#log> },
                        value: quote! { Box<#value> },
                        imports,
                    };
                }
                types
            }
            Storage::Map(map) => {
                let key = rust_type(ctx, map.key_type, namespace)?;
                let FeatureTypes {
                    payload,
                    log,
                    value,
                    mut imports,
                } = map.value.types(ctx, path, namespace)?;
                imports.push(Import::Crdt(Crdt::Nested(NestedCrdt::Map(Map::UWMap))));
                // A map operation embeds its child operation inline. Box it to break
                // recursive union payloads; Moirai's Boxer converts it for the child log.
                return Ok(FeatureTypes {
                    payload: quote! { moirai_crdt::map::uw_map::UWMap<#key, Box<#payload>> },
                    log: quote! { #path::UWMapLog<#key, #log> },
                    value: quote! { rustc_hash::FxHashMap<#key, #value> },
                    imports,
                });
            }
        };

        let wraps_many = matches!(
            &self.storage,
            Storage::Containment { .. }
                | Storage::Attribute(AttributePlan {
                    collection: AttributeCollection::NestedList,
                    ..
                })
        );
        let FeatureTypes {
            payload,
            log,
            value,
            imports,
        } = types;
        types = match self.bounds {
            BoundKind::Optional => FeatureTypes {
                payload: quote! { moirai_crdt::option::Optional<#payload> },
                log: quote! { #path::OptionLog<#log> },
                value: quote! { Option<#value> },
                imports: [
                    imports,
                    vec![Import::Crdt(Crdt::Nested(NestedCrdt::Optional))],
                ]
                .concat(),
            },
            BoundKind::Many if wraps_many => FeatureTypes {
                payload: quote! { moirai_crdt::list::nested_list::NestedList<#payload> },
                log: quote! { #path::NestedListLog<#log> },
                value: quote! { Vec<#value> },
                imports: [imports, vec![Import::Crdt(Crdt::Nested(NestedCrdt::List))]].concat(),
            },
            _ => FeatureTypes {
                payload,
                log,
                value,
                imports,
            },
        };
        Ok(types)
    }

    pub fn into_field(self, ctx: &Ctx, path: &syn::Path) -> Result<Fragment> {
        let name = value_ident(&self.feature.name);
        let FeatureTypes {
            log,
            value,
            imports,
            ..
        } = self.types(ctx, path, None)?;
        Ok(Fragment::new(
            quote! { #name: #log => #value },
            imports,
            self.warnings,
        ))
    }

    pub fn unbox_log(&self, log: TokenStream) -> TokenStream {
        match self.storage {
            Storage::Containment { boxing, .. } if boxing != BoxingStrategy::NoBox => {
                quote! { (#log).inner() }
            }
            _ => log,
        }
    }
}

fn primitive_types(
    primitive: &Primitive,
    rust_type: &TokenStream,
    path: &syn::Path,
) -> FeatureTypes {
    let name = rust_ident(primitive.name());
    let (payload, log, value, log_import) = match primitive {
        Primitive::List => (
            quote! { #path::List<char> },
            quote! { #path::GraphLog<#path::List<char>> },
            quote! { std::string::String },
            Import::Log(Log::Graph),
        ),
        _ => {
            let payload = match primitive {
                Primitive::Flag(_) => quote! { #path::#name },
                _ => quote! { #path::#name<#rust_type> },
            };
            let value = match primitive {
                Primitive::Counter(_) => quote! { #rust_type },
                Primitive::Flag(_) => quote! { bool },
                Primitive::Register(Register::MultiValue | Register::PartiallyOrdered) => {
                    quote! { rustc_hash::FxHashSet<#rust_type> }
                }
                Primitive::Register(_) => quote! { Option<#rust_type> },
                Primitive::List => unreachable!(),
            };
            let log = quote! { #path::VecLog<#payload> };
            (payload, log, value, Import::Log(Log::Vec))
        }
    };
    FeatureTypes {
        payload,
        log,
        value,
        imports: vec![
            log_import,
            Import::Crdt(Crdt::Simple(SimpleCrdt::Primitive(primitive.clone()))),
        ],
    }
}
