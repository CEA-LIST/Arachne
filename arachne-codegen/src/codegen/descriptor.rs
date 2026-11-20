//! Metamodel descriptor emission.
//!
//! The generated CRDT crates keep none of the metamodel the generator read:
//! `classifiers.rs` is pure CRDT type aliases. This module writes the part a
//! *client* needs back out as data — a `metamodel.json` the generated
//! `network_node` serves on `GET /api/metamodel`, so an editor can shape
//! itself to whatever node it connects to without compiling against the
//! generated types.
//!
//! # Descriptor format (`formatVersion` 2)
//!
//! ```json
//! {
//!   "formatVersion": 2,
//!   "package": "behaviortree",
//!   "nsURI": "http://www.example.org/behaviortree",
//!   "rootClasses": ["Root"],
//!   "classes": {
//!     "TreeNode": {
//!       "abstract": true,
//!       "superTypes": [],
//!       "attributes": [
//!         {"name": "ID", "kind": "string", "many": false,
//!          "required": true, "isId": true,
//!          "facets": {"ordered": null, "unique": null}, "annotation": null,
//!          "merge": {"kind": "attribute", "shape": {"kind": "single"},
//!                    "leaf": {"kind": "text"}},
//!          "provenance": {"ordered": "notApplicable", "unique": "notApplicable",
//!                         "leaf": "declared", "presence": "declared"}}
//!       ],
//!       "containments": [
//!         {"name": "children", "target": "TreeNode", "many": true,
//!          "required": false, "ordered": true,
//!          "facets": {"ordered": null, "unique": null}, "annotation": null,
//!          "merge": {"kind": "containment", "shape": {"kind": "sequence"},
//!                    "target": "TreeNode"},
//!          "provenance": {"ordered": "houseDefault", "unique": "notApplicable",
//!                         "leaf": "notApplicable", "presence": "declared"}}
//!       ],
//!       "references": [
//!         {"name": "entry", "target": "BlackboardEntry", "many": false,
//!          "required": false,
//!          "facets": {"ordered": null, "unique": null}, "annotation": null,
//!          "merge": {"kind": "reference", "many": false,
//!                    "target": "BlackboardEntry"},
//!          "provenance": {"ordered": "notApplicable", "unique": "notApplicable",
//!                         "leaf": "notApplicable", "presence": "notApplicable"}}
//!       ]
//!     }
//!   },
//!   "enums": {"Status": ["RUNNING", "SUCCESS", "FAILURE"]}
//! }
//! ```
//!
//! Rules:
//! - `kind` is one of `string`, `int`, `float`, `bool`, `enum`; an `enum`
//!   attribute additionally names its enum class under `"enum"`.
//! - `required` means `lowerBound >= 1`; `many` means the upper bound is
//!   unbounded or greater than one.
//! - A class lists its **declared** features only, plus `superTypes`; clients
//!   flatten the inheritance closure themselves. Enum classes appear only
//!   under `enums`.
//! - A class carrying `urn:arachne:representation` `kind="transparent"` adds
//!   one key, `"transparent"`, naming the field it is represented by. The key
//!   is absent on every other class.
//! - The class set is what `describe` reaches from the root classes, which is
//!   not only what code generation reaches: it also lists `EObject` and the
//!   targets of plain references, so that every name the descriptor writes
//!   resolves.
//! - A classifier of a subpackage is keyed by its root-relative `::` path (`catalog::Item`,
//!   `a::b::Item`) in every place a name is written; classifiers of the root package keep bare
//!   names. When a subpackage classifier is written, a top-level `"packages"` map lists every
//!   subpackage at every depth by that path, as `{"nsURI": ..., "nsPrefix": ...}`; otherwise the
//!   key is absent. `package` and `nsURI` stay the root's.
//! - A classifier is a class, an enum or a datatype by its `xsi:type`. A
//!   datatype is never a class or a root: an attribute typed by one is
//!   described by its instance class, and an enum with no literal is listed
//!   under `enums` as `[]`.
//!
//! # What version 2 added, and why nothing broke
//!
//! Version 1 described a metamodel for an *editor*: enough shape to draw a
//! form. It said nothing about how two concurrent writes to a feature settle,
//! so a node that wanted to merge by the metamodel had to be compiled against
//! it. Version 2 adds, on every attribute, containment and reference entry:
//!
//! - `facets` — `ordered` and `unique` exactly as the `.ecore` file wrote
//!   them, `null` where it was silent.
//! - `annotation` — the `urn:arachne:semantics` `datatype` value that governs
//!   the feature, or `null`: what
//!   [`feature_datatype`](crate::codegen::semantics::feature_datatype)
//!   answers, the feature's own detail or, for an attribute of one of Ecore's
//!   own classes, the package-level detail of the metamodel that names it.
//! - `merge` — the CRDT construction the generator compiles for this feature,
//!   as data, in the closed vocabulary of `moirai_semantics::MergeRule`.
//! - `provenance` — where each of the four facets of that rule came from:
//!   `declared`, `ecoreDefault`, `houseDefault`, `annotation` or
//!   `notApplicable`. This is what keeps "derived" an honest word: a policy
//!   nobody wrote down says so in the file that publishes it.
//!
//! The derivation is [`crate::codegen::semantics::merge_rule`], and its module
//! documents every reading it takes. **Keys were added and none removed**, so
//! the phase 4 `Schema` parser (`deployment/conformance.rs`) and the model
//! editor keep working on a version 2 descriptor: both read only the keys they
//! name.
//!
//! # Identity
//!
//! A descriptor's identity on the model plane is `{nsURI, digest}`, where the
//! digest is [`metamodel_digest`]: a function of the descriptor's content and
//! nothing else, so a reformatted file keeps its models and an edited one
//! does not.

use std::{
    cell::RefCell,
    collections::{BTreeSet, HashSet},
};

use ecore_rs::{
    ctx::Ctx,
    repr::{Annots, Class, Pack, builtin, idx, structural},
};
use log::warn;
use moirai_semantics::{KeyKind, LeafRule, MergeRule, Shape};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::codegen::{ecore, semantics};
use crate::error::{ArachneError, Result};
use crate::literals::{Literals, value_json};

/// Version of the descriptor layout above. Bump on any breaking change so
/// clients can refuse what they do not understand.
const FORMAT_VERSION: u64 = 2;

/// Builds the JSON metamodel descriptor for `pack`.
///
/// Fails with [`ArachneError::RootClassNotFound`] when the package has no
/// root class, with [`ArachneError::UnlistedComponent`] when the roots are abstract classes that
/// leave a containment component's classes out of the descriptor, or
/// [`ArachneError::NoEClass`] when it declares no EClass at all, or
/// [`ArachneError::NoConcreteEClass`] when it declares none but abstract ones and interfaces;
/// with [`ArachneError::DescriptorNotClosed`] when a name the descriptor
/// writes is not one it writes.
pub fn descriptor_json(ctx: &Ctx, pack: &Pack) -> Result<Value> {
    let own_classes = tree_classes(ctx, pack);
    descriptor_with_roots(ctx, pack, &own_classes, &Literals::new())
}

/// The classes of `pack` and of its subpackages at every depth.
pub(crate) fn tree_classes(ctx: &Ctx, pack: &Pack) -> Vec<idx::Class> {
    let mut classes: Vec<idx::Class> = pack.classes().iter().copied().collect();
    for sub in pack.sub() {
        classes.extend(tree_classes(ctx, &ctx[*sub]));
    }
    classes
}

/// The `packages` map: every subpackage of `pack` at every depth by its root-relative path. A
/// subpackage that has no nsURI of its own, shares one, or is named `ecore` (which would key its
/// classifiers like Ecore's own) is refused, as a subpackage nsURI is what names its namespace.
fn subpackages(
    ctx: &Ctx,
    pack: &Pack,
    prefix: &str,
    seen: &mut Vec<String>,
    map: &mut Map<String, Value>,
) -> Result<()> {
    for sub in pack.sub() {
        let sub = &ctx[*sub];
        let path = format!("{prefix}{}", sub.name());
        if path == ecore_rs::repr::ecore::PACKAGE_NAME {
            return Err(ArachneError::Subpackage(format!(
                "`{path}` would key its classifiers like Ecore's own"
            )));
        }
        let Some(ns_uri) = sub.ns_uri().filter(|ns_uri| !ns_uri.is_empty()) else {
            return Err(ArachneError::Subpackage(format!("`{path}` has no nsURI")));
        };
        if seen.iter().any(|seen| seen == ns_uri) {
            return Err(ArachneError::Subpackage(format!(
                "`{path}` shares the nsURI `{ns_uri}`"
            )));
        }
        seen.push(ns_uri.to_string());
        map.insert(
            path.clone(),
            json!({"nsURI": ns_uri, "nsPrefix": sub.ns_prefix()}),
        );
        subpackages(ctx, sub, &format!("{path}::"), seen, map)?;
    }
    Ok(())
}

pub(crate) fn descriptor_with_roots(
    ctx: &Ctx,
    pack: &Pack,
    own_classes: &[idx::Class],
    literals: &Literals,
) -> Result<Value> {
    // The class set `describe` reaches: the package's classes, then Ecore's own but
    // `EObject`, which the generator never emits.
    let package_classes = tree_classes(ctx, pack);
    let ecore_classes: Vec<idx::Class> = match ctx.ecore_pack() {
        Some(ecore_pack) => ctx[ecore_pack]
            .classes()
            .iter()
            .copied()
            .filter(|class| !ecore::is_eobject(ctx, *class))
            .collect(),
        None => Vec::new(),
    };
    let generated: Vec<idx::Class> = package_classes
        .iter()
        .chain(ecore_classes.iter())
        .copied()
        .collect();
    let generated_set: HashSet<idx::Class> = generated.iter().copied().collect();

    let (roots, beside) = crate::codegen::reachability::compute_top_level_roots(
        ctx,
        own_classes,
        &generated,
        &generated_set,
    );
    if roots.is_empty() {
        let name = pack.name().to_string();
        let has = |pred: fn(&ecore_rs::repr::Class) -> bool| {
            own_classes.iter().any(|class| pred(&ctx[*class]))
        };
        return Err(if has(crate::codegen::reachability::is_concrete_eclass) {
            ArachneError::RootClassNotFound(name)
        } else if has(semantics::is_eclass) {
            ArachneError::NoConcreteEClass(name)
        } else {
            ArachneError::NoEClass(name)
        });
    }

    // `EObject` is *listed* although it is not generated. It has no feature,
    // so listing it adds no rule and no field; what it adds is that every
    // name the descriptor writes resolves. A metamodel may declare `EObject`
    // as a supertype (SysON's does, and so does the pet metamodel) and may
    // type a feature by it, and `moirai-semantics` refuses a supertype or a
    // target it cannot find. Dropping the supertype silently would also
    // contradict EMF, where an explicit `EObject` supertype is visible in the
    // reflective API even though it contributes nothing (spec 05 §2.4).
    let mut described_set = generated_set.clone();
    if let Some(eobject) = ecore::eobject(ctx) {
        described_set.insert(eobject);
    }

    let mut reachable: HashSet<idx::Class> = HashSet::new();
    for root in &roots {
        reachable.extend(crate::codegen::reachability::collect_reachable_classes(
            ctx,
            *root,
            &described_set,
        ));
    }
    reachable.extend(
        package_classes
            .iter()
            .filter(|idx| !own_classes.contains(idx))
            .copied(),
    );
    let package_set: HashSet<idx::Class> = package_classes.iter().copied().collect();
    crate::codegen::reachability::follow_reference_targets(
        ctx,
        &mut reachable,
        &package_set,
        &described_set,
    );
    // An abstract class of Ecore that nothing generated extends is left out.
    reachable.retain(|class| {
        !ctx.is_ecore_class(*class)
            || !crate::codegen::reachability::is_uninhabited_polymorphic_class(ctx, &ctx[*class])
    });

    if let Some(component) = beside
        .iter()
        .find(|component| component.iter().any(|class| !reachable.contains(class)))
    {
        let mut names: Vec<String> = component
            .iter()
            .map(|class| format!("`{}`", ctx[*class].name()))
            .collect();
        names.sort_unstable();
        return Err(ArachneError::UnlistedComponent(
            pack.name().to_string(),
            names.join(", "),
        ));
    }

    let mut root_names: Vec<String> = roots
        .iter()
        .map(|idx| semantics::descriptor_class_name(ctx, pack, *idx))
        .collect::<Result<_>>()?;
    root_names.sort_unstable();

    // What the descriptor writes, by identity: its classes, and the enums of the package.
    let written: HashSet<idx::Class> = reachable
        .iter()
        .copied()
        .filter(|class| semantics::is_eclass(&ctx[*class]))
        .chain(
            package_classes
                .iter()
                .copied()
                .filter(|class| semantics::is_eenum(&ctx[*class])),
        )
        .collect();

    // Sorted maps so the emitted descriptor is deterministic.
    let mut classes = Map::new();
    // Only a descriptor that writes a subpackage classifier says where the subpackages are. Its
    // refusals answer before the class loop's, so the cause line says `subpackage`.
    let packages = if written
        .iter()
        .any(|class| ctx[*class].path.last() != pack.idx && !ctx.is_ecore_class(*class))
    {
        let mut packages = Map::new();
        let mut seen: Vec<String> = pack.ns_uri().map(str::to_string).into_iter().collect();
        subpackages(ctx, pack, "", &mut seen, &mut packages)?;
        Some(packages)
    } else {
        None
    };
    let mut included: Vec<(String, &Class)> = reachable
        .iter()
        .map(|idx| &ctx[*idx])
        .filter(|class| semantics::is_eclass(class))
        .map(|class| {
            Ok((
                semantics::descriptor_class_name(ctx, pack, class.idx)?,
                class,
            ))
        })
        .collect::<Result<_>>()?;
    // By the key the class is written under, so a refusal names the same class on every run.
    included.sort_by(|a, b| a.0.cmp(&b.0));
    let mut dropped = Vec::new();
    for (key, class) in included {
        dropped_in_class(class, &key, &mut dropped);
        if classes
            .insert(key.clone(), class_descriptor(ctx, pack, class, &written)?)
            .is_some()
        {
            // Two classifiers on one key would silently become one class with
            // one of the two feature sets. `ecore::` cannot be spelled in a
            // well-formed Ecore name, so this is unreachable for a metamodel
            // EMF would accept; it is checked because the descriptor is
            // emitted for metamodels EMF has never seen.
            return Err(ArachneError::DuplicateDescriptorClass(key));
        }
    }

    // Every enum of the package, whether a feature reaches it or not: an enum
    // only used by operations is still part of the metamodel's vocabulary.
    let mut enums = Map::new();
    let mut enum_literals = Map::new();
    let mut package_enums: Vec<&Class> = package_classes
        .iter()
        .map(|idx| &ctx[*idx])
        .filter(|class| semantics::is_eenum(class))
        .collect();
    package_enums.sort_unstable_by_key(|class| class.name());
    for class in package_enums {
        let key = semantics::descriptor_class_name(ctx, pack, class.idx)?;
        dropped_in_class(class, &key, &mut dropped);
        let names: Vec<&str> = class.literals().iter().map(|lit| lit.name()).collect();
        // shortcut: without the file's text (`descriptor_json`) the parser's one text per literal
        // is the `value` and `literal` is null; `describe` always passes the text's literals.
        let carried = literals.get(&key).cloned().unwrap_or_else(|| {
            class
                .literals()
                .iter()
                .map(|lit| {
                    json!({"name": lit.name(), "value": lit.value().map(value_json), "literal": null})
                })
                .collect()
        });
        enum_literals.insert(key.clone(), json!(carried));
        enums.insert(key, json!(names));
    }

    let mut descriptor = json!({
        "formatVersion": FORMAT_VERSION,
        "package": pack.name(),
        "nsURI": pack.ns_uri(),
        "rootClasses": root_names,
        "classes": classes,
        "enums": enums,
        "enumLiterals": enum_literals,
        "nsPrefix": pack.ns_prefix(),
    });
    if let Some(packages) = packages {
        descriptor["packages"] = Value::Object(packages);
    }
    if has_foreign_annotation(pack.annotations()) {
        dropped.push(("eAnnotation", pack.name().to_owned(), String::new()));
    }
    for class in own_classes.iter().map(|class| &ctx[*class]) {
        if class.typ() == "ecore:EDataType" {
            let key = semantics::descriptor_class_name(ctx, pack, class.idx)?;
            dropped.push(("eDataType", key, String::new()));
        }
    }
    // The operations of classes this descriptor does not list: a descriptor that imports the
    // package may list them, and `import_class` leaves operations behind.
    let unlisted = package_classes
        .iter()
        .map(|idx| &ctx[*idx])
        .filter(|class| semantics::is_eclass(class) && !written.contains(&class.idx))
        .filter_map(|class| {
            let key = semantics::descriptor_class_name(ctx, pack, class.idx).ok()?;
            Some(
                class
                    .operations()
                    .iter()
                    .map(move |operation| ("eOperation", key.clone(), operation.name().to_owned())),
            )
        })
        .flatten()
        .collect();
    DROPPED.with_borrow_mut(|builds| {
        if let Some(builds) = builds {
            builds.push(Build {
                pack: pack.name().to_owned(),
                ns_uri: pack.ns_uri().map(str::to_owned),
                dropped,
                unlisted,
            });
        }
    });
    Ok(descriptor)
}

/// A construct a descriptor build read and did not write: its kind, the classifier (the package
/// for a package's own) and the feature, operation or literal, empty when the construct is the
/// classifier's.
type Dropped = (&'static str, String, String);

/// What one descriptor build dropped, and the package it described.
struct Build {
    pack: String,
    ns_uri: Option<String>,
    dropped: Vec<Dropped>,
    /// Operations of the classes the descriptor does not list, for a descriptor that imports them.
    unlisted: Vec<Dropped>,
}

thread_local! {
    /// The builds since [`record_dropped`] on this thread, `None` while nothing asked for them:
    /// `describe` builds a dependency's descriptor before the one that imports it, so the last is
    /// the file's.
    static DROPPED: RefCell<Option<Vec<Build>>> = const { RefCell::new(None) };
    /// The lines of the constructs `describe` blanked before the parser read them, while recording.
    static IGNORED: RefCell<Option<BTreeSet<String>>> = const { RefCell::new(None) };
}

/// Starts recording what the descriptors built on this thread drop, for [`take_dropped`], and
/// forgets what was recorded before. Without it, building a descriptor records nothing.
pub fn record_dropped() {
    DROPPED.replace(Some(Vec::new()));
    IGNORED.replace(Some(BTreeSet::new()));
}

/// Records the `warning: dropped` lines of constructs blanked in a file that was read.
pub(crate) fn record_ignored(lines: Vec<String>) {
    IGNORED.with_borrow_mut(|set| {
        if let Some(set) = set {
            set.extend(lines);
        }
    });
}

/// The `warning: dropped <kind> at <location>` lines of the descriptor `describe` just built, byte
/// sorted and unique, for the classifiers `descriptor` lists, the package and its datatypes.
/// Stops the recording of this thread, which holds the builds since [`record_dropped`]: call it
/// after the one `describe` that follows.
pub fn take_dropped(descriptor: &Value) -> Vec<String> {
    let line = |(kind, class, member): &Dropped| {
        let at = if member.is_empty() {
            class.clone()
        } else {
            format!("{class}.{member}")
        };
        format!("warning: dropped {kind} at {at}")
    };
    let mut builds = DROPPED.take().unwrap_or_default();
    let mut lines: BTreeSet<String> = builds
        .pop()
        .into_iter()
        .flat_map(|build| build.dropped)
        .map(|dropped| line(&dropped))
        .collect();
    lines.extend(IGNORED.take().unwrap_or_default());
    // `import_class` copies everything of a dependency's class but its operations, which only the
    // dependency's own build saw, under the key the importing descriptor writes. The class it
    // wrote has the dependency's nsURI as origin; a class of another package under that key has
    // not.
    for build in builds {
        for (kind, class, member) in build.dropped.into_iter().chain(build.unlisted) {
            let key = format!("{}::{class}", build.pack);
            let from_build = build.ns_uri.as_deref().is_some_and(|ns_uri| {
                descriptor["classes"]
                    .get(&key)
                    .is_some_and(|written| written["origin"] == ns_uri)
            });
            if kind == "eOperation" && from_build {
                lines.insert(line(&(kind, key, member)));
            }
        }
    }
    lines.into_iter().collect()
}

/// An annotation of a source the descriptor does not read.
fn has_foreign_annotation(annotations: &Annots) -> bool {
    annotations.iter().any(|annotation| {
        !matches!(
            annotation.source(),
            "urn:arachne:semantics" | "urn:arachne:representation" | "subsets" | "redefines"
        )
    })
}

/// What `class` (written under `key`) and its members say that the descriptor does not.
fn dropped_in_class(class: &Class, key: &str, out: &mut Vec<Dropped>) {
    let mut at = |kind, member: &str| out.push((kind, key.to_owned(), member.to_owned()));
    if has_foreign_annotation(class.annotations()) {
        at("eAnnotation", "");
    }
    if semantics::is_eclass(class) && class.instance_class_name().or(class.inst_name()).is_some() {
        at("instanceClassName", "");
    }
    for operation in class.operations() {
        at("eOperation", operation.name());
    }
    for literal in class.literals() {
        if has_foreign_annotation(literal.annotations()) {
            at("eAnnotation", literal.name());
        }
    }
    for feature in class.structural() {
        if has_foreign_annotation(&feature.annotations) {
            at("eAnnotation", &feature.name);
        }
        for (kind, hit) in [
            ("volatile", feature.volatile == Some(true)),
            ("transient", feature.transient == Some(true)),
            ("derived", feature.derived == Some(true)),
            ("resolveProxies", feature.resolve_proxies == Some(false)),
            // `defaultValueLiteral` is carried; a `defaultValue` is not.
            ("defaultValueLiteral", feature.default_value.is_some()),
        ] {
            if hit {
                at(kind, &feature.name);
            }
        }
    }
}

/// The digest half of a metamodel's identity: SHA-256, lowercase hex, over
/// the compact `serde_json` serialization of the parsed descriptor.
///
/// Taken over the parsed value and never over file bytes, so pretty and
/// compact renderings of one descriptor agree. `serde_json` runs without
/// `preserve_order` in this crate, so every object serializes with its keys
/// sorted and the order a file lists them in cannot reach the digest either;
/// a changed class, attribute, enum or `nsURI` can. The generated node binary
/// carries a copy of this rule and the editor computes it with a key-sorted
/// stringify, and `examples/fixtures/metamodel-digests.json` is where the
/// three are held to agree.
pub fn metamodel_digest(descriptor: &Value) -> String {
    format!("{:x}", Sha256::digest(descriptor.to_string()))
}

/// Describes one class: declared features partitioned into attributes,
/// containments and plain references, plus its super types.
///
/// Refuses with [`ArachneError::DescriptorNotClosed`] when a name it writes is not a class or
/// enum in `written`.
fn class_descriptor(
    ctx: &Ctx,
    pack: &Pack,
    class: &Class,
    written: &HashSet<idx::Class>,
) -> Result<Value> {
    let mut super_types: Vec<String> = Vec::new();
    for sup in class.sup() {
        require(ctx, pack, class, *sup, written)?;
        super_types.push(semantics::descriptor_class_name(ctx, pack, *sup)?);
    }
    // Declaration order, before the sort `superTypes` keeps.
    let super_type_order = super_types.clone();
    super_types.sort_unstable();

    let mut attributes = Vec::new();
    let mut containments = Vec::new();
    let mut references = Vec::new();

    for feature in class.structural() {
        let many = feature
            .bounds
            .ubound
            .map(|ubound| ubound > 1)
            .unwrap_or(true);
        let required = feature.bounds.lbound >= 1;

        match feature.kind {
            structural::Typ::EAttribute => {
                let Some((kind, enum_class)) = attribute_kind(ctx, class, feature) else {
                    continue;
                };
                let mut attribute = json!({
                    "name": feature.name,
                    "kind": kind,
                    "many": many,
                    "required": required,
                    "isId": feature.is_id,
                });
                if let Some(enum_class) = enum_class {
                    require(ctx, pack, class, enum_class, written)?;
                    attribute["enum"] =
                        json!(semantics::descriptor_class_name(ctx, pack, enum_class)?);
                }
                add_semantics(&mut attribute, ctx, pack, class, feature, written)?;
                attributes.push(attribute);
            }
            structural::Typ::EReference => {
                let Some(target) = feature.typ else {
                    warn!(
                        "`{}.{}`: externally-typed reference has no resolved target; \
                         omitted from the descriptor",
                        class.name(),
                        feature.name
                    );
                    continue;
                };
                require(ctx, pack, class, target, written)?;
                let target = semantics::descriptor_class_name(ctx, pack, target)?;
                if feature.containment {
                    let mut containment = json!({
                        "name": feature.name,
                        "target": target,
                        "many": many,
                        "required": required,
                        "ordered": feature.ordered.unwrap_or(true),
                    });
                    add_semantics(&mut containment, ctx, pack, class, feature, written)?;
                    add_declarations(&mut containment, ctx, pack, feature)?;
                    containments.push(containment);
                } else {
                    let mut reference = json!({
                        "name": feature.name,
                        "target": target,
                        "many": many,
                        "required": required,
                    });
                    add_semantics(&mut reference, ctx, pack, class, feature, written)?;
                    add_declarations(&mut reference, ctx, pack, feature)?;
                    references.push(reference);
                }
            }
        }
    }

    let mut entry = json!({
        "abstract": class.is_abstract() || class.is_interface(),
        "interface": !ctx.is_ecore_class(class.idx) && class.is_interface(),
        "superTypes": super_types,
        "superTypeOrder": super_type_order,
        "attributes": attributes,
        "containments": containments,
        "references": references,
    });
    // A `urn:arachne:representation` `kind="transparent"` class is compiled
    // as its one named field and gets no record of its own
    // (`classifier/mod.rs:565-567`). The name is written here so a reader of
    // the descriptor renders the class the same way, and the key is absent
    // on every other class so nothing that read a version 2 descriptor before
    // 2026-09-08 sees a change.
    if let Some(field) = semantics::transparent_field(class) {
        entry["transparent"] = json!(field);
    }
    Ok(entry)
}

/// `name` must be a class or enum the descriptor writes, `class` being the one that names it.
fn require(
    ctx: &Ctx,
    pack: &Pack,
    class: &Class,
    name: idx::Class,
    written: &HashSet<idx::Class>,
) -> Result<()> {
    if written.contains(&name) {
        return Ok(());
    }
    Err(ArachneError::DescriptorNotClosed {
        class: semantics::descriptor_class_name(ctx, pack, class.idx)?,
        // Not written, so possibly outside `pack`: named from the document root then.
        missing: semantics::descriptor_class_name(ctx, pack, name).unwrap_or_else(|_| {
            let missing = &ctx[name];
            missing
                .path
                .iter()
                .map(|up| ctx[up].name())
                .chain([missing.name()])
                .collect::<Vec<_>>()
                .join("::")
        }),
    })
}

/// Adds the four `formatVersion` 2 keys to one feature entry: the facets as
/// the `.ecore` file wrote them, the `urn:arachne:semantics` `datatype` value
/// that governs the feature or `null`, the merge rule the generator would
/// compile, and where each facet of that rule came from.
///
/// The `annotation` value is what `semantics::feature_datatype` answers: the
/// feature's own `datatype` detail, or, for an attribute of one of Ecore's own
/// classes, which no metamodel file declares, the package-level detail that
/// names it (`ecore::EAnnotation.source`, say).
///
/// Nothing is removed and nothing existing moves, which is what keeps the
/// phase 4 `Schema` parser (`deployment/conformance.rs`) and the model editor
/// reading these descriptors unchanged: both read only the keys they name.
fn add_semantics(
    entry: &mut Value,
    ctx: &Ctx,
    pack: &Pack,
    class: &Class,
    feature: &structural::Structural,
    written: &HashSet<idx::Class>,
) -> Result<()> {
    let (merge, provenance) = semantics::merge_rule(feature, class, ctx);
    // The merge rule's own class names (`merge.target`, `merge.leaf.class`) are names too.
    let named = match &merge {
        MergeRule::Containment { target, .. }
        | MergeRule::Reference { target, .. }
        | MergeRule::IdentityReference { target, .. } => Some(*target),
        MergeRule::Attribute {
            leaf: LeafRule::Enum { class, .. } | LeafRule::OptionalRegister { class: Some(class) },
            ..
        } => Some(*class),
        _ => None,
    };
    let key = match &merge {
        MergeRule::Attribute { shape, .. } | MergeRule::Containment { shape, .. } => match shape {
            Shape::Keyed {
                key: KeyKind::Enum { class },
            } => Some(*class),
            _ => None,
        },
        _ => None,
    };
    for slot in named.into_iter().chain(key) {
        require(ctx, pack, class, idx::Class::from(slot.index()), written)?;
    }
    entry["facets"] = semantics::facets_json(feature);
    entry["annotation"] = semantics::feature_datatype(feature, class, ctx)
        .map(|value| json!(value))
        .unwrap_or(Value::Null);
    entry["merge"] = semantics::merge_json(&merge, ctx, pack)?;
    entry["provenance"] = semantics::provenance_json(&provenance);
    // The facts the file declares and the rule does not read: bounds, default, flags and keys.
    // Ecore's own classes are Léo's built-ins, which no file declares: his values.
    if ctx.is_ecore_class(class.idx) {
        let many = feature.bounds.ubound.is_none_or(|ubound| ubound > 1);
        entry["lowerBound"] = json!(0);
        entry["upperBound"] = json!(if many { -1 } else { 1 });
        entry["default"] = Value::Null;
        entry["changeable"] = json!(true);
        entry["unsettable"] = json!(false);
        entry["eKeys"] = json!([]);
    } else {
        entry["lowerBound"] = json!(feature.bounds.lbound);
        entry["upperBound"] = json!(feature.bounds.ubound.map_or(-1, |ubound| ubound as i64));
        entry["default"] = json!(feature.default_value_literal);
        entry["changeable"] = json!(feature.changeable.unwrap_or(true));
        entry["unsettable"] = json!(feature.unsettable.unwrap_or(false));
        entry["eKeys"] = json!(feature.keys);
    }
    Ok(())
}

/// Adds to one reference entry what the `.ecore` declares about which end is
/// whose: `eOpposite`, the name of the opposite reference on the target class,
/// and `subsets` and `redefines`, one `{"class", "feature"}` object per entry
/// of the `references` of every `EAnnotation` with that `source`, in document
/// order.
///
/// Each key is written only when the `.ecore` declares at least one such
/// thing, so the descriptor of a metamodel that declares none is unchanged,
/// byte for byte, and keeps its digest. They are data for a reader of the
/// descriptor: no merge rule reads them, and nothing keeps two opposite ends
/// in step.
fn add_declarations(
    entry: &mut Value,
    ctx: &Ctx,
    pack: &Pack,
    feature: &structural::Structural,
) -> Result<()> {
    if let Some(opposite) = feature.opposite() {
        let name = opposite.rsplit('/').next().unwrap_or(opposite);
        entry["eOpposite"] = json!(name);
    }
    for source in ["subsets", "redefines"] {
        let declared: Vec<Value> = feature
            .annotations()
            .iter()
            .filter(|annot| annot.source() == source)
            .flat_map(|annot| annot.references())
            .map(|reference| {
                let (class, name) = class_and_feature(ctx, pack, reference)?;
                Ok(json!({"class": class, "feature": name}))
            })
            .collect::<Result<_>>()?;
        if !declared.is_empty() {
            entry[source] = Value::Array(declared);
        }
    }
    Ok(())
}

/// A `#//a/Class/feature` fragment split into the class, as the descriptor keys it, and the
/// feature. An in-document fragment is relative to the document's root package: its leading
/// segments make a `::` path, below `pack`, only when they name a chain of subpackages from that
/// root, and the chain must pass through `pack`, else the class is outside what is described and
/// the fragment is refused. Any other leading segments, and a reference into another file (a
/// dependency is described whole, under its own bare names), leave the class as its last segment.
fn class_and_feature<'a>(ctx: &Ctx, pack: &Pack, reference: &'a str) -> Result<(String, &'a str)> {
    let (file, path) = reference.rsplit_once('#').unwrap_or(("", reference));
    let (class, feature) = path
        .trim_start_matches('/')
        .rsplit_once('/')
        .unwrap_or(("", path.trim_start_matches('/')));
    let bare = || class.rsplit('/').next().unwrap_or("").to_string();
    let Some((subs, name)) = class
        .rsplit_once('/')
        .or(Some(("", class)))
        .filter(|_| file.is_empty())
    else {
        return Ok((bare(), feature));
    };
    let mut dir = pack;
    // The context's own `[root]` package, above every document, is not the document's root.
    while let Some(up) = dir.sup().filter(|up| ctx[*up].sup().is_some()) {
        dir = &ctx[up];
    }
    let mut seen_pack = dir.idx == pack.idx;
    let mut written = Vec::new();
    for seg in subs.split('/').filter(|seg| !seg.is_empty()) {
        let Some(sub) = dir
            .sub()
            .iter()
            .map(|sub| &ctx[*sub])
            .find(|sub| sub.name() == seg)
        else {
            return Ok((bare(), feature));
        };
        dir = sub;
        if seen_pack {
            written.push(seg);
        }
        seen_pack |= dir.idx == pack.idx;
    }
    if !seen_pack {
        return Err(ArachneError::InvalidModel(format!(
            "the fragment `{reference}` names a class outside the described package `{}`, which has no name to write",
            pack.name()
        )));
    }
    written.push(name);
    Ok((written.join("::"), feature))
}

/// The descriptor `kind` of an attribute, with the enum class name when the
/// kind is `enum`. `None` means the attribute cannot be described and is
/// omitted (already warned about).
fn attribute_kind(
    ctx: &Ctx,
    class: &Class,
    feature: &structural::Structural,
) -> Option<(&'static str, Option<idx::Class>)> {
    let Some(target) = feature.typ else {
        warn!(
            "`{}.{}`: externally-typed attribute has no resolved type; \
             omitted from the descriptor",
            class.name(),
            feature.name
        );
        return None;
    };
    let target = &ctx[target];

    if semantics::is_eenum(target) {
        return Some(("enum", feature.typ));
    }

    let kind = match semantics::value_builtin(target) {
        Some(builtin::Typ::EString) | Some(builtin::Typ::EChar) => "string",
        Some(builtin::Typ::EByte)
        | Some(builtin::Typ::EShort)
        | Some(builtin::Typ::EInt)
        | Some(builtin::Typ::ELong) => "int",
        Some(builtin::Typ::EFloat) | Some(builtin::Typ::EDouble) => "float",
        Some(builtin::Typ::EBoolean) => "bool",
        // `Object` and a datatype whose instance class maps to nothing here
        // have no structure to offer a typed editor; a string field is the
        // honest lowest common denominator.
        Some(builtin::Typ::Object) | None => {
            warn!(
                "`{}.{}`: attribute type `{}` has no typed editor mapping; \
                 described as `string`",
                class.name(),
                feature.name,
                target.name()
            );
            "string"
        }
    };
    Some((kind, None))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde_json::{Map, Value, json};

    use super::metamodel_digest;
    use crate::EcoreParser;

    fn example(file: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples")
            .join(file)
    }

    /// The descriptor `arachne describe` renders for an example metamodel.
    fn descriptor_of(ecore: &str) -> Value {
        let parser = EcoreParser::from_file(example(ecore))
            .unwrap_or_else(|e| panic!("{ecore} should parse: {e}"));
        let pack = crate::find_user_package(&parser.ctx).expect("a user package");
        super::descriptor_json(&parser.ctx, pack).expect("descriptor should build")
    }

    fn bt_descriptor() -> Value {
        descriptor_of("behavior_tree.ecore")
    }

    #[test]
    fn bt_names_root_as_its_only_root_class() {
        let descriptor = bt_descriptor();
        assert_eq!(descriptor["rootClasses"], json!(["Root"]));
    }

    #[test]
    fn bt_enum_appears_under_enums_and_not_under_classes() {
        let descriptor = bt_descriptor();
        assert_eq!(
            (
                &descriptor["enums"]["Status"],
                descriptor["classes"].get("Status")
            ),
            (&json!(["RUNNING", "SUCCESS", "FAILURE"]), None)
        );
    }

    /// The digest is a function of the descriptor's content: rendering it
    /// pretty, compact or with its keys in another order gives one digest, a
    /// second generation from the same `.ecore` gives the same one, and one
    /// changed attribute gives another.
    #[test]
    fn mp5_the_descriptor_digest_is_stable_under_reformatting_and_changes_under_edits() {
        let descriptor = bt_descriptor();
        let digest = metamodel_digest(&descriptor);

        let pretty: Value = serde_json::from_str(&format!("{descriptor:#}")).unwrap();
        let compact: Value = serde_json::from_str(&descriptor.to_string()).unwrap();
        let reversed = Value::Object(descriptor.as_object().unwrap().iter().rev().fold(
            Map::new(),
            |mut map, (key, value)| {
                map.insert(key.clone(), value.clone());
                map
            },
        ));
        assert_eq!(
            [
                metamodel_digest(&pretty),
                metamodel_digest(&compact),
                metamodel_digest(&reversed),
                metamodel_digest(&bt_descriptor()),
            ],
            [
                digest.clone(),
                digest.clone(),
                digest.clone(),
                digest.clone()
            ],
            "a formatter or a second generation moved the digest"
        );

        let mut edited = descriptor.clone();
        let name = edited["classes"]["TreeNode"]["attributes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|attribute| attribute["name"] == "name")
            .expect("TreeNode declares `name`");
        name["required"] = json!(true);
        assert_ne!(
            metamodel_digest(&edited),
            digest,
            "making `TreeNode.name` required left the digest unchanged"
        );
    }

    // SI.05 (`cea-cdrt-knowledge/spec/items/SI.05.md`, section 6.3): two tests that held
    // today's reference rule and the regenerated digests of the checked-in descriptors moved to
    // `tests/si05_inline_replacements.rs`, as the registered exception of VALIDATION.md section 2.

    /// The two forms D6 used to keep on the generated path, as `json.ecore`
    /// writes them since the decision was amended on 2026-09-08: a `uw-map`
    /// containment publishes the `keyed` shape and the class its values are,
    /// not the `Entry` its `.ecore` names; and a class the
    /// `urn:arachne:representation` annotation makes transparent publishes the
    /// field it is represented by under its own `transparent` key while its
    /// features publish ordinary rules.
    #[test]
    fn json_publishes_the_keyed_shape_and_the_transparent_field() {
        let descriptor = descriptor_of("json.ecore");
        assert_eq!(
            (
                &descriptor["classes"]["Object"]["containments"][0]["merge"],
                &descriptor["classes"]["Object"]["containments"][0]["annotation"],
                &descriptor["classes"]["Object"]["transparent"],
            ),
            (
                &json!({
                    "kind": "containment",
                    "shape": {"kind": "keyed", "key": {"kind": "str"}},
                    "target": "Json"
                }),
                &json!("uw-map"),
                &json!("entry"),
            )
        );
        assert_eq!(
            (
                &descriptor["classes"]["Array"]["containments"][0]["merge"],
                &descriptor["classes"]["Array"]["transparent"],
                &descriptor["classes"]["String"]["attributes"][0]["merge"],
                &descriptor["classes"]["String"]["transparent"],
            ),
            (
                &json!({
                    "kind": "containment",
                    "shape": {"kind": "sequence"},
                    "target": "Json"
                }),
                &json!("items"),
                &json!({"kind": "attribute", "shape": {"kind": "single"},
                        "leaf": {"kind": "text"}}),
                &json!("value"),
            )
        );
        // `Entry` is not transparent and keeps its own rules; nothing reaches
        // it any more, because the keyed rule above names `Json` directly.
        assert_eq!(descriptor["classes"]["Entry"].get("transparent"), None);
        assert_eq!(descriptor["classes"]["Json"].get("transparent"), None);
    }

    /// The format version is what the editor and the interpreted node gate on,
    /// so it is asserted rather than left to the fixture digest.
    #[test]
    fn the_descriptor_declares_format_version_two() {
        assert_eq!(bt_descriptor()["formatVersion"], json!(2));
    }

    /* ---------- Ecore's own classes ---------- */

    fn builtins_descriptor() -> Value {
        descriptor_of("pet_metamodels/ecore_builtins.ecore")
    }

    /// The descriptor `arachne describe` emits is one `moirai-semantics`
    /// parses into a table, which is the whole point of emitting it: the
    /// supertypes resolve, the targets resolve, the inherited `eAnnotations`
    /// is visible on every annotated class, and the three refusals cost the
    /// features they name and nothing else.
    #[test]
    fn the_builtins_descriptor_parses_into_a_table() {
        let descriptor = builtins_descriptor();
        let table = moirai_semantics::from_descriptor(&descriptor)
            .expect("the built-ins descriptor is a table");
        let class = |name: &str| {
            table
                .classes
                .iter()
                .find(|class| &*class.name == name)
                .unwrap_or_else(|| panic!("no class `{name}`"))
        };
        assert_eq!(table.classes.len(), 9);
        for name in ["Part", "Port", "ecore::EAnnotation"] {
            assert!(
                class(name)
                    .visible
                    .iter()
                    .any(|(feature, _, _)| &**feature == "eAnnotations"),
                "`{name}` should see the inherited `eAnnotations`"
            );
        }
        assert_eq!(
            table.roots,
            vec![class("Model").slot],
            "`EObject` is listed and is still not a root"
        );
        let unsupported = table
            .classes
            .iter()
            .flat_map(|class| class.declared.iter())
            .filter(|feature| {
                matches!(
                    feature.merge,
                    moirai_semantics::MergeRule::Unsupported { .. }
                )
            })
            .count();
        assert_eq!(
            unsupported, 4,
            "two `EObject` references, one containment, one transient"
        );
    }
}
