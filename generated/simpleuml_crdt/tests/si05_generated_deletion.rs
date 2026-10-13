//! SI.05, decision 2: what the generated runtime does when an update of a
//! link's target is concurrent with the target's deletion.
//!
//! This file drives the generated `SimpleumlLog` alone (and, in its last test,
//! the interpreted `ModelLog` on the containment half of the same schedule).
//! It characterises the baseline the twin comparison of SI.05 is held to, on
//! the schedules Cam's decision 2 names: the model log turns every `Create`
//! *and* every `Update` sink of an object into an `AddVertex` in the reference
//! graph, and only a `DeleteSubtree` causally after an `AddVertex` makes it
//! redundant, so an update concurrent with the deletion leaves a vertex.
//!
//! What it shows (spec `cea-cdrt-knowledge/spec/items/SI.05.md`, section 1):
//! the vertex survives exactly when the object does. An update concurrent
//! with the deletion revives the object in the containment itself, because
//! `NestedListLog` is update-wins, and the interpreted containment does the
//! same (`q1_interp_...`); the reference graph adds nothing to that. No link
//! the deletion saw is readable again, even though its target survives (Q1);
//! a link concurrent with the deletion is readable exactly when the target
//! survives (Q2 against Q3).
//!
//! The model is `Model{ownedElements: [C: Class, D: Class{generalizations:
//! [G]}]}`, written by replica `a` and delivered to `b`; the link is
//! `G.general -> C` (`GeneralizationToClass`, `[0, 1]`). Every schedule then
//! partitions `a` and `b`, lets `a` delete `C` and `b` do something to `C` or
//! to the link, and delivers both ways. Each test asserts what the generated
//! runtime was observed to do on 2026-10-13 at Moirai d996427 and prints it.
//! These are characterisation tests of the baseline: they passed before any
//! SI.05 implementation and must keep passing, since SI.05 changes no
//! generated runtime.
//!
//! ```bash
//! cd generated/simpleuml_crdt && nice -n 10 cargo test -j 2 --test si05_generated_deletion -- --nocapture
//! ```

use std::collections::BTreeSet;

use moirai_crdt::utils::membership::twins_log;
use moirai_macros::typed_graph::Arc as GraphArc;
use moirai_protocol::broadcast::message::EventMessage;
use moirai_protocol::broadcast::tcsb::Tcsb;
use moirai_protocol::crdt::query::Read;
use moirai_protocol::replica::{IsReplica, Replica};
use serde_json::{Value, json};
use simpleuml_crdt::package::{Simpleuml, SimpleumlLog, SimpleumlValue};
use simpleuml_crdt::references::{
    ClassId, GeneralizationGeneralEdge, GeneralizationId, Instance, Refs,
};

type GenReplica = Replica<SimpleumlLog, Tcsb<Simpleuml>>;

/// `Simpleuml` from its JSON spelling, which is the generator's own naming.
fn op(value: Value) -> Simpleuml {
    serde_json::from_value(value.clone())
        .unwrap_or_else(|error| panic!("not a `Simpleuml` operation: {error}\n{value}"))
}

/// An operation on the root `Model`, wrapped in the root arm's `union!` chain.
fn on_model(payload: Value) -> Simpleuml {
    op(json!({"ModelElementKind": {"Classifier": {"Package": {"Model": payload}}}}))
}

/// An operation on `Model.ownedElements`.
fn on_owned(step: Value) -> Simpleuml {
    on_model(json!({"PackageSuper": {"OwnedElements": step}}))
}

/// An operation on the `Class` at `pos` of `Model.ownedElements`.
fn on_class(pos: usize, payload: Value) -> Simpleuml {
    on_owned(json!({"Update": {"pos": pos, "op": {"TType": {"DataType": {"Class": payload}}}}}))
}

fn new_model() -> Simpleuml {
    on_model(json!("New"))
}

fn insert_class(pos: usize) -> Simpleuml {
    on_owned(json!({"Insert": {"pos": pos, "op": {"TType": {"DataType": {"Class": "New"}}}}}))
}

fn delete_owned(pos: usize) -> Simpleuml {
    on_owned(json!({"Delete": {"pos": pos}}))
}

/// One character into `ModelElement.name` of the class at `pos`: the four
/// `<Super>Super` hops from `Class` up to `ModelElement`.
fn rename_class(pos: usize, ch: char) -> Simpleuml {
    on_class(
        pos,
        json!({"DataTypeSuper": {"TTypeSuper": {"ClassifierSuper": {"ModelElementSuper":
            {"Name": {"Set": {"Insert": {"content": ch, "pos": 0}}}}}}}}),
    )
}

fn insert_generalization(class_pos: usize, pos: usize) -> Simpleuml {
    on_class(
        class_pos,
        json!({"Generalizations": {"Insert": {"pos": pos, "op": "New"}}}),
    )
}

fn link(general: &GeneralizationId, target: &ClassId) -> Simpleuml {
    Simpleuml::AddReference(Refs::GeneralizationToClass(GraphArc {
        source: general.clone(),
        target: target.clone(),
        kind: GeneralizationGeneralEdge,
    }))
}

fn read(replica: &GenReplica) -> SimpleumlValue {
    replica.query(&Read::<SimpleumlValue>::new())
}

fn class_vertices(replica: &GenReplica) -> BTreeSet<String> {
    read(replica)
        .refs
        .node_weights()
        .filter_map(|vertex| match vertex {
            Instance::ClassId(id) => Some(id.0.to_string()),
            _ => None,
        })
        .collect()
}

/// The `G.general` arcs the read-out holds, as `(source, target)` path pairs.
fn general_arcs(replica: &GenReplica) -> Vec<(String, String)> {
    let value = read(replica);
    let graph = &value.refs;
    let mut out: Vec<(String, String)> = graph
        .edge_indices()
        .filter_map(|edge| {
            let (from, to) = graph.edge_endpoints(edge)?;
            match (&graph[from], &graph[to]) {
                (Instance::GeneralizationId(source), Instance::ClassId(target)) => {
                    Some((source.0.to_string(), target.0.to_string()))
                }
                _ => None,
            }
        })
        .collect();
    out.sort();
    out
}

/// How many objects `Model.ownedElements` holds in the containment read-out.
fn owned_count(replica: &GenReplica) -> usize {
    let value = serde_json::to_value(read(replica)).expect("the read-out serializes");
    fn find(value: &Value) -> Option<usize> {
        match value {
            Value::Object(map) => {
                if let Some(Value::Array(items)) = map.get("owned_elements") {
                    return Some(items.len());
                }
                map.values().find_map(find)
            }
            Value::Array(items) => items.iter().find_map(find),
            _ => None,
        }
    }
    find(&value).unwrap_or(0)
}

/// The model of every schedule, built on `a` and delivered to `b`, with the
/// paths of `C` and `G` as the reference graph names them.
struct World {
    a: GenReplica,
    b: GenReplica,
    c: ClassId,
    g: GeneralizationId,
}

fn send(replica: &mut GenReplica, operation: Simpleuml) -> EventMessage<Simpleuml> {
    replica
        .send(operation)
        .unwrap_or_else(|error| panic!("the generated intake refused an operation: {error}"))
}

fn world() -> World {
    let (mut a, mut b) = twins_log::<SimpleumlLog>();
    let mut out = Vec::new();
    out.push(send(&mut a, new_model()));
    let before = class_vertices(&a);
    out.push(send(&mut a, insert_class(0)));
    let after_c = class_vertices(&a);
    let c_path: Vec<String> = after_c.difference(&before).cloned().collect();
    assert_eq!(
        c_path.len(),
        1,
        "creating C adds one Class vertex: {c_path:?}"
    );
    out.push(send(&mut a, insert_class(1)));
    out.push(send(&mut a, insert_generalization(1, 0)));
    for message in out {
        b.receive(message);
    }

    let graph = read(&a).refs;
    let c = graph
        .node_weights()
        .find_map(|vertex| match vertex {
            Instance::ClassId(id) if id.0.to_string() == c_path[0] => Some(id.clone()),
            _ => None,
        })
        .expect("C's vertex");
    let generalizations: Vec<GeneralizationId> = graph
        .node_weights()
        .filter_map(|vertex| match vertex {
            Instance::GeneralizationId(id) => Some(id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(generalizations.len(), 1, "one Generalization vertex");
    assert_eq!(owned_count(&a), 2, "C and D are in Model.ownedElements");
    World {
        a,
        b,
        c,
        g: generalizations[0].clone(),
    }
}

/// Deliver every message both ways.
fn exchange(
    world: &mut World,
    from_a: Vec<EventMessage<Simpleuml>>,
    from_b: Vec<EventMessage<Simpleuml>>,
) {
    for message in from_a {
        world.b.receive(message);
    }
    for message in from_b {
        world.a.receive(message);
    }
}

fn report(label: &str, world: &World) -> (bool, bool, usize) {
    let c = world.c.0.to_string();
    let mut seen = None;
    for (name, replica) in [("a", &world.a), ("b", &world.b)] {
        let vertex = class_vertices(replica).contains(&c);
        let arcs = general_arcs(replica);
        let linked = arcs.iter().any(|(_, target)| *target == c);
        let owned = owned_count(replica);
        println!(
            "SI.05 decision 2, {label}, replica {name}: C's vertex in the graph = {vertex}, \
             G.general -> C readable = {linked}, Model.ownedElements holds {owned} (2 = C kept)"
        );
        assert_eq!(
            vertex,
            owned == 2,
            "{label}, replica {name}: C's vertex is in the reference graph exactly when C is in \
             the containment"
        );
        let here = (vertex, linked, owned);
        if let Some(there) = seen {
            assert_eq!(there, here, "{label}: the two generated replicas disagree");
        }
        seen = Some(here);
    }
    seen.expect("two replicas")
}

/// Q1, the schedule decision 2 names: the link exists and is delivered, then
/// `a` deletes `C` while `b` renames it. `C` survives, with its vertex, and
/// the link does not come back.
#[test]
fn q1_an_update_concurrent_with_the_deletion_keeps_c_and_its_vertex_but_not_the_link() {
    let mut world = world();
    let linked = send(&mut world.a, link(&world.g, &world.c));
    world.b.receive(linked);
    assert_eq!(
        general_arcs(&world.b).len(),
        1,
        "the link is readable before the partition"
    );

    let delete = send(&mut world.a, delete_owned(0));
    let rename = send(&mut world.b, rename_class(0, 'x'));
    exchange(&mut world, vec![delete], vec![rename]);

    let (vertex, linked, owned) = report("Q1 link, then delete || rename", &world);
    assert_eq!(
        owned, 2,
        "the rename concurrent with the deletion keeps C in the containment (update-wins)"
    );
    assert!(vertex, "and C's vertex in the reference graph");
    assert!(
        !linked,
        "the link the deletion saw is not readable again, although C survives"
    );
}

/// Q2: the link is made concurrently with the deletion, by the replica that
/// also renames `C`; `C` survives and the link is readable.
#[test]
fn q2_a_link_and_an_update_concurrent_with_the_deletion_stay_readable() {
    let mut world = world();
    let delete = send(&mut world.a, delete_owned(0));
    let rename = send(&mut world.b, rename_class(0, 'x'));
    let linked = send(&mut world.b, link(&world.g, &world.c));
    exchange(&mut world, vec![delete], vec![rename, linked]);

    let (vertex, linked, owned) = report("Q2 delete || (rename, link)", &world);
    assert_eq!(owned, 2, "the concurrent rename keeps C in the containment");
    assert!(vertex, "and C's vertex in the reference graph");
    assert!(
        linked,
        "the link concurrent with the deletion is readable, and its target is alive"
    );
}

/// Q3: the same concurrent link with no update of `C` beside it.
#[test]
fn q3_a_link_concurrent_with_the_deletion_alone_is_not_readable() {
    let mut world = world();
    let delete = send(&mut world.a, delete_owned(0));
    let linked = send(&mut world.b, link(&world.g, &world.c));
    exchange(&mut world, vec![delete], vec![linked]);

    let (vertex, linked, owned) = report("Q3 delete || link", &world);
    assert_eq!(owned, 1, "C is gone from the containment on both replicas");
    assert!(!vertex, "no update kept C's vertex");
    assert!(!linked, "the concurrent link is not readable");
}

/// Q4: after Q1's merge, a new link to the surviving `C` is enabled, because
/// the reference graph holds its vertex again, and it is readable.
#[test]
fn q4_after_q1_a_new_link_to_the_surviving_target_is_accepted_and_readable() {
    let mut world = world();
    let linked = send(&mut world.a, link(&world.g, &world.c));
    world.b.receive(linked);
    let delete = send(&mut world.a, delete_owned(0));
    let rename = send(&mut world.b, rename_class(0, 'x'));
    exchange(&mut world, vec![delete], vec![rename]);

    let relink = world.a.send(link(&world.g, &world.c));
    let accepted = relink.is_ok();
    if let Ok(message) = relink {
        world.b.receive(message);
    }
    println!(
        "SI.05 decision 2, Q4 after Q1, a links G.general -> the surviving C: accepted = {accepted}"
    );
    let (_, linked, owned) = report("Q4 after Q1, relink", &world);
    assert_eq!(owned, 2, "C survived Q1's deletion");
    assert!(
        accepted,
        "the generated intake accepts a new link to the surviving C"
    );
    assert!(linked, "and the link is readable");
}

// ---------------------------------------------------------------------------
// The interpreted path on Q1's containment half
// ---------------------------------------------------------------------------

/// Q1's containment half on the interpreted `ModelLog`, over this crate's own
/// descriptor: `a` deletes `C` while `b` renames it. The interpreted
/// containment is update-wins too, so `C` comes back there as it does on the
/// generated side: the revival Q1 shows is the containment's rule, shared by
/// both paths, and not something the reference graph adds.
#[test]
fn q1_interp_the_interpreted_containment_revives_c_the_same_way() {
    use moirai_interp::testing::opened;
    use moirai_interp::{InstanceOp, LeafOp, ModelOp};
    use moirai_semantics::{ClassSlot, FeatureSlot, MetamodelSemantics, from_descriptor};

    let descriptor: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/metamodel.json"))
            .expect("the generated crate ships its descriptor"),
    )
    .expect("the descriptor is JSON");
    let sem: MetamodelSemantics = from_descriptor(&descriptor).expect("the descriptor parses");
    let class = |name: &str| -> ClassSlot {
        sem.classes
            .iter()
            .find(|class| &*class.name == name)
            .unwrap_or_else(|| panic!("no class {name}"))
            .slot
    };
    let feature = |owner: &str, name: &str| -> FeatureSlot {
        let at = sem.classes[class(owner).index()]
            .visible
            .iter()
            .position(|(feature, _, _)| &**feature == name)
            .unwrap_or_else(|| panic!("no feature {owner}.{name}"));
        FeatureSlot(at as u16)
    };
    let model = class("Model");
    let owned = feature("Model", "ownedElements");
    let on_model = |inner: InstanceOp| ModelOp::Instance(InstanceOp::variant(model, inner));
    let insert = |pos: usize| {
        on_model(InstanceOp::field(
            owned,
            InstanceOp::insert(pos, InstanceOp::variant(class("Class"), InstanceOp::New)),
        ))
    };
    let rename = |pos: usize, ch: char| {
        on_model(InstanceOp::field(
            owned,
            InstanceOp::at(
                pos,
                InstanceOp::variant(
                    class("Class"),
                    InstanceOp::field(
                        feature("Class", "name"),
                        InstanceOp::set(InstanceOp::Leaf(LeafOp::InsertChar { pos: 0, ch })),
                    ),
                ),
            ),
        ))
    };
    let delete = |pos: usize| on_model(InstanceOp::field(owned, InstanceOp::delete(pos)));

    let (mut a, mut b) = opened("si05-q1", &descriptor);
    for operation in [on_model(InstanceOp::New), insert(0), insert(1)] {
        let message = a.send(operation).expect("the interpreted intake takes it");
        b.receive(message);
    }
    let deleted = a.send(delete(0)).expect("a deletes C");
    let renamed = b.send(rename(0, 'x')).expect("b renames C");
    b.receive(deleted);
    a.receive(renamed);

    for (name, replica) in [("a", &a), ("b", &b)] {
        let doc: Value = replica.query(&Read::<Value>::new());
        let owned = doc["ownedElements"].as_array().map_or(0, Vec::len);
        println!(
            "SI.05 decision 2, Q1 on the interpreted path, replica {name}: \
             ownedElements holds {owned}: {}",
            doc["ownedElements"]
        );
        assert_eq!(
            owned, 2,
            "replica {name}: the rename concurrent with the deletion keeps C, as on the \
             generated path"
        );
    }
}
