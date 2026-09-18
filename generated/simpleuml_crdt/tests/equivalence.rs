//! The equivalence oracle over a metamodel we did not write: `ip14` of
//! [`02 Validation Plan`] §4, the SimpleUML arm of the first oracle.
//!
//! # What is being claimed
//!
//! Criterion I-A1, over a second structural metamodel: for the same model
//! edits applied in the same causal order, a replica running the interpreted
//! `ModelLog` over `SimpleUML.ecore`'s table and a replica running this
//! generated crate reach read-outs that are equal under the canonical
//! projection of the validation plan's section 2.
//!
//! `ip13` makes the same claim over `bt.ecore`. The reason for a second
//! driver of the same oracle is stated in the evaluation report's own threat
//! to the equivalence result: it rests on metamodels whose authors also wrote
//! the generator. `examples/SimpleUML.ecore` is not one of those. It is the
//! SimpleUML of the EMF and ATL examples, checked into this repository before
//! any of this phase's work started, and nothing in it was written to reach
//! anything.
//!
//! What that costs, stated rather than hidden: it also reaches no leaf
//! construction that `ip13`, `ip28`, `ip29` and `ip30` have not already
//! proven. See [`the_leaf_arms_simpleuml_reaches_are_all_already_proven`],
//! which enumerates them from the table rather than from a list written here.
//! `ip14` widens the *structural* evidence — multiple inheritance, six
//! `union!`s, a containment onto a union whose members are themselves unions —
//! and widens the leaf evidence not at all.
//!
//! # How the two paths are driven
//!
//! Four replicas: `a` and `b` of the interpreted `ModelLog`, `a` and `b` of
//! the generated `SimpleumlLog`. One [`Edit`] is encoded twice, once into a
//! [`ModelOp`] and once into this crate's typed operation, and the two
//! encodings are handed to the two `a` replicas — or the two `b` replicas —
//! in the same order, with delivery under the test's control and identical on
//! both paths. The read-outs are compared **after every single operation**,
//! not only at the end, so a failure names the edit that broke it rather than
//! the script that contained it.
//!
//! ## The one place this differs from `ip13`, and why
//!
//! `bt.ecore` declares one root class, `Root`, which is concrete and has no
//! subclasses, so the generator compiles it as a `record!` and `ip13` can open
//! the generated arm with a `Root::New` that writes nothing. That is how
//! `ip13` lines the sequence numbers up: the interpreted log opens on a
//! `ModelOp::Install` from `a`, the generated pair answers it with one
//! `Root::New`, and edit *n* is event *n+1* on both paths.
//!
//! `SimpleUML.ecore` declares four root classes and every one of them is
//! abstract, so the generator compiles each as a `union!` and
//! `SimpleumlValue` carries four of them side by side. A `union!` has no `New`
//! arm — its only operations descend into a variant — so there is no
//! generated operation that writes nothing, and no opener to answer `Install`
//! with. This oracle therefore does not send one: minting the root `Model` is
//! the first *edit* of every script, applied to both paths through the same
//! [`Harness::carry`] as every other edit, and replica `a`'s events carry a
//! sequence number one lower on the generated path than on the interpreted
//! one for the whole run.
//!
//! That offset cannot move a tie. `EventId::cmp` (`event/id.rs:91-98`)
//! compares the origin first and reaches the sequence number only when two
//! events share an origin — and within one origin the offset is a constant,
//! so it reorders nothing. This is asserted rather than assumed:
//! [`a_concurrent_tie_breaks_the_same_way_on_both_paths`] drives two writers
//! into the same position of the same sequence and demands the two paths
//! agree on the order, and the twenty concurrent scripts of
//! [`ip14_thirty_seeded_scripts_over_simpleuml_ecore_agree`] would report any
//! tie that broke differently as a difference.
//!
//! # Which root arm is driven
//!
//! `SimpleumlValue` has four root fields — `packageable`, `classifier`,
//! `t_type`, `model_element` — and the interpreted path has one root slot,
//! `Target::Roots` (`moirai-interp/src/node.rs:338-349`), which holds one
//! object of any class in the union of the four closures. The two shapes do
//! not correspond one to one, so this oracle drives one arm, `ModelElement`,
//! with a `Model` at the root: `Model` is SimpleUML's own root concept and
//! `ModelElement`'s concrete closure is the largest of the four. This is the
//! same restriction `ip29` states for `kitchen_sink.ecore`'s three roots.
//!
//! It is a restriction of the *script*, not of the comparison. The projection
//! reads all four root fields, not one, and
//! [`only_one_root_arm_is_ever_written`] asserts on every comparison of every
//! script that exactly one of them is set — so an operation that leaked into
//! another arm would be a failure and not a silence.
//!
//! # What the projection excludes, and why
//!
//! - **Every non-containment reference**: `Property.ttype`, `Property.owner`,
//!   `Association.source`, `Association.target`, `Generalization.general` and
//!   `Packageable.owner`. Design §8: the interpreted path carries a reference
//!   as a string, the generated path carries it as a `typed_graph!` arc in a
//!   second log and `SimpleumlValue::refs` is `serde(skip)`. There is no value
//!   on the generated side to compare against, so the projection drops the key
//!   from the interpreted side and the scripts never write one. This is the
//!   same exclusion `ip13` makes and [`the_only_exclusion_is_the_reference`]
//!   is what keeps it from growing.
//!
//!   Six of SimpleUML's references declare an `eOpposite`, and the parser
//!   drops the pairing with a warning (`ecore-rs`, "`eOpposite` attributes are
//!   currently not supported, ignoring"). That is a pre-existing limit of the
//!   parser, not of this oracle, and it costs the comparison nothing: the
//!   references are excluded on both sides either way.
//!
//! - **Every value that is the default of its feature's rule**, dropped from
//!   both sides by the same function ([`without_defaults`]). `record!`'s `new`
//!   builds one field per feature eagerly, so a `ModelLog` that exists renders
//!   its whole shape — `name` as `""`, `stereotype` as `{}`, `ownedElements`
//!   as `[]` — while the interpreted path has no object until an operation
//!   mints one. A never-written feature is semantically absent whichever path
//!   renders it. `ip13`'s rule, unchanged, over the leaves this metamodel has.
//!
//!   `SimpleUML.ecore` declares no optional — every attribute is single or a
//!   bag and every containment is a sequence — so the optional exemption that
//!   rule carries is not exercised here.
//!   [`simpleuml_declares_no_optional`] says so from the table.
//!
//! - **An object created into an ordered containment and never written
//!   into**, which the generated read-out cannot distinguish from a removed
//!   one. Not part of the projection: a named, separate step applied on top of
//!   it, to both sides, by [`except_unwritten_sequence_children`]. The reason
//!   and the decision behind it are `ip13`'s, at
//!   `generated/bt_crdt/tests/support/mod.rs`, and this file re-states neither
//!   — it inherits the exception and
//!   [`the_thirty_scripts_find_exactly_one_kind_of_difference`] keeps it the
//!   only one.
//!
//! - Nothing else.
//!
//! # The typed encoder
//!
//! The generated operation is built as JSON and then deserialized into
//! `Simpleuml`, so a wrong shape is a `serde_json::from_value` error naming
//! the enum it could not build and never a silent pass. Nothing about
//! `SimpleUML.ecore` is hard-coded: the `<Super>Super` hops come from the
//! class table's `supers`, the `union!` variant chains from its subclass
//! relation, and the field names from `heck`, which is the rule the generator
//! itself names its fields with. `Class.abstract` is the one that would catch
//! a guess — the generated field is the raw identifier `r#abstract` and its
//! `record!` variant is `Abstract` —
//! and [`the_typed_encoder_walks_the_super_hops_and_the_union_variants`]
//! pins the whole chain for `Class`, which reaches `name` through four hops.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use heck::{ToSnakeCase, ToUpperCamelCase};
use moirai_crdt::utils::membership::twins_log;
use moirai_interp::testing::opened;
use moirai_interp::{InstanceOp, LeafOp, ModelLog, ModelOp, Scalar};
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::Tcsb;
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::{IsReplica, Replica};
use moirai_semantics::{
    ClassSemantics, ClassSlot, FeatureSlot, LeafRule, MergeRule, MetamodelSemantics, Shape,
    from_descriptor,
};
use serde_json::{Map, Value, json};
use simpleuml_crdt::package::{Simpleuml, SimpleumlLog, SimpleumlValue};

type InterpReplica = Replica<ModelLog, Tcsb<ModelOp>>;
type GenReplica = Replica<SimpleumlLog, Tcsb<Simpleuml>>;

/// The key the canonical form carries a class name under.
const ECLASS: &str = "eClass";
/// The key the canonical form carries a conflict set under.
const CONFLICT: &str = "__conflict";

/// The root arm this oracle drives, of the four `SimpleumlValue` carries.
const ROOT_ARM: &str = "ModelElement";
/// The concrete class that sits at the root, inside that arm.
const ROOT_CLASS: &str = "Model";

/// The descriptor this crate was generated from, read from the crate itself
/// so that the interpreted arm cannot be running a different metamodel from
/// the one the generated arm was compiled from. `ip13` has two sources to
/// reconcile — `moirai-interp`'s checked-in `BT_DESCRIPTOR` and `bt_crdt`'s
/// `metamodel.json` — and reconciles them in a test. There is one source
/// here, and [`the_descriptor_and_the_generated_crate_name_the_same_roots`]
/// is what stands in its place: it checks the descriptor against the shape
/// the generated Rust actually has.
fn simpleuml_descriptor() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/metamodel.json"))
            .expect("the generated crate ships its descriptor"),
    )
    .expect("the descriptor is JSON")
}

// ---------------------------------------------------------------------------
// 1. The metamodel, read the way both paths read it
// ---------------------------------------------------------------------------

/// A [`MetamodelSemantics`] plus the two derived relations the encoders need:
/// who inherits from whom, and which class the generator would have wrapped
/// in a `union!`.
///
/// `ip13`'s `Meta`, with one change: `root_class` becomes the pair
/// [`Meta::root_arm`] and [`Meta::root_concrete`], because this metamodel's
/// roots are four abstract classes rather than one concrete record.
struct Meta {
    sem: Arc<MetamodelSemantics>,
    /// Direct subclasses, by declaring class slot.
    subs: BTreeMap<u16, Vec<ClassSlot>>,
}

impl Meta {
    fn new(sem: Arc<MetamodelSemantics>) -> Self {
        let mut subs: BTreeMap<u16, Vec<ClassSlot>> = BTreeMap::new();
        for class in &sem.classes {
            for sup in &class.supers {
                subs.entry(sup.0).or_default().push(class.slot);
            }
        }
        Meta { sem, subs }
    }

    fn class(&self, slot: ClassSlot) -> &ClassSemantics {
        &self.sem.classes[slot.index()]
    }

    fn name(&self, slot: ClassSlot) -> &str {
        &self.class(slot).name
    }

    fn slot(&self, name: &str) -> ClassSlot {
        self.sem
            .classes
            .iter()
            .find(|class| &*class.name == name)
            .unwrap_or_else(|| panic!("no class `{name}` in the table"))
            .slot
    }

    fn subs(&self, slot: ClassSlot) -> &[ClassSlot] {
        self.subs.get(&slot.0).map_or(&[], Vec::as_slice)
    }

    /// Every class the descriptor names as a root, in table order. Four here,
    /// and the projection reads all four.
    fn roots(&self) -> &[ClassSlot] {
        &self.sem.roots
    }

    /// The root arm the scripts write into: the declared root class whose
    /// `union!` carries the object at the top of the model.
    fn root_arm(&self) -> ClassSlot {
        let arm = self.slot(ROOT_ARM);
        assert!(
            self.roots().contains(&arm),
            "`{ROOT_ARM}` is not one of the descriptor's roots"
        );
        arm
    }

    /// The concrete class that actually sits at the root.
    fn root_concrete(&self) -> ClassSlot {
        let concrete = self.slot(ROOT_CLASS);
        assert!(
            self.class(self.root_arm()).concrete.contains(&concrete),
            "`{ROOT_CLASS}` is not below `{ROOT_ARM}`"
        );
        concrete
    }

    /// Whether `classifier/mod.rs:36-45` would have emitted a `<Name>Kind`
    /// `union!` for this class: abstract, an interface, or having subclasses.
    fn is_union(&self, slot: ClassSlot) -> bool {
        !self.class(slot).instantiable || !self.subs(slot).is_empty()
    }

    /// The visible slot of one feature on one class: what an
    /// `InstanceOp::Field` carries.
    fn visible_slot(&self, class: ClassSlot, feature: &str) -> FeatureSlot {
        let holder = self.class(class);
        let index = holder
            .visible
            .iter()
            .position(|(name, _, _)| &**name == feature)
            .unwrap_or_else(|| panic!("`{}` cannot see `{feature}`", holder.name));
        FeatureSlot(index as u16)
    }

    /// The class that declares one visible feature, and its rule.
    fn declared(&self, class: ClassSlot, feature: &str) -> (ClassSlot, MergeRule) {
        let holder = self.class(class);
        let (_, owner, slot) = holder
            .visible
            .iter()
            .find(|(name, _, _)| &**name == feature)
            .unwrap_or_else(|| panic!("`{}` cannot see `{feature}`", holder.name));
        let rule = *self
            .sem
            .rule(*owner, *slot)
            .expect("a visible feature has a rule");
        (*owner, rule)
    }

    fn rule(&self, class: ClassSlot, feature: &str) -> MergeRule {
        self.declared(class, feature).1
    }

    /// The chain of classes from `from` up to `to`, `to` included and `from`
    /// excluded: one `<Super>Super` hop per entry on the generated path, and
    /// nothing at all on the interpreted one.
    ///
    /// SimpleUML is where this earns its keep. `Package` extends `Classifier`
    /// *and* `Packageable`, `TType` and `Association` do the same, and the
    /// chain that reaches `name` from `Class` is four hops long:
    /// `data_type_super`, `t_type_super`, `classifier_super`,
    /// `model_element_super`.
    fn super_path(&self, from: ClassSlot, to: ClassSlot) -> Vec<ClassSlot> {
        fn walk(meta: &Meta, at: ClassSlot, to: ClassSlot, acc: &mut Vec<ClassSlot>) -> bool {
            if at == to {
                return true;
            }
            for sup in &meta.class(at).supers {
                acc.push(*sup);
                if walk(meta, *sup, to, acc) {
                    return true;
                }
                acc.pop();
            }
            false
        }
        let mut acc = Vec::new();
        assert!(
            walk(self, from, to, &mut acc),
            "`{}` does not inherit from `{}`",
            self.name(from),
            self.name(to)
        );
        acc
    }

    /// The `union!` variant names to descend, from the declared target of a
    /// containment down to the concrete class actually sitting there. Empty
    /// when the generator emitted the record directly.
    fn union_descent(&self, target: ClassSlot, concrete: ClassSlot) -> Vec<String> {
        if !self.is_union(target) {
            assert_eq!(
                target, concrete,
                "a containment onto a class with no `union!` holds that class only"
            );
            return Vec::new();
        }
        if target == concrete {
            // `classifier/mod.rs:592`: a concrete class with subclasses is
            // the first variant of its own union, named after itself.
            return vec![self.name(target).to_string()];
        }
        let step = self
            .subs(target)
            .iter()
            .copied()
            .find(|sub| self.class(*sub).concrete.contains(&concrete))
            .unwrap_or_else(|| {
                panic!(
                    "`{}` is not below `{}`",
                    self.name(concrete),
                    self.name(target)
                )
            });
        let mut chain = vec![self.name(step).to_string()];
        if self.is_union(step) {
            chain.extend(self.union_descent(step, concrete));
        } else {
            assert_eq!(step, concrete, "a record variant is its own class");
        }
        chain
    }

    /// The single-valued containments of `class` whose target the generator
    /// compiles as a plain record, and which therefore exist on the generated
    /// path from the moment the parent does.
    ///
    /// `SimpleUML.ecore` has none — every containment it declares is a
    /// sequence — so this is empty for every class and the mint of an object
    /// is one operation. [`simpleuml_has_no_mandatory_child`] says so.
    fn mandatory(&self, class: ClassSlot) -> Vec<(String, ClassSlot)> {
        self.class(class)
            .visible
            .iter()
            .filter_map(|(name, owner, slot)| {
                let rule = self.sem.rule(*owner, *slot)?;
                match rule {
                    MergeRule::Containment { shape, target }
                        if *shape == Shape::Single && !self.is_union(*target) =>
                    {
                        Some((name.to_string(), *target))
                    }
                    _ => None,
                }
            })
            .collect()
    }
}

/// One externally tagged enum value: the variant name, and what it carries.
/// Written out rather than through `json!`, whose keys are literals.
fn tagged(variant: impl Into<String>, payload: Value) -> Value {
    let mut map = Map::new();
    map.insert(variant.into(), payload);
    Value::Object(map)
}

/// The Rust field name the generator gives a feature.
fn field_of(feature: &str) -> String {
    let name = feature.to_snake_case();
    // `ident.rs`'s `value_ident` gives a feature whose snake-cased name is a
    // Rust keyword a `_field` suffix rather than making it a raw identifier,
    // so `SimpleUML.ecore`'s `Class.abstract` is the field `abstract_field`
    // and, through `paste!`'s `:camel`, the variant `AbstractField`.
    if RUST_KEYWORDS.contains(&name.as_str()) {
        format!("{name}_field")
    } else {
        name
    }
}

/// The Rust keywords `ident.rs` renames a feature away from. Only `abstract`
/// is reached here; the rest are carried so a metamodel that declares one is a
/// passing test and not a missing field.
const RUST_KEYWORDS: [&str; 52] = [
    "as", "async", "await", "abstract", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "try", "type",
    "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// The `record!` variant a feature becomes: `paste!`'s `:camel` over the field
/// name the generator wrote.
fn variant_of(feature: &str) -> String {
    field_of(feature).to_upper_camel_case()
}

/// The `<Super>Super` field the generator emits for an inherited class
/// (`classifier/mod.rs:62-70`), as its `record!` variant.
fn super_variant(class: &str) -> String {
    format!("{}_super", class.to_snake_case()).to_upper_camel_case()
}

// ---------------------------------------------------------------------------
// 2. The edit script
// ---------------------------------------------------------------------------

/// One step of a path from the model root: which feature, at which position
/// when the feature is ordered, and the concrete class of the object landed
/// on.
#[derive(Clone, Debug, PartialEq)]
struct Hop {
    feature: String,
    at: Option<usize>,
    class: String,
}

/// Where an edit happens: the root object, then the hops down to the object
/// the edit addresses.
#[derive(Clone, Debug, PartialEq, Default)]
struct Path {
    hops: Vec<Hop>,
}

impl Path {
    fn child(&self, hop: Hop) -> Path {
        let mut hops = self.hops.clone();
        hops.push(hop);
        Path { hops }
    }

    fn depth(&self) -> usize {
        self.hops.len()
    }

    fn show(&self) -> String {
        let mut out = String::from("/");
        for hop in &self.hops {
            out.push_str(&hop.feature);
            if let Some(at) = hop.at {
                out.push_str(&format!("[{at}]"));
            }
            out.push_str(&format!(":{}/", hop.class));
        }
        out
    }
}

/// What an edit does to the object its path names.
#[derive(Clone, Debug, PartialEq)]
enum Action {
    /// Mint the object the path names. Only ever used with an empty path, to
    /// put the root `Model` there: every other object is minted by a
    /// [`Action::Create`] against its parent. `ip13` has no equivalent
    /// because `bt.ecore`'s root is a `record!` and its mint is the opener
    /// `Harness::new` sends.
    New,
    /// Mint an object of `class` in `feature`, at `pos` when the feature is
    /// ordered, together with its mandatory children.
    Create {
        feature: String,
        pos: Option<usize>,
        class: String,
    },
    /// Take the child at `pos` out of an ordered containment.
    Delete { feature: String, pos: usize },
    /// `InsertChar` and `DeleteChar` on a text leaf.
    Text { feature: String, op: TextOp },
    /// Unset an optional attribute. `SimpleUML.ecore` declares no optional,
    /// so nothing proposes this; the arm is here so that the encoders stay
    /// `ip13`'s and a metamodel change would be a compile-time concern rather
    /// than a silent gap.
    Unset { feature: String },
    /// Write an enable-wins flag: `Class.abstract`,
    /// `Generalization.isSubstitutable`.
    Flag { feature: String, op: FlagOp },
    /// Write the collection of strings that is `ModelElement.stereotype`: an
    /// add-wins set, because the file writes `ordered="false"` and leaves
    /// `unique` silent, which Ecore reads as `true`.
    Collection { feature: String, op: CollectionOp },
}

#[derive(Clone, Debug, PartialEq)]
enum TextOp {
    Insert {
        pos: usize,
        ch: char,
        /// The whole attribute as the writer sees it after this character
        /// goes in. Only a register binding reads it, and this metamodel
        /// binds none; carried so the encoder stays `ip13`'s.
        after: String,
    },
    Delete {
        pos: usize,
        after: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum FlagOp {
    Enable,
    Disable,
    Clear,
}

#[derive(Clone, Debug, PartialEq)]
enum CollectionOp {
    Add(String),
    Remove(String),
    Clear,
}

/// One edit, addressing an object by a script-local id and, resolved against
/// the writer's own view at the moment it was generated, by a path.
#[derive(Clone, Debug)]
struct Edit {
    /// Script-local object id: `0` is the root, and every `Create` mints the
    /// next one for the object it makes.
    id: u32,
    /// Which replica issues it.
    writer: char,
    path: Path,
    action: Action,
}

impl Edit {
    fn show(&self) -> String {
        let what = match &self.action {
            Action::New => format!("New {ROOT_CLASS}"),
            Action::Create { feature, pos, class } => format!(
                "Create {class} in {feature}{}",
                pos.map_or(String::new(), |p| format!("[{p}]"))
            ),
            Action::Delete { feature, pos } => format!("Delete {feature}[{pos}]"),
            Action::Text {
                feature,
                op: TextOp::Insert { pos, ch, .. },
            } => format!("InsertChar {feature}[{pos}] = {ch:?}"),
            Action::Text {
                feature,
                op: TextOp::Delete { pos, .. },
            } => format!("DeleteChar {feature}[{pos}]"),
            Action::Unset { feature } => format!("Unset {feature}"),
            Action::Flag { feature, op } => format!("{op:?} {feature}"),
            Action::Collection { feature, op } => format!("{op:?} into {feature}"),
        };
        format!(
            "#{} on {} at {} — {what}",
            self.id,
            self.writer,
            self.path.show()
        )
    }
}

/// A whole script: what it is called, and the edits in the order they are
/// issued. `Deliver` marks the points where every pending event crosses.
#[derive(Clone, Debug, Default)]
struct EditScript {
    label: String,
    steps: Vec<Step>,
}

#[derive(Clone, Debug)]
enum Step {
    Edit(Edit),
    /// Deliver everything both writers are holding, `a`'s events first.
    Deliver,
}

// ---------------------------------------------------------------------------
// 3. The two encoders
// ---------------------------------------------------------------------------

/// The class each hop of a path lands on, the concrete root class first.
fn classes_along(meta: &Meta, path: &Path) -> Vec<ClassSlot> {
    let mut out = vec![meta.root_concrete()];
    for hop in &path.hops {
        out.push(meta.slot(&hop.class));
    }
    out
}

/// One edit as a [`ModelOp`]: the interpreted path's encoding.
///
/// Inheritance is flat here — an inherited feature is one visible slot on the
/// instance and no hop at all — which is the whole difference from the typed
/// encoder below. The root is named by the *concrete* class sitting there,
/// which is what `Target::Roots` resolves against.
fn interp_op(meta: &Meta, edit: &Edit) -> ModelOp {
    let classes = classes_along(meta, &edit.path);
    let leaf_class = *classes.last().expect("the root is always there");
    let mut op = interp_action(meta, leaf_class, &edit.action);
    for (index, hop) in edit.path.hops.iter().enumerate().rev() {
        let parent = classes[index];
        let mut step = InstanceOp::variant(meta.slot(&hop.class), op);
        step = match (hop.at, meta.rule(parent, &hop.feature)) {
            (Some(pos), _) => InstanceOp::at(pos, step),
            (None, MergeRule::Containment { shape, .. }) if shape == Shape::Optional => {
                InstanceOp::set(step)
            }
            (None, _) => step,
        };
        op = InstanceOp::field(meta.visible_slot(parent, &hop.feature), step);
    }
    ModelOp::Instance(InstanceOp::variant(meta.root_concrete(), op))
}

/// The interpreted payload that mints an object of `class` together with
/// every mandatory child it has. `SimpleUML.ecore` gives every class an empty
/// chain, so this is `InstanceOp::New`; it is `ip13`'s function unchanged so
/// that the two drivers of the oracle mint the same way.
fn interp_mint(meta: &Meta, class: ClassSlot) -> InstanceOp {
    let chain = mint_chain(meta, class);
    let mut op = InstanceOp::New;
    let mut owners: Vec<ClassSlot> = vec![class];
    owners.extend(chain.iter().map(|(_, target)| *target));
    for (index, (feature, target)) in chain.iter().enumerate().rev() {
        op = InstanceOp::field(
            meta.visible_slot(owners[index], feature),
            InstanceOp::variant(*target, op),
        );
    }
    op
}

/// The one chain of mandatory children a mint has to carry.
fn mint_chain(meta: &Meta, class: ClassSlot) -> Vec<(String, ClassSlot)> {
    let mut chain = Vec::new();
    let mut at = class;
    let mut guard = 0;
    loop {
        let mandatory = meta.mandatory(at);
        assert!(
            mandatory.len() < 2,
            "`{}` has {} mandatory single containments; this oracle mints one \
             chain per create",
            meta.name(at),
            mandatory.len()
        );
        let Some((feature, target)) = mandatory.into_iter().next() else {
            return chain;
        };
        chain.push((feature, target));
        at = target;
        guard += 1;
        assert!(guard < 16, "a cycle of mandatory single containments");
    }
}

fn interp_action(meta: &Meta, class: ClassSlot, action: &Action) -> InstanceOp {
    match action {
        Action::New => interp_mint(meta, class),
        Action::Create { feature, pos, class: made } => {
            let made = meta.slot(made);
            let inner = InstanceOp::variant(made, interp_mint(meta, made));
            let shaped = match meta.rule(class, feature) {
                MergeRule::Containment { shape, .. } => shape,
                other => panic!("`{feature}` is not a containment: {other:?}"),
            };
            let step = match (shaped, pos) {
                (Shape::Sequence, Some(pos)) => InstanceOp::insert(*pos, inner),
                (Shape::Optional, None) => InstanceOp::set(inner),
                (Shape::Single, None) => inner,
                (shape, pos) => panic!("{shape:?} does not take {pos:?}"),
            };
            InstanceOp::field(meta.visible_slot(class, feature), step)
        }
        Action::Delete { feature, pos } => {
            InstanceOp::field(meta.visible_slot(class, feature), InstanceOp::delete(*pos))
        }
        Action::Unset { feature } => {
            InstanceOp::field(meta.visible_slot(class, feature), InstanceOp::unset())
        }
        Action::Flag { feature, op } => {
            let leaf = match op {
                FlagOp::Enable => LeafOp::Enable,
                FlagOp::Disable => LeafOp::Disable,
                FlagOp::Clear => LeafOp::Clear,
            };
            // `Class.abstract` and `Generalization.isSubstitutable` write no
            // `lowerBound`, which Ecore reads as `0`, so both sit under an
            // optional and every write is a `Set` into it.
            let step = match meta.rule(class, feature) {
                MergeRule::Attribute {
                    shape: Shape::Optional,
                    ..
                } => InstanceOp::set(InstanceOp::Leaf(leaf)),
                _ => InstanceOp::Leaf(leaf),
            };
            InstanceOp::field(meta.visible_slot(class, feature), step)
        }
        Action::Collection { feature, op } => {
            let leaf = match op {
                CollectionOp::Add(value) => LeafOp::Add(Scalar::text(value.clone())),
                CollectionOp::Remove(value) => LeafOp::Remove(Scalar::text(value.clone())),
                CollectionOp::Clear => LeafOp::Clear,
            };
            InstanceOp::field(
                meta.visible_slot(class, feature),
                InstanceOp::Leaf(leaf),
            )
        }
        Action::Text { feature, op } => {
            let (shape, leaf) = match meta.rule(class, feature) {
                MergeRule::Attribute { shape, leaf } => (shape, leaf),
                other => panic!("`{feature}` is not an attribute: {other:?}"),
            };
            // The leaf the *table* names decides the operation, exactly as in
            // `ip13`: a text leaf takes the character and the position, a
            // register would take the whole string the writer now sees.
            let leaf_op = match (leaf, op) {
                (LeafRule::Text, TextOp::Insert { pos, ch, .. }) => {
                    LeafOp::InsertChar { pos: *pos, ch: *ch }
                }
                (LeafRule::Text, TextOp::Delete { pos, .. }) => LeafOp::DeleteChar { pos: *pos },
                (
                    LeafRule::Register { .. },
                    TextOp::Insert { after, .. } | TextOp::Delete { after, .. },
                ) => LeafOp::Write(Scalar::text(after.clone())),
                (leaf, op) => panic!("{leaf:?} does not take {op:?}"),
            };
            let step = match shape {
                Shape::Optional => InstanceOp::set(InstanceOp::Leaf(leaf_op)),
                _ => InstanceOp::Leaf(leaf_op),
            };
            InstanceOp::field(meta.visible_slot(class, feature), step)
        }
    }
}

/// One edit as this crate's typed operation, built as JSON by the naming
/// convention the generator uses and then deserialized, so a wrong shape is
/// an error naming the enum and never a silent pass.
fn typed_op(meta: &Meta, edit: &Edit) -> Simpleuml {
    let value = typed_json(meta, edit);
    serde_json::from_value(value.clone()).unwrap_or_else(|error| {
        panic!(
            "the typed encoder built an operation `Simpleuml` cannot take: {error}\n{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        )
    })
}

fn typed_json(meta: &Meta, edit: &Edit) -> Value {
    let classes = classes_along(meta, &edit.path);
    let leaf_class = *classes.last().expect("the root is always there");
    let mut op = typed_action(meta, leaf_class, &edit.action);
    for (index, hop) in edit.path.hops.iter().enumerate().rev() {
        let parent = classes[index];
        let target = match meta.rule(parent, &hop.feature) {
            MergeRule::Containment { target, .. } => target,
            other => panic!("`{}` is not a containment: {other:?}", hop.feature),
        };
        let mut step = union_wrap(meta, target, meta.slot(&hop.class), op);
        step = match (hop.at, meta.rule(parent, &hop.feature)) {
            (Some(pos), _) => json!({ "Update": { "pos": pos, "op": step } }),
            (None, MergeRule::Containment { shape, .. }) if shape == Shape::Optional => {
                json!({ "Set": step })
            }
            (None, _) => step,
        };
        op = feature_wrap(meta, parent, &hop.feature, step);
    }
    // The root arm is a `union!`, not a `record!`: the package operation names
    // `<Root>Kind` and the variant chain descends from the abstract root to
    // the concrete class sitting at the top.
    let arm = meta.root_arm();
    tagged(
        format!("{}Kind", meta.name(arm).to_upper_camel_case()),
        union_wrap(meta, arm, meta.root_concrete(), op),
    )
}

/// Wrap an operation on the record of `class` in the `<Super>Super` hops that
/// carry it from `class` down to the class that declares `feature`, then in
/// the feature's own variant.
fn feature_wrap(meta: &Meta, class: ClassSlot, feature: &str, payload: Value) -> Value {
    let (owner, _) = meta.declared(class, feature);
    let mut value = tagged(variant_of(feature), payload);
    for hop in meta.super_path(class, owner).iter().rev() {
        value = tagged(super_variant(meta.name(*hop)), value);
    }
    value
}

/// Wrap an operation on the record of `concrete` in the `union!` variants
/// that carry it from the containment's declared `target` down to it.
fn union_wrap(meta: &Meta, target: ClassSlot, concrete: ClassSlot, payload: Value) -> Value {
    let mut value = payload;
    for variant in meta.union_descent(target, concrete).iter().rev() {
        value = tagged(variant.clone(), value);
    }
    value
}

/// The typed payload that mints an object of `class` with its mandatory
/// children: `record!`'s `New`, wrapped in one feature per mandatory hop.
fn typed_mint(meta: &Meta, class: ClassSlot) -> Value {
    let chain = mint_chain(meta, class);
    let mut owners: Vec<ClassSlot> = vec![class];
    owners.extend(chain.iter().map(|(_, target)| *target));
    let mut value = json!("New");
    for (index, (feature, target)) in chain.iter().enumerate().rev() {
        let holder = owners[index];
        value = feature_wrap(
            meta,
            holder,
            feature,
            union_wrap(meta, *target, *target, value),
        );
    }
    value
}

fn typed_action(meta: &Meta, class: ClassSlot, action: &Action) -> Value {
    match action {
        Action::New => typed_mint(meta, class),
        Action::Create { feature, pos, class: made } => {
            let made = meta.slot(made);
            let (shape, target) = match meta.rule(class, feature) {
                MergeRule::Containment { shape, target } => (shape, target),
                other => panic!("`{feature}` is not a containment: {other:?}"),
            };
            let inner = union_wrap(meta, target, made, typed_mint(meta, made));
            let step = match (shape, pos) {
                (Shape::Sequence, Some(pos)) => json!({ "Insert": { "pos": pos, "op": inner } }),
                (Shape::Optional, None) => json!({ "Set": inner }),
                (Shape::Single, None) => inner,
                (shape, pos) => panic!("{shape:?} does not take {pos:?}"),
            };
            feature_wrap(meta, class, feature, step)
        }
        Action::Delete { feature, pos } => {
            feature_wrap(meta, class, feature, json!({ "Delete": { "pos": pos } }))
        }
        Action::Unset { feature } => feature_wrap(meta, class, feature, json!("Unset")),
        Action::Flag { feature, op } => {
            let leaf = match op {
                FlagOp::Enable => json!("Enable"),
                FlagOp::Disable => json!("Disable"),
                FlagOp::Clear => json!("Clear"),
            };
            let step = match meta.rule(class, feature) {
                MergeRule::Attribute {
                    shape: Shape::Optional,
                    ..
                } => json!({ "Set": leaf }),
                _ => leaf,
            };
            feature_wrap(meta, class, feature, step)
        }
        Action::Collection { feature, op } => {
            let leaf = match op {
                CollectionOp::Add(value) => tagged("Add", Value::String(value.clone())),
                CollectionOp::Remove(value) => tagged("Remove", Value::String(value.clone())),
                CollectionOp::Clear => json!("Clear"),
            };
            feature_wrap(meta, class, feature, leaf)
        }
        Action::Text { feature, op } => {
            let shape = match meta.rule(class, feature) {
                MergeRule::Attribute { shape, .. } => shape,
                other => panic!("`{feature}` is not an attribute: {other:?}"),
            };
            let leaf = match op {
                TextOp::Insert { pos, ch, .. } => {
                    json!({ "Insert": { "content": ch, "pos": pos } })
                }
                TextOp::Delete { pos, .. } => json!({ "Delete": { "pos": pos } }),
            };
            let step = match shape {
                Shape::Optional => json!({ "Set": leaf }),
                _ => leaf,
            };
            feature_wrap(meta, class, feature, step)
        }
    }
}

// ---------------------------------------------------------------------------
// 4. The projection of the generated read-out onto the canonical form
// ---------------------------------------------------------------------------

/// The generated `Read` value in the canonical form of `02 Validation Plan`
/// §2: `_super` hops flattened, `union!` wrappers unwrapped into an `eClass`
/// key, `Vec<char>` joined, a bag's `HashMap<String, usize>` spread into the
/// sorted array with repeats a bag reads as, sequences in read order, keys
/// sorted, conflicts by class name and then by canonical bytes.
///
/// All four root arms are read, not the one the scripts drive. Exactly one of
/// them may hold anything, and this is where that is checked: an operation
/// that leaked into `packageable`, `classifier` or `t_type` would fail here
/// rather than be projected away.
fn project(meta: &Meta, value: &SimpleumlValue) -> Value {
    let raw = serde_json::to_value(value).expect("the generated read-out serializes");
    let mut held: Vec<(String, Value)> = Vec::new();
    for root in meta.roots() {
        let field = field_of(meta.name(*root));
        let found = raw
            .get(&field)
            .unwrap_or_else(|| panic!("`SimpleumlValue` has no field `{field}`"));
        if let Some(object) = project_slot(meta, *root, found) {
            held.push((meta.name(*root).to_string(), object));
        }
    }
    assert!(
        held.len() <= 1,
        "{} root arms hold an object at once — {} — and the interpreted path \
         has one root slot to compare them against",
        held.len(),
        held.iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    match held.pop() {
        None => Value::Null,
        Some((_, object)) => without_defaults(meta, object),
    }
}

fn project_object(meta: &Meta, class: ClassSlot, value: &Value) -> Value {
    let mut out = Map::new();
    out.insert(ECLASS.to_string(), Value::String(meta.name(class).to_string()));
    for (name, owner, slot) in &meta.class(class).visible {
        let Some(rule) = meta.sem.rule(*owner, *slot) else {
            continue;
        };
        match rule {
            // Design §8: a reference is a string on the interpreted path and
            // a `typed_graph!` arc on the generated one, so there is nothing
            // here to compare and `canon` drops the other side's key too.
            MergeRule::Reference { .. } | MergeRule::Unsupported { .. } => continue,
            MergeRule::Attribute { shape, leaf } => {
                let found = locate(meta, class, *owner, name, value);
                let projected = match (shape, leaf) {
                    (Shape::Single, LeafRule::Text) => Some(chars(found)),
                    (Shape::Optional, LeafRule::Text) => {
                        if found.is_null() {
                            None
                        } else {
                            Some(chars(found))
                        }
                    }
                    // `VecLog<EWFlag>` reads as a bare `bool`, which is what
                    // `LeafLog::FlagEw` renders too.
                    (Shape::Single, LeafRule::Flag { .. }) => Some(found.clone()),
                    // Under an `OptionLog` the same flag reads `Option<bool>`,
                    // and `None` is the unwritten optional the interpreted
                    // path carries no key for.
                    (Shape::Optional, LeafRule::Flag { .. }) => {
                        if found.is_null() {
                            None
                        } else {
                            Some(found.clone())
                        }
                    }
                    // `VecLog<AWSet<String>>` reads as a `HashSet<String>`;
                    // `LeafLog::SetAw` renders the same values sorted.
                    (Shape::Set { .. }, LeafRule::Text) => Some(sorted_strings(found)),
                    // `AWBagLog<String>` reads as a map of value to count;
                    // `LeafLog::Bag` renders the same counts as a sorted
                    // array with repeats. `SimpleUML.ecore` reaches no bag
                    // since a silent `unique` became Ecore's `true`, and the
                    // arm stays so a metamodel that does is a projection and
                    // not a panic.
                    (Shape::Bag, LeafRule::Text) => Some(bag(found)),
                    (shape, leaf) => panic!(
                        "no projection for {shape:?} of {leaf:?} on `{}.{name}`; \
                         `SimpleUML.ecore` has none and this oracle claims none",
                        meta.name(class)
                    ),
                };
                if let Some(projected) = projected {
                    out.insert(name.to_string(), projected);
                }
            }
            MergeRule::Containment { shape, target } => {
                let found = locate(meta, class, *owner, name, value);
                match shape {
                    Shape::Single => {
                        if let Some(object) = project_slot(meta, *target, found) {
                            out.insert(name.to_string(), object);
                        }
                    }
                    Shape::Optional => {
                        if !found.is_null()
                            && let Some(object) = project_slot(meta, *target, found)
                        {
                            out.insert(name.to_string(), object);
                        }
                    }
                    Shape::Sequence => {
                        let items = found
                            .as_array()
                            .unwrap_or_else(|| panic!("`{name}` reads as an array"))
                            .iter()
                            .filter_map(|item| project_slot(meta, *target, item))
                            .collect();
                        out.insert(name.to_string(), Value::Array(items));
                    }
                    shape => panic!("a containment cannot be {shape:?}"),
                }
            }
        }
    }
    Value::Object(out)
}

/// One containment site: the record directly, or whatever the `union!`
/// wrapper holds, flattened across nested unions and re-ordered the way §2
/// asks for.
fn project_slot(meta: &Meta, target: ClassSlot, value: &Value) -> Option<Value> {
    if !meta.is_union(target) {
        return Some(project_object(meta, target, value));
    }
    let mut held = Vec::new();
    collect_union(meta, target, value, &mut held);
    match held.len() {
        0 => None,
        1 => Some(held.pop().expect("just counted").1),
        _ => {
            // `union!`'s own `Conflict` is sorted by variant rank
            // (`union.rs:96-100`); §2 asks for class name and then canonical
            // bytes, which is what the interpreted read-out already does.
            let mut keyed: Vec<(String, Vec<u8>, Value)> = held
                .into_iter()
                .map(|(name, value)| {
                    let bytes = serde_json::to_vec(&value).unwrap_or_default();
                    (name, bytes, value)
                })
                .collect();
            keyed.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
            let mut out = Map::new();
            out.insert(
                CONFLICT.to_string(),
                Value::Array(keyed.into_iter().map(|(_, _, value)| value).collect()),
            );
            Some(Value::Object(out))
        }
    }
}

fn collect_union(meta: &Meta, target: ClassSlot, value: &Value, out: &mut Vec<(String, Value)>) {
    match value {
        Value::String(word) if word == "Unset" => {}
        Value::Object(map) if map.len() == 1 => {
            let (key, payload) = map.iter().next().expect("just checked");
            match key.as_str() {
                "Value" => collect_child(meta, target, payload, out),
                "Conflict" => {
                    for child in payload
                        .as_array()
                        .unwrap_or_else(|| panic!("a `Conflict` holds an array"))
                    {
                        collect_child(meta, target, child, out);
                    }
                }
                other => panic!("`{other}` is not a `union!` state"),
            }
        }
        other => panic!("`{other}` is not a `union!` value"),
    }
}

fn collect_child(meta: &Meta, target: ClassSlot, value: &Value, out: &mut Vec<(String, Value)>) {
    let Value::Object(map) = value else {
        panic!("a `union!` child is its variant object");
    };
    let (variant, payload) = map.iter().next().expect("a variant carries one payload");
    let class = meta.slot(variant);
    if class != target && meta.is_union(class) {
        collect_union(meta, class, payload, out);
    } else {
        out.push((
            meta.name(class).to_string(),
            project_object(meta, class, payload),
        ));
    }
}

/// The JSON of one feature, reached from the record of `class` through the
/// `<Super>Super` fields that carry it to the class that declares it.
fn locate<'a>(
    meta: &Meta,
    class: ClassSlot,
    owner: ClassSlot,
    feature: &str,
    value: &'a Value,
) -> &'a Value {
    let mut at = value;
    for hop in meta.super_path(class, owner) {
        let field = format!("{}_super", meta.name(hop).to_snake_case());
        at = at.get(&field).unwrap_or_else(|| {
            panic!(
                "`{}` has no `{field}` on the way to `{feature}`",
                meta.name(class)
            )
        });
    }
    at.get(field_of(feature)).unwrap_or_else(|| {
        panic!(
            "`{}` has no field `{}` for `{feature}`",
            meta.name(owner),
            field_of(feature)
        )
    })
}

/// A `Vec<char>` as a string.
fn chars(value: &Value) -> Value {
    let text: String = value
        .as_array()
        .unwrap_or_else(|| panic!("a text attribute reads as an array of characters: {value}"))
        .iter()
        .map(|ch| {
            ch.as_str()
                .unwrap_or_else(|| panic!("a character reads as a string"))
                .to_string()
        })
        .collect();
    Value::String(text)
}

/// A `HashSet<String>` as `leaf.rs`'s `sorted_array` renders one.
fn sorted_strings(value: &Value) -> Value {
    let mut held: Vec<String> = value
        .as_array()
        .unwrap_or_else(|| panic!("a set reads as an array: {value}"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("a set of strings holds strings: {item}"))
                .to_string()
        })
        .collect();
    held.sort();
    Value::Array(held.into_iter().map(Value::String).collect())
}

/// A `HashMap<String, usize>` as `leaf.rs`'s bag arm renders one: every value
/// repeated as many times as its count, sorted the way `Scalar`'s own `Ord`
/// sorts `Scalar::Str`, which is `String`'s.
fn bag(value: &Value) -> Value {
    let mut spread: Vec<String> = Vec::new();
    for (key, count) in value
        .as_object()
        .unwrap_or_else(|| panic!("a bag reads as a map of value to count: {value}"))
    {
        let count = count.as_u64().expect("a count is a number");
        for _ in 0..count {
            spread.push(key.clone());
        }
    }
    spread.sort();
    Value::Array(spread.into_iter().map(Value::String).collect())
}

// ---------------------------------------------------------------------------
// 4b. Defaults, dropped from both sides by the same function
// ---------------------------------------------------------------------------

/// A canonical document with every default-valued key dropped, and a root
/// that is entirely default spelled `null`.
///
/// `ip13`'s rule, unchanged, over the leaves this metamodel has. Called on
/// both sides of the comparison, each with its own table, and it is the
/// *only* place either side is pruned, so the two sides cannot drift. The
/// whole argument for it is in
/// `generated/bt_crdt/tests/support/mod.rs`; what matters here is that the
/// rule is the same one, the exemption it carries for an optional included —
/// every single-valued attribute of `SimpleUML.ecore` is one, because the file
/// writes no `lowerBound` and Ecore's default for one is `0`.
fn without_defaults(meta: &Meta, value: Value) -> Value {
    let value = drop_defaults(meta, value);
    if only_a_class(&value) {
        Value::Null
    } else {
        value
    }
}

/// An object carrying nothing but its class name.
fn only_a_class(value: &Value) -> bool {
    matches!(value, Value::Object(map) if map.len() == 1 && map.contains_key(ECLASS))
}

fn drop_defaults(meta: &Meta, value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| drop_defaults(meta, item))
                .collect(),
        ),
        Value::Object(map) => {
            // A conflict set carries no `eClass` of its own, so its keys are
            // left alone and only its members are walked.
            let class = map
                .get(ECLASS)
                .and_then(Value::as_str)
                .map(|name| meta.slot(name));
            let mut out = Map::new();
            for (key, item) in map {
                let item = drop_defaults(meta, item);
                if let Some(class) = class
                    && key != ECLASS
                    && is_default(meta.rule(class, &key), &item)
                {
                    continue;
                }
                out.insert(key, item);
            }
            Value::Object(out)
        }
        other => other,
    }
}

/// Whether a canonical value, its own defaults already dropped, is the
/// default of the rule the feature carrying it is bound to.
fn is_default(rule: MergeRule, value: &Value) -> bool {
    let empty_collection = |value: &Value| value.as_array().is_some_and(|items| items.is_empty());
    let empty_map = |value: &Value| value.as_object().is_some_and(|entries| entries.is_empty());
    match rule {
        // Both are dropped from the interpreted side before this runs and
        // never appear on the generated side at all.
        MergeRule::Reference { .. } | MergeRule::Unsupported { .. } => false,
        MergeRule::Attribute { shape, leaf } => match shape {
            Shape::Optional => false,
            Shape::Single => match leaf {
                LeafRule::Text => value.as_str() == Some(""),
                LeafRule::Counter { .. } => value.as_f64() == Some(0.0),
                LeafRule::Flag { .. } => value.as_bool() == Some(false),
                LeafRule::Register { .. } | LeafRule::Enum { .. } => value.is_null(),
            },
            Shape::Sequence | Shape::Set { .. } | Shape::Bag | Shape::OrderedSet => {
                empty_collection(value)
            }
            Shape::Keyed { .. } => empty_map(value),
        },
        MergeRule::Containment { shape, .. } => match shape {
            Shape::Optional => false,
            Shape::Single => only_a_class(value),
            Shape::Sequence | Shape::Set { .. } | Shape::Bag | Shape::OrderedSet => {
                empty_collection(value)
            }
            Shape::Keyed { .. } => empty_map(value),
        },
    }
}

// ---------------------------------------------------------------------------
// 4c. The one named exclusion of the equivalence criterion
// ---------------------------------------------------------------------------

/// A canonical document with the criterion's one named exception removed: an
/// object created into an **ordered containment and never written into**.
///
/// `ip13`'s exception, inherited whole. `UWMapLog::execute_query`
/// (`moirai-crdt/src/map/uw_map.rs:199-210`) keeps a child only when its
/// value differs from the default, which is how the generated path spells
/// *removed*, and `NestedListLog` sits on that map, so this is every ordered
/// containment. Every containment `SimpleUML.ecore` declares is one.
///
/// Applied to both sides by this one function, on top of the projection and
/// never inside it. It is the ONLY exclusion of its kind and
/// [`the_thirty_scripts_find_exactly_one_kind_of_difference`] is what keeps
/// that honest.
fn except_unwritten_sequence_children(meta: &Meta, mut value: Value) -> Value {
    for _ in 0..16 {
        let next = without_defaults(meta, drop_empty_sequence_children(value.clone()));
        if next == value {
            break;
        }
        value = next;
    }
    value
}

/// One pass of the rule above: every array element carrying nothing but its
/// class name, gone.
fn drop_empty_sequence_children(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(drop_empty_sequence_children)
                .filter(|item| !only_a_class(item))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, item)| (key, drop_empty_sequence_children(item)))
                .collect(),
        ),
        other => other,
    }
}

/// The interpreted read-out with every non-containment reference dropped and
/// every default-valued key with it.
fn canon(meta: &Meta, value: Value) -> Value {
    without_defaults(meta, strip_references(meta, value))
}

/// The interpreted read-out with every non-containment reference dropped and
/// nothing else touched: sparse exactly as `eval::read` wrote it, so a key is
/// absent precisely when nothing has been written under it. This is the view
/// the script generator proposes against.
fn strip_references(meta: &Meta, value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| strip_references(meta, item))
                .collect(),
        ),
        Value::Object(map) => {
            let class = map
                .get(ECLASS)
                .and_then(Value::as_str)
                .map(|name| meta.slot(name));
            let mut out = Map::new();
            for (key, item) in map {
                if let Some(class) = class
                    && key != ECLASS
                    && matches!(meta.rule(class, &key), MergeRule::Reference { .. })
                {
                    continue;
                }
                out.insert(key, strip_references(meta, item));
            }
            Value::Object(out)
        }
        other => other,
    }
}

/// The first place two canonical documents differ, as a path and the two
/// values, so a failure is readable without a diff tool.
fn difference(left: &Value, right: &Value, at: &str) -> Option<String> {
    match (left, right) {
        (Value::Object(l), Value::Object(r)) => {
            let mut keys: Vec<&String> = l.keys().chain(r.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let here = format!("{at}/{key}");
                match (l.get(key), r.get(key)) {
                    (Some(l), Some(r)) => {
                        if let Some(found) = difference(l, r, &here) {
                            return Some(found);
                        }
                    }
                    (Some(l), None) => {
                        return Some(format!(
                            "{here}: interpreted has {l}, generated has no key"
                        ));
                    }
                    (None, Some(r)) => {
                        return Some(format!(
                            "{here}: interpreted has no key, generated has {r}"
                        ));
                    }
                    (None, None) => {}
                }
            }
            None
        }
        (Value::Array(l), Value::Array(r)) => {
            if l.len() != r.len() {
                return Some(format!(
                    "{at}: interpreted holds {} items, generated holds {}\n  \
                     interpreted: {left}\n  generated:   {right}",
                    l.len(),
                    r.len()
                ));
            }
            for (index, (l, r)) in l.iter().zip(r).enumerate() {
                if let Some(found) = difference(l, r, &format!("{at}[{index}]")) {
                    return Some(found);
                }
            }
            None
        }
        (l, r) if l == r => None,
        (l, r) => Some(format!("{at}: interpreted {l}, generated {r}")),
    }
}

// ---------------------------------------------------------------------------
// 5. The harness
// ---------------------------------------------------------------------------

/// Two `twins_log` pairs, one per path, driven by one script.
struct Harness {
    /// The table the generated arm was compiled from, which is also the one
    /// the projection and the script generator read.
    gen_meta: Meta,
    /// The table the interpreted arm runs.
    interp_meta: Meta,
    ia: InterpReplica,
    ib: InterpReplica,
    ga: GenReplica,
    gb: GenReplica,
    /// Events not yet delivered, by the replica that wrote them.
    pending_a: Vec<(EventMessage<ModelOp>, EventMessage<Simpleuml>)>,
    pending_b: Vec<(EventMessage<ModelOp>, EventMessage<Simpleuml>)>,
    /// How many operations each edit cost, how many were refused, and how
    /// many times the two read-outs were compared.
    ops: usize,
    refused: usize,
    comparisons: usize,
}

impl Harness {
    /// No generated opener, unlike `ip13`: every root of this metamodel is a
    /// `union!` and a `union!` has no operation that writes nothing. See the
    /// module header for why the sequence-number offset that leaves cannot
    /// move a tie.
    fn new(gen_descriptor: &Value, interp_descriptor: &Value) -> Harness {
        let gen_meta = Meta::new(Arc::new(
            from_descriptor(gen_descriptor).expect("the checked-in descriptor parses"),
        ));
        let interp_meta = Meta::new(Arc::new(
            from_descriptor(interp_descriptor).expect("the interpreted descriptor parses"),
        ));
        let (ia, ib) = opened("equivalence", interp_descriptor);
        let (ga, gb) = twins_log::<SimpleumlLog>();
        Harness {
            gen_meta,
            interp_meta,
            ia,
            ib,
            ga,
            gb,
            pending_a: Vec::new(),
            pending_b: Vec::new(),
            ops: 0,
            refused: 0,
            comparisons: 0,
        }
    }

    fn interp_doc(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ia } else { &self.ib };
        canon(&self.interp_meta, replica.query(&Read::<Value>::new()))
    }

    /// What a writer sitting at one replica actually sees: the interpreted
    /// read-out with references gone and nothing else pruned. The script
    /// generator proposes against this and never against [`Harness::interp_doc`],
    /// whose whole job is to erase the difference between an object at its
    /// defaults and no object at all.
    fn interp_view(&self, writer: char) -> Value {
        let replica = if writer == 'a' { &self.ia } else { &self.ib };
        strip_references(&self.interp_meta, replica.query(&Read::<Value>::new()))
    }

    fn gen_raw(&self, writer: char) -> SimpleumlValue {
        let replica = if writer == 'a' { &self.ga } else { &self.gb };
        replica.query(&Read::<SimpleumlValue>::new())
    }

    fn gen_doc(&self, writer: char) -> Value {
        project(&self.gen_meta, &self.gen_raw(writer))
    }

    /// Both replicas of both paths, compared. `Ok` when the four read-outs
    /// are two equal pairs.
    fn compare(&mut self) -> Result<(), String> {
        for writer in ['a', 'b'] {
            let interp =
                except_unwritten_sequence_children(&self.interp_meta, self.interp_doc(writer));
            let generated =
                except_unwritten_sequence_children(&self.gen_meta, self.gen_doc(writer));
            self.comparisons += 1;
            if interp != generated {
                let where_ = difference(&interp, &generated, "")
                    .unwrap_or_else(|| "the documents differ but no key does".to_string());
                return Err(format!(
                    "replica {writer}: {where_}\n  interpreted: {}\n  generated:   {}",
                    serde_json::to_string(&interp).unwrap_or_default(),
                    serde_json::to_string(&generated).unwrap_or_default(),
                ));
            }
        }
        Ok(())
    }

    /// One edit, encoded twice and sent to the two replicas of its writer,
    /// with the read-outs compared afterwards. This is what a *test* uses.
    fn apply(&mut self, edit: &Edit) -> Result<(), String> {
        self.carry(edit)?;
        self.compare()
    }

    /// The same, without comparing: `Ok(true)` when both intakes took the
    /// edit, `Ok(false)` when both refused it, `Err` when they disagreed.
    fn carry(&mut self, edit: &Edit) -> Result<bool, String> {
        let interp = interp_op(&self.interp_meta, edit);
        let generated = typed_op(&self.gen_meta, edit);
        self.ops += 1;
        let mut taken = false;
        let (interp_event, gen_event) = if edit.writer == 'a' {
            (self.ia.send(interp), self.ga.send(generated))
        } else {
            (self.ib.send(interp), self.gb.send(generated))
        };
        match (interp_event, gen_event) {
            (Ok(interp_event), Ok(gen_event)) => {
                let pending = if edit.writer == 'a' {
                    &mut self.pending_a
                } else {
                    &mut self.pending_b
                };
                pending.push((interp_event, gen_event));
                taken = true;
            }
            (Err(_), Err(_)) => {
                // Both intakes refused it, which is itself an equality worth
                // having: `ModelLog::is_enabled` and the generated
                // `is_enabled` agree.
                self.refused += 1;
            }
            (interp_event, _) => {
                return Err(format!(
                    "the two intakes disagree on {}: interpreted {}, generated {}",
                    edit.show(),
                    if interp_event.is_ok() {
                        "accepted"
                    } else {
                        "refused"
                    },
                    if interp_event.is_ok() {
                        "refused"
                    } else {
                        "accepted"
                    },
                ));
            }
        }
        Ok(taken)
    }

    /// Everything both writers hold, delivered to the other, `a` first, with
    /// the read-outs compared after every crossing.
    fn deliver(&mut self) -> Result<(), String> {
        for (interp_event, gen_event) in std::mem::take(&mut self.pending_a) {
            self.ib.receive(interp_event);
            self.gb.receive(gen_event);
            self.compare()?;
        }
        for (interp_event, gen_event) in std::mem::take(&mut self.pending_b) {
            self.ia.receive(interp_event);
            self.ga.receive(gen_event);
            self.compare()?;
        }
        Ok(())
    }

    /// The same crossing without comparing, for the generator's shadow.
    fn cross(&mut self) {
        for (interp_event, gen_event) in std::mem::take(&mut self.pending_a) {
            self.ib.receive(interp_event);
            self.gb.receive(gen_event);
        }
        for (interp_event, gen_event) in std::mem::take(&mut self.pending_b) {
            self.ia.receive(interp_event);
            self.ga.receive(gen_event);
        }
    }

    /// One whole script. `Ok(())` when every read-out after every operation
    /// was equal; the error names the edit that broke it.
    fn run(&mut self, script: &EditScript) -> Result<(), String> {
        for (index, step) in script.steps.iter().enumerate() {
            match step {
                Step::Edit(edit) => self.apply(edit).map_err(|reason| {
                    format!("{}: edit {index} — {}\n{reason}", script.label, edit.show())
                })?,
                Step::Deliver => self.deliver().map_err(|reason| {
                    format!("{}: delivery after edit {index}\n{reason}", script.label)
                })?,
            }
        }
        // Nothing may be left in flight: a script that ends divergent is a
        // script that never tested the merge.
        self.deliver()
            .map_err(|reason| format!("{}: final delivery\n{reason}", script.label))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Seeded generation over `SimpleUML.ecore`'s real shape
// ---------------------------------------------------------------------------

/// splitmix64, written out so that a seed means the same script on every
/// machine and under every version of `rand`.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        assert!(bound > 0);
        (self.next() % bound as u64) as usize
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

const ALPHABET: [char; 6] = ['a', 'b', 'c', 'd', 'e', 'f'];
/// The values `ModelElement.stereotype` is filled from. Few and short, so
/// that the same value really is added twice and removed once across one
/// script and the count a bag keeps is exercised rather than only its
/// membership.
const STEREOTYPES: [&str; 3] = ["entity", "control", "boundary"];

/// One thing a writer could do to the model it is looking at.
#[derive(Clone, Debug)]
struct Candidate {
    path: Path,
    class: String,
    feature: String,
    kind: Kind,
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    /// Mint into an empty single-valued containment.
    CreateSingle,
    /// Mint into an ordered containment holding `len` children.
    CreateAt(usize),
    /// Take one of `len` children out.
    DeleteAt(usize),
    /// Put a character into a text of this length.
    InsertChar(usize),
    /// Take one out of a text of this length.
    DeleteChar(usize),
    /// Unset an optional that is currently there.
    UnsetOptional,
    /// Write an enable-wins flag.
    WriteFlag,
    /// Write the bag, which currently holds these values.
    WriteCollection(Vec<String>),
}

/// Everything the writer looking at `doc` could do, in a deterministic order.
fn candidates(meta: &Meta, doc: &Value, depth_cap: usize, room: bool) -> Vec<Candidate> {
    let mut out = Vec::new();
    if doc.is_null() {
        return out;
    }
    collect_candidates(meta, doc, &Path::default(), depth_cap, room, &mut out);
    out
}

fn collect_candidates(
    meta: &Meta,
    object: &Value,
    path: &Path,
    depth_cap: usize,
    room: bool,
    out: &mut Vec<Candidate>,
) {
    let Some(class_name) = object.get(ECLASS).and_then(Value::as_str) else {
        return;
    };
    let class = meta.slot(class_name);
    for (name, owner, slot) in &meta.class(class).visible {
        let Some(rule) = meta.sem.rule(*owner, *slot) else {
            continue;
        };
        let here = |kind| Candidate {
            path: path.clone(),
            class: class_name.to_string(),
            feature: name.to_string(),
            kind,
        };
        match rule {
            MergeRule::Reference { .. } | MergeRule::Unsupported { .. } => {}
            MergeRule::Attribute { shape, leaf } => match (shape, leaf) {
                (Shape::Single, LeafRule::Text) => {
                    let len = object
                        .get(&**name)
                        .and_then(Value::as_str)
                        .map_or(0, |text| text.chars().count());
                    out.push(here(Kind::InsertChar(len)));
                    if len > 0 {
                        out.push(here(Kind::DeleteChar(len)));
                    }
                }
                (Shape::Optional, LeafRule::Text) => {
                    match object.get(&**name).and_then(Value::as_str) {
                        None => out.push(here(Kind::InsertChar(0))),
                        Some(text) => {
                            let len = text.chars().count();
                            out.push(here(Kind::InsertChar(len)));
                            if len > 0 {
                                out.push(here(Kind::DeleteChar(len)));
                            }
                            out.push(here(Kind::UnsetOptional));
                        }
                    }
                }
                (Shape::Single | Shape::Optional, LeafRule::Flag { .. }) => {
                    out.push(here(Kind::WriteFlag))
                }
                (Shape::Set { .. }, LeafRule::Text) => {
                    let held: Vec<String> = object
                        .get(&**name)
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default();
                    out.push(here(Kind::WriteCollection(held)));
                }
                _ => {}
            },
            MergeRule::Containment { shape, target } => match shape {
                Shape::Single | Shape::Optional => {
                    match object.get(&**name) {
                        None => {
                            if room && path.depth() < depth_cap {
                                out.push(here(Kind::CreateSingle));
                            }
                        }
                        Some(child) => {
                            // A conflict is compared but never descended
                            // into: both paths refuse a *local* second
                            // variant (`union.rs:130-145`,
                            // `node.rs:1086-1108`), so aiming an edit at one
                            // would only test the refusal.
                            if child.get(ECLASS).is_some() {
                                collect_candidates(
                                    meta,
                                    child,
                                    &path.child(Hop {
                                        feature: name.to_string(),
                                        at: None,
                                        class: child[ECLASS].as_str().unwrap_or("").to_string(),
                                    }),
                                    depth_cap,
                                    room,
                                    out,
                                );
                            }
                        }
                    }
                    let _ = target;
                }
                Shape::Sequence => {
                    let items = object.get(&**name).and_then(Value::as_array);
                    let len = items.map_or(0, Vec::len);
                    if room && path.depth() < depth_cap {
                        out.push(here(Kind::CreateAt(len)));
                    }
                    if len > 0 {
                        out.push(here(Kind::DeleteAt(len)));
                    }
                    for (index, child) in items.into_iter().flatten().enumerate() {
                        if child.get(ECLASS).is_some() {
                            collect_candidates(
                                meta,
                                child,
                                &path.child(Hop {
                                    feature: name.to_string(),
                                    at: Some(index),
                                    class: child[ECLASS].as_str().unwrap_or("").to_string(),
                                }),
                                depth_cap,
                                room,
                                out,
                            );
                        }
                    }
                }
                _ => {}
            },
        }
    }
}

/// How many objects a canonical document holds, which is what caps a script's
/// growth.
fn object_count(doc: &Value) -> usize {
    match doc {
        Value::Object(map) => {
            let here = usize::from(map.contains_key(ECLASS));
            here + map.values().map(object_count).sum::<usize>()
        }
        Value::Array(items) => items.iter().map(object_count).sum(),
        _ => 0,
    }
}

/// One edit, drawn from what the writer can see.
fn propose(meta: &Meta, doc: &Value, rng: &mut Rng, writer: char, id: &mut u32) -> Option<Edit> {
    const OBJECT_CAP: usize = 22;
    const DEPTH_CAP: usize = 5;

    let room = object_count(doc) < OBJECT_CAP;
    let candidates = candidates(meta, doc, DEPTH_CAP, room);
    if candidates.is_empty() {
        return None;
    }
    let candidate = candidates[rng.below(candidates.len())].clone();
    let class = meta.slot(&candidate.class);
    let text_at = |path: &Path, feature: &str| -> String {
        let mut at = doc;
        for hop in &path.hops {
            at = match hop.at {
                Some(index) => &at[&hop.feature][index],
                None => &at[&hop.feature],
            };
        }
        at.get(feature)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let action = match candidate.kind {
        Kind::CreateSingle | Kind::CreateAt(_) => {
            let MergeRule::Containment { target, .. } = meta.rule(class, &candidate.feature) else {
                unreachable!("a create candidate is a containment")
            };
            let allowed = &meta.class(target).concrete;
            if allowed.is_empty() {
                return None;
            }
            let made = allowed[rng.below(allowed.len())];
            Action::Create {
                feature: candidate.feature.clone(),
                pos: match candidate.kind {
                    Kind::CreateAt(len) => Some(rng.below(len + 1)),
                    _ => None,
                },
                class: meta.name(made).to_string(),
            }
        }
        Kind::DeleteAt(len) => Action::Delete {
            feature: candidate.feature.clone(),
            pos: rng.below(len),
        },
        Kind::InsertChar(len) => {
            let pos = rng.below(len + 1);
            let ch = *rng.pick(&ALPHABET);
            let mut after: Vec<char> = text_at(&candidate.path, &candidate.feature)
                .chars()
                .collect();
            after.insert(pos.min(after.len()), ch);
            Action::Text {
                feature: candidate.feature.clone(),
                op: TextOp::Insert {
                    pos,
                    ch,
                    after: after.into_iter().collect(),
                },
            }
        }
        Kind::DeleteChar(len) => {
            let pos = rng.below(len);
            let mut after: Vec<char> = text_at(&candidate.path, &candidate.feature)
                .chars()
                .collect();
            if pos < after.len() {
                after.remove(pos);
            }
            Action::Text {
                feature: candidate.feature.clone(),
                op: TextOp::Delete {
                    pos,
                    after: after.into_iter().collect(),
                },
            }
        }
        Kind::UnsetOptional => Action::Unset {
            feature: candidate.feature.clone(),
        },
        Kind::WriteFlag => Action::Flag {
            feature: candidate.feature.clone(),
            op: match rng.below(8) {
                0 => FlagOp::Clear,
                1..=4 => FlagOp::Enable,
                _ => FlagOp::Disable,
            },
        },
        Kind::WriteCollection(held) => {
            // A removal is proposed only for a value the writer can see. That
            // was a hard requirement while `ModelElement.stereotype` was a bag
            // — `AWBag::Remove` is `Counter<usize>::Dec(1)` (`aw_bag.rs:84`)
            // and removing what is not there underflows a `usize` in a debug
            // build, identically on both paths — and it is kept now that it is
            // an add-wins set, because a remove of an absent value is the
            // concurrent case the matrix drives on purpose and not something a
            // seeded script should stumble into at random.
            let op = match rng.below(8) {
                0 => CollectionOp::Clear,
                1..=3 if !held.is_empty() => {
                    CollectionOp::Remove(held[rng.below(held.len())].clone())
                }
                _ => CollectionOp::Add((*rng.pick(&STEREOTYPES)).to_string()),
            };
            Action::Collection {
                feature: candidate.feature.clone(),
                op,
            }
        }
    };
    if matches!(action, Action::Create { .. }) {
        *id += 1;
    }
    Some(Edit {
        id: *id,
        writer,
        path: candidate.path,
        action,
    })
}

/// The edit every script opens with: the root `Model`, and the mandatory
/// children the generated path would materialise the moment its log exists.
/// `SimpleUML.ecore` has none of the latter, so this is one edit.
fn open_the_model(meta: &Meta) -> Vec<Edit> {
    let mut edits = vec![Edit {
        id: 1,
        writer: 'a',
        path: Path::default(),
        action: Action::New,
    }];
    let root = meta.root_concrete();
    edits.extend(meta.mandatory(root).into_iter().map(|(feature, target)| Edit {
        id: 1,
        writer: 'a',
        path: Path::default(),
        action: Action::Create {
            feature,
            pos: None,
            class: meta.name(target).to_string(),
        },
    }));
    edits
}

/// Ten sequential scripts and twenty concurrent ones, over `SimpleUML.ecore`.
///
/// The generator runs against a shadow pair driven by the same edits, so it
/// proposes against what a writer would actually be looking at. The shadow is
/// carried and never compared: a difference is the finding `ip14` exists to
/// report, and a generator that halted on the first one would only ever
/// report the first one.
fn seeded_script(meta: &Meta, seed: u64, concurrent: bool) -> EditScript {
    let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x1234_5678);
    let mut steps: Vec<Step> = open_the_model(meta).into_iter().map(Step::Edit).collect();
    steps.push(Step::Deliver);
    let mut id = 1;

    let mut shadow = Harness::new(&simpleuml_descriptor(), &simpleuml_descriptor());
    for step in &steps {
        match step {
            Step::Edit(edit) => {
                shadow.carry(edit).expect("the opening edit applies");
            }
            Step::Deliver => shadow.cross(),
        }
    }

    // An edit the shadow could not carry is *kept* and generation stops
    // there: dropping it is how an oracle quietly stops testing the thing it
    // was written for, and the run in `ip14` has to meet the same edit and
    // say so.
    let mut broken = false;
    if concurrent {
        for _ in 0..3 {
            if broken {
                break;
            }
            let edits = 2 + rng.below(3);
            for _ in 0..edits {
                for writer in ['a', 'b'] {
                    if broken {
                        break;
                    }
                    let doc = shadow.interp_view(writer);
                    if let Some(edit) = propose(meta, &doc, &mut rng, writer, &mut id) {
                        broken = shadow.carry(&edit).is_err();
                        steps.push(Step::Edit(edit));
                    }
                }
            }
            shadow.cross();
            steps.push(Step::Deliver);
        }
    } else {
        for index in 0..24 {
            if broken {
                break;
            }
            let writer = if index % 3 == 0 { 'b' } else { 'a' };
            let doc = shadow.interp_view(writer);
            if let Some(edit) = propose(meta, &doc, &mut rng, writer, &mut id) {
                broken = shadow.carry(&edit).is_err();
                steps.push(Step::Edit(edit));
                steps.push(Step::Deliver);
                shadow.cross();
            }
        }
    }
    EditScript {
        label: format!(
            "{} seed {seed}",
            if concurrent { "concurrent" } else { "sequential" }
        ),
        steps,
    }
}

// ---------------------------------------------------------------------------
// 7. The tests
// ---------------------------------------------------------------------------

fn meta() -> Meta {
    Meta::new(Arc::new(
        from_descriptor(&simpleuml_descriptor()).expect("the descriptor parses"),
    ))
}

/// The descriptor and the compiled crate agree on what the roots are.
///
/// `ip13` reconciles two checked-in copies of `bt.ecore`'s descriptor. There
/// is one copy here, so what stands in its place is this: the four root
/// classes the descriptor names are exactly the four fields the generated
/// `SimpleumlValue` carries, and every one of them is `Unset` before anything
/// is written. If the generator's root rule and the table's ever part, the
/// projection would be reading fields that mean something else and this is
/// where it would show.
#[test]
fn the_descriptor_and_the_generated_crate_name_the_same_roots() {
    let meta = meta();
    let named: BTreeSet<String> = meta
        .roots()
        .iter()
        .map(|root| field_of(meta.name(*root)))
        .collect();
    let raw = serde_json::to_value(SimpleumlValue::default()).expect("the read-out serializes");
    let compiled: BTreeSet<String> = raw
        .as_object()
        .expect("the package value is an object")
        .keys()
        .cloned()
        .collect();
    assert_eq!(
        named, compiled,
        "the descriptor's `rootClasses` and `SimpleumlValue`'s fields differ"
    );
    assert_eq!(named.len(), 4, "SimpleUML declares four roots: {named:?}");
    for field in &named {
        assert_eq!(
            raw[field],
            json!("Unset"),
            "root arm `{field}` is a `union!` and starts `Unset`"
        );
    }
    // And every one of them is abstract, which is why none of them can open
    // the generated arm the way `bt.ecore`'s `Root` opens `ip13`'s.
    for root in meta.roots() {
        assert!(
            !meta.class(*root).instantiable,
            "`{}` is a concrete root; the opener argument in the module header \
             assumes none is",
            meta.name(*root)
        );
    }
}

/// Every shape `SimpleUML.ecore` reaches, named from the table: six
/// optionals, one add-wins set and five sequence containments, and nothing
/// else.
///
/// The file writes a `lowerBound` on four references and nowhere else, so
/// every attribute it declares and every single-valued containment is
/// optional, which is Ecore's `0` and not a house rule. It used to reach one
/// bag, `ModelElement.stereotype`, which declares `ordered="false"` and leaves
/// `unique` silent; a silent `unique` is Ecore's `true`, so that feature is an
/// add-wins set and no bag is left here.
#[test]
fn simpleuml_reaches_six_optionals_one_set_and_five_sequences() {
    let meta = meta();
    let mut shapes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for class in &meta.sem.classes {
        for (name, owner, slot) in &class.visible {
            let Some(rule) = meta.sem.rule(*owner, *slot) else {
                continue;
            };
            // `visible` repeats an inherited feature on every subclass; the
            // declaring class is what makes each one count once.
            let shape = match rule {
                MergeRule::Attribute { shape, .. } | MergeRule::Containment { shape, .. } => {
                    shape
                }
                _ => continue,
            };
            let at = format!("{}.{name}", meta.name(*owner));
            let held = shapes.entry(format!("{shape:?}")).or_default();
            if !held.contains(&at) {
                held.push(at);
            }
        }
    }
    for held in shapes.values_mut() {
        held.sort();
    }
    let counts: Vec<(&str, usize)> = shapes
        .iter()
        .map(|(shape, held)| (shape.as_str(), held.len()))
        .collect();
    assert_eq!(
        counts,
        vec![("Optional", 6), ("Sequence", 5), ("Set { tie: AddWins }", 1)],
        "the shapes `SimpleUML.ecore` reaches: {shapes:?}"
    );
    assert_eq!(
        shapes["Set { tie: AddWins }"],
        vec!["ModelElement.stereotype"],
        "the one set, which used to be the one bag"
    );
}

/// No class has a single-valued containment onto a record, so a mint is one
/// operation and there is no mandatory-child chain to carry.
#[test]
fn simpleuml_has_no_mandatory_child() {
    let meta = meta();
    for class in &meta.sem.classes {
        assert!(
            meta.mandatory(class.slot).is_empty(),
            "`{}` has a mandatory child: {:?}",
            class.name,
            meta.mandatory(class.slot)
        );
    }
}

/// The typed encoder's two hard parts, written out: the `union!` descent from
/// an abstract root through two more unions to a record, and the four
/// `<Super>Super` hops multiple inheritance puts between `Class` and the
/// `name` that `ModelElement` declares.
///
/// Both are derived from the table by [`typed_json`] and neither is written
/// anywhere in this file; this test is the assertion that what the table
/// derives is what the generated Rust actually has, and every byte of it
/// would be a `serde_json::from_value` error if it were not.
#[test]
fn the_typed_encoder_walks_the_super_hops_and_the_union_variants() {
    let meta = meta();
    let a_class = Path::default().child(Hop {
        feature: "ownedElements".to_string(),
        at: Some(0),
        class: "Class".to_string(),
    });

    // `Class.abstract` is declared on `Class`, so no super hop; but `abstract`
    // is a Rust keyword, so the generated field is `abstract_field` and its
    // `record!` variant `AbstractField`. It is optional, like every attribute
    // of this file, so the write is a `Set` into the optional that holds it.
    let flag = Edit {
        id: 2,
        writer: 'a',
        path: a_class.clone(),
        action: Action::Flag {
            feature: "abstract".to_string(),
            op: FlagOp::Enable,
        },
    };
    assert_eq!(
        typed_json(&meta, &flag),
        json!({"ModelElementKind": {"Classifier": {"Package": {"Model": {
            "PackageSuper": {"OwnedElements": {"Update": {"pos": 0, "op":
                {"TType": {"DataType": {"Class": {"AbstractField": {"Set": "Enable"}}}}}
            }}}
        }}}}}),
        "the root descends `ModelElementKind -> Classifier -> Package -> Model` \
         and the child `Packageable -> TType -> DataType -> Class`"
    );

    // `Class` reaches `name` through `DataType`, `TType`, `Classifier` and
    // `ModelElement`, which is the deepest inheritance chain in the file.
    let text = Edit {
        id: 2,
        writer: 'a',
        path: a_class,
        action: Action::Text {
            feature: "name".to_string(),
            op: TextOp::Insert {
                pos: 0,
                ch: 'a',
                after: "a".to_string(),
            },
        },
    };
    assert_eq!(
        typed_json(&meta, &text),
        json!({"ModelElementKind": {"Classifier": {"Package": {"Model": {
            "PackageSuper": {"OwnedElements": {"Update": {"pos": 0, "op":
                {"TType": {"DataType": {"Class": {"DataTypeSuper": {"TTypeSuper":
                    {"ClassifierSuper": {"ModelElementSuper": {"Name":
                        {"Set": {"Insert": {"content": "a", "pos": 0}}}}}}}}}}}
            }}}
        }}}}}),
        "`Class` sees `name` four `<Super>Super` hops away"
    );

    // And both deserialize into an operation this crate can take.
    typed_op(&meta, &flag);
    typed_op(&meta, &text);
}

/// `ip14`: thirty seeded edit scripts over `SimpleUML.ecore`, ten sequential
/// and twenty with two writers diverging and merging, every one equal under
/// the canonical projection after every operation and after every delivery.
#[test]
fn ip14_thirty_seeded_scripts_over_simpleuml_ecore_agree() {
    let meta = meta();
    let mut failures = Vec::new();
    let mut edits = 0;
    let mut ops = 0;
    let mut refused = 0;
    let mut comparisons = 0;
    for seed in 0..30u64 {
        let concurrent = seed >= 10;
        let script = seeded_script(&meta, seed, concurrent);
        edits += script
            .steps
            .iter()
            .filter(|step| matches!(step, Step::Edit(_)))
            .count();
        let mut harness = Harness::new(&simpleuml_descriptor(), &simpleuml_descriptor());
        if let Err(reason) = harness.run(&script) {
            failures.push(reason);
        }
        ops += harness.ops;
        refused += harness.refused;
        comparisons += harness.comparisons;
    }
    assert!(
        edits > 30 * 10,
        "thirty scripts came to only {edits} edits; the generator stopped early"
    );
    assert!(
        failures.is_empty(),
        "{} of thirty scripts differ.\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
    println!(
        "ip14: 30 scripts, {edits} edits, {ops} operations, {comparisons} comparisons, \
         {refused} refused by both intakes"
    );
}

/// The story the validation plan tells under I-A1, over this metamodel: two
/// people concurrently rename the same `Class` and each add a `Property` to
/// its `attributes`.
#[test]
fn ip14_the_validation_plans_own_story() {
    let meta = meta();
    let script = rename_and_add_script(&meta);
    let mut harness = Harness::new(&simpleuml_descriptor(), &simpleuml_descriptor());
    harness.run(&script).expect("the two paths agree");

    // And what they agree on is the merge, not an empty document.
    let doc = harness.interp_doc('a');
    let class = &doc["ownedElements"][0];
    assert_eq!(class[ECLASS], json!("Class"));
    assert_eq!(
        class["name"].as_str().expect("a name").chars().count(),
        2,
        "both renames survived, character by character: {}",
        class["name"]
    );
    assert_eq!(
        class["attributes"]
            .as_array()
            .expect("attributes read as an array")
            .len(),
        2,
        "both properties survived"
    );
}

fn rename_and_add_script(meta: &Meta) -> EditScript {
    let mut steps: Vec<Step> = open_the_model(meta).into_iter().map(Step::Edit).collect();
    steps.push(Step::Deliver);
    let class = Path::default().child(Hop {
        feature: "ownedElements".to_string(),
        at: Some(0),
        class: "Class".to_string(),
    });
    steps.push(Step::Edit(Edit {
        id: 2,
        writer: 'a',
        path: Path::default(),
        action: Action::Create {
            feature: "ownedElements".to_string(),
            pos: Some(0),
            class: "Class".to_string(),
        },
    }));
    steps.push(Step::Deliver);
    for (writer, ch) in [('a', 'x'), ('b', 'y')] {
        steps.push(Step::Edit(Edit {
            id: 2,
            writer,
            path: class.clone(),
            action: Action::Text {
                feature: "name".to_string(),
                op: TextOp::Insert {
                    pos: 0,
                    ch,
                    after: ch.to_string(),
                },
            },
        }));
        steps.push(Step::Edit(Edit {
            id: 3,
            writer,
            path: class.clone(),
            action: Action::Create {
                feature: "attributes".to_string(),
                pos: Some(0),
                class: "Property".to_string(),
            },
        }));
        // A property created and never written into is the criterion's one
        // named exception, so each writer also names its own.
        steps.push(Step::Edit(Edit {
            id: 3,
            writer,
            path: class.child(Hop {
                feature: "attributes".to_string(),
                at: Some(0),
                class: "Property".to_string(),
            }),
            action: Action::Text {
                feature: "name".to_string(),
                op: TextOp::Insert {
                    pos: 0,
                    ch: writer,
                    after: writer.to_string(),
                },
            },
        }));
    }
    steps.push(Step::Deliver);
    EditScript {
        label: "the validation plan's own story".to_string(),
        steps,
    }
}

/// The module header's claim about the sequence-number offset, asserted
/// rather than assumed.
///
/// `ip13` opens the generated arm with a `Root::New` so that edit *n* is
/// event *n+1* on both paths; no such opener exists here, so replica `a`'s
/// generated events run one sequence number behind its interpreted ones for
/// the whole run. `EventId::cmp` reaches the sequence number only when two
/// events share an origin, and within an origin the offset is constant, so a
/// tie between `a` and `b` must break the same way on both paths. This drives
/// exactly that tie: both writers insert a child at position 0 of the same
/// empty `ownedElements`, concurrently, and the two paths must put them in
/// the same order.
#[test]
fn a_concurrent_tie_breaks_the_same_way_on_both_paths() {
    let meta = meta();
    let mut steps: Vec<Step> = open_the_model(&meta).into_iter().map(Step::Edit).collect();
    steps.push(Step::Deliver);
    for writer in ['a', 'b'] {
        steps.push(Step::Edit(Edit {
            id: 2,
            writer,
            path: Path::default(),
            action: Action::Create {
                feature: "ownedElements".to_string(),
                pos: Some(0),
                class: "Enumeration".to_string(),
            },
        }));
        // Name it, so neither is the unwritten sequence child the criterion
        // excludes and both are actually there to be ordered.
        steps.push(Step::Edit(Edit {
            id: 2,
            writer,
            path: Path::default().child(Hop {
                feature: "ownedElements".to_string(),
                at: Some(0),
                class: "Enumeration".to_string(),
            }),
            action: Action::Text {
                feature: "name".to_string(),
                op: TextOp::Insert {
                    pos: 0,
                    ch: writer,
                    after: writer.to_string(),
                },
            },
        }));
    }
    steps.push(Step::Deliver);
    let script = EditScript {
        label: "a concurrent tie at one position".to_string(),
        steps,
    };
    let mut harness = Harness::new(&simpleuml_descriptor(), &simpleuml_descriptor());
    harness
        .run(&script)
        .expect("the two paths break the tie the same way");

    let doc = harness.interp_doc('a');
    let names: Vec<String> = doc["ownedElements"]
        .as_array()
        .expect("two children")
        .iter()
        .map(|child| child["name"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(names.len(), 2, "both inserts survived: {doc}");
    assert_eq!(
        harness.interp_doc('a'),
        harness.interp_doc('b'),
        "the two interpreted replicas converged"
    );
    assert_eq!(
        harness.gen_doc('a'),
        harness.gen_doc('b'),
        "the two generated replicas converged"
    );
    println!("the tie resolved to {names:?} on both paths");
}

/// One root arm holds the model and the other three stay `Unset`, on the raw
/// generated read-out, over every script.
///
/// The projection reads all four and asserts the same thing on every
/// comparison; this checks it where it cannot be projected away, on the value
/// the generated log actually renders.
#[test]
fn only_one_root_arm_is_ever_written() {
    let meta = meta();
    for seed in 0..30u64 {
        let script = seeded_script(&meta, seed, seed >= 10);
        let mut harness = Harness::new(&simpleuml_descriptor(), &simpleuml_descriptor());
        harness.run(&script).expect("the script runs");
        for writer in ['a', 'b'] {
            let raw = serde_json::to_value(harness.gen_raw(writer)).expect("serializes");
            let set: Vec<String> = raw
                .as_object()
                .expect("an object")
                .iter()
                .filter(|(_, value)| *value != &json!("Unset"))
                .map(|(key, _)| key.clone())
                .collect();
            assert_eq!(
                set,
                vec![field_of(ROOT_ARM)],
                "seed {seed}, replica {writer}: the written root arms are {set:?}"
            );
        }
    }
}

/// The only thing the projection drops from the interpreted side is a
/// non-containment reference, and the scripts never write one.
///
/// Six of SimpleUML's references are dropped this way, and the test names
/// them from the table rather than from a list written here.
#[test]
fn the_only_exclusion_is_the_reference() {
    let meta = meta();
    let mut dropped: Vec<String> = Vec::new();
    for class in &meta.sem.classes {
        for (name, owner, slot) in &class.visible {
            let Some(rule) = meta.sem.rule(*owner, *slot) else {
                continue;
            };
            if matches!(rule, MergeRule::Reference { .. }) && *owner == class.slot {
                dropped.push(format!("{}.{name}", class.name));
            }
        }
    }
    dropped.sort();
    assert_eq!(
        dropped,
        vec![
            "Association.source",
            "Association.target",
            "Generalization.general",
            "Packageable.owner",
            "Property.owner",
            "Property.ttype",
        ],
        "the set of excluded references changed"
    );

    // And nothing else is excluded: `strip_references` is the only function
    // that removes a key from the interpreted side, and it removes exactly a
    // `MergeRule::Reference`.
    let doc = json!({
        ECLASS: "Property",
        "name": "p",
        "ttype": "an identifier the generated path does not carry",
        "owner": "another one",
        "stereotype": [],
        "taggedValue": [],
    });
    let stripped = strip_references(&meta, doc);
    assert_eq!(
        stripped,
        json!({
            ECLASS: "Property",
            "name": "p",
            "stereotype": [],
            "taggedValue": [],
        })
    );
}

/// Over all thirty scripts, the criterion's one named exception is the only
/// difference there is: re-pruning both documents by it leaves nothing.
///
/// This is what keeps [`except_unwritten_sequence_children`] from becoming a
/// place where a second inequality could hide. A difference of any other kind
/// would surface here as a residual.
#[test]
fn the_thirty_scripts_find_exactly_one_kind_of_difference() {
    let meta = meta();
    let mut residual = Vec::new();
    let mut excepted = 0;
    for seed in 0..30u64 {
        let script = seeded_script(&meta, seed, seed >= 10);
        let mut harness = Harness::new(&simpleuml_descriptor(), &simpleuml_descriptor());
        harness.run(&script).expect("the script runs");
        for writer in ['a', 'b'] {
            let interp = harness.interp_doc(writer);
            let generated = harness.gen_doc(writer);
            if interp != generated {
                excepted += 1;
                let interp = except_unwritten_sequence_children(&meta, interp);
                let generated = except_unwritten_sequence_children(&meta, generated);
                if interp != generated {
                    residual.push(format!(
                        "seed {seed}, replica {writer}: {}",
                        difference(&interp, &generated, "").unwrap_or_default()
                    ));
                }
            }
        }
    }
    assert!(
        residual.is_empty(),
        "a difference the named exception does not account for:\n{}",
        residual.join("\n")
    );
    println!(
        "ip14: {excepted} of 60 end states differ before the named exception \
         is taken off, and none after"
    );
}

/// Which `LeafLog` arms `SimpleUML.ecore` actually reaches, minted from the
/// table by `LeafLog::for_rule`, `for_set` and `for_bag` and named by the
/// library's own `LeafLog::kind` rather than by a mapping written here.
///
/// The point of the test is the arithmetic in the paper's equivalence
/// sentence: `ip14` widens the structural evidence and widens the leaf
/// evidence not at all. Every arm below is already proven by `ip13` (text),
/// `ip28` (the enable-wins flag) or `ip29` (the bag).
#[test]
fn the_leaf_arms_simpleuml_reaches_are_all_already_proven() {
    let meta = meta();
    let mut arms: BTreeSet<&'static str> = BTreeSet::new();
    for class in &meta.sem.classes {
        for (_, owner, slot) in &class.visible {
            let Some(MergeRule::Attribute { shape, leaf }) = meta.sem.rule(*owner, *slot) else {
                continue;
            };
            let arm = match shape {
                Shape::Bag => moirai_interp::LeafLog::for_bag(),
                Shape::Set { tie } => moirai_interp::LeafLog::for_set(*tie),
                Shape::OrderedSet => moirai_interp::LeafLog::for_ordered_set(),
                _ => moirai_interp::LeafLog::for_rule(*leaf),
            };
            arms.insert(arm.kind());
        }
    }
    let found: Vec<&str> = arms.iter().copied().collect();
    assert_eq!(
        found,
        vec!["add-wins set", "enable-wins flag", "text"],
        "the arms `SimpleUML.ecore` reaches changed"
    );
    println!("ip14 reaches the `LeafLog` arms {found:?}, all three already proven");
}

/// The control: the same script run twice, once with both paths holding
/// `SimpleUML.ecore`'s own table, where it must be equal, and once with
/// `ModelElement.name` bound to a multi-value register on the interpreted
/// side alone, where the harness **must** report inequality.
///
/// This is `ip15`'s construction repeated over this driver rather than a test
/// of its own: `ip15` discharges I-A2 and does it over `bt.ecore`. What it is
/// doing here is keeping `ip14` from resting on an oracle that has never been
/// seen to fail. Every other test in this file passed on its first run, and
/// that is only evidence if this one fails on its first run too.
#[test]
fn the_oracle_reports_a_difference_when_there_is_one() {
    let meta = meta();
    let script = concurrent_name_script(&meta);

    let mut faithful = Harness::new(&simpleuml_descriptor(), &simpleuml_descriptor());
    faithful
        .run(&script)
        .expect("with the same table on both sides the two paths agree");

    let mut mutated = Harness::new(&simpleuml_descriptor(), &register_name_descriptor());
    let verdict = mutated.run(&script);
    let reason = verdict.expect_err(
        "`ModelElement.name` bound to a multi-value register on the interpreted side \
         alone is a genuine difference; an oracle that reports equality here is \
         worthless",
    );
    assert!(
        reason.contains("name"),
        "the failure should name the feature that was rebound: {reason}"
    );
    println!("the control: the oracle reported the mutation —\n{reason}");
}

/// Two writers appending to the same `Model.name` while divergent: a text
/// leaf merges them character by character, a multi-value register keeps both
/// and reads out a conflict.
fn concurrent_name_script(meta: &Meta) -> EditScript {
    let mut steps: Vec<Step> = open_the_model(meta).into_iter().map(Step::Edit).collect();
    steps.push(Step::Deliver);
    for (writer, ch) in [('a', 'x'), ('b', 'y')] {
        steps.push(Step::Edit(Edit {
            id: 1,
            writer,
            path: Path::default(),
            action: Action::Text {
                feature: "name".to_string(),
                op: TextOp::Insert {
                    pos: 0,
                    ch,
                    after: ch.to_string(),
                },
            },
        }));
    }
    steps.push(Step::Deliver);
    EditScript {
        label: "two writers on one name".to_string(),
        steps,
    }
}

/// `metamodel.json` with `ModelElement.name` bound to a multi-value register
/// instead of a text: the mutation the control above demands the oracle
/// catch.
fn register_name_descriptor() -> Value {
    let mut descriptor = simpleuml_descriptor();
    let attributes = descriptor["classes"]["ModelElement"]["attributes"]
        .as_array_mut()
        .expect("`ModelElement` has attributes");
    let name = attributes
        .iter_mut()
        .find(|attribute| attribute["name"] == json!("name"))
        .expect("`ModelElement` declares `name`");
    name["merge"] = json!({
        "kind": "attribute",
        "shape": {"kind": "single"},
        "leaf": {"kind": "register", "tie": "mv"}
    });
    descriptor
}

/// **The one difference `ip14` reports that is not a merge**, pinned here as
/// the smallest script that shows it: the generated read-out of a nested list
/// depends on whether anyone read it while the events were arriving.
///
/// `a` creates a `DataType` at `Model.ownedElements[0]` and everyone sees it.
/// Then `a` writes one character into its `name`, `b` concurrently writes one
/// into the same `name`, and `a` deletes the element. `b` takes `a`'s three
/// events. Update-wins says `b`'s write survives the delete it is concurrent
/// with and the element stays, holding `b`'s character, which is what the
/// interpreted path reads either way and what the generated path reads when
/// nothing is read in between. Read the generated log once after each of the
/// three deliveries and it reads the empty document instead — the element is
/// gone.
///
/// It is not the facet defaults and not the interpreted path. It is
/// `moirai-protocol`'s `CachedLog`, which `NestedListLog` holds its positions
/// list in (`nested_list.rs:58`): its `effect` replays one operation onto the
/// materialised value rather than recomputing whenever the incoming event's
/// version compares `Greater` to the *previous event's*, and `Version`'s
/// `partial_cmp` answers `Greater` for two events of one origin from the
/// origin's own sequence without looking at what else each has seen. A
/// concurrent event therefore takes the replay path, and the replay is only
/// valid for an event that is causally after everything the log holds. The
/// cache is populated by a read, which is why the state depends on whether
/// anyone looked.
///
/// Ignored, not deleted: it is a defect of the merge layer this oracle sits
/// on, it has a fix that is not this task's to make, and `ip14`,
/// `only_one_root_arm_is_ever_written` and
/// `the_thirty_scripts_find_exactly_one_kind_of_difference` fail on it.
#[test]
#[ignore = "a reproducer for a `CachedLog` defect in moirai-protocol, not a gate"]
fn a_read_between_deliveries_changes_what_the_generated_nested_list_holds() {
    let root = Path::default();
    let child = root.clone().child(Hop {
        feature: "ownedElements".to_string(),
        at: Some(0),
        class: "DataType".to_string(),
    });
    let mk = |id: u32, writer: char, path: Path, action: Action| Edit {
        id,
        writer,
        path,
        action,
    };
    let insert = |ch: char| Action::Text {
        feature: "name".to_string(),
        op: TextOp::Insert {
            pos: 0,
            ch,
            after: ch.to_string(),
        },
    };

    let run = |read_between: bool| -> Value {
        let mut h = Harness::new(&simpleuml_descriptor(), &simpleuml_descriptor());
        h.carry(&mk(1, 'a', root.clone(), Action::New))
            .expect("the root mints");
        h.cross();
        h.carry(&mk(
            2,
            'a',
            root.clone(),
            Action::Create {
                feature: "ownedElements".to_string(),
                pos: Some(0),
                class: "DataType".to_string(),
            },
        ))
        .expect("the child is created");
        h.cross();
        h.carry(&mk(3, 'a', child.clone(), insert('e')))
            .expect("`a` writes");
        h.carry(&mk(3, 'b', child.clone(), insert('b')))
            .expect("`b` writes");
        h.carry(&mk(
            3,
            'a',
            root.clone(),
            Action::Delete {
                feature: "ownedElements".to_string(),
                pos: 0,
            },
        ))
        .expect("`a` deletes");
        for (interp_event, gen_event) in std::mem::take(&mut h.pending_a) {
            h.ib.receive(interp_event);
            h.gb.receive(gen_event);
            if read_between {
                let _ = h.gen_doc('b');
            }
        }
        h.gen_doc('b')
    };

    let unread = run(false);
    let read = run(true);
    assert_ne!(
        unread, read,
        "the defect this pins is gone; make both `ip14` and this one a gate again"
    );
    assert_eq!(
        unread["ownedElements"],
        json!([{ECLASS: "DataType", "name": "b"}]),
        "with nothing read in between, update-wins keeps the element"
    );
    assert_eq!(
        read.get("ownedElements"),
        None,
        "with a read after each delivery, the element is gone"
    );
}
