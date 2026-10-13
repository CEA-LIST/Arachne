//! ED.10 (`cea-cdrt-knowledge/spec/items/ED.10.md`): a UML class-diagram metamodel that uses
//! Eclipse UML2's names for the class-diagram subset is shipped with the examples as
//! `examples/umlclass.ecore`, beside `examples/umlclass.metamodel.json`, which is what
//! `arachne describe` writes for it; and nothing else under `examples/` moves.
//!
//! Ground truth, none of it computed by Arachne:
//! - A2: the table below, written by hand from the spec's section 4 (the class-diagram subset,
//!   every class and feature name checked against Eclipse UML2's `UML.ecore` at registration,
//!   every bound, kind, target, opposite and merge decided there).
//! - A3: AG.02's rule R-e, as AR.02's D5 runs it (copied below line for line), and the spec's
//!   reading of the owning end as the containment's `eOpposite`.
//! - A4: the sha256 of every file under `examples/` at the spec commit, taken by `sha256sum`.
//! - A5: the markers only Eclipse UML2's own `UML.ecore` carries.
//!
//! A1 is the identity check: the checked-in descriptor is `arachne describe`'s output, byte for
//! byte, so the node hosts exactly what the tool writes.
//!
//! ```bash
//! nice -n 10 cargo test -j 2 -p arachne-codegen --test ed10_umlclass_metamodel -- --nocapture
//! ```
//!
//! A claim failure panics with `ED10 claim:`; a harness failure with `set-up:`.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

use arachne_codegen::{EcoreParser, descriptor_json, find_user_package, metamodel_digest};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

// SI.05 (`cea-cdrt-knowledge/spec/items/SI.05.md`, section 6.2): from SI.05 on `describe` names the
// identity rule on every in-scope reference. This file guards everything else, so what it reads is
// `describe`'s output with that rule read back as today's (`si05_support::read_back`), the
// identity before SI.05; `si05_identity_reference_emitter.rs` holds the rule itself.
#[path = "si05_support/mod.rs"]
mod si05_support;

const ECORE: &str = "examples/umlclass.ecore";
const DESCRIPTOR: &str = "examples/umlclass.metamodel.json";
const PACKAGE: &str = "umlclass";
const NS_URI: &str = "http://cea.fr/crdt/umlclass/0.1";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// What `arachne describe <ecore>` writes, as a value, or why it writes nothing.
fn try_describe(ecore: &Path) -> Result<Value, String> {
    if !ecore.is_file() {
        return Err(format!("{} is not there", ecore.display()));
    }
    let parser = EcoreParser::from_file(ecore)
        .map_err(|e| format!("{} does not parse: {e}", ecore.display()))?;
    let pack = find_user_package(&parser.ctx)
        .map_err(|e| format!("{} has no user package: {e}", ecore.display()))?;
    descriptor_json(&parser.ctx, pack)
        .map(si05_support::read_back)
        .map_err(|e| format!("{} is not describable: {e}", ecore.display()))
}

/// The descriptor ED.10 ships, as the node would read it, or why there is none.
fn shipped() -> Result<Value, String> {
    let path = repo().join(DESCRIPTOR);
    let text =
        fs::read_to_string(&path).map_err(|e| format!("{DESCRIPTOR} cannot be read: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{DESCRIPTOR} is not JSON: {e}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/* ---------- A1: the checked-in descriptor is what the tool writes ---------- */

/// A1. `examples/umlclass.metamodel.json` is, byte for byte, what `arachne describe
/// examples/umlclass.ecore` writes (`{descriptor:#}` and a newline, `arachne-cli`'s
/// `run_describe`), and it names the package and nsURI of section 4.
#[test]
fn ed10_a1_the_checked_in_descriptor_is_what_arachne_describe_writes() {
    let mut fail = Vec::new();
    match try_describe(&repo().join(ECORE)) {
        Err(why) => fail.push(why),
        Ok(described) => {
            let rendered = format!("{described:#}\n");
            match fs::read(repo().join(DESCRIPTOR)) {
                Err(e) => fail.push(format!("{DESCRIPTOR} cannot be read: {e}")),
                Ok(bytes) => {
                    let same = bytes == rendered.as_bytes();
                    println!(
                        "ED10-A1 digest {}, package {}, nsURI {}, checked-in bytes equal to \
                         `describe` {same} ({} and {} bytes)",
                        metamodel_digest(&described),
                        described["package"],
                        described["nsURI"],
                        bytes.len(),
                        rendered.len()
                    );
                    if !same {
                        fail.push(format!(
                            "{DESCRIPTOR} is not what `arachne describe {ECORE}` writes"
                        ));
                    }
                }
            }
            if described["package"] != json!(PACKAGE) || described["nsURI"] != json!(NS_URI) {
                fail.push(format!(
                    "the package is {} with nsURI {}, not {PACKAGE} with {NS_URI}",
                    described["package"], described["nsURI"]
                ));
            }
        }
    }
    if !fail.is_empty() {
        println!("ED10-A1 not met: {}", fail.join("; "));
    }
    assert!(
        fail.is_empty(),
        "ED10 claim: the shipped descriptor is not what `arachne describe` writes for the \
         shipped metamodel:\n  {}",
        fail.join("\n  ")
    );
}

/* ---------- A2: the class-diagram subset, with UML2's names ---------- */

/// What a feature of the table is.
#[derive(Clone, Copy)]
enum K {
    /// `EString`, a text leaf.
    Text,
    /// `EInt` merged as a multi-value register (`urn:arachne:semantics` `mv-register`): a bound
    /// is a value, never a counter two writers would sum.
    Int,
    /// `EBoolean`, an enable-wins flag.
    Flag,
    /// An attribute typed by one of the three enums, a multi-value register of its literals.
    Enum(&'static str),
    /// A containment of the named class, many-valued and so a sequence.
    Contain(&'static str),
    /// A non-containment reference to the named class.
    Ref(&'static str),
}

struct F {
    name: &'static str,
    kind: K,
    many: bool,
    required: bool,
    /// The `eOpposite` the descriptor carries, when section 4 declares one.
    opposite: Option<&'static str>,
    /// `(class, feature)` of every `subsets` the descriptor carries, in order.
    subsets: &'static [(&'static str, &'static str)],
}

const fn f(name: &'static str, kind: K, many: bool, required: bool) -> F {
    F {
        name,
        kind,
        many,
        required,
        opposite: None,
        subsets: &[],
    }
}

const fn opp(mut feature: F, opposite: &'static str) -> F {
    feature.opposite = Some(opposite);
    feature
}

const fn sub(mut feature: F, subsets: &'static [(&'static str, &'static str)]) -> F {
    feature.subsets = subsets;
    feature
}

struct C {
    name: &'static str,
    is_abstract: bool,
    supers: &'static [&'static str],
    features: &'static [F],
}

/// Section 4's table: 26 classes, 3 enums, every class and feature name one of Eclipse UML2's.
const CLASSES: &[C] = &[
    C {
        name: "Element",
        is_abstract: true,
        supers: &[],
        features: &[f("ownedComment", K::Contain("Comment"), true, false)],
    },
    C {
        name: "Comment",
        is_abstract: false,
        supers: &["Element"],
        features: &[
            f("body", K::Text, false, false),
            f("annotatedElement", K::Ref("Element"), true, false),
        ],
    },
    C {
        name: "NamedElement",
        is_abstract: true,
        supers: &["Element"],
        features: &[
            f("name", K::Text, false, false),
            f("visibility", K::Enum("VisibilityKind"), false, false),
        ],
    },
    C {
        name: "PackageableElement",
        is_abstract: true,
        supers: &["NamedElement"],
        features: &[],
    },
    C {
        name: "Package",
        is_abstract: false,
        supers: &["PackageableElement"],
        features: &[f(
            "packagedElement",
            K::Contain("PackageableElement"),
            true,
            false,
        )],
    },
    C {
        name: "Type",
        is_abstract: true,
        supers: &["PackageableElement"],
        features: &[],
    },
    C {
        name: "TypedElement",
        is_abstract: true,
        supers: &["NamedElement"],
        features: &[f("type", K::Ref("Type"), false, false)],
    },
    C {
        name: "MultiplicityElement",
        is_abstract: true,
        supers: &["Element"],
        features: &[
            f("lower", K::Int, false, false),
            f("upper", K::Int, false, false),
        ],
    },
    C {
        name: "Classifier",
        is_abstract: true,
        supers: &["Type"],
        features: &[
            f("isAbstract", K::Flag, false, true),
            opp(
                f("generalization", K::Contain("Generalization"), true, false),
                "specific",
            ),
        ],
    },
    C {
        name: "BehavioredClassifier",
        is_abstract: true,
        supers: &["Classifier"],
        features: &[opp(
            f(
                "interfaceRealization",
                K::Contain("InterfaceRealization"),
                true,
                false,
            ),
            "implementingClassifier",
        )],
    },
    C {
        name: "Class",
        is_abstract: false,
        supers: &["BehavioredClassifier"],
        features: &[
            f("ownedAttribute", K::Contain("Property"), true, false),
            f("ownedOperation", K::Contain("Operation"), true, false),
        ],
    },
    C {
        name: "Interface",
        is_abstract: false,
        supers: &["Classifier"],
        features: &[
            f("ownedAttribute", K::Contain("Property"), true, false),
            f("ownedOperation", K::Contain("Operation"), true, false),
        ],
    },
    C {
        name: "DataType",
        is_abstract: false,
        supers: &["Classifier"],
        features: &[
            f("ownedAttribute", K::Contain("Property"), true, false),
            f("ownedOperation", K::Contain("Operation"), true, false),
        ],
    },
    C {
        name: "PrimitiveType",
        is_abstract: false,
        supers: &["DataType"],
        features: &[],
    },
    C {
        name: "Enumeration",
        is_abstract: false,
        supers: &["DataType"],
        features: &[f(
            "ownedLiteral",
            K::Contain("EnumerationLiteral"),
            true,
            false,
        )],
    },
    C {
        name: "EnumerationLiteral",
        is_abstract: false,
        supers: &["NamedElement"],
        features: &[],
    },
    C {
        name: "Feature",
        is_abstract: true,
        supers: &["NamedElement"],
        features: &[f("isStatic", K::Flag, false, true)],
    },
    C {
        name: "BehavioralFeature",
        is_abstract: true,
        supers: &["Feature"],
        features: &[f("ownedParameter", K::Contain("Parameter"), true, false)],
    },
    C {
        name: "Property",
        is_abstract: false,
        supers: &["Feature", "MultiplicityElement", "TypedElement"],
        features: &[
            f("aggregation", K::Enum("AggregationKind"), false, false),
            f("default", K::Text, false, false),
            f("isDerived", K::Flag, false, true),
        ],
    },
    C {
        name: "Operation",
        is_abstract: false,
        supers: &["BehavioralFeature"],
        features: &[],
    },
    C {
        name: "Parameter",
        is_abstract: false,
        supers: &["MultiplicityElement", "TypedElement"],
        features: &[f(
            "direction",
            K::Enum("ParameterDirectionKind"),
            false,
            false,
        )],
    },
    C {
        name: "Association",
        is_abstract: false,
        supers: &["Classifier"],
        features: &[
            f("memberEnd", K::Ref("Property"), true, true),
            sub(
                f("ownedEnd", K::Contain("Property"), true, false),
                &[("Association", "memberEnd")],
            ),
            sub(
                f("navigableOwnedEnd", K::Ref("Property"), true, false),
                &[("Association", "ownedEnd")],
            ),
        ],
    },
    C {
        name: "Generalization",
        is_abstract: false,
        supers: &["Element"],
        features: &[
            f("general", K::Ref("Classifier"), false, true),
            opp(
                f("specific", K::Ref("Classifier"), false, true),
                "generalization",
            ),
        ],
    },
    C {
        name: "Dependency",
        is_abstract: false,
        supers: &["PackageableElement"],
        features: &[
            f("client", K::Ref("NamedElement"), true, true),
            f("supplier", K::Ref("NamedElement"), true, true),
        ],
    },
    C {
        name: "Usage",
        is_abstract: false,
        supers: &["Dependency"],
        features: &[],
    },
    C {
        name: "InterfaceRealization",
        is_abstract: false,
        supers: &["Dependency"],
        features: &[
            sub(
                f("contract", K::Ref("Interface"), false, true),
                &[("Dependency", "supplier")],
            ),
            sub(
                opp(
                    f(
                        "implementingClassifier",
                        K::Ref("BehavioredClassifier"),
                        false,
                        true,
                    ),
                    "interfaceRealization",
                ),
                &[("Dependency", "client")],
            ),
        ],
    },
];

/// Section 4's enums, literals in declaration order, as Eclipse UML2 declares them.
const ENUMS: &[(&str, &[&str])] = &[
    ("AggregationKind", &["none", "shared", "composite"]),
    ("ParameterDirectionKind", &["in", "inout", "out", "return"]),
    (
        "VisibilityKind",
        &["public", "private", "protected", "package"],
    ),
];

/// One entry as the table sees it: which array it sits in, and every key the table decides.
fn expected_entry(feature: &F) -> (&'static str, Value) {
    let mut entry = json!({
        "name": feature.name,
        "many": feature.many,
        "required": feature.required,
    });
    let array = match feature.kind {
        K::Text | K::Int | K::Flag | K::Enum(_) => {
            let shape = if feature.required {
                "single"
            } else {
                "optional"
            };
            let (kind, leaf) = match feature.kind {
                K::Text => ("string", json!({"kind": "text"})),
                K::Int => ("int", json!({"kind": "register", "tie": "mv"})),
                K::Flag => ("bool", json!({"kind": "flag", "wins": "enable"})),
                K::Enum(e) => ("enum", json!({"kind": "enum", "class": e, "tie": "mv"})),
                _ => unreachable!(),
            };
            entry["kind"] = json!(kind);
            entry["isId"] = json!(false);
            if let K::Enum(e) = feature.kind {
                entry["enum"] = json!(e);
            }
            entry["merge"] = json!({"kind": "attribute", "shape": {"kind": shape}, "leaf": leaf});
            "attributes"
        }
        K::Contain(target) => {
            entry["target"] = json!(target);
            entry["merge"] =
                json!({"kind": "containment", "shape": {"kind": "sequence"}, "target": target});
            "containments"
        }
        K::Ref(target) => {
            entry["target"] = json!(target);
            entry["merge"] = json!({"kind": "reference", "many": feature.many, "target": target});
            "references"
        }
    };
    if let Some(o) = feature.opposite {
        entry["eOpposite"] = json!(o);
    }
    if !feature.subsets.is_empty() {
        entry["subsets"] = Value::Array(
            feature
                .subsets
                .iter()
                .map(|(c, f)| json!({"class": c, "feature": f}))
                .collect(),
        );
    }
    (array, entry)
}

/// The keys of an entry the table decides, taken off a descriptor entry; `redefines` is read
/// too, so that one the table does not declare shows as a difference.
fn projected_entry(entry: &Value, array: &str) -> Value {
    let mut keys = vec![
        "name",
        "many",
        "required",
        "merge",
        "eOpposite",
        "subsets",
        "redefines",
    ];
    if array == "attributes" {
        keys.extend(["kind", "isId", "enum"]);
    } else {
        keys.push("target");
    }
    let mut out = serde_json::Map::new();
    for key in keys {
        if let Some(value) = entry.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(out)
}

/// A2. The descriptor holds exactly section 4's subset: the package and nsURI, the three enums
/// with their literals in order, the 26 classes with their abstractness and supertypes, and on
/// each class exactly the declared features of the table, each in the array its kind puts it,
/// with its bounds, target, merge, `eOpposite` and `subsets` as the table writes them, and a
/// root class under which a Package may be the model's root.
#[test]
fn ed10_a2_the_descriptor_holds_the_class_diagram_subset_with_uml2_names() {
    let descriptor = match shipped() {
        Ok(d) => d,
        Err(why) => {
            println!("ED10-A2 no descriptor: {why}");
            panic!("ED10 claim: the class-diagram metamodel is not shipped: {why}");
        }
    };
    let mut diff = Vec::new();
    for (key, want) in [
        ("formatVersion", json!(2)),
        ("package", json!(PACKAGE)),
        ("nsURI", json!(NS_URI)),
    ] {
        if descriptor[key] != want {
            diff.push(format!("{key} is {}, not {want}", descriptor[key]));
        }
    }
    let enums: BTreeMap<&str, Value> = ENUMS.iter().map(|(e, l)| (*e, json!(l))).collect();
    if descriptor["enums"] != json!(enums) {
        diff.push(format!(
            "enums are {}, not {}",
            descriptor["enums"],
            json!(enums)
        ));
    }
    let classes = descriptor["classes"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let want_names: BTreeSet<&str> = CLASSES.iter().map(|c| c.name).collect();
    let have_names: BTreeSet<&str> = classes.keys().map(String::as_str).collect();
    for extra in have_names.difference(&want_names) {
        diff.push(format!("class {extra} is not in section 4's table"));
    }
    let mut compared = 0;
    for class in CLASSES {
        let Some(body) = classes.get(class.name) else {
            diff.push(format!("class {} is missing", class.name));
            continue;
        };
        if body["abstract"] != json!(class.is_abstract) {
            diff.push(format!(
                "{}: abstract {}, not {}",
                class.name, body["abstract"], class.is_abstract
            ));
        }
        let mut supers: Vec<&str> = class.supers.to_vec();
        supers.sort_unstable();
        if body["superTypes"] != json!(supers) {
            diff.push(format!(
                "{}: superTypes {}, not {supers:?}",
                class.name, body["superTypes"]
            ));
        }
        let mut want: BTreeMap<(&str, String), Value> = BTreeMap::new();
        for feature in class.features {
            let (array, entry) = expected_entry(feature);
            want.insert((array, feature.name.to_string()), entry);
        }
        let mut have: BTreeMap<(&str, String), Value> = BTreeMap::new();
        for array in ["attributes", "containments", "references"] {
            for entry in body[array].as_array().map(Vec::as_slice).unwrap_or(&[]) {
                let name = entry["name"].as_str().unwrap_or("").to_string();
                have.insert((array, name), projected_entry(entry, array));
            }
        }
        for (key, entry) in &want {
            compared += 1;
            match have.get(key) {
                None => diff.push(format!(
                    "{}: no `{}` under {} (it is {})",
                    class.name,
                    key.1,
                    key.0,
                    have.keys()
                        .find(|(_, n)| *n == key.1)
                        .map(|(a, _)| format!("under {a}"))
                        .unwrap_or_else(|| "absent".to_string())
                )),
                Some(got) if got != entry => {
                    diff.push(format!("{}.{}: {got}, not {entry}", class.name, key.1))
                }
                Some(_) => {}
            }
        }
        for key in have.keys() {
            if !want.contains_key(key) && !want.keys().any(|(_, n)| *n == key.1) {
                diff.push(format!(
                    "{}: `{}` under {} is not in section 4's table",
                    class.name, key.1, key.0
                ));
            }
        }
    }
    // A Package may be the model's root: some root class is Package or one of its supertypes.
    let package_line: BTreeSet<&str> =
        ["Package", "PackageableElement", "NamedElement", "Element"].into();
    let roots: Vec<&str> = descriptor["rootClasses"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !roots.iter().any(|r| package_line.contains(r)) {
        diff.push(format!(
            "no root class admits a Package as the model's root: {roots:?}"
        ));
    }
    println!(
        "ED10-A2 classes {} (table {}), enums {}, feature entries compared {compared}, \
         differing {}; rootClasses {roots:?}",
        classes.len(),
        CLASSES.len(),
        descriptor["enums"].as_object().map_or(0, |e| e.len()),
        diff.len()
    );
    for line in &diff {
        println!("ED10-A2 {line}");
    }
    assert!(
        diff.is_empty(),
        "ED10 claim: the descriptor is not section 4's class-diagram subset:\n  {}",
        diff.join("\n  ")
    );
}

/* ---------- A3: the owning ends, read from the descriptor ---------- */

/// One feature as R-e reads it. Nothing here is a class or feature name of any language.
/// Copied from `ar02_descriptor_declarations.rs` (AR.02's D5), line for line.
struct Feat {
    is_ref: bool,
    /// Listed under `containments`, and its target class: where a new element may be created.
    containment: Option<String>,
    derived: bool,
    many: bool,
    /// `Target/eOpposite`, the opposite qualified by the class that declares it.
    opp: Option<String>,
    /// `subsets` then `redefines`, qualified.
    up: Vec<String>,
    redefines: Vec<String>,
}

/// The descriptor as R-e reads it: per class, its supertypes, whether it is abstract, and its
/// features in the order the descriptor lists them.
struct Model {
    classes: BTreeMap<String, (Vec<String>, bool, Vec<(String, Feat)>)>,
}

fn qualified(list: &Value) -> Vec<String> {
    list.as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .map(|r| {
            format!(
                "{}/{}",
                r["class"].as_str().unwrap_or(""),
                r["feature"].as_str().unwrap_or("")
            )
        })
        .collect()
}

impl Model {
    fn of(descriptor: &Value) -> Self {
        let mut classes = BTreeMap::new();
        for (name, body) in descriptor["classes"].as_object().expect("classes") {
            let supers = body["superTypes"]
                .as_array()
                .map(|s| {
                    s.iter()
                        .filter_map(|s| s.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let mut feats = Vec::new();
            for array in ["attributes", "containments", "references"] {
                for f in body[array].as_array().map(Vec::as_slice).unwrap_or(&[]) {
                    let merge = &f["merge"];
                    let mut up = qualified(&f["subsets"]);
                    up.extend(qualified(&f["redefines"]));
                    feats.push((
                        f["name"].as_str().unwrap_or("").to_string(),
                        Feat {
                            is_ref: array != "attributes",
                            containment: (array == "containments")
                                .then(|| f["target"].as_str().unwrap_or("").to_string()),
                            derived: merge["kind"] == json!("unsupported")
                                && merge["reason"] == json!("derived"),
                            many: f["many"] == json!(true),
                            opp: f["eOpposite"]
                                .as_str()
                                .map(|o| format!("{}/{o}", f["target"].as_str().unwrap_or(""))),
                            up,
                            redefines: qualified(&f["redefines"]),
                        },
                    ));
                }
            }
            classes.insert(
                name.clone(),
                (supers, body["abstract"] == json!(true), feats),
            );
        }
        Model { classes }
    }

    /// The class and its supertypes, in the order AG.02's script walks them.
    fn supers(&self, class: &str) -> Vec<String> {
        let (mut out, mut todo) = (Vec::<String>::new(), vec![class.to_string()]);
        while let Some(x) = todo.pop() {
            if let Some((sup, _, _)) = self.classes.get(&x) {
                if !out.contains(&x) {
                    out.push(x.clone());
                    todo.extend(sup.iter().cloned());
                }
            }
        }
        out
    }

    /// R-e: the owning end of an element of `class` created in containment `holder` (`Class/feature`)
    /// of its owner. AG.02 section 7's `owning_end`, line for line, over the descriptor.
    fn owning_end(&self, class: &str, holder: &str) -> BTreeSet<String> {
        let mut q: Vec<(String, &Feat)> = Vec::new();
        for s in self.supers(class) {
            for (name, feat) in &self.classes[&s].2 {
                q.push((format!("{s}/{name}"), feat));
            }
        }
        let index: HashMap<&str, &Feat> = q.iter().map(|(n, f)| (n.as_str(), *f)).collect();
        let (holder_class, holder_feature) = holder.split_once('/').expect("Class/feature");
        let opposite = match index.get(holder) {
            Some(f) => f.opp.clone(),
            None => self.classes.get(holder_class).and_then(|(_, _, feats)| {
                feats
                    .iter()
                    .find(|(n, _)| n == holder_feature)
                    .and_then(|(_, f)| f.opp.clone())
            }),
        };
        let Some(opposite) = opposite else {
            return BTreeSet::new();
        };
        let mut redefined_by: HashMap<&str, &str> = HashMap::new();
        for (n, f) in &q {
            for r in &f.redefines {
                redefined_by.insert(r.as_str(), n.as_str());
            }
        }
        let fin = |start: &str| -> String {
            let mut n = start.to_string();
            let mut steps = 0;
            while let Some(next) = redefined_by.get(n.as_str()) {
                n = next.to_string();
                steps += 1;
                assert!(steps < 1000, "set-up: a redefinition cycle through {n}");
            }
            n
        };
        let closure = |n: &str| -> BTreeSet<String> {
            let (mut seen, mut todo) = (BTreeSet::from([n.to_string()]), vec![n.to_string()]);
            while let Some(x) = todo.pop() {
                if let Some(f) = index.get(x.as_str()) {
                    for m in &f.up {
                        if seen.insert(m.clone()) {
                            todo.push(m.clone());
                        }
                    }
                }
            }
            seen
        };
        let mut ends = BTreeSet::new();
        for (d, f) in &q {
            if redefined_by.contains_key(d.as_str()) || !f.derived {
                continue;
            }
            let reach = closure(d);
            if !reach.contains(&opposite) {
                continue;
            }
            for s in reach.iter().filter(|s| *s != d && **s != opposite) {
                let end = fin(s);
                if let Some(e) = index.get(end.as_str()) {
                    if e.is_ref && !e.derived && !e.many {
                        ends.insert(
                            end.split_once('/')
                                .map(|(_, f)| f)
                                .unwrap_or("")
                                .to_string(),
                        );
                    }
                }
            }
        }
        ends
    }
}

/// A3. Which end of a relationship is its owner, read from the descriptor alone.
///
/// (a) AG.02's R-e, unchanged, run over every containment and every concrete class it may hold,
/// finds no owning end anywhere: UML2 makes the owning end of a Generalization and of an
/// InterfaceRealization the containment's own `eOpposite`, which R-e excludes by design, and
/// this metamodel declares no derived feature. So the agent, applying R-e, writes nothing on a
/// model of this metamodel, and in particular no wrong end.
///
/// (b) The owning end is the containment's `eOpposite`: `Classifier.generalization` names
/// `specific`, and `BehavioredClassifier.interfaceRealization` names `implementingClassifier`,
/// each a single-valued, required, non-derived reference of the held class whose target the
/// holder conforms to and which names the containment back.
#[test]
fn ed10_a3_the_owning_end_of_a_relationship_is_read_from_the_descriptor() {
    let descriptor = match shipped() {
        Ok(d) => d,
        Err(why) => {
            println!("ED10-A3 no descriptor: {why}");
            panic!("ED10 claim: the class-diagram metamodel is not shipped: {why}");
        }
    };
    let model = Model::of(&descriptor);
    let mut judged = 0;
    let mut r_e = Vec::new();
    for (holder_class, (_, _, feats)) in &model.classes {
        for (name, feat) in feats {
            let Some(target) = &feat.containment else {
                continue;
            };
            for (class, (_, is_abstract, _)) in &model.classes {
                if *is_abstract || !model.supers(class).contains(target) {
                    continue;
                }
                judged += 1;
                let ends = model.owning_end(class, &format!("{holder_class}/{name}"));
                if !ends.is_empty() {
                    r_e.push(format!("{class} in {holder_class}.{name}: {ends:?}"));
                }
            }
        }
    }
    let mut fail = Vec::new();
    if judged == 0 {
        fail.push("R-e judged no holder and class pair".to_string());
    }
    if !r_e.is_empty() {
        fail.push(format!("R-e finds an owning end: {r_e:?}"));
    }
    let entry = |class: &str, array: &str, feature: &str| -> Option<Value> {
        descriptor["classes"][class][array]
            .as_array()?
            .iter()
            .find(|e| e["name"] == json!(feature))
            .cloned()
    };
    let mut read = Vec::new();
    for (holder, containment, held, end, end_target) in [
        (
            "Classifier",
            "generalization",
            "Generalization",
            "specific",
            "Classifier",
        ),
        (
            "BehavioredClassifier",
            "interfaceRealization",
            "InterfaceRealization",
            "implementingClassifier",
            "BehavioredClassifier",
        ),
    ] {
        let holding = entry(holder, "containments", containment);
        let named = holding
            .as_ref()
            .and_then(|h| h["eOpposite"].as_str().map(str::to_string));
        let back = entry(held, "references", end);
        let ok = holding.as_ref().is_some_and(|h| h["target"] == json!(held))
            && named.as_deref() == Some(end)
            && back.as_ref().is_some_and(|b| {
                b["many"] == json!(false)
                    && b["required"] == json!(true)
                    && b["target"] == json!(end_target)
                    && b["eOpposite"] == json!(containment)
                    && b["merge"]["kind"] == json!("reference")
                    && model.supers(holder).iter().any(|s| s == end_target)
            });
        read.push(format!(
            "{held} in {holder}.{containment}: eOpposite {named:?}, owning end derivable {ok}"
        ));
        if !ok {
            fail.push(format!(
                "the owning end of a {held} held in {holder}.{containment} is not `{end}` by \
                 the containment's eOpposite: holder {holding:?}, end {back:?}"
            ));
        }
    }
    println!(
        "ED10-A3 R-e: holder and class pairs judged {judged}, with an owning end {}",
        r_e.len()
    );
    for line in &read {
        println!("ED10-A3 {line}");
    }
    assert!(
        fail.is_empty(),
        "ED10 claim: the owning ends are not what the descriptor says they are:\n  {}",
        fail.join("\n  ")
    );
}

/* ---------- A4: nothing else under examples/ moves ---------- */

/// A4. The guard: every file under `examples/` at the spec commit but `README.md` keeps its
/// bytes (`sha256sum`, 2026-09-30, on `dbce103`), the four checked-in descriptors keep the
/// digests the fixture names, and the only new files are the two ED.10 ships.
#[test]
fn ed10_a4_every_other_file_of_examples_keeps_its_bytes() {
    // ED.16a (spec/items/ED.16a.md, decision D5 (a)) moves the notation metamodel: its
    // `.ecore` leaves this list (its bytes are the executor's), its descriptor and the digest
    // fixture are pinned at the bytes and digest ED.16a registered.
    const TODAY: [(&str, &str); 19] = [
        (
            "GENERIC_NETWORK_GUIDE.md",
            "01ab74a6ad9ea4e0f7cf0465b10661f8953d095eafef3bb87d684ccc9c15adac",
        ),
        (
            "SimpleUML.ecore",
            "31296010f3cde494863c4d55b4fd0d1adaf56023dc07ccc6d694c6bec0be411c",
        ),
        (
            "UML.ecore",
            "421e95a6b1946e9705a136011a0fc02aa7bf9b2871cba3e83bf11f11234176b6",
        ),
        (
            "behavior_tree.ecore",
            "d658b5c5b7a71dbdc49dda87d20a151c25336aabc16cd2c38b5b05f41dd663d5",
        ),
        (
            "bt.metamodel.json",
            "ff4525108d4ecb65d297002c7bc02f22978966d347af3678712de07a5c6e1263",
        ),
        (
            "class_diagram.ecore",
            "b8645c0fb97cbe9debed4af2937b2e63ac71ab0b18bd23542aca611267a6a05b",
        ),
        (
            "class_hierarchy.ecore",
            "bc1d7f1a40522d9905e6576caf72422a87f42c307a936084f6fdeb5e3cdbf7d3",
        ),
        (
            "conference.ecore",
            "4b30cbcf9411caa86ca4a6e74ef835b9944747d319fa3a6440b5d505741a0346",
        ),
        (
            "fixtures/metamodel-digests.json",
            "9d217db433c8a2d72b9badc5264869b3f2b65b84ad95cfe3047c79a244476879",
        ),
        (
            "json.ecore",
            "fd88ee6b27546e51d9948fccb709ba95e082a0ed343b171aae1e71a94adc4b9c",
        ),
        (
            "json.metamodel.json",
            "efdbde163d711b39e102ae7dc98585c3cb981ad9be82914ba4f4270e10113ec1",
        ),
        (
            "notation.metamodel.json",
            "04ebb5c15dad375284d4e5e681a5e634130525c614517d373763e6040eead03f",
        ),
        (
            "pet_metamodels/abstract_inherits_concrete.ecore",
            "d11e29a90768c6d9c3226a3d8289b0158733972b99bfd8c8daa41a1bfeb1b6e6",
        ),
        (
            "pet_metamodels/concrete_inherits_concrete.ecore",
            "b2277e3bfa6951675e0575fddf6548175ed17c83ff275044ec944ceb2198be57",
        ),
        (
            "pet_metamodels/concrete_polymorphic_targets.ecore",
            "16dba6831820fee59f1072c029696e674de623c04b781d0fc98982d548aa89d8",
        ),
        (
            "pet_metamodels/ecore_builtins.ecore",
            "108465fab2d43a002c81a49fcf2f6dc1b4aa0da3fa6a7f64059e9704d1c33149",
        ),
        (
            "pet_metamodels/kitchen_sink.ecore",
            "dd765685c7072f6f0250c70bba8d1214d37250c5cb5af114d9e93e2c9c7d4a1b",
        ),
        (
            "pet_metamodels/multiple_inheritance.ecore",
            "ce39e5d5a6566e814588e44d2f81028bdc1226f5ae3b7937e8292d14cda7b7bc",
        ),
        (
            "uml.metamodel.json",
            "b000aa0d2b562a41763d3e77829fd9363224fdcade914d42846b9cb0c54690a5",
        ),
    ];
    /// The fixture's descriptors and the digest each must keep.
    const DIGESTS: [(&str, &str); 4] = [
        (
            "bt.metamodel.json",
            "b899ec335e7dc46303d5a7871a1370a12db8858718bea57dc6f58f01c474feb5",
        ),
        (
            "uml.metamodel.json",
            "21c50dea71938b861233b72116f5ead094fe1bbb3d67bf623864f52c3ead1c3f",
        ),
        (
            "json.metamodel.json",
            "a9d83505175d8d6708b86c7eb64d356d64197f066f752c1a9ea842c76a45bc30",
        ),
        (
            "notation.metamodel.json",
            "12e7d2beab5a6d0a44090ed0512228c170a19879a7ac25093d817d58fe5c4de1",
        ),
    ];
    const NEW: [&str; 3] = [
        "umlclass.ecore",
        "umlclass.metamodel.json",
        "notation.ecore",
    ];
    // AR.04 (spec/items/AR.04.md, section 6): the annotated SysML metamodel and its descriptor
    // are added under `examples/sysml/`, a directory no node's METAMODEL_DIR reads.
    const AR04_ADDED: [&str; 2] = ["sysml/sysml.ecore", "sysml/sysml.metamodel.json"];
    let examples = repo().join("examples");
    let mut differing = Vec::new();
    for (file, today) in TODAY {
        match fs::read(examples.join(file)) {
            Ok(bytes) => {
                let sha = sha256_hex(&bytes);
                if sha != today {
                    differing.push(format!("{file}: sha256 {sha}, today {today}"));
                }
            }
            Err(e) => differing.push(format!("{file}: {e}")),
        }
    }
    for (file, today) in DIGESTS {
        let digest = fs::read_to_string(examples.join(file))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map(|v| metamodel_digest(&v))
            .unwrap_or_default();
        if digest != today {
            differing.push(format!("{file}: digest {digest}, today {today}"));
        }
    }
    let mut present = Vec::new();
    let mut todo = vec![examples.clone()];
    while let Some(dir) = todo.pop() {
        for entry in fs::read_dir(&dir)
            .expect("set-up: listing examples/")
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                todo.push(path);
            } else {
                present.push(
                    path.strip_prefix(&examples)
                        .expect("under examples/")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    present.sort();
    let unexpected: Vec<&String> = present
        .iter()
        .filter(|p| {
            p.as_str() != "README.md"
                && !TODAY.iter().any(|(f, _)| f == p)
                && !NEW.contains(&p.as_str())
                && !AR04_ADDED.contains(&p.as_str())
        })
        .collect();
    println!(
        "ED10-A4 files guarded {}, digests guarded {}, differing {}, files under examples/ {}, \
         unexpected {}",
        TODAY.len(),
        DIGESTS.len(),
        differing.len(),
        present.len(),
        unexpected.len()
    );
    for line in &differing {
        println!("ED10-A4 {line}");
    }
    for file in &unexpected {
        println!("ED10-A4 unexpected file {file}");
    }
    assert!(
        differing.is_empty() && unexpected.is_empty(),
        "ED10 claim: something else under examples/ moved:\n  {}\n  unexpected: {unexpected:?}",
        differing.join("\n  ")
    );
}

/* ---------- A5: our own .ecore, not Eclipse UML2's text ---------- */

/// A5. Decision D6: the metamodel uses UML2's names and is written here; it carries none of the
/// markers only Eclipse UML2's own `UML.ecore` carries (GenModel documentation, UML2's
/// annotation and type namespaces, CDO and extended-metadata annotations, `union` and
/// `duplicates` annotations).
#[test]
fn ed10_a5_the_ecore_is_written_here_and_not_copied_from_uml2() {
    const MARKERS: [&str; 7] = [
        "http://www.eclipse.org/uml2",
        "http://www.eclipse.org/emf/2002/GenModel",
        "key=\"documentation\"",
        "http://www.eclipse.org/emf/CDO",
        "ExtendedMetaData",
        "source=\"union\"",
        "source=\"duplicates\"",
    ];
    let text = match fs::read_to_string(repo().join(ECORE)) {
        Ok(t) => t,
        Err(e) => {
            println!("ED10-A5 no metamodel: {e}");
            panic!("ED10 claim: the class-diagram metamodel is not shipped: {ECORE}: {e}");
        }
    };
    let found: Vec<(&str, usize)> = MARKERS
        .iter()
        .map(|m| (*m, text.matches(m).count()))
        .filter(|(_, n)| *n > 0)
        .collect();
    println!(
        "ED10-A5 {ECORE}: {} bytes, UML2-only markers found {}",
        text.len(),
        found.len()
    );
    assert!(
        found.is_empty(),
        "ED10 claim: {ECORE} carries text only Eclipse UML2's UML.ecore carries: {found:?}"
    );
}
