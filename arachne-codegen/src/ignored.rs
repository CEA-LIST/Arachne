//! What `describe` reads past: constructs EMF writes routinely and the descriptor does not carry.
//!
//! The parser refuses them, and it is Léo's, so they are blanked in the text before it reads it:
//! each ignored byte becomes a space, line breaks stay, so every line and column the parser reports
//! is the file's own. Besides `xmi:id` and `eFactoryInstance` on a package, `eExceptions` on an
//! operation, a `details` with neither `value` nor child element and a `contents` or `references`
//! child of an annotation, nothing is ignored.

use std::ops::Range;

use roxmltree::{Document, Node};

const ECORE: &str = "http://www.eclipse.org/emf/2002/Ecore";
const XMI: &str = "http://www.omg.org/XMI";

/// Where an element sits, for the location of what is ignored in it.
struct Place {
    local: String,
    loc: Option<String>,
    /// The `::` path of a package below the root, `Some` for a package.
    sub: Option<String>,
    annotation: bool,
    /// `loc` is the root package's own, so a dependency's name is not put in front of it.
    root: bool,
}

struct Scan {
    blank: Vec<Range<usize>>,
    /// (kind, location), the location as `take_dropped` prints it.
    found: Vec<(&'static str, String)>,
    /// The name to put in front of a classifier key, for a file loaded as a dependency.
    prefix: Option<String>,
}

impl Scan {
    fn found(&mut self, kind: &'static str, place: &Place) {
        let loc = place.loc.clone().unwrap_or_else(|| "None".to_owned());
        let loc = match &self.prefix {
            Some(prefix) if !place.root => format!("{prefix}::{loc}"),
            _ => loc,
        };
        self.found.push((kind, loc));
    }

    fn element(&mut self, node: Node, parent: Option<&Place>) {
        let tag = node.tag_name();
        let (ns, local) = (tag.namespace(), tag.name());
        let ours = matches!(ns, None | Some(ECORE));
        let name = node.attribute("name");
        let mut loc = parent.and_then(|p| p.loc.clone());
        let mut root = parent.is_some_and(|p| p.root);
        let mut sub = None;
        match parent {
            None if node.has_tag_name((ECORE, "EPackage")) => {
                (loc, sub, root) = (name.map(str::to_owned), Some(String::new()), true);
            }
            Some(p) if ours && p.sub.is_some() && matches!(local, "EPackage" | "eSubpackages") => {
                let path = p.sub.as_deref().unwrap();
                let path = format!(
                    "{path}{}{}",
                    if path.is_empty() { "" } else { "::" },
                    name.unwrap_or("")
                );
                (loc, sub, root) = (Some(path.clone()), Some(path), false);
            }
            Some(p) if ours && p.sub.is_some() && local == "eClassifiers" => {
                let path = p.sub.as_deref().unwrap();
                let key = format!(
                    "{path}{}{}",
                    if path.is_empty() { "" } else { "::" },
                    name.unwrap_or("")
                );
                (loc, root) = (Some(key), false);
            }
            Some(p)
                if ours
                    && matches!(p.local.as_str(), "eClassifiers" | "eOperations")
                    && matches!(
                        local,
                        "eStructuralFeatures" | "eOperations" | "eLiterals" | "eParameters"
                    ) =>
            {
                loc = Some(format!(
                    "{}.{}",
                    p.loc.as_deref().unwrap_or("None"),
                    name.unwrap_or("None")
                ));
                root = false;
            }
            Some(p) if ours && p.annotation => {
                let kind = match local {
                    "details" if !node.has_attribute("value") => Some("annotationDetail"),
                    "contents" => Some("annotationContents"),
                    "references" => Some("annotationReferences"),
                    _ => None,
                };
                if let Some(kind) = kind {
                    // A `details` with a child element keeps the parser's refusal.
                    if kind != "annotationDetail" || !node.children().any(|c| c.is_element()) {
                        self.blank.push(node.range());
                        self.found(kind, p);
                    }
                    return;
                }
            }
            _ => {}
        }
        let place = Place {
            local: local.to_owned(),
            loc,
            sub,
            annotation: ours && local == "eAnnotations",
            root,
        };
        if place.sub.is_some() {
            for attribute in node.attributes() {
                let kind = match (attribute.namespace(), attribute.name()) {
                    (Some(XMI), "id") => "xmi:id",
                    (None, "eFactoryInstance") => "eFactoryInstance",
                    _ => continue,
                };
                self.blank.push(attribute.range());
                self.found(kind, &place);
            }
        }
        if ours
            && local == "eOperations"
            && let Some(attribute) = node
                .attributes()
                .find(|a| a.namespace().is_none() && a.name() == "eExceptions")
        {
            self.blank.push(attribute.range());
            self.found("eExceptions", &place);
        }
        for child in node.children().filter(Node::is_element) {
            self.element(child, Some(&place));
        }
    }
}

/// `text` with every ignored construct replaced by spaces, and the `warning: dropped` lines of
/// those constructs. A file whose XML roxmltree refuses comes back unchanged: the parser refuses it
/// the same way. A dependency's classifier keys carry its package's name, as EC.04 keys imported
/// classes.
pub(crate) fn blank(text: &str, dependency: bool) -> (String, Vec<String>) {
    let Ok(document) = Document::parse(text) else {
        return (text.to_owned(), Vec::new());
    };
    let root = document.root_element();
    let mut scan = Scan {
        blank: Vec::new(),
        found: Vec::new(),
        prefix: dependency.then(|| root.attribute("name").unwrap_or("None").to_owned()),
    };
    scan.element(root, None);
    let mut bytes = text.as_bytes().to_vec();
    for range in scan.blank {
        for byte in &mut bytes[range] {
            if !matches!(byte, b'\r' | b'\n') {
                *byte = b' ';
            }
        }
    }
    let lines = scan
        .found
        .into_iter()
        .map(|(kind, at)| format!("warning: dropped {kind} at {at}"))
        .collect();
    // Whole characters are blanked, so the bytes stay valid UTF-8.
    (
        String::from_utf8(bytes).expect("blanked text is UTF-8"),
        lines,
    )
}
