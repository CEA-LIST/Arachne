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
        "Board" => Some(Instance::BoardId(BoardId(path.clone()))),
        "Card" => Some(Instance::CardId(CardId(path.clone()))),
        "Lane" => Some(Instance::LaneId(LaneId(path.clone()))),
        _ => None,
    }
}
pub fn instance_path(instance: &Instance) -> &__references::ObjectPath {
    match instance {
        Instance::BoardId(id) => &id.0,
        Instance::CardId(id) => &id.0,
        Instance::LaneId(id) => &id.0,
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CardBlockerEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CardRelatedEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LaneBoardEdge;
__references::typed_graph! {
    types { graph = ReferenceManager, vertex_kind = Instance, edge_kind = Ref, arc_kind =
    Refs, }, vertices { BoardId, CardId, LaneId }, edges { CardBlockerEdge[0, 1],
    CardRelatedEdge[0, *], LaneBoardEdge[0, 1] }, arcs { CardToCard : CardId ->
    CardId(CardBlockerEdge), CardToCard2 : CardId -> CardId(CardRelatedEdge), LaneToBoard
    : LaneId -> BoardId(LaneBoardEdge) }
}
