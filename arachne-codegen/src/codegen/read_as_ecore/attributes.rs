//! Reads replicated attributes and converts their values to XMI attributes.

use ecore_rs::repr::{Class, Structural, builtin::Typ as BuiltinTyp, structural};
use proc_macro2::TokenStream;
use quote::quote;

use super::ReadAsEcoreGenerator;
use crate::codegen::{
    classifier::{classifier_ident, inherited_field_ident},
    datatype::crdt::{Primitive, Register},
    feature::{
        bounds::BoundKind,
        plan::{AttributeCollection, AttributePlan, FeaturePlan, Storage},
    },
    ident::{rust_ident, value_ident},
};

impl ReadAsEcoreGenerator<'_> {
    pub(super) fn scalar_to_string(&self, typ: &Class, value: TokenStream) -> TokenStream {
        if typ.is_enum() {
            let path = self.path();
            let enum_ident = classifier_ident(self.ctx, typ);
            let variants = typ
                .literals()
                .iter()
                .map(|literal| {
                    let variant = crate::codegen::ident::type_ident(literal.name());
                    let serialized = literal.name();
                    quote! { #path::#enum_ident::#variant => #serialized.to_string() }
                })
                .collect::<Vec<_>>();

            if variants.is_empty() {
                quote! { format!("{:?}", #value) }
            } else {
                quote! {
                    match #value {
                        #(#variants,)*
                    }
                }
            }
        } else {
            match typ.name().parse::<BuiltinTyp>() {
                Ok(BuiltinTyp::EString)
                | Ok(BuiltinTyp::EChar)
                | Ok(BuiltinTyp::EBoolean)
                | Ok(BuiltinTyp::EByte)
                | Ok(BuiltinTyp::EShort)
                | Ok(BuiltinTyp::EInt)
                | Ok(BuiltinTyp::ELong)
                | Ok(BuiltinTyp::EFloat)
                | Ok(BuiltinTyp::EDouble) => quote! { (#value).to_string() },
                _ => quote! { format!("{:?}", #value) },
            }
        }
    }

    fn primitive_value_to_values(
        &self,
        typ: &Class,
        primitive: &Primitive,
        value: TokenStream,
    ) -> TokenStream {
        match primitive {
            Primitive::List => quote! { vec![(#value).to_string()] },
            Primitive::Register(Register::MultiValue | Register::PartiallyOrdered) => {
                let item = rust_ident("value");
                let scalar = self.scalar_to_string(typ, quote! { #item });
                quote! {
                    {
                        let mut values = (#value)
                            .iter()
                            .map(|#item| #scalar)
                            .collect::<Vec<_>>();
                        values.sort();
                        values
                    }
                }
            }
            Primitive::Register(
                Register::LastWriterWins | Register::Fair | Register::TotallyOrdered,
            ) => {
                let item = rust_ident("value");
                let scalar = self.scalar_to_string(typ, quote! { #item });
                quote! {
                    match (#value).as_ref() {
                        Some(#item) => vec![#scalar],
                        None => Vec::new(),
                    }
                }
            }
            _ => {
                let scalar = self.scalar_to_string(typ, value);
                quote! { vec![#scalar] }
            }
        }
    }

    fn attribute_value_to_values(
        &self,
        attribute: &AttributePlan<'_>,
        value: TokenStream,
    ) -> TokenStream {
        let typ = attribute.typ;
        match attribute.collection {
            AttributeCollection::Scalar => {
                self.primitive_value_to_values(typ, &attribute.primitive, value)
            }
            AttributeCollection::Sequence => {
                let item = rust_ident("value");
                let scalar = self.scalar_to_string(typ, quote! { #item });
                quote! {
                    (#value).iter().map(|#item| #scalar).collect::<Vec<_>>()
                }
            }
            AttributeCollection::NestedList => {
                let item = rust_ident("value");
                let inner =
                    self.primitive_value_to_values(typ, &attribute.primitive, quote! { #item });
                quote! {
                    {
                        let mut values = Vec::new();
                        for #item in &(#value) {
                            values.extend(#inner);
                        }
                        values
                    }
                }
            }
            AttributeCollection::Bag => {
                let item = rust_ident("value");
                let count = rust_ident("count");
                let scalar = self.scalar_to_string(typ, quote! { #item });
                quote! {
                    {
                        let mut values = Vec::new();
                        for (#item, #count) in &(#value) {
                            for _ in 0..*#count {
                                values.push(#scalar);
                            }
                        }
                        values.sort();
                        values
                    }
                }
            }
            AttributeCollection::Set(_) => {
                let item = rust_ident("value");
                let scalar = self.scalar_to_string(typ, quote! { #item });
                quote! {
                    {
                        let mut values = (#value)
                            .iter()
                            .map(|#item| #scalar)
                            .collect::<Vec<_>>();
                        values.sort();
                        values
                    }
                }
            }
        }
    }

    pub(super) fn attribute_collect_from_log(
        &self,
        feature: &Structural,
        log: TokenStream,
        attrs: TokenStream,
    ) -> anyhow::Result<TokenStream> {
        self.attribute_collect_from_plan(&FeaturePlan::attribute(self.ctx, feature)?, log, attrs)
    }

    pub(super) fn attribute_collect_from_plan(
        &self,
        plan: &FeaturePlan<'_>,
        log: TokenStream,
        attrs: TokenStream,
    ) -> anyhow::Result<TokenStream> {
        let Storage::Attribute(attribute) = &plan.storage else {
            anyhow::bail!("Expected an attribute plan for `{}`", plan.feature.name);
        };
        let xml_name = plan.feature.name.as_str();
        let value = rust_ident("value");
        let path = self.path();
        let read_type = plan.types(self.ctx, &path, Some(&path))?.value;
        let values = self.attribute_value_to_values(attribute, quote! { #value });

        Ok(if matches!(plan.bounds, BoundKind::Optional) {
            quote! {
                {
                    let optional_value = xmi_read::<_, #read_type>(#log);
                    if let Some(#value) = optional_value {
                        (#attrs).push_values(#xml_name, #values, true);
                    }
                }
            }
        } else {
            let force = matches!(plan.bounds, BoundKind::Single) || plan.feature.bounds.lbound > 0;
            quote! {
                {
                    let #value = xmi_read::<_, #read_type>(#log);
                    (#attrs).push_values(#xml_name, #values, #force);
                }
            }
        })
    }

    fn generate_attribute_collect(
        &self,
        feature: &Structural,
        log: TokenStream,
        attrs: TokenStream,
    ) -> anyhow::Result<TokenStream> {
        let accessor = value_ident(&feature.name);
        self.attribute_collect_from_log(feature, quote! { (#log).#accessor() }, attrs)
    }

    pub(super) fn generate_attribute_visits(
        &self,
        class: &Class,
        log: TokenStream,
        path: TokenStream,
        attrs: TokenStream,
    ) -> anyhow::Result<Vec<TokenStream>> {
        let mut visits = Vec::new();

        for super_idx in class.sup() {
            let super_class = self.class(*super_idx);
            let accessor = inherited_field_ident(super_class);
            let inherited_log = self.unbox_log_expr(
                class.idx,
                &accessor.to_string(),
                quote! { (#log).#accessor() },
            );
            visits.extend(self.generate_attribute_visits(
                super_class,
                inherited_log,
                path.clone(),
                attrs.clone(),
            )?);
        }

        for feature in class
            .structural()
            .iter()
            .filter(|feature| feature.transient != Some(true))
        {
            match feature.kind {
                structural::Typ::EAttribute => visits.push(self.generate_attribute_collect(
                    feature,
                    log.clone(),
                    attrs.clone(),
                )?),
                structural::Typ::EReference if !feature.containment && self.has_references() => {
                    let feature_key = self.reference_feature_key(class, feature);
                    let xml_name = feature.name.as_str();
                    visits.push(quote! {
                        (#attrs).push_values(
                            #xml_name,
                            refs.values(#path, #feature_key),
                            false,
                        );
                    });
                }
                structural::Typ::EReference => {}
            }
        }

        Ok(visits)
    }
}
