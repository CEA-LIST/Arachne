//! Adapts namespace-aware XML nodes to the existing Ecore context.
//! roxmltree handles XML syntax, escaping, namespaces and source positions.

use log::warn;
use roxmltree::{Document, Node};

use crate::{
    ctx::{ClassCtx, PathCtx},
    prelude::*,
    repr::{structural, Annot, Bounds, ELit, Operation, Param, Structural},
};

const ECORE: &str = "http://www.eclipse.org/emf/2002/Ecore";
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
const XMI: &str = "http://www.omg.org/XMI";
const TYPED_ATTRIBUTES: &[&str] = &[
    "name",
    "eType",
    "lowerBound",
    "upperBound",
    "ordered",
    "unique",
];

pub fn parse(input: &str, ctx: &mut Ctx) -> Res<()> {
    let doc = Document::parse(input)
        .map_err(|err| error!("Failed to parse XML for `ecore:EPackage`: {}", err))?;
    let root = doc.root_element();
    if !root.has_tag_name((ECORE, "EPackage")) {
        return located(
            root,
            Err(error!("expected an `ecore:EPackage` root element")),
        );
    }
    package(root, &mut ctx.enter_root_pack()?)
}

fn package(node: Node<'_, '_>, ctx: &mut PathCtx<'_>) -> Res<()> {
    located(
        node,
        (|| {
            attributes(
                node,
                &[
                    "name",
                    "nsURI",
                    "nsPrefix",
                    "xmi:version",
                    "xsi:schemaLocation",
                ],
            )?;
            ctx.add_and_enter_sub_pack_mut(required(node, "name")?)?;
            if let Some(uri) = node.attribute("nsURI") {
                ctx.current_pack_mut().set_ns_uri(uri);
            }
            if let Some(prefix) = node.attribute("nsPrefix") {
                ctx.current_pack_mut().set_ns_prefix(prefix);
            }
            for child in elements(node)? {
                match element_name(child)? {
                    "EPackage" | "eSubpackages" => package(child, ctx)?,
                    "eAnnotations" => ctx.add_annotation(annotation(child)?),
                    "eClassifiers" => class(child, ctx)?,
                    _ => bail!("Unsupported package element `{}`", child.tag_name().name()),
                }
            }
            ctx.enter_sup_pack()?;
            Ok(())
        })(),
    )
}

fn class(node: Node<'_, '_>, ctx: &mut PathCtx<'_>) -> Res<()> {
    located(
        node,
        (|| {
            attributes(
                node,
                &[
                    "xsi:type",
                    "name",
                    "instanceTypeName",
                    "instanceClassName",
                    "abstract",
                    "interface",
                    "eSuperTypes",
                ],
            )?;
            let mut ctx = ctx.enter_class(
                xsi_type(node)?,
                required(node, "name")?,
                node.attribute("instanceTypeName"),
                boolean(node, "abstract")?,
                boolean(node, "interface")?,
            )?;
            if let Some(name) = node.attribute("instanceClassName") {
                ctx.set_instance_class_name(name);
            }
            for child in elements(node)? {
                match element_name(child)? {
                    "eAnnotations" => ctx.add_annotation(annotation(child)?),
                    "eLiterals" => ctx.add_literal(literal(child)?),
                    "eOperations" => {
                        let op = operation(child, &mut ctx)?;
                        ctx.add_operation(op);
                    }
                    "eStructuralFeatures" => {
                        let feature = feature(child, &mut ctx)?;
                        ctx.add_structural(feature);
                    }
                    _ => bail!(
                        "Unsupported classifier element `{}`",
                        child.tag_name().name()
                    ),
                }
            }
            for superclass in node
                .attribute("eSuperTypes")
                .into_iter()
                .flat_map(str::split_whitespace)
            {
                let idx = ctx.resolve_etype(&etype(node, superclass)?)?;
                ctx.add_sup_class(idx);
            }
            ctx.finalize();
            Ok(())
        })(),
    )
}

fn feature(node: Node<'_, '_>, ctx: &mut ClassCtx<'_, '_>) -> Res<Structural> {
    located(
        node,
        (|| {
            attributes(
                node,
                &[
                    "xsi:type",
                    "name",
                    "eType",
                    "lowerBound",
                    "upperBound",
                    "eOpposite",
                    "containment",
                    "iD",
                    "ordered",
                    "changeable",
                    "volatile",
                    "transient",
                    "derived",
                    "unsettable",
                    "unique",
                    "defaultValue",
                    "defaultValueLiteral",
                    "resolveProxies",
                ],
            )?;
            let name = required(node, "name")?;
            let kind = structural::Typ::from_xsi_type(xsi_type(node)?)?;
            let bounds =
                kind.parse_bounds(node.attribute("lowerBound"), node.attribute("upperBound"))?;
            let typ = ctx.resolve_etype(&etype(node, required(node, "eType")?)?)?;
            let mut feature = Structural::new(name, kind, typ, bounds);
            for (name, setter) in [
                (
                    "containment",
                    Structural::set_containment as fn(&mut Structural, bool),
                ),
                ("iD", Structural::set_is_id),
                ("ordered", Structural::set_ordered),
                ("changeable", Structural::set_changeable),
                ("volatile", Structural::set_volatile),
                ("transient", Structural::set_transient),
                ("derived", Structural::set_derived),
                ("unique", Structural::set_unique),
                ("resolveProxies", Structural::set_resolve_proxies),
            ] {
                if let Some(value) = boolean(node, name)? {
                    setter(&mut feature, value);
                }
            }
            feature.try_set_default_value(node.attribute("defaultValue"));
            feature.try_set_default_value_literal(node.attribute("defaultValueLiteral"));
            for flag in ["ordered", "unique"] {
                if bounds.ubound == Some(1) && node.attribute(flag).is_some() {
                    warn!("Structural feature `{}` sets `{}` while `upperBound=1`; this has no effect", name, flag);
                }
            }
            if boolean(node, "unsettable")? == Some(true) {
                warn!("Structural feature `{}` of class `{}` sets `unsettable=\"true\"`, which is currently not supported, ignoring",
                name, ctx.current().name());
            }
            if let Some(opposite) = node.attribute("eOpposite") {
                warn!("Structural feature `{}` of class `{}` sets `eOpposite=\"{}\"`: `eOpposite` attributes are currently not supported, ignoring",
                name, ctx.current().name(), opposite);
            }
            for annot in annotations(node)? {
                feature.add_annotation(annot);
            }
            Ok(feature)
        })(),
    )
}

fn operation(node: Node<'_, '_>, ctx: &mut ClassCtx<'_, '_>) -> Res<Operation> {
    located(
        node,
        (|| {
            attributes(node, TYPED_ATTRIBUTES)?;
            let mut op = Operation::new(
                required(node, "name")?,
                optional_type(node, ctx)?,
                bounds(node)?,
            );
            if let Some(value) = boolean(node, "ordered")? {
                op.set_ordered(value);
            }
            if let Some(value) = boolean(node, "unique")? {
                op.set_unique(value);
            }
            for child in elements(node)? {
                match element_name(child)? {
                    "eAnnotations" => op.add_annotation(annotation(child)?),
                    "eParameters" => op.add_parameter(parameter(child, ctx)?),
                    _ => bail!(
                        "Unsupported operation element `{}`",
                        child.tag_name().name()
                    ),
                }
            }
            Ok(op)
        })(),
    )
}

fn parameter(node: Node<'_, '_>, ctx: &mut ClassCtx<'_, '_>) -> Res<Param> {
    located(
        node,
        (|| {
            attributes(node, TYPED_ATTRIBUTES)?;
            empty(node)?;
            let mut param = Param::new(
                required(node, "name")?,
                bounds(node)?,
                optional_type(node, ctx)?,
            );
            if let Some(value) = boolean(node, "ordered")? {
                param.set_ordered(value);
            }
            if let Some(value) = boolean(node, "unique")? {
                param.set_unique(value);
            }
            Ok(param)
        })(),
    )
}

fn literal(node: Node<'_, '_>) -> Res<ELit> {
    located(
        node,
        (|| {
            attributes(node, &["name", "value", "literal"])?;
            let mut lit = ELit::new(
                required(node, "name")?,
                node.attribute("value")
                    .or_else(|| node.attribute("literal")),
            );
            for annot in annotations(node)? {
                lit.add_annotation(annot);
            }
            Ok(lit)
        })(),
    )
}

fn annotation(node: Node<'_, '_>) -> Res<Annot> {
    located(
        node,
        (|| {
            attributes(node, &["source", "references"])?;
            let mut annot = Annot::with_capacity(required(node, "source")?, 3);
            for reference in node
                .attribute("references")
                .into_iter()
                .flat_map(str::split_whitespace)
            {
                annot.add_reference(reference);
            }
            for detail in elements(node)? {
                if element_name(detail)? != "details" {
                    bail!(
                        "Unsupported annotation element `{}`",
                        detail.tag_name().name()
                    );
                }
                attributes(detail, &["key", "value"])?;
                empty(detail)?;
                annot.insert(required(detail, "key")?, required(detail, "value")?)?;
            }
            annot.shrink_to_fit();
            Ok(annot)
        })(),
    )
}

fn annotations(node: Node<'_, '_>) -> Res<Vec<Annot>> {
    elements(node)?
        .map(|child| {
            if element_name(child)? != "eAnnotations" {
                bail!(
                    "Unsupported element `{}`; expected an annotation",
                    child.tag_name().name()
                );
            }
            annotation(child)
        })
        .collect()
}

fn required<'a>(node: Node<'a, '_>, name: &str) -> Res<&'a str> {
    node.attribute(name).ok_or_else(|| {
        error!(
            "Missing attribute `{}` in `{}`",
            name,
            node.tag_name().name()
        )
    })
}

fn xsi_type(node: Node<'_, '_>) -> Res<String> {
    let typ = node.attribute((XSI, "type")).ok_or_else(|| {
        error!(
            "Missing attribute `xsi:type` in `{}`",
            node.tag_name().name()
        )
    })?;
    ecore_type(node, typ)
}

fn ecore_type(node: Node<'_, '_>, typ: &str) -> Res<String> {
    let (prefix, name) = typ
        .split_once(':')
        .map_or((None, typ), |(prefix, name)| (Some(prefix), name));
    if node.lookup_namespace_uri(prefix) != Some(ECORE) {
        bail!("Unsupported Ecore type `{}`", typ);
    }
    Ok(format!("ecore:{name}"))
}

/// Normalize an aliased EDataType QName for the existing Ecore link resolver.
fn etype(node: Node<'_, '_>, typ: &str) -> Res<String> {
    if let Some((head, tail)) = typ.split_once(char::is_whitespace) {
        if head.rsplit(':').next() == Some("EDataType") {
            return Ok(format!("{} {}", ecore_type(node, head)?, tail.trim_start()));
        }
    }
    Ok(typ.to_string())
}

fn optional_type(node: Node<'_, '_>, ctx: &mut ClassCtx<'_, '_>) -> Res<Option<idx::Class>> {
    node.attribute("eType")
        .map(|typ| ctx.resolve_etype(&etype(node, typ)?))
        .transpose()
}

fn bounds(node: Node<'_, '_>) -> Res<Bounds> {
    Bounds::from_typed_element_str(node.attribute("lowerBound"), node.attribute("upperBound"))
}

fn boolean(node: Node<'_, '_>, name: &str) -> Res<Option<bool>> {
    node.attribute(name)
        .map(|value| match value {
            "true" | "True" | "1" => Ok(true),
            "false" | "False" | "0" => Ok(false),
            _ => Err(error!("Expected boolean for `{}`, got `{}`", name, value)),
        })
        .transpose()
}

fn attributes(node: Node<'_, '_>, allowed: &[&str]) -> Res<()> {
    for attr in node.attributes() {
        let name = match (attr.namespace(), attr.name()) {
            (None, name) => name,
            (Some(XSI), "type") => "xsi:type",
            (Some(XSI), "schemaLocation") => "xsi:schemaLocation",
            (Some(XMI), "version") => "xmi:version",
            _ => bail!(
                "Unsupported namespaced attribute `{}` in `{}`",
                attr.name(),
                node.tag_name().name()
            ),
        };
        if !allowed.contains(&name) {
            bail!(
                "Unsupported attribute `{}` in `{}`",
                name,
                node.tag_name().name()
            );
        }
    }
    Ok(())
}

fn element_name<'a>(node: Node<'a, '_>) -> Res<&'a str> {
    if node.tag_name().namespace().is_some_and(|ns| ns != ECORE) {
        bail!(
            "Unsupported element namespace in `{}`",
            node.tag_name().name()
        );
    }
    Ok(node.tag_name().name())
}

fn elements<'a, 'input>(node: Node<'a, 'input>) -> Res<impl Iterator<Item = Node<'a, 'input>>> {
    if node
        .children()
        .any(|child| child.is_text() && !child.text().unwrap_or("").trim().is_empty())
    {
        bail!("Unexpected text in `{}`", node.tag_name().name());
    }
    Ok(node.children().filter(Node::is_element))
}

fn empty(node: Node<'_, '_>) -> Res<()> {
    if elements(node)?.next().is_some() {
        bail!("Unexpected content in `{}`", node.tag_name().name());
    }
    Ok(())
}

fn located<T>(node: Node<'_, '_>, result: Res<T>) -> Res<T> {
    result.context(|| {
        let pos = node.document().text_pos_at(node.range().start);
        format!(
            "at `{}`{}, line {}, column {}",
            node.tag_name().name(),
            node.attribute("name")
                .map(|name| format!(" named `{name}`"))
                .unwrap_or_default(),
            pos.row,
            pos.col
        )
    })
}
