/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_protocol::state::event_graph::EventGraph;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_crdt::register::unique_register::LwwRegister;
    pub use moirai_crdt::register::unique_register::FairRegister;
    pub use moirai_crdt::register::po_register::PORegister;
    pub use moirai_crdt::register::to_register::TORegister;
    pub use moirai_crdt::flag::dw_flag::DWFlag;
    pub use moirai_crdt::register::mv_register::MVRegister;
    pub use moirai_crdt::set::aw_set::AWSet;
    pub use moirai_crdt::set::rw_set::RWSet;
}
__classifiers::record!(
    Class { name : __classifiers::EventGraph < __classifiers::List < char > >,
    qualified_name : __classifiers::VecLog < __classifiers::LwwRegister <
    std::string::String > >, author : __classifiers::VecLog < __classifiers::FairRegister
    < std::string::String > >, stereotype : __classifiers::VecLog <
    __classifiers::PORegister < std::string::String > >, layer : __classifiers::VecLog <
    __classifiers::TORegister < std::string::String > >, is_abstract :
    __classifiers::VecLog < __classifiers::DWFlag >, visibility : __classifiers::VecLog <
    __classifiers::MVRegister < Visibility > >, tags : __classifiers::VecLog <
    __classifiers::AWSet < std::string::String >>, invariants : __classifiers::VecLog <
    __classifiers::RWSet < std::string::String >>, }
);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Visibility {
    #[default]
    Public,
    Protected,
    Package,
    Private,
}
__classifiers::record!(
    Feature { name : __classifiers::EventGraph < __classifiers::List < char > >, typ :
    __classifiers::VecLog < __classifiers::MVRegister < PrimitiveType > >, visibility :
    __classifiers::VecLog < __classifiers::TORegister < Visibility > >, }
);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PrimitiveType {
    #[default]
    String,
    Number,
    Boolean,
    Void,
}
__classifiers::record!(
    Relation { label : __classifiers::VecLog < __classifiers::MVRegister <
    std::string::String > >, typ : __classifiers::VecLog < __classifiers::TORegister <
    RelationType > >, }
);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RelationType {
    #[default]
    Associates,
    Aggregates,
    Composes,
    Implements,
    Extends,
}
