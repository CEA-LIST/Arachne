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
        [.., __references::Variant("package")] => {
            Some(Instance::PackageId(PackageId(path.clone())))
        }
        [.., __references::Variant("model")] => {
            Some(Instance::ModelId(ModelId(path.clone())))
        }
        [.., __references::Field("packagesuper")] => {
            Some(Instance::PackageId(PackageId(path.clone())))
        }
        [.., __references::Variant("datatype")] => {
            Some(Instance::DataTypeId(DataTypeId(path.clone())))
        }
        [.., __references::Field("attributes"), __references::ListElement(_)] => {
            Some(Instance::PropertyId(PropertyId(path.clone())))
        }
        [.., __references::Variant("class")] => {
            Some(Instance::ClassId(ClassId(path.clone())))
        }
        [.., __references::Field("generalizations"), __references::ListElement(_)] => {
            Some(Instance::GeneralizationId(GeneralizationId(path.clone())))
        }
        [.., __references::Field("datatypesuper")] => {
            Some(Instance::DataTypeId(DataTypeId(path.clone())))
        }
        [.., __references::Variant("primitivetype")] => {
            Some(Instance::PrimitiveTypeId(PrimitiveTypeId(path.clone())))
        }
        [.., __references::Variant("enumeration")] => {
            Some(Instance::EnumerationId(EnumerationId(path.clone())))
        }
        [.., __references::Variant("association")] => {
            Some(Instance::AssociationId(AssociationId(path.clone())))
        }
        [.., __references::Variant("property")] => {
            Some(Instance::PropertyId(PropertyId(path.clone())))
        }
        _ => None,
    }
}
pub fn instance_path(instance: &Instance) -> &__references::ObjectPath {
    match instance {
        Instance::ModelId(id) => &id.0,
        Instance::PackageId(id) => &id.0,
        Instance::ClassId(id) => &id.0,
        Instance::GeneralizationId(id) => &id.0,
        Instance::DataTypeId(id) => &id.0,
        Instance::PropertyId(id) => &id.0,
        Instance::AssociationId(id) => &id.0,
        Instance::PrimitiveTypeId(id) => &id.0,
        Instance::EnumerationId(id) => &id.0,
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModelOwnerEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackageOwnerEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClassOwnerEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeneralizationGeneralEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DataTypeOwnerEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PropertyOwnerEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PropertyTtypeEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssociationOwnerEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssociationSourceEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssociationTargetEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimitiveTypeOwnerEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnumerationOwnerEdge;
__references::typed_graph! {
    types { graph = ReferenceManager, vertex_kind = Instance, edge_kind = Ref, arc_kind =
    Refs, }, vertices { ModelId, PackageId, ClassId, GeneralizationId, DataTypeId,
    PropertyId, AssociationId, PrimitiveTypeId, EnumerationId }, edges {
    ModelOwnerEdge[0, 1], PackageOwnerEdge[0, 1], ClassOwnerEdge[0, 1],
    GeneralizationGeneralEdge[0, 1], DataTypeOwnerEdge[0, 1], PropertyOwnerEdge[1, 1],
    PropertyTtypeEdge[1, 1], AssociationOwnerEdge[0, 1], AssociationSourceEdge[1, 1],
    AssociationTargetEdge[1, 1], PrimitiveTypeOwnerEdge[0, 1], EnumerationOwnerEdge[0, 1]
    }, arcs { ModelToModel : ModelId -> ModelId(ModelOwnerEdge), ModelToPackage : ModelId
    -> PackageId(ModelOwnerEdge), PackageToModel : PackageId ->
    ModelId(PackageOwnerEdge), PackageToPackage : PackageId ->
    PackageId(PackageOwnerEdge), ClassToModel : ClassId -> ModelId(ClassOwnerEdge),
    ClassToPackage : ClassId -> PackageId(ClassOwnerEdge), GeneralizationToClass :
    GeneralizationId -> ClassId(GeneralizationGeneralEdge), DataTypeToModel : DataTypeId
    -> ModelId(DataTypeOwnerEdge), DataTypeToPackage : DataTypeId ->
    PackageId(DataTypeOwnerEdge), PropertyToClass : PropertyId ->
    ClassId(PropertyOwnerEdge), PropertyToClass2 : PropertyId ->
    ClassId(PropertyTtypeEdge), PropertyToDataType : PropertyId ->
    DataTypeId(PropertyOwnerEdge), PropertyToDataType2 : PropertyId ->
    DataTypeId(PropertyTtypeEdge), PropertyToPrimitiveType : PropertyId ->
    PrimitiveTypeId(PropertyTtypeEdge), PropertyToEnumeration : PropertyId ->
    EnumerationId(PropertyTtypeEdge), AssociationToModel : AssociationId ->
    ModelId(AssociationOwnerEdge), AssociationToPackage : AssociationId ->
    PackageId(AssociationOwnerEdge), AssociationToClass : AssociationId ->
    ClassId(AssociationSourceEdge), AssociationToClass2 : AssociationId ->
    ClassId(AssociationTargetEdge), PrimitiveTypeToModel : PrimitiveTypeId ->
    ModelId(PrimitiveTypeOwnerEdge), PrimitiveTypeToPackage : PrimitiveTypeId ->
    PackageId(PrimitiveTypeOwnerEdge), EnumerationToModel : EnumerationId ->
    ModelId(EnumerationOwnerEdge), EnumerationToPackage : EnumerationId ->
    PackageId(EnumerationOwnerEdge) }
}
