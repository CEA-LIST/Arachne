/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __references {
    pub use moirai_macros::typed_graph;
    pub use moirai_protocol::state::object_path::ObjectPath;
}
pub fn instance_from_sink_kind(
    kind: &str,
    path: &__references::ObjectPath,
) -> Option<Instance> {
    match kind {
        "Class" => Some(Instance::ClassId(ClassId(path.clone()))),
        "Relation" => Some(Instance::RelationId(RelationId(path.clone()))),
        _ => None,
    }
}
pub fn instance_path(instance: &Instance) -> &__references::ObjectPath {
    match instance {
        Instance::ClassId(id) => &id.0,
        Instance::RelationId(id) => &id.0,
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelationSourceEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelationTargetEdge;
__references::typed_graph! {
    types { graph = ReferenceManager, vertex_kind = Instance, edge_kind = Ref, arc_kind =
    Refs, }, vertices { ClassId, RelationId }, edges { RelationSourceEdge[0, 1],
    RelationTargetEdge[0, 1] }, arcs { RelationToClass : RelationId ->
    ClassId(RelationSourceEdge), RelationToClass2 : RelationId ->
    ClassId(RelationTargetEdge) }
}
