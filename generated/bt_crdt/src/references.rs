/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __references {
    pub use moirai_macros::typed_graph;
    pub use moirai_protocol::state::object_path::ObjectPath;
    pub use moirai_protocol::state::object_path::PathSegment::{
        Field, ListElement, MapEntry, Variant,
    };
}
pub fn instance_from_path(path: &__references::ObjectPath) -> Option<Instance> {
    let segs = path.segments();
    match segs {
        [.., __references::Field("outflowports"), __references::ListElement(_)] => {
            Some(Instance::OutFlowPortId(OutFlowPortId(path.clone())))
        }
        [.., __references::Field("inflowports"), __references::ListElement(_)] => {
            Some(Instance::InFlowPortId(InFlowPortId(path.clone())))
        }
        [.., __references::Field("entries"), __references::ListElement(_)] => {
            Some(Instance::BlackboardEntryId(BlackboardEntryId(path.clone())))
        }
        _ => None,
    }
}
pub fn instance_path(instance: &Instance) -> &__references::ObjectPath {
    match instance {
        Instance::OutFlowPortId(id) => &id.0,
        Instance::InFlowPortId(id) => &id.0,
        Instance::BlackboardEntryId(id) => &id.0,
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OutFlowPortEntryEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InFlowPortEntryEdge;
__references::typed_graph! {
    types { graph = ReferenceManager, vertex_kind = Instance, edge_kind = Ref, arc_kind =
    Refs, }, vertices { OutFlowPortId, InFlowPortId, BlackboardEntryId }, edges {
    OutFlowPortEntryEdge[0, 1], InFlowPortEntryEdge[0, 1] }, arcs {
    OutFlowPortToBlackboardEntry : OutFlowPortId ->
    BlackboardEntryId(OutFlowPortEntryEdge), InFlowPortToBlackboardEntry : InFlowPortId
    -> BlackboardEntryId(InFlowPortEntryEdge) }
}
