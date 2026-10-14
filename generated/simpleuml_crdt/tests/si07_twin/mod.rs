//! SI.07, claim P1, the part its two twin drivers share
//! (`si07_p1_twin_board.rs` over the board of `si05board.ecore`, whose
//! `Board.pinned` is an optional containment of `Card`, and
//! `si07_p1_twin_keyed.rs` over `si05keyed.ecore`, whose `Board.slots` is a
//! keyed collection of `Card`s).
//!
//! The spec is `cea-cdrt-knowledge/spec/items/SI.07.md`, claim P1 and its
//! method. In one paragraph: three replicas of the interpreted `ModelLog`, on
//! the descriptor with the identity rule, and three replicas of the generated
//! log of the same metamodel (Arachne's own `generate`, its sources copied
//! into `fixtures/si07/<name>/`) are driven by one schedule, each edit encoded
//! once per path, sent by the same writer on both paths and delivered in the
//! same order on both. Every edit costs one event on each path and the
//! interpreted `Install` is answered by one generated opener, so every event
//! carries the same `(origin, sequence, Lamport)` tag on both paths; the
//! harness asserts it at every send. A concurrent schedule runs in every
//! delivery order causal broadcast allows at every replica, and after every
//! run the three replicas of each path are projected and the two paths are
//! held equal, replica by replica.
//!
//! The projection, per replica: the containment the driver reads (the titles
//! of `Board.cards` in order, and the card in the optional or at each key,
//! with its title and its rank) and the links, as `(source, feature, target)`
//! of labels. A label is given by the schedule when it creates a card: on the
//! interpreted side the card's served identity, read on the writer right after
//! the create, names a card of `Board.cards`; on the generated side the one
//! vertex the create added to the reference graph names it. The card in the
//! optional or at a key is labelled by its place on both sides, since its
//! served identity may move on a later write (ST.04's OB2) while the
//! generated vertex is the place itself. An identity or a vertex that maps to
//! no label is a failure, never skipped.
//!
//! The generated runtime is the ground truth here: an arc is read when its
//! two vertices are in the graph; a vertex is added by every create or update
//! of its record and taken away by a `DeleteSubtree` that saw it; a link adds
//! no vertex. The interpreted path must read the same links.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use moirai_crdt::utils::membership::triplet_log;
use moirai_interp::eval::read_identified;
use moirai_interp::testing::install;
use moirai_interp::{ModelLog, ModelOp};
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::Tcsb;
use moirai_protocol::replica::{IsReplica, Replica};
use serde_json::Value;

pub type InterpReplica = Replica<ModelLog, Tcsb<ModelOp>>;
pub type InterpMsg = EventMessage<ModelOp>;

/// The key an element's store identity is served under.
pub const ID: &str = "__id";

/// The generated side, three replicas of one generated log.
pub trait GenSide {
    type Msg: Clone + Debug;
    fn fresh() -> Self;
    /// The opener that answers the interpreted `Install`: the root
    /// `record!`'s `New`, which writes nothing.
    fn opener() -> Value;
    fn send(&mut self, at: usize, op: Value) -> Result<Self::Msg, String>;
    fn receive(&mut self, at: usize, message: Self::Msg);
    fn tag(message: &Self::Msg) -> String;
    /// The typed read-out's containment, serialized.
    fn value(&self, at: usize) -> Value;
    /// The reference graph: every vertex serialized, every arc as
    /// `(source index, target index, edge kind)`.
    fn graph(&self, at: usize) -> (Vec<Value>, Vec<(usize, usize, String)>);
    /// The feature an arc kind of the graph is, `blocker` or `related`.
    fn feature_of(edge: &str) -> &'static str;
}

/// One edit, as sent on both paths.
#[derive(Clone, Debug)]
pub struct Pair<M: Clone> {
    pub what: String,
    pub interp: InterpMsg,
    pub generated: M,
}

/// What a replica of either path serves, projected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projection {
    /// The containment, as the driver normalises it.
    pub containment: Value,
    /// `(source label, feature, target label)`.
    pub links: BTreeSet<(String, String, String)>,
}

pub struct Twin<G: GenSide> {
    pub interp: [InterpReplica; 3],
    pub gen_side: G,
    /// Interpreted identity -> label, for the cards of `Board.cards`.
    pub identities: BTreeMap<String, String>,
    /// Generated vertex (serialized) -> label.
    pub vertices: BTreeMap<String, String>,
    pub sent: usize,
}

impl<G: GenSide> Twin<G> {
    /// Both paths opened on `descriptor`: the interpreted `Install` from
    /// replica 0, answered by the generated opener from replica 0, both
    /// delivered to replicas 1 and 2.
    pub fn new(descriptor: &Value) -> Twin<G> {
        let (mut a, mut b, mut c) = triplet_log::<ModelLog>();
        let event = a
            .send(install("si07", descriptor))
            .expect("a fresh log takes its first `Install`");
        b.receive(event.clone());
        c.receive(event.clone());
        let mut gen_side = G::fresh();
        let opener = gen_side
            .send(0, G::opener())
            .expect("a fresh generated log takes its opener");
        let tag = interp_tag(&event);
        assert_eq!(
            tag,
            G::tag(&opener),
            "harness: the opener is not the interpreted Install's tag"
        );
        gen_side.receive(1, opener.clone());
        gen_side.receive(2, opener);
        Twin {
            interp: [a, b, c],
            gen_side,
            identities: BTreeMap::new(),
            vertices: BTreeMap::new(),
            sent: 1,
        }
    }

    /// Send one edit on replica `at` of both paths; the generated intake is
    /// asked first and must take it, since every edit of these schedules is
    /// one a writer holding the generated runtime may send, and the
    /// interpreted intake must then take it too. The two events must carry
    /// one tag.
    pub fn send(
        &mut self,
        at: usize,
        what: &str,
        interp: ModelOp,
        generated: Value,
    ) -> Pair<G::Msg> {
        let gen_message = self.gen_side.send(at, generated.clone()).unwrap_or_else(|why| {
            panic!("set-up: the generated intake refused `{what}` on replica {at}: {why}\n{generated}")
        });
        let interp_message = self.interp[at].send(interp.clone()).unwrap_or_else(|why| {
            panic!(
                "the interpreted intake refused `{what}` on replica {at}, which the generated one took: {why:?}"
            )
        });
        assert_eq!(
            interp_tag(&interp_message),
            G::tag(&gen_message),
            "harness: `{what}` carries different tags on the two paths"
        );
        self.sent += 1;
        Pair {
            what: what.to_string(),
            interp: interp_message,
            generated: gen_message,
        }
    }

    /// Deliver one edit to replica `at` of both paths.
    pub fn deliver(&mut self, at: usize, pair: &Pair<G::Msg>) {
        self.interp[at].receive(pair.interp.clone());
        self.gen_side.receive(at, pair.generated.clone());
    }

    /// Send on `at` and deliver to the two others.
    pub fn broadcast(
        &mut self,
        at: usize,
        what: &str,
        interp: ModelOp,
        generated: Value,
    ) -> Pair<G::Msg> {
        let pair = self.send(at, what, interp, generated);
        for other in 0..3 {
            if other != at {
                self.deliver(other, &pair);
            }
        }
        pair
    }

    /// The generated vertices replica `at` holds, serialized.
    pub fn vertex_set(&self, at: usize) -> BTreeSet<String> {
        self.gen_side
            .graph(at)
            .0
            .iter()
            .map(Value::to_string)
            .collect()
    }

    /// Label the one vertex replica `at` holds that `before` did not.
    pub fn label_new_vertex(&mut self, at: usize, before: &BTreeSet<String>, label: &str) {
        let after = self.vertex_set(at);
        let added: Vec<&String> = after.difference(before).collect();
        assert_eq!(
            added.len(),
            1,
            "harness: creating `{label}` added {} vertices to the generated graph: {added:?}",
            added.len()
        );
        self.vertices.insert(added[0].clone(), label.to_string());
    }

    /// The generated vertex a label names, as the path an arc carries.
    pub fn vertex_path(&self, label: &str) -> Value {
        let text = self
            .vertices
            .iter()
            .find(|(_, held)| held.as_str() == label)
            .map(|(vertex, _)| vertex.clone())
            .unwrap_or_else(|| panic!("harness: no generated vertex is labelled `{label}`"));
        let parsed: Value = serde_json::from_str(&text).expect("a vertex is JSON");
        parsed
            .as_object()
            .and_then(|map| map.values().next())
            .cloned()
            .expect("a vertex is one tagged path")
    }

    /// The interpreted identity a label names.
    pub fn identity(&self, label: &str) -> String {
        self.identities
            .iter()
            .find(|(_, held)| held.as_str() == label)
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| panic!("harness: no interpreted identity is labelled `{label}`"))
    }

    /// The identified read of replica `at` of the interpreted path.
    pub fn identified(&self, at: usize) -> Value {
        let log = self.interp[at].state();
        match log.semantics() {
            Some(sem) => read_identified(sem, log.root()),
            None => Value::Null,
        }
    }

    /// The generated links replica `at` reads, as labels.
    pub fn gen_links(&self, at: usize) -> Result<BTreeSet<(String, String, String)>, String> {
        let (vertices, arcs) = self.gen_side.graph(at);
        let label = |index: usize| -> Result<String, String> {
            let text = vertices[index].to_string();
            self.vertices
                .get(&text)
                .cloned()
                .ok_or_else(|| format!("generated vertex {text} carries no label"))
        };
        let mut out = BTreeSet::new();
        for (from, to, edge) in arcs {
            out.insert((label(from)?, G::feature_of(&edge).to_string(), label(to)?));
        }
        Ok(out)
    }

    /// The interpreted links of one card value, as labels, `source` being the
    /// card's own label.
    pub fn interp_links_of(
        &self,
        source: &str,
        card: &Value,
        out: &mut BTreeSet<(String, String, String)>,
    ) -> Result<(), String> {
        for feature in ["blocker", "related"] {
            let targets: Vec<String> = match card.get(feature) {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::String(id)) => vec![id.clone()],
                Some(Value::Array(items)) => items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(str::to_string)
                            .ok_or_else(|| format!("K3: `{feature}` holds {item}"))
                    })
                    .collect::<Result<_, _>>()?,
                Some(other) => return Err(format!("K3: `{feature}` reads as {other}")),
            };
            for id in targets {
                let target = self
                    .identities
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| format!("interpreted identity {id} carries no label"))?;
                out.insert((source.to_string(), feature.to_string(), target));
            }
        }
        Ok(())
    }
}

/// `origin@seq L<lamport>` of an interpreted event.
pub fn interp_tag(message: &InterpMsg) -> String {
    let event = message.event();
    format!(
        "{}@{} L{}",
        event.id().origin_id(),
        event.id().seq(),
        event.lamport().val()
    )
}

/// The characters of a generated optional text (`Option<Vec<char>>`,
/// serialized), as a string; `None` when the optional is unset.
pub fn gen_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Array(chars) => Some(
            chars
                .iter()
                .map(|c| c.as_str().expect("a char serializes as a string"))
                .collect(),
        ),
        other => panic!("a generated text reads as {other}"),
    }
}

/// An interpreted optional text, `None` when the key is absent.
pub fn interp_text(value: Option<&Value>) -> Option<String> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(other) => panic!("an interpreted text reads as {other}"),
    }
}

/// Every interleaving of `streams` that keeps each stream's own order.
pub fn interleavings(streams: &[Vec<(usize, usize)>]) -> Vec<Vec<(usize, usize)>> {
    fn go(
        streams: &[Vec<(usize, usize)>],
        at: &mut Vec<usize>,
        acc: &mut Vec<(usize, usize)>,
        out: &mut Vec<Vec<(usize, usize)>>,
    ) {
        let mut done = true;
        for index in 0..streams.len() {
            if at[index] < streams[index].len() {
                done = false;
                acc.push(streams[index][at[index]]);
                at[index] += 1;
                go(streams, at, acc, out);
                at[index] -= 1;
                acc.pop();
            }
        }
        if done {
            out.push(acc.clone());
        }
    }
    let mut out = Vec::new();
    go(
        streams,
        &mut vec![0; streams.len()],
        &mut Vec::new(),
        &mut out,
    );
    out
}

/// One run of a concurrent schedule: the twin once every writer has sent, and
/// what each replica has still to deliver, as streams (each in the order
/// causal broadcast imposes on it, the streams concurrent with one another).
pub struct Run<G: GenSide> {
    pub twin: Twin<G>,
    pub incoming: [Vec<Vec<Pair<G::Msg>>>; 3],
}

/// Run `schedule` in every delivery order; after each run, `project` is
/// asked for both paths' projection of each replica, which must agree path
/// to path (under [`Sides::Both`]) and replica to replica, and `check` reads
/// the agreed generated projection.
/// Then `after`, when given, runs on the delivered twin (a later edit,
/// delivered to all) and the two paths are compared again. Answers the number
/// of runs.
pub fn every_order<G: GenSide>(
    label: &str,
    sides: Sides,
    schedule: impl Fn() -> Run<G>,
    project: impl Fn(&Twin<G>, usize) -> (Projection, Projection),
    check: impl Fn(&Projection),
    after: Option<&dyn Fn(&mut Twin<G>) -> &'static str>,
    check_after: impl Fn(&Projection),
) -> usize {
    let probe = schedule();
    let per_replica: Vec<Vec<Vec<(usize, usize)>>> = probe
        .incoming
        .iter()
        .map(|streams| {
            let indexed: Vec<Vec<(usize, usize)>> = streams
                .iter()
                .enumerate()
                .map(|(stream, messages)| (0..messages.len()).map(|i| (stream, i)).collect())
                .collect();
            interleavings(&indexed)
        })
        .collect();
    let mut combos: Vec<Vec<usize>> = vec![Vec::new()];
    for options in &per_replica {
        combos = combos
            .into_iter()
            .flat_map(|combo| {
                (0..options.len()).map(move |choice| {
                    let mut next = combo.clone();
                    next.push(choice);
                    next
                })
            })
            .collect();
    }
    for combo in &combos {
        let mut run = schedule();
        for (replica, &choice) in combo.iter().enumerate() {
            for &(stream, index) in &per_replica[replica][choice] {
                let pair = run.incoming[replica][stream][index].clone();
                run.twin.deliver(replica, &pair);
            }
        }
        let agreed = compare_sides(
            &run.twin,
            &project,
            &format!("{label}, orders {combo:?}"),
            sides,
        );
        check(&agreed);
        if let Some(after) = after {
            let what = after(&mut run.twin);
            let agreed = compare_sides(
                &run.twin,
                &project,
                &format!("{label}, orders {combo:?}, then {what}"),
                sides,
            );
            check_after(&agreed);
        }
    }
    combos.len()
}

/// Both paths' projections of every replica: the generated one and the
/// interpreted one equal at each replica, and every replica equal to replica
/// 0. Answers replica 0's.
pub fn compare<G: GenSide>(
    twin: &Twin<G>,
    project: &impl Fn(&Twin<G>, usize) -> (Projection, Projection),
    label: &str,
) -> Projection {
    compare_sides(twin, project, label, Sides::Both)
}

/// Which paths a run is held to: both, compared replica by replica (the
/// oracle), or the generated one alone (the harness's rehearsal, which holds
/// the hand-computed answers to the generated runtime whatever the
/// interpreted path reads).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sides {
    Both,
    GeneratedOnly,
}

/// [`compare`], or, under [`Sides::GeneratedOnly`], the generated replicas
/// held to one another alone.
pub fn compare_sides<G: GenSide>(
    twin: &Twin<G>,
    project: &impl Fn(&Twin<G>, usize) -> (Projection, Projection),
    label: &str,
    sides: Sides,
) -> Projection {
    let mut first: Option<Projection> = None;
    for at in 0..3 {
        let (interp, generated) = project(twin, at);
        if sides == Sides::Both {
            assert_eq!(
                interp, generated,
                "P1 parity, {label}: replica {at} of the interpreted path (left) reads other than the \
                 generated runtime (right)"
            );
        }
        match &first {
            None => first = Some(generated),
            Some(held) => assert_eq!(
                &generated, held,
                "{label}: replica {at} serves other than replica 0"
            ),
        }
    }
    first.expect("three replicas")
}
