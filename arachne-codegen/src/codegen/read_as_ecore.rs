use ecore_rs::{
    ctx::Ctx,
    repr::{Class, Structural, builtin::Typ as BuiltinTyp, idx, structural},
};
use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::{
    PACKAGE_PATH_MOD,
    codegen::{
        annotation::{DatatypeOverride, datatype_override, transparent_field, uw_map_spec},
        classifier::{
            classifier_ident, classifier_log_ident, has_subclasses, inherited_field_ident,
            is_instantiable_class, polymorphic_kind_ident, polymorphic_kind_log_ident,
        },
        datatype::{
            crdt::{Primitive, Register},
            to_crdt::ToCrdt,
        },
        feature::bounds::{BoundKind, normalize_bounds},
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
        ident::{classifier_type_ident_with_suffix, rust_ident, value_ident},
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
}

impl<'a> ReadAsEcoreGenerator<'a> {
    pub fn new(
        ctx: &'a Ctx,
        pack_idx: idx::Pack,
        package_classes: Vec<idx::Class>,
        root_class_indices: Vec<idx::Class>,
        ref_analysis: &'a ReferenceAnalysis,
    ) -> Self {
        Self {
            ctx,
            pack_idx,
            package_classes,
            root_class_indices,
            ref_analysis,
        }
    }

    fn path(&self) -> syn::Path {
        syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, PACKAGE_PATH_MOD)).unwrap()
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
        let path = self.path();
        if self.has_references() {
            quote! { refs: Option<&petgraph::graph::DiGraph<#path::Instance, #path::Ref>>, }
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
            quote! { Some(&refs), }
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

    fn has_wrapper_descendant(&self, class: &Class) -> bool {
        class.sub().iter().any(|idx| {
            let sub = self.class(*idx);
            self.generates_concrete_wrapper(sub) || self.has_wrapper_descendant(sub)
        })
    }

    fn record_is_generated(&self, class: &Class) -> bool {
        if class.is_enum()
            || transparent_field(class).is_some()
            || self.is_uw_map_entry_helper(class)
        {
            return false;
        }

        if is_instantiable_class(class) {
            return true;
        }

        if !(class.is_abstract() || class.is_interface()) || class.sub().is_empty() {
            return false;
        }

        !class.sup().is_empty()
            || class.structural().iter().any(|feature| {
                feature.kind == structural::Typ::EAttribute
                    || (feature.kind == structural::Typ::EReference && feature.containment)
            })
            || self.has_wrapper_descendant(class)
    }

    fn union_is_generated(&self, class: &Class) -> bool {
        !class.is_enum()
            && (class.is_abstract() || class.is_interface() || has_subclasses(class))
            && !class.sub().is_empty()
    }

    fn class_writer_ident(&self, class: &Class) -> Ident {
        rust_ident(format!(
            "__ecore_write_{}",
            classifier_ident(self.ctx, class)
                .to_string()
                .to_snake_case()
        ))
    }

    fn class_attr_ident(&self, class: &Class) -> Ident {
        rust_ident(format!(
            "__ecore_collect_attrs_{}",
            classifier_ident(self.ctx, class)
                .to_string()
                .to_snake_case()
        ))
    }

    fn class_children_ident(&self, class: &Class) -> Ident {
        rust_ident(format!(
            "__ecore_write_children_{}",
            classifier_ident(self.ctx, class)
                .to_string()
                .to_snake_case()
        ))
    }

    fn union_writer_ident(&self, class: &Class) -> Ident {
        rust_ident(format!(
            "__ecore_write_{}",
            polymorphic_kind_ident(self.ctx, class)
                .to_string()
                .to_snake_case()
        ))
    }

    fn reference_helper_ident(&self, class: &Class, feature: &Structural) -> Ident {
        rust_ident(format!(
            "__ecore_refs_{}_{}",
            classifier_ident(self.ctx, class)
                .to_string()
                .to_snake_case(),
            value_ident(&feature.name)
        ))
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

    fn scalar_to_string(&self, typ: &Class, value: TokenStream) -> TokenStream {
        if typ.is_enum() {
            quote! { format!("{:?}", #value) }
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

    fn primitive_value_to_values(
        &self,
        typ: &Class,
        primitive: &Primitive,
        value: TokenStream,
    ) -> TokenStream {
        match primitive {
            Primitive::List => quote! {
                vec![(#value).iter().collect::<String>()]
            },
            Primitive::Register(Register::MultiValue | Register::PartiallyOrdered) => {
                let item = rust_ident("__value");
                let scalar = self.scalar_to_string(typ, quote! { #item });
                quote! {
                    {
                        let mut __values = (#value)
                            .iter()
                            .map(|#item| #scalar)
                            .collect::<Vec<_>>();
                        __values.sort();
                        __values
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
                let item = rust_ident("__value");
                let scalar = self.scalar_to_string(typ, quote! { #item });
                quote! {
                    (#value).iter().map(|#item| #scalar).collect::<Vec<_>>()
                }
            }
            (false, true) => {
                let item = rust_ident("__value");
                let inner = self.primitive_value_to_values(typ, &primitive, quote! { #item });
                quote! {
                    {
                        let mut __values = Vec::new();
                        for #item in &(#value) {
                            __values.extend(#inner);
                        }
                        __values
                    }
                }
            }
            (false, false) => {
                let item = rust_ident("__value");
                let count = rust_ident("__count");
                let scalar = self.scalar_to_string(typ, quote! { #item });
                quote! {
                    {
                        let mut __values = Vec::new();
                        for (#item, #count) in &(#value) {
                            for _ in 0..*#count {
                                __values.push(#scalar);
                            }
                        }
                        __values.sort();
                        __values
                    }
                }
            }
            (true, false) => {
                let item = rust_ident("__value");
                let scalar = self.scalar_to_string(typ, quote! { #item });
                quote! {
                    {
                        let mut __values = (#value)
                            .iter()
                            .map(|#item| #scalar)
                            .collect::<Vec<_>>();
                        __values.sort();
                        __values
                    }
                }
            }
        }
    }

    fn attribute_collect_from_log(
        &self,
        feature: &Structural,
        log: TokenStream,
        attrs: TokenStream,
    ) -> TokenStream {
        let xml_name = feature.name.as_str();
        let (bound_kind, _) = normalize_bounds(feature.bounds, &feature.name);
        let value = rust_ident("__value");

        match bound_kind {
            BoundKind::Single => {
                let primitive = self.primitive_for_attribute(feature);
                let typ = self.class(feature.typ.expect("attribute should have a type"));
                let values = self.primitive_value_to_values(typ, &primitive, quote! { #value });
                quote! {
                    {
                        let #value = __xmi_read(#log);
                        __xmi_push_attr(#attrs, #xml_name, #values, true);
                    }
                }
            }
            BoundKind::Optional => {
                let primitive = self.primitive_for_attribute(feature);
                let typ = self.class(feature.typ.expect("attribute should have a type"));
                let values = self.primitive_value_to_values(typ, &primitive, quote! { #value });
                quote! {
                    {
                        let __optional_value = __xmi_read(#log);
                        if let Some(#value) = __optional_value {
                            __xmi_push_attr(#attrs, #xml_name, #values, false);
                        }
                    }
                }
            }
            BoundKind::Many => {
                let values = self.collection_value_to_values(feature, quote! { #value });
                let force = feature.bounds.lbound > 0;
                quote! {
                    {
                        let #value = __xmi_read(#log);
                        __xmi_push_attr(#attrs, #xml_name, #values, #force);
                    }
                }
            }
        }
    }

    fn generate_attribute_collect(&self, feature: &Structural) -> TokenStream {
        let accessor = value_ident(&feature.name);
        self.attribute_collect_from_log(feature, quote! { log.#accessor() }, quote! { attrs })
    }

    fn call_target_writer(
        &self,
        target_class: &Class,
        out: TokenStream,
        element_name: Option<&str>,
        path_expr: TokenStream,
        log_expr: TokenStream,
        indent_expr: TokenStream,
        root: bool,
    ) -> TokenStream {
        let refs_arg = self.refs_arg();

        if self.union_is_generated(target_class) {
            let writer = self.union_writer_ident(target_class);
            let element = match element_name {
                Some(name) => quote! { Some(#name) },
                None => quote! { None },
            };
            quote! {
                #writer(#out, #element, #path_expr, #log_expr, #refs_arg #indent_expr);
            }
        } else {
            let writer = self.class_writer_ident(target_class);
            let element = element_name
                .map(str::to_string)
                .unwrap_or_else(|| self.root_element(target_class));
            let xmi_type = if root {
                quote! { None }
            } else {
                let typ = self.xmi_type(target_class);
                quote! { Some(#typ) }
            };
            quote! {
                #writer(#out, #element, #xmi_type, #path_expr, #log_expr, #refs_arg #indent_expr);
            }
        }
    }

    fn generate_containment_write(
        &self,
        feature: &Structural,
        log: TokenStream,
        base_path: TokenStream,
        include_field_in_path: bool,
        out: TokenStream,
        indent: TokenStream,
    ) -> TokenStream {
        if let Some(spec) = uw_map_spec(feature) {
            return self.generate_uw_map_write(
                feature,
                spec.key_feature,
                spec.value_feature,
                log,
                base_path,
                include_field_in_path,
                out,
                indent,
            );
        }

        let target_class = self.class(feature.typ.expect("containment should have a type"));
        let path_field = value_ident(&feature.name).to_string();
        let xml_name = feature.name.as_str();
        let (bound_kind, _) = normalize_bounds(feature.bounds, &feature.name);
        let child_base_path = if include_field_in_path {
            quote! { #base_path.clone().field(#path_field) }
        } else {
            quote! { #base_path.clone() }
        };

        match bound_kind {
            BoundKind::Single => self.call_target_writer(
                target_class,
                out,
                Some(xml_name),
                child_base_path,
                log,
                indent,
                false,
            ),
            BoundKind::Optional => {
                let call = self.call_target_writer(
                    target_class,
                    out,
                    Some(xml_name),
                    quote! { __child_path },
                    quote! { __child },
                    indent,
                    false,
                );
                quote! {
                    if let Some(__child) = #log.child() {
                        let __child_path = #child_base_path;
                        #call
                    }
                }
            }
            BoundKind::Many => {
                let call = self.call_target_writer(
                    target_class,
                    out,
                    Some(xml_name),
                    quote! { __child_path },
                    quote! { __child },
                    indent,
                    false,
                );
                quote! {
                    {
                        let __list_base_path = #child_base_path;
                        let __positions =
                            moirai_protocol::crdt::eval::BorrowedRead::read_ref(#log.positions());
                        for __event_id in __positions {
                            if let Some(__child) = #log.children().get_child(__event_id) {
                                let __child_path =
                                    __list_base_path.clone().list_element(__event_id.clone());
                                #call
                            }
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_uw_map_write(
        &self,
        feature: &Structural,
        key_feature_name: String,
        value_feature_name: String,
        log: TokenStream,
        base_path: TokenStream,
        include_field_in_path: bool,
        out: TokenStream,
        indent: TokenStream,
    ) -> TokenStream {
        let target_class = self.class(feature.typ.expect("uw-map should have a target type"));
        let key_feature = target_class
            .structural()
            .iter()
            .find(|candidate| candidate.name == key_feature_name)
            .expect("uw-map key feature should exist");
        let value_feature = target_class
            .structural()
            .iter()
            .find(|candidate| candidate.name == value_feature_name)
            .expect("uw-map value feature should exist");
        let key_class = self.class(key_feature.typ.expect("uw-map key should have a type"));
        let key_to_string = self.scalar_to_string(key_class, quote! { __key });
        let path_field = value_ident(&feature.name).to_string();
        let xml_name = feature.name.as_str();
        let key_xml_name = key_feature.name.as_str();
        let xmi_type = self.xmi_type(target_class);
        let map_base_path = if include_field_in_path {
            quote! { #base_path.clone().field(#path_field) }
        } else {
            quote! { #base_path.clone() }
        };

        let value_tokens = match value_feature.kind {
            structural::Typ::EAttribute => {
                let collect = self.attribute_collect_from_log(
                    value_feature,
                    quote! { __child },
                    quote! { &mut __attrs },
                );
                quote! {
                    #collect
                }
            }
            structural::Typ::EReference => {
                let value_target =
                    self.class(value_feature.typ.expect("uw-map value should have a type"));
                let value_xml_name = value_feature.name.as_str();
                let value_path_field = value_ident(&value_feature.name).to_string();
                self.call_target_writer(
                    value_target,
                    quote! { &mut __entry_children },
                    Some(value_xml_name),
                    quote! { __entry_path.clone().field(#value_path_field) },
                    quote! { __child },
                    quote! { #indent + 1 },
                    false,
                )
            }
        };

        quote! {
            {
                let __map_base_path = #map_base_path;
                let mut __entries = #log.children().iter().collect::<Vec<_>>();
                __entries.sort_by_key(|(__key, _)| format!("{:?}", __key));

                for (__key, __child) in __entries {
                    let __entry_path = __map_base_path.clone().map_entry(format!("{:?}", __key));
                    let mut __attrs = Vec::new();
                    __attrs.push(("xmi:id", __xmi_path_id(&__entry_path)));
                    __attrs.push(("xmi:type", #xmi_type.to_string()));
                    __xmi_push_attr(
                        &mut __attrs,
                        #key_xml_name,
                        vec![#key_to_string],
                        true,
                    );

                    let mut __entry_children = String::new();
                    #value_tokens
                    __xmi_write_open(#out, #indent, #xml_name, &__attrs, __entry_children.is_empty());
                    if !__entry_children.is_empty() {
                        (#out).push_str(&__entry_children);
                        __xmi_write_close(#out, #indent, #xml_name);
                    }
                }
            }
        }
    }

    fn generate_class_attr_fn(&self, class: &Class) -> TokenStream {
        if !self.record_is_generated(class) {
            return quote! {};
        }

        let path = self.path();
        let fn_ident = self.class_attr_ident(class);
        let log_ty = classifier_log_ident(self.ctx, class);
        let refs_param = self.refs_param();
        let refs_arg = self.refs_arg();

        let inherited = class
            .sup()
            .iter()
            .map(|super_idx| {
                let super_class = self.class(*super_idx);
                let accessor = inherited_field_ident(super_class);
                let collect = self.class_attr_ident(super_class);
                quote! {
                    #collect(log.#accessor(), path, #refs_arg attrs);
                }
            })
            .collect::<Vec<_>>();

        let attributes = class
            .structural()
            .iter()
            .filter(|feature| feature.kind == structural::Typ::EAttribute)
            .map(|feature| self.generate_attribute_collect(feature));

        let references = class
            .structural()
            .iter()
            .filter(|feature| feature.kind == structural::Typ::EReference && !feature.containment)
            .map(|feature| {
                let helper = self.reference_helper_ident(class, feature);
                let xml_name = feature.name.as_str();
                if self.has_references() {
                    quote! {
                        __xmi_push_attr(attrs, #xml_name, #helper(refs, path), false);
                    }
                } else {
                    quote! {}
                }
            });

        quote! {
            #[allow(dead_code, unused_variables)]
            fn #fn_ident(
                log: &#path::#log_ty,
                path: &#path::ObjectPath,
                #refs_param
                attrs: &mut Vec<(&'static str, String)>,
            ) {
                #(#inherited)*
                #(#attributes)*
                #(#references)*
            }
        }
    }

    fn generate_class_children_fn(&self, class: &Class) -> TokenStream {
        if !self.record_is_generated(class) {
            return quote! {};
        }

        let path = self.path();
        let fn_ident = self.class_children_ident(class);
        let log_ty = classifier_log_ident(self.ctx, class);
        let refs_param = self.refs_param();
        let refs_arg = self.refs_arg();

        let inherited = class.sup().iter().map(|super_idx| {
            let super_class = self.class(*super_idx);
            let accessor = inherited_field_ident(super_class);
            let write_children = self.class_children_ident(super_class);
            let inherited_path = accessor.to_string();
            quote! {
                #write_children(
                    out,
                    &path.clone().field(#inherited_path),
                    log.#accessor(),
                    #refs_arg
                    indent,
                );
            }
        });

        let containments = class
            .structural()
            .iter()
            .filter(|feature| feature.kind == structural::Typ::EReference && feature.containment)
            .map(|feature| {
                let accessor = value_ident(&feature.name);
                self.generate_containment_write(
                    feature,
                    quote! { log.#accessor() },
                    quote! { path },
                    true,
                    quote! { out },
                    quote! { indent },
                )
            });

        quote! {
            #[allow(dead_code, unused_variables)]
            fn #fn_ident(
                out: &mut String,
                path: &#path::ObjectPath,
                log: &#path::#log_ty,
                #refs_param
                indent: usize,
            ) {
                #(#inherited)*
                #(#containments)*
            }
        }
    }

    fn generate_class_writer_fn(&self, class: &Class) -> TokenStream {
        if !self.record_is_generated(class) {
            return quote! {};
        }

        let path = self.path();
        let fn_ident = self.class_writer_ident(class);
        let log_ty = classifier_log_ident(self.ctx, class);
        let collect_attrs = self.class_attr_ident(class);
        let write_children = self.class_children_ident(class);
        let refs_param = self.refs_param();
        let refs_arg = self.refs_arg();

        quote! {
            #[allow(dead_code, unused_variables)]
            fn #fn_ident(
                out: &mut String,
                element_name: &str,
                xmi_type: Option<&str>,
                path: #path::ObjectPath,
                log: &#path::#log_ty,
                #refs_param
                indent: usize,
            ) {
                let mut attrs = Vec::new();
                attrs.push(("xmi:id", __xmi_path_id(&path)));
                if let Some(xmi_type) = xmi_type {
                    attrs.push(("xmi:type", xmi_type.to_string()));
                }
                #collect_attrs(log, &path, #refs_arg &mut attrs);

                let mut children = String::new();
                #write_children(&mut children, &path, log, #refs_arg indent + 1);

                __xmi_write_open(out, indent, element_name, &attrs, children.is_empty());
                if !children.is_empty() {
                    out.push_str(&children);
                    __xmi_write_close(out, indent, element_name);
                }
            }
        }
    }

    fn generate_transparent_variant_writer(
        &self,
        subclass: &Class,
        field: &Structural,
        out: TokenStream,
        element_name: TokenStream,
        xmi_type: TokenStream,
        path: TokenStream,
        log: TokenStream,
        indent: TokenStream,
    ) -> TokenStream {
        let mut_body = match field.kind {
            structural::Typ::EAttribute => {
                let collect =
                    self.attribute_collect_from_log(field, log.clone(), quote! { &mut __attrs });
                quote! {
                    #collect
                }
            }
            structural::Typ::EReference => self.generate_containment_write(
                field,
                log,
                quote! { __path },
                false,
                quote! { &mut __children },
                quote! { #indent + 1 },
            ),
        };

        let subclass_type = self.xmi_type(subclass);
        quote! {
            {
                let __path = #path;
                let mut __attrs = Vec::new();
                __attrs.push(("xmi:id", __xmi_path_id(&__path)));
                if #xmi_type {
                    __attrs.push(("xmi:type", #subclass_type.to_string()));
                }

                let mut __children = String::new();
                #mut_body

                __xmi_write_open(#out, #indent, #element_name, &__attrs, __children.is_empty());
                if !__children.is_empty() {
                    (#out).push_str(&__children);
                    __xmi_write_close(#out, #indent, #element_name);
                }
            }
        }
    }

    fn generate_union_writer_fn(&self, class: &Class) -> TokenStream {
        if !self.union_is_generated(class) {
            return quote! {};
        }

        let path = self.path();
        let fn_ident = self.union_writer_ident(class);
        let log_ty = polymorphic_kind_log_ident(self.ctx, class);
        let container_ty = classifier_type_ident_with_suffix(self.ctx, class, "KindContainer");
        let child_ty = classifier_type_ident_with_suffix(self.ctx, class, "KindChild");
        let refs_param = self.refs_param();
        let refs_arg = self.refs_arg();

        let child_arms = class
            .sub()
            .iter()
            .map(|sub_idx| {
                let subclass = self.class(*sub_idx);
                let variant = classifier_ident(self.ctx, subclass);
                let variant_path = variant.to_string().to_lowercase();
                let subclass_type = self.xmi_type(subclass);
                let root_element = self.root_element(subclass);

                if let Some(field_name) = transparent_field(subclass) {
                    let field = subclass
                        .structural()
                        .iter()
                        .find(|feature| feature.name == field_name)
                        .expect("transparent field should exist");
                    let body = self.generate_transparent_variant_writer(
                        subclass,
                        field,
                        quote! { out },
                        quote! { __element_name },
                        quote! { __emit_xmi_type },
                        quote! { path.clone().variant(#variant_path) },
                        quote! { __child_log },
                        quote! { indent },
                    );
                    quote! {
                        #path::#child_ty::#variant(__child_log) => {
                            let (__element_name, __emit_xmi_type) = match element_name {
                                Some(name) => (name, true),
                                None => (#root_element, false),
                            };
                            #body
                        }
                    }
                } else if self.union_is_generated(subclass) {
                    let writer = self.union_writer_ident(subclass);
                    quote! {
                        #path::#child_ty::#variant(__child_log) => {
                            #writer(
                                out,
                                element_name,
                                path.clone().variant(#variant_path),
                                __child_log,
                                #refs_arg
                                indent,
                            );
                        }
                    }
                } else {
                    let writer = self.class_writer_ident(subclass);
                    quote! {
                        #path::#child_ty::#variant(__child_log) => {
                            let (__element_name, __xmi_type) = match element_name {
                                Some(name) => (name, Some(#subclass_type)),
                                None => (#root_element, None),
                            };
                            #writer(
                                out,
                                __element_name,
                                __xmi_type,
                                path.clone().variant(#variant_path),
                                __child_log,
                                #refs_arg
                                indent,
                            );
                        }
                    }
                }
            })
            .collect::<Vec<_>>();

        quote! {
            #[allow(dead_code, unused_variables)]
            fn #fn_ident(
                out: &mut String,
                element_name: Option<&str>,
                path: #path::ObjectPath,
                log: &#path::#log_ty,
                #refs_param
                indent: usize,
            ) {
                match &log.child {
                    #path::#container_ty::Unset => {}
                    #path::#container_ty::Value(__child) => {
                        match __child.as_ref() {
                            #(#child_arms,)*
                        }
                    }
                    #path::#container_ty::Conflicts(__children) => {
                        for __child in __children {
                            match __child {
                                #(#child_arms,)*
                            }
                        }
                    }
                }
            }
        }
    }

    fn reference_connection_names(&self) -> Vec<Ident> {
        let mut counts = std::collections::HashMap::<String, usize>::new();

        self.ref_analysis
            .refs
            .iter()
            .map(|reference| {
                let source_class = self.class(reference.source_class);
                let target_class = self.class(reference.target_class);
                let source_name = classifier_ident(self.ctx, source_class);
                let target_name = classifier_ident(self.ctx, target_class);
                let base_name = format!("{source_name}To{target_name}");
                let suffix = counts.entry(base_name.clone()).or_insert(0);
                let unique_name = if *suffix == 0 {
                    base_name
                } else {
                    format!("{base_name}{}", *suffix + 1)
                };
                *suffix += 1;
                rust_ident(unique_name)
            })
            .collect()
    }

    fn generate_reference_helpers(&self) -> TokenStream {
        if !self.has_references() {
            return quote! {};
        }

        let path = self.path();
        let connection_names = self.reference_connection_names();
        let package_set = self.package_class_set();
        let mut helpers = Vec::new();

        for class_idx in &self.package_classes {
            let class = self.class(*class_idx);
            for feature in class.structural().iter().filter(|feature| {
                feature.kind == structural::Typ::EReference && !feature.containment
            }) {
                let helper = self.reference_helper_ident(class, feature);
                let source_family = self
                    .concrete_family(class.idx)
                    .into_iter()
                    .collect::<std::collections::HashSet<_>>();

                let arms = self
                    .ref_analysis
                    .refs
                    .iter()
                    .zip(connection_names.iter())
                    .filter(|(reference, _)| {
                        reference.reference_name == feature.name
                            && source_family.contains(&reference.source_class)
                            && package_set.contains(&reference.target_class)
                    })
                    .map(|(reference, connection)| {
                        let source_class = self.class(reference.source_class);
                        let target_class = self.class(reference.target_class);
                        let source_id =
                            classifier_type_ident_with_suffix(self.ctx, source_class, "Id");
                        let target_id =
                            classifier_type_ident_with_suffix(self.ctx, target_class, "Id");
                        quote! {
                            (
                                #path::Instance::#source_id(__source),
                                #path::Instance::#target_id(__target),
                                #path::Ref::#connection(_),
                            ) if &__source.0 == path => {
                                __values.push(format!("#{}", __xmi_path_id(&__target.0)));
                            }
                        }
                    })
                    .collect::<Vec<_>>();

                helpers.push(quote! {
                    #[allow(dead_code, unused_variables)]
                    fn #helper(
                        refs: Option<&petgraph::graph::DiGraph<#path::Instance, #path::Ref>>,
                        path: &#path::ObjectPath,
                    ) -> Vec<String> {
                        let mut __values = Vec::new();
                        let Some(refs) = refs else {
                            return __values;
                        };

                        use petgraph::visit::EdgeRef as _;
                        for __edge in refs.edge_references() {
                            let __source = &refs[__edge.source()];
                            let __target = &refs[__edge.target()];
                            match (__source, __target, __edge.weight()) {
                                #(#arms,)*
                                _ => {}
                            }
                        }

                        __values.sort();
                        __values
                    }
                });
            }
        }

        quote! { #(#helpers)* }
    }

    fn generate_helpers(&self) -> TokenStream {
        quote! {
            #[derive(Debug, Clone, Copy, Default)]
            pub struct ReadAsEcore;

            impl __package::QueryOperation for ReadAsEcore {
                type Response = String;
            }

            fn __xmi_read<L>(log: &L) -> <L as __package::IsLog>::Value
            where
                L: __package::IsLog
                    + __package::EvalNested<
                        __package::Read<<L as __package::IsLog>::Value>,
                    >,
            {
                log.execute_query(
                    __package::Read::<<L as __package::IsLog>::Value>::new(),
                )
            }

            fn __xmi_escape(value: &str) -> String {
                let mut escaped = String::new();
                for ch in value.chars() {
                    match ch {
                        '&' => escaped.push_str("&amp;"),
                        '<' => escaped.push_str("&lt;"),
                        '>' => escaped.push_str("&gt;"),
                        '"' => escaped.push_str("&quot;"),
                        '\'' => escaped.push_str("&apos;"),
                        _ => escaped.push(ch),
                    }
                }
                escaped
            }

            fn __xmi_indent(out: &mut String, indent: usize) {
                for _ in 0..indent {
                    out.push_str("  ");
                }
            }

            fn __xmi_path_id(path: &__package::ObjectPath) -> String {
                let raw = path.to_string();
                let mut id = String::with_capacity(raw.len() + 3);
                id.push_str("id");
                for ch in raw.chars() {
                    if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                        id.push(ch);
                    } else {
                        id.push('_');
                    }
                }
                id
            }

            fn __xmi_push_attr(
                attrs: &mut Vec<(&'static str, String)>,
                name: &'static str,
                mut values: Vec<String>,
                force: bool,
            ) {
                values.retain(|value| force || !value.is_empty());
                if values.is_empty() && !force {
                    return;
                }
                attrs.push((name, values.join(" ")));
            }

            fn __xmi_write_open(
                out: &mut String,
                indent: usize,
                name: &str,
                attrs: &[(&str, String)],
                empty: bool,
            ) {
                __xmi_indent(out, indent);
                out.push('<');
                out.push_str(name);
                for (key, value) in attrs {
                    out.push(' ');
                    out.push_str(key);
                    out.push_str("=\"");
                    out.push_str(&__xmi_escape(value));
                    out.push('"');
                }
                if empty {
                    out.push_str("/>\n");
                } else {
                    out.push_str(">\n");
                }
            }

            fn __xmi_write_close(out: &mut String, indent: usize, name: &str) {
                __xmi_indent(out, indent);
                out.push_str("</");
                out.push_str(name);
                out.push_str(">\n");
            }
        }
    }

    fn generate_query_impl(&self) -> TokenStream {
        let path = self.path();
        let package_log_name =
            crate::codegen::ident::type_ident_with_suffix(self.package_name(), "Log");
        let package_name = self.package_name();
        let ns_prefix = self.ns_prefix();
        let ns_uri = self.ns_uri();
        let refs_read = if self.has_references() {
            quote! {
                let refs = __xmi_read(&self.reference_manager_log);
            }
        } else {
            quote! {}
        };
        let refs_call_arg = self.refs_call_arg();

        let roots = self.root_class_indices.iter().map(|root_idx| {
            let root_class = self.class(*root_idx);
            let root_field = value_ident(root_class.name());
            let root_log_field =
                crate::codegen::ident::value_ident_with_suffix(root_class.name(), "log");
            let root_path_field = root_field.to_string();
            let root_path = quote! {
                #path::ObjectPath::new(#package_name).field(#root_path_field)
            };

            if self.union_is_generated(root_class) {
                let writer = self.union_writer_ident(root_class);
                quote! {
                    #writer(
                        &mut out,
                        None,
                        #root_path,
                        &self.#root_log_field,
                        #refs_call_arg
                        1,
                    );
                }
            } else {
                let writer = self.class_writer_ident(root_class);
                let element_name = self.root_element(root_class);
                quote! {
                    #writer(
                        &mut out,
                        #element_name,
                        None,
                        #root_path,
                        &self.#root_log_field,
                        #refs_call_arg
                        1,
                    );
                }
            }
        });

        quote! {
            impl #path::EvalNested<ReadAsEcore> for #package_log_name {
                fn execute_query(
                    &self,
                    _q: ReadAsEcore,
                ) -> <ReadAsEcore as #path::QueryOperation>::Response {
                    #refs_read

                    let mut out = String::new();
                    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");

                    let root_attrs = vec![
                        ("xmi:version", "2.0".to_string()),
                        ("xmlns:xmi", "http://www.omg.org/XMI".to_string()),
                        ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance".to_string()),
                        (concat!("xmlns:", #ns_prefix), #ns_uri.to_string()),
                    ];

                    __xmi_write_open(&mut out, 0, "xmi:XMI", &root_attrs, false);
                    #(#roots)*
                    __xmi_write_close(&mut out, 0, "xmi:XMI");
                    out
                }
            }
        }
    }
}

impl Generate for ReadAsEcoreGenerator<'_> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let helpers = self.generate_helpers();
        let reference_helpers = self.generate_reference_helpers();

        let class_attr_fns = self
            .package_classes
            .iter()
            .map(|class_idx| self.generate_class_attr_fn(self.class(*class_idx)));
        let class_children_fns = self
            .package_classes
            .iter()
            .map(|class_idx| self.generate_class_children_fn(self.class(*class_idx)));
        let class_writer_fns = self
            .package_classes
            .iter()
            .map(|class_idx| self.generate_class_writer_fn(self.class(*class_idx)));
        let union_writer_fns = self
            .package_classes
            .iter()
            .map(|class_idx| self.generate_union_writer_fn(self.class(*class_idx)));
        let query_impl = self.generate_query_impl();

        let tokens = quote! {
            #helpers
            #reference_helpers
            #(#class_attr_fns)*
            #(#class_children_fns)*
            #(#class_writer_fns)*
            #(#union_writer_fns)*
            #query_impl
        };

        Ok(Fragment::new(
            tokens,
            vec![
                Import::Protocol(Protocol::ObjectPath),
                Import::Protocol(Protocol::Read),
                Import::Protocol(Protocol::EvalNested),
                Import::Protocol(Protocol::QueryOperation),
            ],
            vec![],
        ))
    }
}
