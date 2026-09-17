//! The crate generated from `examples/pet_metamodels/ecore_builtins.ecore`, driven on two replicas.
//!
//! `Part` extends `Element`, which extends Ecore's `EModelElement`, so a part carries annotations:
//! a `source` and `details` as a map from a key to an optional text. The two features that refer to
//! an object of any class, `EAnnotation.references` and `Part.subject`, are both typed by `EObject`
//! and are not generated; `Port.annotated`, typed by `EModelElement`, is. Every test builds a model
//! with one part, holding one port and one annotation, and checks that both replicas read the same
//! thing.

use std::collections::{BTreeMap, BTreeSet};

use annotated::{
    classifiers::{EcoreEAnnotation, EcoreEModelElement, Element, Model, Part, Port},
    package::{Annotated, AnnotatedLog, AnnotatedValue},
    references::{
        EcoreEAnnotationId, Instance, PartId, PortAnnotatedEdge, PortId, Ref, Refs, instance_path,
    },
};
use moirai_crdt::{
    list::{eg_walker::List, nested_list::NestedList},
    map::uw_map::UWMap,
    option::Optional,
    register::mv_register::MVRegister,
    utils::membership::twins_log,
};
use moirai_macros::typed_graph::Arc;
use moirai_protocol::{crdt::query::Read, replica::IsReplica, state::object_path::ObjectPath};
use petgraph::visit::EdgeRef;

/// Sends `op` from `from` and delivers it to `to`.
fn deliver<R: IsReplica<AnnotatedLog>>(from: &mut R, to: &mut R, op: Annotated) {
    let event = from
        .send(op)
        .unwrap_or_else(|rejection| panic!("operation rejected: {rejection:?}"));
    to.receive(event);
}

fn read<R: IsReplica<AnnotatedLog>>(replica: &R) -> AnnotatedValue {
    replica.query(Read::new())
}

/// An operation on the first part of the model.
fn on_part(op: Part) -> Annotated {
    Annotated::Model(Model::Parts(NestedList::Update { pos: 0, op }))
}

/// An operation on the features the first part inherits from `EModelElement`.
fn on_model_element(op: EcoreEModelElement) -> Annotated {
    on_part(Part::ElementSuper(Element::EModelElementSuper(op)))
}

/// An operation on the first annotation of the first part.
fn on_annotation(op: EcoreEAnnotation) -> Annotated {
    on_model_element(EcoreEModelElement::EAnnotations(NestedList::Update {
        pos: 0,
        op: op.into(),
    }))
}

fn put(key: &str, value: Option<&str>) -> Annotated {
    on_annotation(EcoreEAnnotation::Details(UWMap::Update(
        key.to_string(),
        MVRegister::Write(value.map(str::to_string)),
    )))
}

fn remove(key: &str) -> Annotated {
    on_annotation(EcoreEAnnotation::Details(UWMap::Remove(key.to_string())))
}

/// Two replicas holding a model with one part, which holds one port and one annotation.
fn replicas<R: IsReplica<AnnotatedLog>>(a: &mut R, b: &mut R) {
    deliver(
        a,
        b,
        Annotated::Model(Model::Parts(NestedList::Insert {
            pos: 0,
            op: Part::New,
        })),
    );
    deliver(
        a,
        b,
        on_part(Part::Ports(NestedList::Insert {
            pos: 0,
            op: Port::New,
        })),
    );
    deliver(
        a,
        b,
        on_model_element(EcoreEModelElement::EAnnotations(NestedList::Insert {
            pos: 0,
            op: EcoreEAnnotation::New.into(),
        })),
    );
}

/// The `source` of every annotation of the first part.
fn sources(value: &AnnotatedValue) -> Vec<Option<String>> {
    value.model.parts[0]
        .element_super
        .e_model_element_super
        .e_annotations
        .iter()
        .map(|annotation| {
            annotation
                .source
                .as_ref()
                .map(|chars| chars.iter().collect())
        })
        .collect()
}

/// The `details` of the first annotation of the first part, with the values of each key.
fn details(value: &AnnotatedValue) -> BTreeMap<String, BTreeSet<Option<String>>> {
    value.model.parts[0]
        .element_super
        .e_model_element_super
        .e_annotations[0]
        .details
        .iter()
        .map(|(key, values)| (key.clone(), values.iter().cloned().collect()))
        .collect()
}

fn values(values: &[Option<&str>]) -> BTreeSet<Option<String>> {
    values
        .iter()
        .map(|value| value.map(str::to_string))
        .collect()
}

/// A vertex as its kind and path. The path is displayed, not debugged: the debug form of an event
/// id holds the index of its replica in the local replica's own table, which differs between
/// replicas.
fn vertex(instance: &Instance) -> String {
    let debug = format!("{instance:?}");
    let kind = debug.split('(').next().unwrap_or_default();
    format!("{kind} {}", instance_path(instance))
}

fn arc(kind: Ref, source: &Instance, target: &Instance) -> String {
    format!("{kind:?} {} -> {}", vertex(source), vertex(target))
}

/// Every arc of the reference manager, as its kind, source and target.
fn arcs(value: &AnnotatedValue) -> BTreeSet<String> {
    value
        .refs
        .edge_references()
        .map(|edge| {
            arc(
                edge.weight().clone(),
                &value.refs[edge.source()],
                &value.refs[edge.target()],
            )
        })
        .collect()
}

/// The path of the only vertex `select` picks.
fn vertex_path(value: &AnnotatedValue, select: impl Fn(&Instance) -> Option<&ObjectPath>) -> ObjectPath {
    let paths: Vec<_> = value.refs.node_weights().filter_map(|v| select(v)).collect();
    assert_eq!(paths.len(), 1, "expected one such vertex: {paths:?}");
    paths[0].clone()
}

#[test]
fn an_annotation_and_its_source_reach_the_other_replica() {
    let (mut a, mut b) = twins_log::<AnnotatedLog>();
    replicas(&mut a, &mut b);

    for (pos, content) in "sysml".chars().enumerate() {
        deliver(
            &mut a,
            &mut b,
            on_annotation(EcoreEAnnotation::Source(Optional::Set(List::Insert {
                content,
                pos,
            }))),
        );
    }

    for value in [read(&a), read(&b)] {
        assert_eq!(sources(&value), [Some("sysml".to_string())]);
    }
    assert_eq!(read(&a).model, read(&b).model);
}

#[test]
fn concurrent_puts_on_one_key_converge_to_one_entry() {
    let (mut a, mut b) = twins_log::<AnnotatedLog>();
    replicas(&mut a, &mut b);

    let from_a = a.send(put("keyword", Some("x"))).unwrap();
    let from_b = b.send(put("keyword", Some("y"))).unwrap();
    a.receive(from_b);
    b.receive(from_a);

    for value in [read(&a), read(&b)] {
        let details = details(&value);
        assert_eq!(details.len(), 1, "{details:?}");
        assert_eq!(details["keyword"], values(&[Some("x"), Some("y")]));
    }
    assert_eq!(read(&a).model, read(&b).model);
}

#[test]
fn concurrent_puts_on_different_keys_are_both_kept() {
    let (mut a, mut b) = twins_log::<AnnotatedLog>();
    replicas(&mut a, &mut b);

    let from_a = a.send(put("first", Some("x"))).unwrap();
    let from_b = b.send(put("second", Some("y"))).unwrap();
    a.receive(from_b);
    b.receive(from_a);

    for value in [read(&a), read(&b)] {
        assert_eq!(
            details(&value),
            BTreeMap::from([
                ("first".to_string(), values(&[Some("x")])),
                ("second".to_string(), values(&[Some("y")])),
            ])
        );
    }
    assert_eq!(read(&a).model, read(&b).model);
}

#[test]
fn a_removed_key_is_gone_and_loses_to_a_concurrent_put() {
    let (mut a, mut b) = twins_log::<AnnotatedLog>();
    replicas(&mut a, &mut b);
    deliver(&mut a, &mut b, put("kept", Some("x")));
    deliver(&mut a, &mut b, put("contested", Some("x")));

    deliver(&mut a, &mut b, remove("kept"));
    for value in [read(&a), read(&b)] {
        assert!(!details(&value).contains_key("kept"), "{:?}", details(&value));
    }

    let from_a = a.send(remove("contested")).unwrap();
    let from_b = b.send(put("contested", Some("y"))).unwrap();
    a.receive(from_b);
    b.receive(from_a);

    for value in [read(&a), read(&b)] {
        assert_eq!(
            details(&value),
            BTreeMap::from([("contested".to_string(), values(&[Some("y")]))])
        );
    }
    assert_eq!(read(&a).model, read(&b).model);
}

#[test]
fn a_key_without_a_value_round_trips() {
    let (mut a, mut b) = twins_log::<AnnotatedLog>();
    replicas(&mut a, &mut b);

    deliver(&mut a, &mut b, put("keyword", None));
    deliver(&mut a, &mut b, put("other", Some("x")));
    deliver(&mut b, &mut a, put("other", None));

    for value in [read(&a), read(&b)] {
        assert_eq!(
            details(&value),
            BTreeMap::from([
                ("keyword".to_string(), values(&[None])),
                ("other".to_string(), values(&[None])),
            ])
        );
    }
    assert_eq!(read(&a).model, read(&b).model);
}

/// Nothing is generated for `Part.subject` or `EAnnotation.references`, the two features typed by
/// `EObject`: the generated sources hold no vertex kind, edge or arc for them, no object is given a
/// second vertex to stand for an object of any class, and `Port.annotated`, typed by
/// `EModelElement`, is generated and stored as any reference is.
#[test]
fn a_reference_to_an_object_of_any_class_is_not_generated() {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let references = std::fs::read_to_string(crate_dir.join("src/references.rs"))
        .expect("reading the generated references module");
    let package = std::fs::read_to_string(crate_dir.join("src/package.rs"))
        .expect("reading the generated package module");

    for absent in ["EcoreEObject", "Subject", "References", "object_from_sink_kind"] {
        assert!(
            !references.contains(absent),
            "`references.rs` should not mention `{absent}`"
        );
    }
    assert!(!package.contains("object_from_sink_kind"));
    assert!(references.contains("PortAnnotatedEdge"));

    let (mut a, mut b) = twins_log::<AnnotatedLog>();
    replicas(&mut a, &mut b);

    let value = read(&a);
    let part = vertex_path(&value, |v| match v {
        Instance::PartId(PartId(path)) => Some(path),
        _ => None,
    });
    let port = vertex_path(&value, |v| match v {
        Instance::PortId(PortId(path)) => Some(path),
        _ => None,
    });
    let annotation = vertex_path(&value, |v| match v {
        Instance::EcoreEAnnotationId(EcoreEAnnotationId(path)) => Some(path),
        _ => None,
    });
    assert_eq!(
        value.refs.node_weights().map(vertex).collect::<BTreeSet<_>>(),
        BTreeSet::from([
            vertex(&Instance::PartId(PartId(part.clone()))),
            vertex(&Instance::PortId(PortId(port.clone()))),
            vertex(&Instance::EcoreEAnnotationId(EcoreEAnnotationId(
                annotation.clone()
            ))),
        ])
    );

    // `Port.annotated` refers to the part.
    deliver(
        &mut a,
        &mut b,
        Annotated::AddReference(Refs::PortToPart(Arc {
            source: PortId(port.clone()),
            target: PartId(part.clone()),
            kind: PortAnnotatedEdge,
        })),
    );

    let expected = BTreeSet::from([arc(
        Ref::PortToPart(PortAnnotatedEdge),
        &Instance::PortId(PortId(port.clone())),
        &Instance::PartId(PartId(part.clone())),
    )]);
    assert_eq!(arcs(&read(&a)), expected);
    assert_eq!(arcs(&read(&b)), expected);
}
