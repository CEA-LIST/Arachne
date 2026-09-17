//! Ecore's own classes that metamodels extend or use as types.
//!
//! A metamodel may name a class of Ecore itself, for instance SysON's `Element` extends
//! `EModelElement` so that its instances carry annotations, and features are often typed by
//! `EObject`. EMF resolves such a name to its compiled Ecore package. The context mirrors that
//! with a package named `ecore`, beside the `[builtin]` datatypes, holding the classes listed in
//! [`Typ`]. The package is added the first time a metamodel refers to one of its classes, so a
//! metamodel that never does parses exactly as if the package did not exist.
//!
//! The classes hold what the parser would hold after reading their declarations in EMF's
//! `Ecore.ecore`, with three differences:
//!
//! - `eOpposite` cannot be represented, so `EModelElement.eAnnotations` and
//!   `EAnnotation.eModelElement` do not know they are opposites;
//! - `EObject` is abstract, where `Ecore.ecore` declares it concrete;
//! - `EObject` has none of its operations (`eClass`, `eContainer`, `eGet`...), whose types are
//!   Ecore classes and datatypes outside this package.
//!
//! Features typed by `EString` use the builtin datatype.

prelude! {
    repr::*,
}

/// Namespace URI of Ecore.
pub const NS_URI: &str = "http://www.eclipse.org/emf/2002/Ecore";
/// Namespace prefix of Ecore.
pub const NS_PREFIX: &str = "ecore";
/// Name of the package holding Ecore's classes.
pub const PACKAGE_NAME: &str = "ecore";
/// Source of the annotations Ecore puts on its own classes.
const ANNOTATION_SOURCE: &str = NS_URI;

/// The classes of Ecore a metamodel can refer to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Typ {
    EAnnotation,
    EModelElement,
    ENamedElement,
    EObject,
    EStringToStringMapEntry,
}

impl Display for Typ {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.name().fmt(fmt)
    }
}

impl Typ {
    /// All of them, in the order `Ecore.ecore` declares them.
    pub const ALL: [Typ; 5] = [
        Typ::EAnnotation,
        Typ::EModelElement,
        Typ::ENamedElement,
        Typ::EObject,
        Typ::EStringToStringMapEntry,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Typ::EAnnotation => "EAnnotation",
            Typ::EModelElement => "EModelElement",
            Typ::ENamedElement => "ENamedElement",
            Typ::EObject => "EObject",
            Typ::EStringToStringMapEntry => "EStringToStringMapEntry",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|typ| typ.name() == name)
    }

    fn is_abstract(self) -> bool {
        matches!(self, Typ::EModelElement | Typ::ENamedElement | Typ::EObject)
    }

    fn instance_class_name(self) -> Option<&'static str> {
        match self {
            Typ::EStringToStringMapEntry => Some("java.util.Map$Entry"),
            _ => None,
        }
    }
}

/// Path, inside Ecore, of the classifier `s` names by Ecore's namespace URI.
///
/// `s` is the value of an `eType` or of one `eSuperTypes` token: `ecore:EClass` may lead it, as it
/// leads an `eType`. Returns `None` if `s` does not name Ecore by its namespace URI.
///
/// ```rust
/// # use ecore_rs::repr::ecore::classifier_path;
/// let uri = "http://www.eclipse.org/emf/2002/Ecore#//EModelElement";
/// assert_eq!(classifier_path(uri), Some("EModelElement"));
/// let etype = "ecore:EClass http://www.eclipse.org/emf/2002/Ecore#//EObject";
/// assert_eq!(classifier_path(etype), Some("EObject"));
/// assert_eq!(classifier_path("#//EObject"), None);
/// ```
pub fn classifier_path(s: &str) -> Option<&str> {
    let s = s.trim();
    let s = s
        .strip_prefix("ecore:EClass")
        .filter(|rest| rest.starts_with(char::is_whitespace))
        .map_or(s, str::trim_start);
    s.strip_prefix(NS_URI)?.strip_prefix("#//")
}

/// Adds the Ecore package and its classes to `ctx`, and returns the package and the class of each
/// [`Typ`], in the order of [`Typ::ALL`].
pub(crate) fn populate(ctx: &mut Ctx) -> Res<(idx::Pack, [idx::Class; 5])> {
    let top = ctx.top_pack();
    let pack = ctx.raw_add_pack(PACKAGE_NAME, top);
    ctx[pack].set_ns_uri(NS_URI);
    ctx[pack].set_ns_prefix(NS_PREFIX);
    let path = Path::of_idx(ctx, pack);

    let mut classes = Vec::with_capacity(Typ::ALL.len());
    for typ in Typ::ALL {
        let idx = ctx.raw_add_class(|idx| {
            let mut class = Class::new(
                idx,
                path.clone(),
                "ecore:EClass",
                typ.name(),
                None as Option<String>,
                Some(typ.is_abstract()),
                None,
            );
            if let Some(name) = typ.instance_class_name() {
                class.set_instance_class_name(name)
            }
            class
        })?;
        classes.push(idx);
    }
    let classes: [idx::Class; 5] = classes
        .try_into()
        .expect("[fatal] one class per Ecore type");
    // `Typ::ALL` lists the variants in declaration order.
    let class = |typ: Typ| classes[typ as usize];
    let string = ctx.get_builtin_idx(builtin::Typ::EString)?;

    use structural::Typ::{EAttribute, EReference};
    let feature = |name: &str, kind: structural::Typ, typ, ubound| -> Res<Structural> {
        Ok(Structural::new(
            name,
            kind,
            typ,
            kind.parse_bounds(None, ubound)?,
        ))
    };
    let constraints = |constraints: &str| -> Res<Annot> {
        let mut annot = Annot::with_capacity(ANNOTATION_SOURCE, 1);
        annot.insert("constraints", constraints)?;
        Ok(annot)
    };

    // `EAnnotation`, `Ecore.ecore` lines 11-24.
    let annotation = class(Typ::EAnnotation);
    ctx.add_sup_class(class(Typ::EModelElement), annotation);
    ctx[annotation].add_annotation(constraints("WellFormed WellFormedSourceURI")?);
    ctx[annotation].add_structural(feature("source", EAttribute, string, None)?);
    let mut details = feature(
        "details",
        EReference,
        class(Typ::EStringToStringMapEntry),
        Some("-1"),
    )?;
    details.set_containment(true);
    details.set_resolve_proxies(false);
    ctx[annotation].add_structural(details);
    // The container of the annotation, opposite of `EModelElement.eAnnotations`.
    let mut model_element = feature("eModelElement", EReference, class(Typ::EModelElement), None)?;
    model_element.set_transient(true);
    model_element.set_resolve_proxies(false);
    ctx[annotation].add_structural(model_element);
    let mut contents = feature("contents", EReference, class(Typ::EObject), Some("-1"))?;
    contents.set_containment(true);
    contents.set_resolve_proxies(false);
    ctx[annotation].add_structural(contents);
    let references = feature("references", EReference, class(Typ::EObject), Some("-1"))?;
    ctx[annotation].add_structural(references);

    // `EModelElement`, `Ecore.ecore` lines 183-189.
    let model_element = class(Typ::EModelElement);
    let mut get_annotation = Operation::new(
        "getEAnnotation",
        Some(class(Typ::EAnnotation)),
        Bounds::from_typed_element_str(None, None)?,
    );
    get_annotation.add_parameter(Param::new(
        "source",
        Bounds::from_typed_element_str(None, None)?,
        string,
    ));
    ctx[model_element].add_operation(get_annotation);
    // Opposite of `EAnnotation.eModelElement`.
    let mut annotations = feature(
        "eAnnotations",
        EReference,
        class(Typ::EAnnotation),
        Some("-1"),
    )?;
    annotations.set_containment(true);
    annotations.set_resolve_proxies(false);
    ctx[model_element].add_structural(annotations);

    // `ENamedElement`, `Ecore.ecore` lines 190-195.
    let named_element = class(Typ::ENamedElement);
    ctx.add_sup_class(class(Typ::EModelElement), named_element);
    ctx[named_element].add_annotation(constraints("WellFormedName")?);
    ctx[named_element].add_structural(feature("name", EAttribute, string, None)?);

    // `EObject`, `Ecore.ecore` line 196, has no structural feature.

    // `EStringToStringMapEntry`, `Ecore.ecore` lines 493-496.
    let entry = class(Typ::EStringToStringMapEntry);
    ctx[entry].add_structural(feature("key", EAttribute, string, None)?);
    ctx[entry].add_structural(feature("value", EAttribute, string, None)?);

    Ok((pack, classes))
}
