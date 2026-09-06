//! A seeded generator of operations over a [`Schema`]: well-formed ones,
//! drawn over the descriptor's classes, features and kinds, and malformed
//! ones, each wrong in one of the ways the structural check refuses. Shared
//! by the property test mp32 and the M-E7 harness, which is why it is a
//! file both include rather than a test-local helper.
//!
//! The generator reads the schema the way the check does — by families and
//! feature tables — but it never consults the check, so a disagreement
//! between the two is a finding and not a tautology. The random source is
//! the crate's own SplitMix64 (`moirai_network::workload::Rng`), so a run is
//! reproducible from its seed.

use moirai_network::workload::Rng;
use serde_json::{Value, json};

use crate::conformance::{Feature, Kind, Schema, Shape};

/// One step of a path from the root: a key of an object, or a fresh element
/// appended to a list.
#[derive(Debug, Clone)]
enum Step {
    Key(String),
    Element(String),
}

/// The seven ways an operation can be malformed under a descriptor, as the
/// check names them.
pub const WAYS: [&str; 7] = [
    "unknown-feature",
    "class-tag",
    "wrong-kind",
    "list-under-single",
    "object-under-many",
    "enum-literal",
    "scalar-for-object",
];

pub struct Generator<'a> {
    schema: &'a Schema,
    rng: Rng,
    /// When set, every position is 0, so each operation applies to any
    /// state a log may be in: what the receive harness needs, since it
    /// delivers the operations to a real replica.
    applicable: bool,
}

impl<'a> Generator<'a> {
    pub fn new(schema: &'a Schema, seed: u64, applicable: bool) -> Self {
        Self {
            schema,
            rng: Rng::new(seed),
            applicable,
        }
    }

    fn pick<'s, T>(&mut self, items: &'s [T]) -> &'s T {
        &items[self.rng.below(items.len())]
    }

    fn position(&mut self, len: usize) -> usize {
        if self.applicable || len == 0 {
            0
        } else {
            self.rng.below(len)
        }
    }

    fn letter(&mut self) -> char {
        (b'a' + self.rng.below(26) as u8) as char
    }

    /// A random object slot: the path to it and the family allowed there,
    /// at most `depth` containments below the root.
    fn slot(&mut self, depth: usize) -> (Vec<Step>, Vec<String>) {
        let mut path = Vec::new();
        let mut family = self.schema.root_family().to_vec();
        for _ in 0..depth {
            if self.rng.below(2) == 0 {
                break;
            }
            let class = self.pick(&family).clone();
            let containments: Vec<Feature> = self
                .schema
                .class(&class)
                .map(|class| {
                    class
                        .features
                        .values()
                        .filter(|feature| matches!(feature.shape, Shape::Containment { .. }))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            if containments.is_empty() {
                break;
            }
            let feature = self.pick(&containments).clone();
            let Shape::Containment { target } = &feature.shape else {
                unreachable!("filtered to containments");
            };
            let next = self
                .schema
                .class(target)
                .map(|class| class.family.clone())
                .unwrap_or_default();
            if next.is_empty() {
                break;
            }
            path.push(if feature.many {
                Step::Element(feature.name.clone())
            } else {
                Step::Key(feature.name.clone())
            });
            family = next;
        }
        (path, family)
    }

    /// The scalar leaf for an attribute of `kind`.
    fn scalar(&mut self, kind: Kind, enumeration: Option<&(String, Vec<String>)>) -> Value {
        match kind {
            Kind::String => {
                let (ch, pos) = (self.letter(), self.position(4));
                string_insert(ch, pos)
            }
            Kind::Enum => {
                let literals = enumeration
                    .map(|(_, literals)| literals.as_slice())
                    .unwrap_or(&[]);
                let literal = self.pick(literals).clone();
                let pos = self.position(literal.chars().count());
                string_insert(literal.chars().nth(pos).unwrap_or('?'), pos)
            }
            Kind::Int | Kind::Float => {
                json!({ "Number": { "Inc": (1 + self.rng.below(5)) as f64 } })
            }
            Kind::Bool => {
                json!({ "Boolean": if self.rng.below(2) == 0 { "Enable" } else { "Disable" } })
            }
        }
    }

    /// A well-formed leaf under `feature`, wrapped for its `many`.
    fn feature_value(&mut self, feature: &Feature, depth: usize) -> Value {
        let value = match &feature.shape {
            Shape::Attribute { kind, enumeration } => self.scalar(*kind, enumeration.as_ref()),
            Shape::Reference { .. } => {
                let (ch, pos) = (self.letter(), self.position(4));
                string_insert(ch, pos)
            }
            Shape::Containment { target } => {
                let family = self
                    .schema
                    .class(target)
                    .map(|class| class.family.clone())
                    .unwrap_or_default();
                self.element(&family, depth)
            }
        };
        if feature.many {
            json!({ "Array": { "Insert": { "pos": 0, "op": value } } })
        } else {
            value
        }
    }

    /// A well-formed operation on an object of some class of `family`: a
    /// character of its class tag, or a write into one of its features.
    fn element(&mut self, family: &[String], depth: usize) -> Value {
        let class = self.pick(family).clone();
        let features: Vec<Feature> = self
            .schema
            .class(&class)
            .map(|class| {
                class
                    .features
                    .values()
                    .filter(|feature| {
                        depth > 0 || !matches!(feature.shape, Shape::Containment { .. })
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if features.is_empty() || self.rng.below(4) == 0 {
            let name = self.pick(family).clone();
            let pos = self.position(name.chars().count());
            let ch = name.chars().nth(pos).unwrap_or('?');
            return json!({ "Object": { "Update": ["eClass", string_insert(ch, pos)] } });
        }
        let feature = self.pick(&features).clone();
        let value = self.feature_value(&feature, depth.saturating_sub(1));
        json!({ "Object": { "Update": [feature.name, value] } })
    }

    /// One well-formed operation, as the node receives it.
    pub fn conforming(&mut self) -> Value {
        let (path, family) = self.slot(2);
        let leaf = self.element(&family, 1);
        json!({ "JsonKind": wrap(&path, leaf) })
    }

    /// One malformed operation and the way it is malformed. A way the
    /// descriptor offers no slot for (an enum literal in a descriptor with
    /// no enum attribute) falls through to the next.
    pub fn malformed(&mut self) -> (Value, &'static str) {
        let first = self.rng.below(WAYS.len());
        for offset in 0..WAYS.len() {
            let way = WAYS[(first + offset) % WAYS.len()];
            if let Some(op) = self.malformed_as(way) {
                return (op, way);
            }
        }
        unreachable!("`unknown-feature` always has a slot")
    }

    fn malformed_as(&mut self, way: &str) -> Option<Value> {
        let (path, family) = self.slot(2);
        let leaf = match way {
            "unknown-feature" => {
                let (ch, pos) = (self.letter(), self.position(4));
                json!({ "Object": { "Update": ["colour", string_insert(ch, pos)] } })
            }
            "class-tag" => {
                // A character no allowed class has at that position.
                let pos = self.position(3);
                let ch = (b'a'..=b'z')
                    .chain(b'A'..=b'Z')
                    .map(char::from)
                    .find(|ch| !family.iter().any(|name| name.chars().nth(pos) == Some(*ch)))?;
                json!({ "Object": { "Update": ["eClass", string_insert(ch, pos)] } })
            }
            "wrong-kind" => {
                let attribute = self.attribute_in(&family, |_| true)?;
                let Shape::Attribute { kind, .. } = attribute.shape else {
                    unreachable!("filtered to attributes");
                };
                let wrong = match kind {
                    Kind::String | Kind::Enum => json!({ "Number": { "Inc": 1.0 } }),
                    Kind::Int | Kind::Float | Kind::Bool => string_insert('x', 0),
                };
                let value = if attribute.many {
                    json!({ "Array": { "Insert": { "pos": 0, "op": wrong } } })
                } else {
                    wrong
                };
                json!({ "Object": { "Update": [attribute.name, value] } })
            }
            "list-under-single" => {
                let containment = self.containment_in(&family, false)?;
                let child = json!({ "Object": { "Update": ["eClass", string_insert('S', 0)] } });
                json!({ "Object": { "Update": [containment.name,
                    { "Array": { "Insert": { "pos": 0, "op": child } } }] } })
            }
            "object-under-many" => {
                let containment = self.containment_in(&family, true)?;
                let child = json!({ "Object": { "Update": ["eClass", string_insert('S', 0)] } });
                json!({ "Object": { "Update": [containment.name, child] } })
            }
            "enum-literal" => {
                let attribute = self.attribute_in(&family, |feature| {
                    matches!(
                        feature.shape,
                        Shape::Attribute {
                            kind: Kind::Enum,
                            ..
                        }
                    )
                })?;
                let Shape::Attribute {
                    enumeration: Some((_, literals)),
                    ..
                } = &attribute.shape
                else {
                    unreachable!("filtered to enum attributes");
                };
                let pos = self.position(3);
                let ch = (b'a'..=b'z')
                    .chain(b'A'..=b'Z')
                    .map(char::from)
                    .find(|ch| {
                        !literals
                            .iter()
                            .any(|literal| literal.chars().nth(pos) == Some(*ch))
                    })?;
                json!({ "Object": { "Update": [attribute.name.clone(), string_insert(ch, pos)] } })
            }
            "scalar-for-object" => {
                // A string where an object belongs: the slot itself, or a
                // containment under it.
                match self.containment_in(&family, false) {
                    Some(containment) => {
                        json!({ "Object": { "Update": [containment.name, string_insert('x', 0)] } })
                    }
                    None => string_insert('x', 0),
                }
            }
            _ => unreachable!("not a way"),
        };
        Some(json!({ "JsonKind": wrap(&path, leaf) }))
    }

    fn attribute_in(
        &mut self,
        family: &[String],
        keep: impl Fn(&Feature) -> bool,
    ) -> Option<Feature> {
        let attributes: Vec<Feature> = family
            .iter()
            .filter_map(|name| self.schema.class(name))
            .flat_map(|class| class.features.values())
            .filter(|feature| matches!(feature.shape, Shape::Attribute { .. }) && keep(feature))
            .cloned()
            .collect();
        (!attributes.is_empty()).then(|| self.pick(&attributes).clone())
    }

    fn containment_in(&mut self, family: &[String], many: bool) -> Option<Feature> {
        let containments: Vec<Feature> = family
            .iter()
            .filter_map(|name| self.schema.class(name))
            .flat_map(|class| class.features.values())
            .filter(|feature| {
                feature.many == many && matches!(feature.shape, Shape::Containment { .. })
            })
            .cloned()
            .collect();
        (!containments.is_empty()).then(|| self.pick(&containments).clone())
    }
}

fn string_insert(ch: char, pos: usize) -> Value {
    json!({ "String": { "Insert": { "content": ch.to_string(), "pos": pos } } })
}

/// `leaf` wrapped for `path`, innermost step last: an `Object.Update` per
/// key, an `Object.Update` carrying an `Array.Insert` at 0 per element.
fn wrap(path: &[Step], leaf: Value) -> Value {
    path.iter().rev().fold(leaf, |inner, step| match step {
        Step::Key(key) => json!({ "Object": { "Update": [key, inner] } }),
        Step::Element(key) => json!({ "Object": { "Update": [key,
            { "Array": { "Insert": { "pos": 0, "op": inner } } }] } }),
    })
}
