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
        "Attribute" => Some(Instance::AttributeId(AttributeId(path.clone()))),
        "Reference" => Some(Instance::ReferenceId(ReferenceId(path.clone()))),
        "Class" => Some(Instance::ClassId(ClassId(path.clone()))),
        "DataType" => Some(Instance::DataTypeId(DataTypeId(path.clone()))),
        _ => None,
    }
}
pub fn instance_path(instance: &Instance) -> &__references::ObjectPath {
    match instance {
        Instance::AttributeId(id) => &id.0,
        Instance::ReferenceId(id) => &id.0,
        Instance::ClassId(id) => &id.0,
        Instance::DataTypeId(id) => &id.0,
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AttributeTypEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReferenceOppositeEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReferenceTypEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClassSupertypesEdge;
__references::typed_graph! {
    types { graph = ReferenceManager, vertex_kind = Instance, edge_kind = Ref, arc_kind =
    Refs, }, vertices { AttributeId, ReferenceId, ClassId, DataTypeId }, edges {
    AttributeTypEdge[1, 1], ReferenceOppositeEdge[0, 1], ReferenceTypEdge[1, 1],
    ClassSupertypesEdge[0, *] }, arcs { AttributeToClass : AttributeId ->
    ClassId(AttributeTypEdge), AttributeToDataType : AttributeId ->
    DataTypeId(AttributeTypEdge), ReferenceToReference : ReferenceId ->
    ReferenceId(ReferenceOppositeEdge), ReferenceToClass : ReferenceId ->
    ClassId(ReferenceTypEdge), ReferenceToDataType : ReferenceId ->
    DataTypeId(ReferenceTypEdge), ClassToClass : ClassId -> ClassId(ClassSupertypesEdge)
    }
}
