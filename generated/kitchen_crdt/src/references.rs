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
        [.., __references::Field("foo")] => Some(Instance::FooId(FooId(path.clone()))),
        [.., __references::Field("bar")] => Some(Instance::BarId(BarId(path.clone()))),
        [.., __references::Field("baz")] => Some(Instance::BazId(BazId(path.clone()))),
        _ => None,
    }
}
pub fn instance_path(instance: &Instance) -> &__references::ObjectPath {
    match instance {
        Instance::FooId(id) => &id.0,
        Instance::BarId(id) => &id.0,
        Instance::BazId(id) => &id.0,
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BarMyFooEdge;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BazFooEdge;
__references::typed_graph! {
    types { graph = ReferenceManager, vertex_kind = Instance, edge_kind = Ref, arc_kind =
    Refs, }, vertices { FooId, BarId, BazId }, edges { BarMyFooEdge[0, 1], BazFooEdge[1,
    1] }, arcs { BarToFoo : BarId -> FooId(BarMyFooEdge), BazToFoo : BazId ->
    FooId(BazFooEdge) }
}
