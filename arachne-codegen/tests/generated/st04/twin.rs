//! ST.04's twin, B5 of `cea-cdrt-knowledge/spec/items/ST.04.md`: the runtime
//! Arachne generates for `st04.ecore` against the interpreted one, on every
//! schedule of the claim and of its guards, for every leaf kind.
//!
//! `generated_crates.rs` generates the crate `st04` from `st04.ecore` with the
//! generator at this checkout, against the model-plane Moirai checkout, and
//! copies this file into its `tests/`. So the generated side is `OptionLog`,
//! `record!` and the leaf logs as the library holds them now, composed as
//! Arachne composes an optional attribute (`OptionLog` over the leaf's log:
//! `OptionLog<VecLog<Counter<i32>>>`, `OptionLog<GraphLog<List<char>>>`,
//! `OptionLog<VecLog<LwwRegister<String>>>`, and so on) inside a record held
//! by an optional containment (`OptionLog<BoxLog>`, `OptionLog<MidLog>`),
//! and the interpreted side is `moirai_interp::ModelLog` on the descriptor
//! Arachne wrote beside the crate (`metamodel.json`, which
//! `generated_crates.rs` holds equal to
//! `moirai-interp/tests/fixtures/st04.metamodel.json`).
//!
//! Each seat is a pair of replicas, one per runtime, fed the same operation
//! in the same order. From the moment T (the root's first write) has been
//! delivered everywhere, after every send and every delivery the seat that
//! moved is read on both runtimes, the generated `Read` projected onto the
//! canonical form by [`project`], written here by hand for this metamodel
//! from the typed value, and the two must be equal (the spec's P7 counts the
//! differences). At the end of every schedule the generated replicas must
//! also serve the value the rule fixes, which the spec derives by hand (P8):
//! the removal does to an optional attribute what its own unset does, so a
//! leaf it emptied is an absent key, a leaf W reached holds W, and a text
//! holds what its reset recorded, the empty string.
//!
//! The canonical form of the interpreted side is its default read, what
//! `GET /api/model/{id}/state` serves: `Replica::query(&Read::<Value>::new())`.
use std::sync::Arc;

use moirai_crdt::counter::resettable_counter::Counter;
use moirai_crdt::flag::dw_flag::DWFlag;
use moirai_crdt::flag::ew_flag::EWFlag;
use moirai_crdt::list::eg_walker::List;
use moirai_crdt::option::Optional;
use moirai_crdt::register::mv_register::MVRegister;
use moirai_crdt::register::po_register::PORegister;
use moirai_crdt::register::to_register::TORegister;
use moirai_crdt::register::unique_register::Register;
use moirai_crdt::utils::membership::triplet_log;
use moirai_interp::testing::{class_slot, feature_slot, install};
use moirai_interp::{InstanceOp, LeafOp, ModelLog, ModelOp, Scalar};
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::{StateSnapshot, Tcsb};
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::{IsReplica, Replica};
use moirai_semantics::{MetamodelSemantics, from_descriptor};
use serde_json::{Map, Value, json};
use st04::classifiers::{Box as Bx, BoxValue, Color, Holder, Mid, MidValue};
use st04::package::{St04, St04Log, St04Value};

type Interp = Replica<ModelLog, Tcsb<ModelOp>>;
type Gen = Replica<St04Log, Tcsb<St04>>;

/// The descriptor Arachne wrote beside this crate.
const DESCRIPTOR: &str = include_str!("../metamodel.json");

fn descriptor() -> Value {
    serde_json::from_str(DESCRIPTOR).expect("metamodel.json is JSON")
}

// ---------------------------------------------------------------------------
// The leaf kinds, their writes and their values
// ---------------------------------------------------------------------------

/// Every leaf the derivation can hold under an optional attribute, one
/// optional attribute of `Box` and of `Mid` each (the spec's section 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Text,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    Ew,
    Dw,
    Ch,
    Mv,
    Lww,
    Fair,
    Po,
    To,
    Emv,
    Elww,
    Efair,
    Epo,
    Eto,
}

const KINDS: [Kind; 20] = [
    Kind::Text,
    Kind::Byte,
    Kind::Short,
    Kind::Int,
    Kind::Long,
    Kind::Float,
    Kind::Double,
    Kind::Ew,
    Kind::Dw,
    Kind::Ch,
    Kind::Mv,
    Kind::Lww,
    Kind::Fair,
    Kind::Po,
    Kind::To,
    Kind::Emv,
    Kind::Elww,
    Kind::Efair,
    Kind::Epo,
    Kind::Eto,
];

/// Which write to a leaf: X (the formation's), W (the race's, on the leaf),
/// `Again` (W's target written again after the race), `Rebuild` (the leaf
/// written again where W did not reach it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Which {
    X,
    W,
    Again,
    Rebuild,
}

impl Which {
    fn int(self) -> i64 {
        match self {
            Which::X | Which::Again => 1,
            Which::W => 10,
            Which::Rebuild => 5,
        }
    }

    fn ch(self) -> char {
        match self {
            Which::X => 'a',
            Which::W => 'w',
            Which::Again => 'v',
            Which::Rebuild => 'r',
        }
    }

    fn text(self) -> String {
        match self {
            Which::X => "x",
            Which::W => "w",
            Which::Again => "v",
            Which::Rebuild => "r",
        }
        .to_string()
    }

    /// The `Color` literal, by declaration position (red 0, green 1, blue 2).
    fn literal(self) -> u16 {
        match self {
            Which::X => 1,
            Which::W => 2,
            Which::Again | Which::Rebuild => 0,
        }
    }

    fn color(self) -> Color {
        match self.literal() {
            0 => Color::Red,
            1 => Color::Green,
            _ => Color::Blue,
        }
    }
}

impl Kind {
    fn feature(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Byte => "byte",
            Kind::Short => "short",
            Kind::Int => "int",
            Kind::Long => "long",
            Kind::Float => "float",
            Kind::Double => "double",
            Kind::Ew => "ew",
            Kind::Dw => "dw",
            Kind::Ch => "ch",
            Kind::Mv => "mv",
            Kind::Lww => "lww",
            Kind::Fair => "fair",
            Kind::Po => "po",
            Kind::To => "to",
            Kind::Emv => "emv",
            Kind::Elww => "elww",
            Kind::Efair => "efair",
            Kind::Epo => "epo",
            Kind::Eto => "eto",
        }
    }

    /// A flag: W disables the disable-wins flag (the leaf then holds W and
    /// reads false, and must stay); every other write enables. An enable-wins
    /// flag keeps no disable, so W enables it.
    fn enables(self, which: Which) -> bool {
        !(self == Kind::Dw && which == Which::W)
    }

    /// Every kind but text empties under a removal that had seen its writes.
    fn empties(self) -> bool {
        self != Kind::Text
    }

    /// What the leaf reads after a removal that had seen X and nothing
    /// concurrent reached it.
    fn kept(self) -> Option<Value> {
        (!self.empties()).then(|| json!(""))
    }

    /// The interpreted leaf operation of `which`; `color` is the enum's slot.
    fn interp_op(self, which: Which, color: u16) -> LeafOp {
        match self {
            Kind::Text => LeafOp::InsertChar {
                pos: 0,
                ch: which.ch(),
            },
            Kind::Byte | Kind::Short | Kind::Int | Kind::Long => {
                LeafOp::Inc(Scalar::Int(which.int()))
            }
            Kind::Float | Kind::Double => LeafOp::Inc(Scalar::float(which.int() as f64)),
            Kind::Ew | Kind::Dw => {
                if self.enables(which) {
                    LeafOp::Enable
                } else {
                    LeafOp::Disable
                }
            }
            Kind::Ch => LeafOp::Write(Scalar::Char(which.ch())),
            Kind::Mv | Kind::Lww | Kind::Fair | Kind::Po | Kind::To => {
                LeafOp::Write(Scalar::text(which.text()))
            }
            Kind::Emv | Kind::Elww | Kind::Efair | Kind::Epo | Kind::Eto => {
                LeafOp::Write(Scalar::Enum(color, which.literal()))
            }
        }
    }

    /// The canonical value right after `which` in its schedule (the spec's
    /// section 4): X on a fresh leaf; W on the leaf X wrote, concurrently with
    /// a removal that had seen X; `Again` after W; `Rebuild` on a leaf the
    /// removal emptied (or a text it left empty).
    fn value(self, which: Which) -> Value {
        match self {
            Kind::Text => json!(match which {
                Which::X => "a",
                Which::W => "w",
                Which::Again => "vw",
                Which::Rebuild => "r",
            }),
            Kind::Byte | Kind::Short | Kind::Int | Kind::Long => json!(match which {
                Which::X => 1,
                Which::W => 10,
                Which::Again => 11,
                Which::Rebuild => 5,
            }),
            Kind::Float | Kind::Double => json!(match which {
                Which::X => 1.0,
                Which::W => 10.0,
                Which::Again => 11.0,
                Which::Rebuild => 5.0,
            }),
            Kind::Ew | Kind::Dw => json!(self.enables(which)),
            Kind::Ch => json!(which.ch().to_string()),
            Kind::Mv | Kind::Lww | Kind::Fair | Kind::Po | Kind::To => json!(which.text()),
            Kind::Emv | Kind::Elww | Kind::Efair | Kind::Epo | Kind::Eto => {
                json!(["red", "green", "blue"][which.literal() as usize])
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The projection of the generated read onto the canonical form, by hand
// ---------------------------------------------------------------------------

/// A register read as a set, in the canonical form `leaf.rs`'s
/// `many_valued` gives it: nothing is `null`, one value is that value, more
/// than one is a conflict, sorted.
fn many(mut values: Vec<(u64, Value)>) -> Value {
    values.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.to_string().cmp(&b.1.to_string()))
    });
    match values.len() {
        0 => Value::Null,
        1 => values.remove(0).1,
        _ => json!({"__conflict": values.into_iter().map(|(_, v)| v).collect::<Vec<_>>()}),
    }
}

fn color_name(color: &Color) -> Value {
    json!(match color {
        Color::Red => "red",
        Color::Green => "green",
        Color::Blue => "blue",
    })
}

fn color_rank(color: &Color) -> u64 {
    match color {
        Color::Red => 0,
        Color::Green => 1,
        Color::Blue => 2,
    }
}

fn float(value: f64) -> Value {
    serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
}

/// Every optional attribute of a `BoxValue` or a `MidValue` (the same
/// twenty fields), projected: `None` is an absent key; `Some` is the leaf's
/// canonical value, whatever it holds (an `LwwRegister` holding nothing is
/// `Some(None)`, which is `null`; `None` alone would not tell the two apart,
/// and serde writes both as `null`, which is why this reads the typed value).
macro_rules! leaves {
    ($v:expr, $out:expr) => {{
        let v = $v;
        let out: &mut Map<String, Value> = $out;
        let mut put = |name: &str, value: Option<Value>| {
            if let Some(value) = value {
                out.insert(name.to_string(), value);
            }
        };
        put(
            "text",
            v.text.as_ref().map(|t| json!(t.iter().collect::<String>())),
        );
        put("byte", v.byte.map(|n| json!(n)));
        put("short", v.short.map(|n| json!(n)));
        put("int", v.int.map(|n| json!(n)));
        put("long", v.long.map(|n| json!(n)));
        put("float", v.float.map(|n| float(n as f64)));
        put("double", v.double.map(float));
        put("ew", v.ew.map(Value::Bool));
        put("dw", v.dw.map(Value::Bool));
        put(
            "ch",
            v.ch.as_ref().map(|s| {
                many(
                    s.iter()
                        .map(|c| (*c as u64, json!(c.to_string())))
                        .collect(),
                )
            }),
        );
        put(
            "mv",
            v.mv.as_ref()
                .map(|s| many(s.iter().map(|t| (0, json!(t))).collect())),
        );
        put("lww", v.lww.as_ref().map(|o| json!(o)));
        put("fair", v.fair.as_ref().map(|o| json!(o)));
        put(
            "po",
            v.po.as_ref()
                .map(|s| many(s.iter().map(|t| (0, json!(t))).collect())),
        );
        put("to", v.to.as_ref().map(|o| json!(o)));
        put(
            "emv",
            v.emv
                .as_ref()
                .map(|s| many(s.iter().map(|c| (color_rank(c), color_name(c))).collect())),
        );
        put(
            "elww",
            v.elww
                .as_ref()
                .map(|o| o.as_ref().map_or(Value::Null, color_name)),
        );
        put(
            "efair",
            v.efair
                .as_ref()
                .map(|o| o.as_ref().map_or(Value::Null, color_name)),
        );
        put(
            "epo",
            v.epo
                .as_ref()
                .map(|s| many(s.iter().map(|c| (color_rank(c), color_name(c))).collect())),
        );
        put(
            "eto",
            v.eto
                .as_ref()
                .map(|o| o.as_ref().map_or(Value::Null, color_name)),
        );
    }};
}

/// `St04Value` in the canonical form: `Holder` with `eClass` and `tick`; an
/// optional containment that is `None` an absent key; a `Box` and a `Mid`
/// objects with their `eClass`, `k`, and every optional attribute by
/// [`leaves!`].
fn project(value: &St04Value) -> Value {
    let holder = &value.holder;
    let mut out = Map::new();
    out.insert("eClass".into(), json!("Holder"));
    out.insert("tick".into(), json!(holder.tick));
    if let Some(boxed) = &holder.inner {
        out.insert("inner".into(), box_of(boxed));
    }
    Value::Object(out)
}

fn box_of(value: &BoxValue) -> Value {
    let mut out = Map::new();
    out.insert("eClass".into(), json!("Box"));
    out.insert("k".into(), json!(value.k));
    leaves!(value, &mut out);
    if let Some(held) = &value.mid {
        out.insert("mid".into(), mid_of(held));
    }
    Value::Object(out)
}

fn mid_of(value: &MidValue) -> Value {
    let mut out = Map::new();
    out.insert("eClass".into(), json!("Mid"));
    out.insert("k".into(), json!(value.k));
    leaves!(value, &mut out);
    Value::Object(out)
}

#[test]
fn st04_b5_the_projection_reads_a_hand_written_generated_value() {
    let mut mid = MidValue {
        k: 1,
        ..Default::default()
    };
    mid.po = Some(["z".to_string()].into_iter().collect());
    mid.eto = Some(None);
    mid.dw = Some(false);
    let mut boxed = BoxValue {
        k: 10,
        ..Default::default()
    };
    boxed.text = Some(vec!['h', 'i']);
    boxed.byte = Some(3);
    boxed.long = Some(-4);
    boxed.float = Some(2.5);
    boxed.double = Some(0.0);
    boxed.ew = Some(false);
    boxed.ch = Some(['b', 'a'].into_iter().collect());
    boxed.mv = Some(Default::default());
    boxed.lww = Some(None);
    boxed.fair = Some(Some("q".to_string()));
    boxed.emv = Some([Color::Blue, Color::Red].into_iter().collect());
    boxed.elww = Some(Some(Color::Green));
    boxed.mid = Some(mid);
    let mut value = St04Value::default();
    value.holder.tick = 1;
    value.holder.inner = Some(boxed);
    assert_eq!(
        project(&value),
        json!({
            "eClass": "Holder", "tick": 1,
            "inner": {
                "eClass": "Box", "k": 10, "text": "hi", "byte": 3, "long": -4, "float": 2.5,
                "double": 0.0, "ew": false, "ch": {"__conflict": ["a", "b"]}, "mv": null,
                "lww": null, "fair": "q", "emv": {"__conflict": ["red", "blue"]},
                "elww": "green",
                "mid": {"eClass": "Mid", "k": 1, "po": "z", "eto": null, "dw": false}
            }
        })
    );
    assert_eq!(
        project(&St04Value::default()),
        json!({"eClass": "Holder", "tick": 0})
    );
}

// ---------------------------------------------------------------------------
// The operations, on both runtimes
// ---------------------------------------------------------------------------

/// Where the optional attribute sits and what removes it, as in the spec:
/// `Flat` (`Box.<leaf>`, U unsets `Holder.inner`), `Deep` (`Mid.<leaf>` in
/// `Box.mid`, U unsets `Holder.inner`), `Direct` (the guard: `Box.<leaf>`,
/// U unsets it itself).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Flat,
    Deep,
    Direct,
}

impl Place {
    fn targets(self) -> &'static [Target] {
        match self {
            Place::Deep => &[Target::Outer, Target::Sibling, Target::Leaf],
            _ => &[Target::Sibling, Target::Leaf],
        }
    }

    fn owner(self) -> &'static str {
        if self == Place::Deep { "Mid" } else { "Box" }
    }
}

/// The places of the claim and of its guard; those of the removal-alone
/// guard; those of the claim.
const TWIN: [Place; 3] = [Place::Flat, Place::Deep, Place::Direct];
const ALONE: [Place; 3] = [Place::Flat, Place::Deep, Place::Direct];
const CLAIM: [Place; 2] = [Place::Flat, Place::Deep];

/// What W reaches: the owner's `k` (`Sibling`, +10), the leaf itself
/// (`Leaf`, [`Which::W`]), or, in `Deep`, `Box.k` (`Outer`, +10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Outer,
    Sibling,
    Leaf,
}

/// One operation, as each runtime spells it.
#[derive(Clone, Debug)]
struct Op {
    interp: ModelOp,
    generated: St04,
}

fn set_or_unset<O>(op: Option<O>) -> Optional<O> {
    match op {
        Some(op) => Optional::Set(op),
        None => Optional::Unset,
    }
}

/// The generated operation on the record enum `$E` (`Bx` or `Mid`, whose
/// variants for the twenty optional attributes are spelled alike) that sets
/// the leaf of `$kind` with `$which` (`Some`), or unsets it (`None`).
macro_rules! leaf_op {
    ($E:ident, $kind:expr, $which:expr) => {{
        let w: Option<Which> = $which;
        match $kind {
            Kind::Text => $E::Text(set_or_unset(w.map(|w| List::Insert {
                content: w.ch(),
                pos: 0,
            }))),
            Kind::Byte => $E::Byte(set_or_unset(w.map(|w| Counter::Inc(w.int() as i8)))),
            Kind::Short => $E::Short(set_or_unset(w.map(|w| Counter::Inc(w.int() as i16)))),
            Kind::Int => $E::Int(set_or_unset(w.map(|w| Counter::Inc(w.int() as i32)))),
            Kind::Long => $E::Long(set_or_unset(w.map(|w| Counter::Inc(w.int())))),
            Kind::Float => $E::Float(set_or_unset(w.map(|w| Counter::Inc(w.int() as f32)))),
            Kind::Double => $E::Double(set_or_unset(w.map(|w| Counter::Inc(w.int() as f64)))),
            Kind::Ew => $E::Ew(set_or_unset(w.map(|w| {
                if Kind::Ew.enables(w) {
                    EWFlag::Enable
                } else {
                    EWFlag::Disable
                }
            }))),
            Kind::Dw => $E::Dw(set_or_unset(w.map(|w| {
                if Kind::Dw.enables(w) {
                    DWFlag::Enable
                } else {
                    DWFlag::Disable
                }
            }))),
            Kind::Ch => $E::Ch(set_or_unset(w.map(|w| MVRegister::Write(w.ch())))),
            Kind::Mv => $E::Mv(set_or_unset(w.map(|w| MVRegister::Write(w.text())))),
            Kind::Lww => $E::Lww(set_or_unset(w.map(|w| Register::Write(w.text())))),
            Kind::Fair => $E::Fair(set_or_unset(w.map(|w| Register::Write(w.text())))),
            Kind::Po => $E::Po(set_or_unset(w.map(|w| PORegister::Write(w.text())))),
            Kind::To => $E::To(set_or_unset(w.map(|w| TORegister::Write(w.text())))),
            Kind::Emv => $E::Emv(set_or_unset(w.map(|w| MVRegister::Write(w.color())))),
            Kind::Elww => $E::Elww(set_or_unset(w.map(|w| Register::Write(w.color())))),
            Kind::Efair => $E::Efair(set_or_unset(w.map(|w| Register::Write(w.color())))),
            Kind::Epo => $E::Epo(set_or_unset(w.map(|w| PORegister::Write(w.color())))),
            Kind::Eto => $E::Eto(set_or_unset(w.map(|w| TORegister::Write(w.color())))),
        }
    }};
}

struct Ops {
    sem: Arc<MetamodelSemantics>,
    color: u16,
}

impl Ops {
    fn new(desc: &Value) -> Self {
        let sem = Arc::new(from_descriptor(desc).expect("Arachne's descriptor parses"));
        let color = sem
            .enums
            .iter()
            .position(|entry| &*entry.name == "Color")
            .expect("st04.ecore declares Color") as u16;
        Ops { sem, color }
    }

    fn is(&self, class: &str, inner: InstanceOp) -> InstanceOp {
        InstanceOp::variant(class_slot(&self.sem, class), inner)
    }

    fn at(&self, class: &str, feature: &str, inner: InstanceOp) -> InstanceOp {
        InstanceOp::field(
            feature_slot(&self.sem, class_slot(&self.sem, class), feature),
            inner,
        )
    }

    fn inc(by: i64) -> InstanceOp {
        InstanceOp::Leaf(LeafOp::Inc(Scalar::Int(by)))
    }

    /// `op` on the `Box` in `Holder.inner`, on both runtimes.
    fn in_box(&self, interp: InstanceOp, generated: Bx) -> Op {
        Op {
            interp: self
                .is(
                    "Holder",
                    self.at("Holder", "inner", InstanceOp::set(self.is("Box", interp))),
                )
                .into_model_op(),
            generated: St04::Holder(Holder::Inner(Optional::Set(generated))),
        }
    }

    /// `op` on the `Mid` in `Box.mid`, on both runtimes.
    fn in_mid(&self, interp: InstanceOp, generated: Mid) -> Op {
        self.in_box(
            self.at("Box", "mid", InstanceOp::set(self.is("Mid", interp))),
            Bx::Mid(Optional::Set(generated)),
        )
    }

    fn tick(&self) -> Op {
        Op {
            interp: self
                .is("Holder", self.at("Holder", "tick", Self::inc(1)))
                .into_model_op(),
            generated: St04::Holder(Holder::Tick(Counter::Inc(1))),
        }
    }

    /// A write of `which` into the leaf of `kind` at `place`.
    fn write_leaf(&self, place: Place, kind: Kind, which: Which) -> Op {
        let leaf = InstanceOp::set(InstanceOp::Leaf(kind.interp_op(which, self.color)));
        let interp = self.at(place.owner(), kind.feature(), leaf);
        match place {
            Place::Deep => self.in_mid(interp, leaf_op!(Mid, kind, Some(which))),
            _ => self.in_box(interp, leaf_op!(Bx, kind, Some(which))),
        }
    }

    /// `k` +`by`: the owner's (`Sibling`) or the `Box`'s (`Outer`).
    fn inc_k(&self, place: Place, target: Target, by: i64) -> Op {
        match (place, target) {
            (Place::Deep, Target::Sibling) => self.in_mid(
                self.at("Mid", "k", Self::inc(by)),
                Mid::K(Counter::Inc(by as i32)),
            ),
            (_, Target::Sibling) | (Place::Deep, Target::Outer) => self.in_box(
                self.at("Box", "k", Self::inc(by)),
                Bx::K(Counter::Inc(by as i32)),
            ),
            (place, target) => panic!("set-up: no counter k for {target:?} in {place:?}"),
        }
    }

    fn x(&self, place: Place, kind: Kind) -> Op {
        self.write_leaf(place, kind, Which::X)
    }

    fn y(&self, place: Place) -> Op {
        self.inc_k(place, Target::Sibling, 2)
    }

    fn w(&self, place: Place, kind: Kind, target: Target) -> Op {
        match target {
            Target::Leaf => self.write_leaf(place, kind, Which::W),
            _ => self.inc_k(place, target, 10),
        }
    }

    /// U, the removal of the place.
    fn removal(&self, place: Place, kind: Kind) -> Op {
        match place {
            Place::Flat | Place::Deep => Op {
                interp: self
                    .is("Holder", self.at("Holder", "inner", InstanceOp::unset()))
                    .into_model_op(),
                generated: St04::Holder(Holder::Inner(Optional::Unset)),
            },
            Place::Direct => self.in_box(
                self.at("Box", kind.feature(), InstanceOp::unset()),
                leaf_op!(Bx, kind, None),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// The twins
// ---------------------------------------------------------------------------

/// One seat on both runtimes.
struct Seat {
    interp: Interp,
    generated: Gen,
}

impl Seat {
    fn interp_doc(&self) -> Value {
        self.interp.query(&Read::<Value>::new())
    }

    fn generated_doc(&self) -> Value {
        project(&self.generated.query(&Read::<St04Value>::new()))
    }
}

/// Three seats, what was sent on both runtimes, and every difference seen.
struct Twins {
    seats: Vec<Seat>,
    sent: Vec<(EventMessage<ModelOp>, EventMessage<St04>)>,
    armed: bool,
    label: String,
    differences: Vec<String>,
    comparisons: usize,
    steps: usize,
    /// Called with the seats after every step; the joiner test takes its
    /// snapshot there.
    on_step: Option<Box<dyn FnMut(usize, &[Seat])>>,
}

impl Twins {
    fn new(desc: &Value, label: String) -> Self {
        let (mut ia, mut ib, mut ic) = triplet_log::<ModelLog>();
        let opening = ia
            .send(install("m", desc))
            .expect("a fresh log takes its `Install`");
        ib.receive(opening.clone());
        ic.receive(opening);
        let (ga, gb, gc) = triplet_log::<St04Log>();
        Twins {
            seats: vec![
                Seat {
                    interp: ia,
                    generated: ga,
                },
                Seat {
                    interp: ib,
                    generated: gb,
                },
                Seat {
                    interp: ic,
                    generated: gc,
                },
            ],
            sent: Vec::new(),
            armed: false,
            label,
            differences: Vec::new(),
            comparisons: 0,
            steps: 0,
            on_step: None,
        }
    }

    fn compare(&mut self, seat: usize, when: &str) {
        if !self.armed {
            return;
        }
        self.comparisons += 1;
        let interp = self.seats[seat].interp_doc();
        let generated = self.seats[seat].generated_doc();
        if interp != generated {
            self.differences.push(format!(
                "{}: seat {seat} {when}: interpreted {interp} | generated {generated}",
                self.label
            ));
        }
    }

    fn stepped(&mut self) {
        self.steps += 1;
        if let Some(hook) = self.on_step.as_mut() {
            hook(self.steps, &self.seats);
        }
    }

    fn send(&mut self, seat: usize, op: &Op) -> usize {
        let interp = self.seats[seat]
            .interp
            .send(op.interp.clone())
            .unwrap_or_else(|why| {
                panic!(
                    "{}: the interpreted seat {seat} refused {op:?}: {why:?}",
                    self.label
                )
            });
        let generated = self.seats[seat]
            .generated
            .send(op.generated.clone())
            .unwrap_or_else(|why| {
                panic!(
                    "{}: the generated seat {seat} refused {op:?}: {why}",
                    self.label
                )
            });
        self.sent.push((interp, generated));
        self.compare(seat, &format!("after sending {:?}", op.generated));
        self.stepped();
        self.sent.len() - 1
    }

    fn deliver(&mut self, seat: usize, index: usize) {
        let (interp, generated) = self.sent[index].clone();
        self.seats[seat].interp.receive(interp);
        self.seats[seat].generated.receive(generated);
        self.compare(seat, &format!("after delivering operation {index}"));
        self.stepped();
    }

    fn arm(&mut self) {
        self.armed = true;
        for seat in 0..self.seats.len() {
            self.compare(seat, "once T was delivered everywhere");
        }
    }

    /// Two concurrent operations on seats `p` and `q`, each delivered to the
    /// other writer, and to the third seat in the order `first_p`.
    fn race(&mut self, (p, q): (usize, usize), op_p: &Op, op_q: &Op, first_p: bool) {
        let r = 3 - p - q;
        let ep = self.send(p, op_p);
        let eq = self.send(q, op_q);
        self.deliver(p, eq);
        self.deliver(q, ep);
        if first_p {
            self.deliver(r, ep);
            self.deliver(r, eq);
        } else {
            self.deliver(r, eq);
            self.deliver(r, ep);
        }
    }

    /// T from seat 0, delivered everywhere; the comparisons start then.
    fn opening(&mut self, ops: &Ops) {
        let t = self.send(0, &ops.tick());
        self.deliver(1, t);
        self.deliver(2, t);
        self.arm();
    }

    fn generated_docs(&self) -> Vec<Value> {
        self.seats.iter().map(Seat::generated_doc).collect()
    }

    fn interp_docs(&self) -> Vec<Value> {
        self.seats.iter().map(Seat::interp_doc).collect()
    }
}

// ---------------------------------------------------------------------------
// The schedules and the documents, as in the spec
// ---------------------------------------------------------------------------

const SEATS: [(usize, usize); 6] = [(0, 1), (1, 0), (0, 2), (2, 0), (1, 2), (2, 1)];

#[derive(Clone, Copy, Debug)]
struct Schedule {
    kind: Kind,
    place: Place,
    xy: (usize, usize),
    x_first: bool,
    uw: (usize, usize),
    u_first: bool,
    target: Target,
}

impl Schedule {
    fn label(&self) -> String {
        format!(
            "{:?} {:?}: X on {} Y on {}, third X first {}; U on {} W({:?}) on {}, third U \
             first {}",
            self.kind,
            self.place,
            self.xy.0,
            self.xy.1,
            self.x_first,
            self.uw.0,
            self.target,
            self.uw.1,
            self.u_first
        )
    }
}

fn schedules(kind: Kind, place: Place) -> Vec<Schedule> {
    let mut out = Vec::new();
    for xy in SEATS {
        for x_first in [true, false] {
            for uw in SEATS {
                for u_first in [true, false] {
                    for &target in place.targets() {
                        out.push(Schedule {
                            kind,
                            place,
                            xy,
                            x_first,
                            uw,
                            u_first,
                            target,
                        });
                    }
                }
            }
        }
    }
    out
}

/// T, then X (the leaf) and Y (the owner's `k` +2) raced, then U and W
/// raced, on both runtimes.
fn play(ops: &Ops, twins: &mut Twins, schedule: &Schedule) {
    let (kind, place) = (schedule.kind, schedule.place);
    twins.opening(ops);
    twins.race(
        schedule.xy,
        &ops.x(place, kind),
        &ops.y(place),
        schedule.x_first,
    );
    twins.race(
        schedule.uw,
        &ops.removal(place, kind),
        &ops.w(place, kind, schedule.target),
        schedule.u_first,
    );
}

fn mid(k: i64, kind: Kind, leaf: Option<Value>) -> Value {
    let mut out = json!({"eClass": "Mid", "k": k});
    if let Some(leaf) = leaf {
        out[kind.feature()] = leaf;
    }
    out
}

fn boxed(k: i64, kind: Kind, leaf: Option<Value>, below: Option<Value>) -> Value {
    let mut out = json!({"eClass": "Box", "k": k});
    if let Some(leaf) = leaf {
        out[kind.feature()] = leaf;
    }
    if let Some(below) = below {
        out["mid"] = below;
    }
    out
}

fn document(content: Option<Value>) -> Value {
    let mut doc = json!({"eClass": "Holder", "tick": 1});
    if let Some(content) = content {
        doc["inner"] = content;
    }
    doc
}

/// The rule's value once every seat has delivered T, X, Y, U and W (spec
/// section 2).
fn expected(schedule: &Schedule) -> Value {
    let kind = schedule.kind;
    let kept = kind.kept();
    let w = Some(kind.value(Which::W));
    let content = match (schedule.place, schedule.target) {
        (Place::Flat, Target::Sibling) => boxed(10, kind, kept, None),
        (Place::Flat, Target::Leaf) => boxed(0, kind, w, None),
        (Place::Deep, Target::Outer) => {
            boxed(10, kind, None, kept.map(|leaf| mid(0, kind, Some(leaf))))
        }
        (Place::Deep, Target::Sibling) => boxed(0, kind, None, Some(mid(10, kind, kept))),
        (Place::Deep, Target::Leaf) => boxed(0, kind, None, Some(mid(0, kind, w))),
        (Place::Direct, Target::Sibling) => boxed(12, kind, kept, None),
        (Place::Direct, Target::Leaf) => boxed(2, kind, w, None),
        (place, target) => panic!("set-up: no {target:?} in {place:?}"),
    };
    document(Some(content))
}

/// U alone.
fn unset_alone(kind: Kind, place: Place) -> Value {
    let kept = kind.kept();
    match place {
        Place::Flat => document(kept.map(|leaf| boxed(0, kind, Some(leaf), None))),
        Place::Deep => {
            document(kept.map(|leaf| boxed(0, kind, None, Some(mid(0, kind, Some(leaf))))))
        }
        Place::Direct => document(Some(boxed(2, kind, kept, None))),
    }
}

/// The later writes of the spec's B2, from one writer: W's target again,
/// then, where W did not reach the leaf, the leaf written again. Each is
/// followed by what every seat must then serve.
fn later_writes(ops: &Ops, schedule: &Schedule) -> Vec<(Op, Value)> {
    let (kind, place) = (schedule.kind, schedule.place);
    let kept = kind.kept();
    let a = Some(kind.value(Which::Again));
    let r = Some(kind.value(Which::Rebuild));
    let (again_op, again) = match (place, schedule.target) {
        (Place::Flat, Target::Sibling) => (
            ops.inc_k(place, Target::Sibling, 1),
            boxed(11, kind, kept.clone(), None),
        ),
        (Place::Deep, Target::Outer) => (
            ops.inc_k(place, Target::Outer, 1),
            boxed(
                11,
                kind,
                None,
                kept.clone().map(|leaf| mid(0, kind, Some(leaf))),
            ),
        ),
        (Place::Deep, Target::Sibling) => (
            ops.inc_k(place, Target::Sibling, 1),
            boxed(0, kind, None, Some(mid(11, kind, kept.clone()))),
        ),
        (Place::Direct, Target::Sibling) => (
            ops.inc_k(place, Target::Sibling, 1),
            boxed(13, kind, kept.clone(), None),
        ),
        (Place::Deep, Target::Leaf) => (
            ops.write_leaf(place, kind, Which::Again),
            boxed(0, kind, None, Some(mid(0, kind, a))),
        ),
        (Place::Flat, Target::Leaf) => (
            ops.write_leaf(place, kind, Which::Again),
            boxed(0, kind, a, None),
        ),
        (Place::Direct, Target::Leaf) => (
            ops.write_leaf(place, kind, Which::Again),
            boxed(2, kind, a, None),
        ),
        (place, target) => panic!("set-up: no later writes for {target:?} in {place:?}"),
    };
    let mut out = vec![(again_op, document(Some(again)))];
    if schedule.target != Target::Leaf {
        let rebuilt = match (place, schedule.target) {
            (Place::Flat, Target::Sibling) => boxed(11, kind, r, None),
            (Place::Deep, Target::Outer) => boxed(11, kind, None, Some(mid(0, kind, r))),
            (Place::Deep, Target::Sibling) => boxed(0, kind, None, Some(mid(11, kind, r))),
            (Place::Direct, Target::Sibling) => boxed(13, kind, r, None),
            (place, target) => panic!("set-up: nothing rebuilt for {target:?} in {place:?}"),
        };
        out.push((
            ops.write_leaf(place, kind, Which::Rebuild),
            document(Some(rebuilt)),
        ));
    }
    out
}

/// Every generated seat's read against `want`; one line per seat that
/// serves something else.
fn generated_against(twins: &Twins, want: &Value, label: &str) -> Vec<String> {
    twins
        .generated_docs()
        .iter()
        .enumerate()
        .filter(|(_, served)| *served != want)
        .map(|(seat, served)| format!("{label}: generated seat {seat} serves {served}, not {want}"))
        .collect()
}

/// Whether the generated seats part.
fn generated_parted(twins: &Twins) -> bool {
    twins.generated_docs().windows(2).any(|p| p[0] != p[1])
}

/// Run `f` for every kind on two threads, the results in kind order.
fn per_kind<T: Send>(f: impl Fn(Kind) -> T + Sync) -> Vec<(Kind, T)> {
    const WORKERS: usize = 2;
    let f = &f;
    let mut out: Vec<(usize, Kind, T)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..WORKERS)
            .map(|worker| {
                scope.spawn(move || {
                    KINDS
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| index % WORKERS == worker)
                        .map(|(index, &kind)| (index, kind, f(kind)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("a worker thread panicked"))
            .collect()
    });
    out.sort_by_key(|(index, _, _)| *index);
    out.into_iter()
        .map(|(_, kind, value)| (kind, value))
        .collect()
}

/// One kind's counts in one test.
#[derive(Default)]
struct Counts {
    runs: usize,
    comparisons: usize,
    differences: Vec<String>,
    values: Vec<String>,
    parted: Vec<String>,
}

fn summary(
    test: &str,
    places: &[Place],
    per: &[(Kind, Counts)],
) -> (usize, usize, usize, Vec<String>) {
    let (mut runs, mut comparisons, mut differences, mut values, mut parted) = (0, 0, 0, 0, 0);
    let mut first = Vec::new();
    for (kind, c) in per {
        runs += c.runs;
        comparisons += c.comparisons;
        differences += c.differences.len();
        values += c.values.len();
        parted += c.parted.len();
        let by_place: Vec<String> = places
            .iter()
            .map(|place| {
                let prefix = format!("{kind:?} {place:?}:");
                let count =
                    |lines: &[String]| lines.iter().filter(|l| l.starts_with(&prefix)).count();
                format!(
                    "{place:?} {} {} {}",
                    count(&c.differences),
                    count(&c.values),
                    count(&c.parted)
                )
            })
            .collect();
        println!(
            "ST04-B5 {test}: {kind:?}: runs {}, twin differences {}, generated values other than \
             expected {}, generated runs parted {} (per place: {})",
            c.runs,
            c.differences.len(),
            c.values.len(),
            c.parted.len(),
            by_place.join("; ")
        );
        if first.len() < 6 {
            first.extend(
                c.differences
                    .iter()
                    .chain(&c.values)
                    .chain(&c.parted)
                    .take(2)
                    .cloned(),
            );
        }
    }
    println!(
        "ST04-B5 {test}: runs {runs}, twin comparisons {comparisons}, twin differences \
         {differences} (P7), generated values other than expected {values} (P8), generated runs \
         parted {parted} (P9)"
    );
    for line in first.iter().take(6) {
        println!("ST04-B5 {line}");
    }
    (differences, values, parted, first)
}

fn verdict(test: &str, (differences, values, parted, first): (usize, usize, usize, Vec<String>)) {
    assert!(
        differences == 0 && values == 0 && parted == 0,
        "ST.04 B5 {test}: {differences} twin differences, {values} generated values other than \
         expected and {parted} generated runs parted; first:\n  {}",
        first
            .iter()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// B5
// ---------------------------------------------------------------------------

/// Every schedule of the claim (per kind: Flat 288, Deep 432) and of the
/// direct guard (Direct 288), 20,160 in all: twin equality after every step
/// once T is everywhere; at the end the generated seats serve one document,
/// the rule's.
#[test]
fn st04_b5_every_schedule_both_runtimes_agree_after_every_step_and_serve_the_rule() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let per = per_kind(|kind| {
        let mut c = Counts::default();
        for place in TWIN {
            for schedule in schedules(kind, place) {
                c.runs += 1;
                let label = schedule.label();
                let mut twins = Twins::new(&desc, label.clone());
                play(&ops, &mut twins, &schedule);
                c.values
                    .extend(generated_against(&twins, &expected(&schedule), &label));
                if generated_parted(&twins) {
                    c.parted.push(format!(
                        "{label}: the generated seats part: {:?}",
                        twins.generated_docs()
                    ));
                }
                c.comparisons += twins.comparisons;
                c.differences.append(&mut twins.differences);
            }
        }
        c
    });
    verdict("schedules", summary("schedules", &TWIN, &per));
}

/// U alone, in `Flat`, `Deep` and `Direct`: 12 formations x 3 seats of U,
/// 108 per kind.
#[test]
fn st04_b5_a_removal_alone_agrees_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let per = per_kind(|kind| {
        let mut c = Counts::default();
        for place in ALONE {
            for xy in SEATS {
                for x_first in [true, false] {
                    for u in 0..3 {
                        c.runs += 1;
                        let label = format!(
                            "{kind:?} {place:?}: X on {} Y on {}, third X first {x_first}; U \
                             alone on {u}",
                            xy.0, xy.1
                        );
                        let mut twins = Twins::new(&desc, label.clone());
                        twins.opening(&ops);
                        twins.race(xy, &ops.x(place, kind), &ops.y(place), x_first);
                        let removal = twins.send(u, &ops.removal(place, kind));
                        for seat in 0..3 {
                            if seat != u {
                                twins.deliver(seat, removal);
                            }
                        }
                        c.values.extend(generated_against(
                            &twins,
                            &unset_alone(kind, place),
                            &label,
                        ));
                        c.comparisons += twins.comparisons;
                        c.differences.append(&mut twins.differences);
                    }
                }
            }
        }
        c
    });
    verdict("removal alone", summary("removal alone", &ALONE, &per));
}

/// After the race, one seat makes the later writes of [`later_writes`], each
/// delivered everywhere; each seat takes that turn in its own run. X on 0,
/// Y on 1, the third taking X first; U and W in every seating, the third
/// taking U first; every target; the three places: 126 runs per kind.
#[test]
fn st04_b5_writes_after_the_race_agree_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let per = per_kind(|kind| {
        let mut c = Counts::default();
        for place in TWIN {
            for uw in SEATS {
                for &target in place.targets() {
                    for writer in 0..3 {
                        c.runs += 1;
                        let schedule = Schedule {
                            kind,
                            place,
                            xy: (0, 1),
                            x_first: true,
                            uw,
                            u_first: true,
                            target,
                        };
                        let label = format!("{}; then seat {writer} writes", schedule.label());
                        let mut twins = Twins::new(&desc, label.clone());
                        play(&ops, &mut twins, &schedule);
                        for (op, want) in later_writes(&ops, &schedule) {
                            let index = twins.send(writer, &op);
                            for seat in 0..3 {
                                if seat != writer {
                                    twins.deliver(seat, index);
                                }
                            }
                            c.values.extend(generated_against(
                                &twins,
                                &want,
                                &format!("{label}, after {:?}", op.generated),
                            ));
                        }
                        c.comparisons += twins.comparisons;
                        c.differences.append(&mut twins.differences);
                    }
                }
            }
        }
        c
    });
    verdict(
        "writes after the race",
        summary("writes after the race", &TWIN, &per),
    );
}

/// A joiner `d` on each runtime adopts, after the same step, the same seat's
/// causal snapshot and log (each through a serde round trip), delivers every
/// operation sent, in the order sent, and must serve on each runtime what
/// its twin serves and the rule's document; then it makes the later writes
/// of [`later_writes`], delivered to the three, and all four seats agree
/// across runtimes and serve what those say. X on 0, Y on 1; U and W on 0
/// and 1 in both assignments; every target; the two places of the claim;
/// each seat as donor after each of the 15 steps: 450 joins per kind.
#[test]
fn st04_b5_a_joiner_by_state_transfer_agrees_on_both_runtimes() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    let per = per_kind(|kind| {
        let mut c = Counts::default();
        for place in CLAIM {
            for uw in [(0, 1), (1, 0)] {
                for &target in place.targets() {
                    let schedule = Schedule {
                        kind,
                        place,
                        xy: (0, 1),
                        x_first: true,
                        uw,
                        u_first: true,
                        target,
                    };
                    for donor in 0..3 {
                        for at in 1..=15 {
                            c.runs += 1;
                            let label = format!(
                                "{}; d joins from seat {donor} after step {at}",
                                schedule.label()
                            );
                            let taken: std::rc::Rc<std::cell::RefCell<Option<Seat>>> =
                                Default::default();
                            let mut twins = Twins::new(&desc, label.clone());
                            let slot = taken.clone();
                            twins.on_step = Some(Box::new(move |step, seats| {
                                if step == at {
                                    *slot.borrow_mut() = Some(join(&seats[donor]));
                                }
                            }));
                            play(&ops, &mut twins, &schedule);
                            twins.on_step = None;
                            let d = taken
                                .borrow_mut()
                                .take()
                                .expect("set-up: the schedule has 15 steps");
                            twins.seats.push(d);
                            let sent = twins.sent.clone();
                            for (interp, generated) in sent {
                                twins.seats[3].interp.receive(interp);
                                twins.seats[3].generated.receive(generated);
                            }
                            twins.compare(3, "once the joiner delivered everything");
                            let want = expected(&schedule);
                            let joined = twins.seats[3].generated_doc();
                            if joined != want {
                                c.values.push(format!(
                                    "{label}: the generated joiner serves {joined}, not {want}"
                                ));
                            }
                            for (op, want) in later_writes(&ops, &schedule) {
                                let index = twins.send(3, &op);
                                for seat in 0..3 {
                                    twins.deliver(seat, index);
                                }
                                c.values.extend(generated_against(
                                    &twins,
                                    &want,
                                    &format!("{label}, d wrote {:?}", op.generated),
                                ));
                            }
                            c.comparisons += twins.comparisons;
                            c.differences.append(&mut twins.differences);
                        }
                    }
                }
            }
        }
        c
    });
    verdict("joiner", summary("joiner", &CLAIM, &per));
}

/// A fourth seat joining by state transfer from `donor` on both runtimes:
/// bootstrapped knowing only itself on the donor's log, adopting the donor's
/// snapshot and log, each through a serde round trip.
fn join(donor: &Seat) -> Seat {
    let log: ModelLog = serde_json::from_str(
        &serde_json::to_string(donor.interp.log()).expect("the interpreted log serializes"),
    )
    .expect("the interpreted log comes back");
    let snapshot: StateSnapshot<ModelOp> = serde_json::from_str(
        &serde_json::to_string(&donor.interp.snapshot()).expect("the snapshot serializes"),
    )
    .expect("the snapshot comes back");
    let mut interp: Interp =
        Replica::bootstrap_with_log_id("d".to_string(), &["d"], donor.interp.log_id().clone());
    interp.adopt(snapshot, log);

    let log: St04Log = serde_json::from_str(
        &serde_json::to_string(donor.generated.log()).expect("the generated log serializes"),
    )
    .expect("the generated log comes back");
    let snapshot: StateSnapshot<St04> = serde_json::from_str(
        &serde_json::to_string(&donor.generated.snapshot()).expect("the snapshot serializes"),
    )
    .expect("the snapshot comes back");
    let mut generated: Gen =
        Replica::bootstrap_with_log_id("d".to_string(), &["d"], donor.generated.log_id().clone());
    generated.adopt(snapshot, log);
    Seat { interp, generated }
}

// ---------------------------------------------------------------------------
// Observation, outside the claim: not a pass criterion
// ---------------------------------------------------------------------------

/// The spec's OB1, printed and not asserted: per kind, in `Flat` with W on
/// `Box.k`, the documents each runtime's three seats serve at the end of one
/// schedule where the third seat delivers U first, and the generated node's
/// served JSON (`serde`) for the same seats, which writes `None` and
/// `Some(None)` of a single-value register alike.
#[test]
#[ignore = "ST.04 observation OB1, not a pass criterion: run with --ignored --nocapture"]
fn st04_observation_what_each_runtime_serves_per_kind() {
    let desc = descriptor();
    let ops = Ops::new(&desc);
    for kind in KINDS {
        let schedule = Schedule {
            kind,
            place: Place::Flat,
            xy: (0, 1),
            x_first: true,
            uw: (0, 1),
            u_first: true,
            target: Target::Sibling,
        };
        let mut twins = Twins::new(&desc, schedule.label());
        play(&ops, &mut twins, &schedule);
        let served: Vec<Value> = twins
            .seats
            .iter()
            .map(|seat| {
                let raw = serde_json::to_value(seat.generated.query(&Read::<St04Value>::new()))
                    .expect("serializes");
                raw["holder"]["inner"][kind.feature()].clone()
            })
            .collect();
        println!(
            "ST04-OB1 {kind:?}: interpreted {:?} | generated {:?} | generated serde {:?}",
            twins
                .interp_docs()
                .iter()
                .map(|doc| doc["inner"].to_string())
                .collect::<Vec<_>>(),
            twins
                .generated_docs()
                .iter()
                .map(|doc| doc["inner"].to_string())
                .collect::<Vec<_>>(),
            served
        );
    }
}
