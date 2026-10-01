//! Reads replicated attributes and converts their values to XMI attributes.

use ecore_rs::repr::{Class, Structural, builtin::Typ as BuiltinTyp, structural};
use proc_macro2::TokenStream;
use quote::quote;

use super::ReadAsEcoreGenerator;
use crate::codegen::{
    annotation::{DatatypeOverride, datatype_override},
    classifier::{classifier_ident, inherited_field_ident},
    datatype::{
        crdt::{Primitive, Register},
        to_crdt::ToCrdt,
    },
    feature::{
        attribute::primitive_value_type,
        bounds::{BoundKind, normalize_bounds},
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

    fn primitive_for_attribute(&self, feature: &Structural) -> Primitive {
        let class_typ = self.class(feature.typ.expect("attribute should have a type"));
        let mut primitive = if class_typ.is_enum() {
            Primitive::Register(Register::MultiValue)
        } else {
            let typ: BuiltinTyp = class_typ
                .name()
                .parse()
                .unwrap_or_else(|_| panic!("Failed to parse type: {}", class_typ.name()));
            typ.to_crdt_container()
        };

        if let Some(DatatypeOverride::Primitive(override_primitive)) = datatype_override(feature) {
            primitive = override_primitive;
        }

        primitive
    }

    fn attribute_read_type(&self, feature: &Structural) -> TokenStream {
        let typ = self.class(feature.typ.expect("attribute should have a type"));
        let rust_type = if typ.is_enum() {
            let enum_ident = classifier_ident(self.ctx, typ);
            quote! { #enum_ident }
        } else {
            let builtin: BuiltinTyp = typ
                .name()
                .parse()
                .unwrap_or_else(|_| panic!("Failed to parse type: {}", typ.name()));
            builtin
                .to_rust_type()
                .expect("Supported Ecore attributes should have a Rust value type")
        };
        let primitive = self.primitive_for_attribute(feature);
        let scalar_type = primitive_value_type(&primitive, &rust_type);
        let (bound_kind, _) = normalize_bounds(feature.bounds, &feature.name);

        match (
            bound_kind,
            feature.unique.unwrap_or(true),
            feature.ordered.unwrap_or(true),
        ) {
            (BoundKind::Single, _, _) => scalar_type,
            (BoundKind::Optional, _, _) => quote! { Option<#scalar_type> },
            (BoundKind::Many, false, true) => quote! { Vec<#scalar_type> },
            (BoundKind::Many, true, true) => quote! { Vec<#rust_type> },
            (BoundKind::Many, false, false) => {
                quote! { rustc_hash::FxHashMap<#rust_type, usize> }
            }
            (BoundKind::Many, true, false) => {
                quote! { rustc_hash::FxHashSet<#rust_type> }
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

    fn collection_value_to_values(&self, feature: &Structural, value: TokenStream) -> TokenStream {
        let typ = self.class(feature.typ.expect("attribute should have a type"));
        let primitive = self.primitive_for_attribute(feature);
        let unique = feature.unique.unwrap_or(true);
        let ordered = feature.ordered.unwrap_or(true);

        match (unique, ordered) {
            (true, true) => {
                let item = rust_ident("value");
                let scalar = self.scalar_to_string(typ, quote! { #item });
                quote! {
                    (#value).iter().map(|#item| #scalar).collect::<Vec<_>>()
                }
            }
            (false, true) => {
                let item = rust_ident("value");
                let inner = self.primitive_value_to_values(typ, &primitive, quote! { #item });
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
            (false, false) => {
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
            (true, false) => {
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
    ) -> TokenStream {
        let xml_name = feature.name.as_str();
        let (bound_kind, _) = normalize_bounds(feature.bounds, &feature.name);
        let value = rust_ident("value");
        let read_type = self.attribute_read_type(feature);

        match bound_kind {
            BoundKind::Single => {
                let primitive = self.primitive_for_attribute(feature);
                let typ = self.class(feature.typ.expect("attribute should have a type"));
                let values = self.primitive_value_to_values(typ, &primitive, quote! { #value });
                quote! {
                    {
                        let #value = xmi_read::<_, #read_type>(#log);
                        (#attrs).push_values(#xml_name, #values, true);
                    }
                }
            }
            BoundKind::Optional => {
                let primitive = self.primitive_for_attribute(feature);
                let typ = self.class(feature.typ.expect("attribute should have a type"));
                let values = self.primitive_value_to_values(typ, &primitive, quote! { #value });
                quote! {
                    {
                        let optional_value = xmi_read::<_, #read_type>(#log);
                        if let Some(#value) = optional_value {
                            (#attrs).push_values(#xml_name, #values, true);
                        }
                    }
                }
            }
            BoundKind::Many => {
                let values = self.collection_value_to_values(feature, quote! { #value });
                let force = feature.bounds.lbound > 0;
                quote! {
                    {
                        let #value = xmi_read::<_, #read_type>(#log);
                        (#attrs).push_values(#xml_name, #values, #force);
                    }
                }
            }
        }
    }

    fn generate_attribute_collect(
        &self,
        feature: &Structural,
        log: TokenStream,
        attrs: TokenStream,
    ) -> TokenStream {
        let accessor = value_ident(&feature.name);
        self.attribute_collect_from_log(feature, quote! { (#log).#accessor() }, attrs)
    }

    pub(super) fn generate_attribute_visits(
        &self,
        class: &Class,
        log: TokenStream,
        path: TokenStream,
        attrs: TokenStream,
    ) -> Vec<TokenStream> {
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
            ));
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
                )),
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

        visits
    }
}
