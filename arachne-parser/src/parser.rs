//! Adapts namespace-aware XML nodes to the existing Ecore context.
//! roxmltree handles XML syntax, escaping, namespaces and source positions.

use std::borrow::Cow;

use log::warn;
use roxmltree::{Document, Node, TextPos};

use crate::{
    ctx::{ClassCtx, PathCtx},
    prelude::*,
    repr::{structural, Annot, Bounds, ELit, Operation, Param, Structural},
};

const ECORE: &str = "http://www.eclipse.org/emf/2002/Ecore";
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
const XMI: &str = "http://www.omg.org/XMI";
const XSI_DECLARATION: &str = " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";
const TYPED_ATTRIBUTES: &[&str] = &[
    "name",
    "eType",
    "lowerBound",
    "upperBound",
    "ordered",
    "unique",
];

pub fn parse(input: &str, ctx: &mut Ctx) -> Res<()> {
    let (input, positions) = with_xsi_namespace(input);
    let doc = Document::parse(&input).map_err(|err| {
        let message = err.to_string().replacen(
            &format!(" at {}", err.pos()),
            &format!(" at {}", positions.original(err.pos())),
            1,
        );
        error!("Failed to parse XML for `ecore:EPackage`: {}", message)
    })?;
    let root = doc.root_element();
    if !root.has_tag_name((ECORE, "EPackage")) {
        return located(
            &positions,
            root,
            Err(error!("expected an `ecore:EPackage` root element")),
        );
    }
    package(&positions, root, &mut ctx.enter_root_pack()?)
}

/// The injected declaration has no newline, so only columns on its line can move.
struct SourcePositions {
    inserted_at: Option<TextPos>,
}

impl SourcePositions {
    fn original(&self, mut pos: TextPos) -> TextPos {
        if let Some(start) = self.inserted_at {
            if pos.row == start.row && pos.col > start.col {
                pos.col = start
                    .col
                    .max(pos.col.saturating_sub(XSI_DECLARATION.len() as u32));
            }
        }
        pos
    }
}

fn with_xsi_namespace(input: &str) -> (Cow<'_, str>, SourcePositions) {
    let Some(offset) = missing_xsi_declaration(input) else {
        return (Cow::Borrowed(input), SourcePositions { inserted_at: None });
    };
    let before = &input[..offset];
    let pos = TextPos::new(
        before.bytes().filter(|byte| *byte == b'\n').count() as u32 + 1,
        before.chars().rev().take_while(|ch| *ch != '\n').count() as u32 + 1,
    );
    let mut declared = String::with_capacity(input.len() + XSI_DECLARATION.len());
    declared.push_str(before);
    declared.push_str(XSI_DECLARATION);
    declared.push_str(&input[offset..]);
    (
        Cow::Owned(declared),
        SourcePositions {
            inserted_at: Some(pos),
        },
    )
}

/// Find the root name, checking namespace attributes on every element. Quoted values,
/// comments, processing instructions, CDATA and DTD contents are not declarations.
fn missing_xsi_declaration(input: &str) -> Option<usize> {
    if !input.contains("xsi:") {
        return None;
    }
    let mut cursor = 0;
    let mut root_name_end = None;
    while let Some(relative) = input[cursor..].find('<') {
        let start = cursor + relative;
        let rest = &input[start..];
        let skipped = if rest.starts_with("<!--") {
            Some(("-->", 4))
        } else if rest.starts_with("<?") {
            Some(("?>", 2))
        } else if rest.starts_with("<![CDATA[") {
            Some(("]]>", 9))
        } else {
            None
        };
        if let Some((end, prefix)) = skipped {
            cursor = start + prefix + rest[prefix..].find(end)? + end.len();
            continue;
        }
        let is_element = !rest.starts_with("<!") && !rest.starts_with("</");
        if is_element && root_name_end.is_none() {
            root_name_end = Some(
                start
                    + 1
                    + rest[1..]
                        .find(|ch: char| ch.is_ascii_whitespace() || ch == '/' || ch == '>')?,
            );
        }
        let mut quote = None;
        let mut brackets = 0usize;
        cursor = input.len();
        for (relative, ch) in rest.char_indices().skip(1) {
            let offset = start + relative;
            if let Some(delimiter) = quote {
                if ch == delimiter {
                    quote = None;
                }
                continue;
            }
            match ch {
                '\'' | '"' => quote = Some(ch),
                '[' if !is_element => brackets += 1,
                ']' if !is_element => brackets = brackets.saturating_sub(1),
                '>' if brackets == 0 => {
                    cursor = offset + 1;
                    break;
                }
                'x' if is_element
                    && input.as_bytes()[offset - 1].is_ascii_whitespace()
                    && input[offset..]
                        .strip_prefix("xmlns:xsi")
                        .is_some_and(|tail| tail.trim_start().starts_with('=')) =>
                {
                    return None;
                }
                _ => {}
            }
        }
    }
    root_name_end
}

fn package(positions: &SourcePositions, node: Node<'_, '_>, ctx: &mut PathCtx<'_>) -> Res<()> {
    located(
        positions,
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
                    "EPackage" | "eSubpackages" => package(positions, child, ctx)?,
                    "eAnnotations" => ctx.add_annotation(annotation(positions, child)?),
                    "eClassifiers" => class(positions, child, ctx)?,
                    _ => bail!("Unsupported package element `{}`", child.tag_name().name()),
                }
            }
            ctx.enter_sup_pack()?;
            Ok(())
        })(),
    )
}

fn class(positions: &SourcePositions, node: Node<'_, '_>, ctx: &mut PathCtx<'_>) -> Res<()> {
    located(
        positions,
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
                    "eAnnotations" => ctx.add_annotation(annotation(positions, child)?),
                    "eLiterals" => ctx.add_literal(literal(positions, child)?),
                    "eOperations" => {
                        let op = operation(positions, child, &mut ctx)?;
                        ctx.add_operation(op);
                    }
                    "eStructuralFeatures" => {
                        let feature = feature(positions, child, &mut ctx)?;
                        ctx.add_structural(feature);
                    }
                    _ => bail!(
                        "Unsupported classifier element `{}`",
                        child.tag_name().name()
                    ),
                }
            }
            let mut supertypes = node
                .attribute("eSuperTypes")
                .into_iter()
                .flat_map(str::split_whitespace);
            while let Some(superclass) = supertypes.next() {
                let superclass = if superclass.rsplit(':').next() == Some("EClass") {
                    let head = ecore_type(node, superclass)?;
                    let path = supertypes
                        .next()
                        .ok_or_else(|| error!("Missing Ecore class path in `eSuperTypes`"))?;
                    format!("{head} {path}")
                } else {
                    etype(node, superclass)?
                };
                let idx = ctx.resolve_etype(&superclass)?;
                ctx.add_sup_class(idx);
            }
            ctx.finalize();
            Ok(())
        })(),
    )
}

fn feature(
    positions: &SourcePositions,
    node: Node<'_, '_>,
    ctx: &mut ClassCtx<'_, '_>,
) -> Res<Structural> {
    located(
        positions,
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
                    "eKeys",
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
                ("unsettable", Structural::set_unsettable),
                ("unique", Structural::set_unique),
                ("resolveProxies", Structural::set_resolve_proxies),
            ] {
                if let Some(value) = boolean(node, name)? {
                    setter(&mut feature, value);
                }
            }
            feature.try_set_default_value(node.attribute("defaultValue"));
            feature.try_set_default_value_literal(node.attribute("defaultValueLiteral"));
            for key in node
                .attribute("eKeys")
                .into_iter()
                .flat_map(str::split_whitespace)
            {
                feature.add_key(key);
            }
            for flag in ["ordered", "unique"] {
                if bounds.ubound == Some(1) && node.attribute(flag).is_some() {
                    warn!("Structural feature `{}` sets `{}` while `upperBound=1`; this has no effect", name, flag);
                }
            }
            if let Some(opposite) = node.attribute("eOpposite") {
                feature.set_opposite(opposite);
                warn!("Structural feature `{}` of class `{}` sets `eOpposite=\"{}\"`: `eOpposite` attributes are currently not supported, ignoring",
                name, ctx.current().name(), opposite);
            }
            for annot in annotations(positions, node)? {
                feature.add_annotation(annot);
            }
            Ok(feature)
        })(),
    )
}

fn operation(
    positions: &SourcePositions,
    node: Node<'_, '_>,
    ctx: &mut ClassCtx<'_, '_>,
) -> Res<Operation> {
    located(
        positions,
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
                    "eAnnotations" => op.add_annotation(annotation(positions, child)?),
                    "eParameters" => op.add_parameter(parameter(positions, child, ctx)?),
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

fn parameter(
    positions: &SourcePositions,
    node: Node<'_, '_>,
    ctx: &mut ClassCtx<'_, '_>,
) -> Res<Param> {
    located(
        positions,
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

fn literal(positions: &SourcePositions, node: Node<'_, '_>) -> Res<ELit> {
    located(
        positions,
        node,
        (|| {
            attributes(node, &["name", "value", "literal"])?;
            let mut lit = ELit::new(
                required(node, "name")?,
                node.attribute("value")
                    .or_else(|| node.attribute("literal")),
            );
            for annot in annotations(positions, node)? {
                lit.add_annotation(annot);
            }
            Ok(lit)
        })(),
    )
}

fn annotation(positions: &SourcePositions, node: Node<'_, '_>) -> Res<Annot> {
    located(
        positions,
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

fn annotations(positions: &SourcePositions, node: Node<'_, '_>) -> Res<Vec<Annot>> {
    elements(node)?
        .map(|child| {
            if element_name(child)? != "eAnnotations" {
                bail!(
                    "Unsupported element `{}`; expected an annotation",
                    child.tag_name().name()
                );
            }
            annotation(positions, child)
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

/// Normalize an aliased Ecore classifier QName for the existing Ecore link resolver.
fn etype(node: Node<'_, '_>, typ: &str) -> Res<String> {
    if let Some((head, tail)) = typ.split_once(char::is_whitespace) {
        if matches!(head.rsplit(':').next(), Some("EDataType" | "EClass")) {
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

fn located<T>(positions: &SourcePositions, node: Node<'_, '_>, result: Res<T>) -> Res<T> {
    result.context(|| {
        let pos = positions.original(node.document().text_pos_at(node.range().start));
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
