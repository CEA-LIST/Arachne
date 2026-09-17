/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_protocol::state::graph_log::GraphLog;
    pub use moirai_crdt::option::OptionLog;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_crdt::register::unique_register::LwwRegister;
    pub use moirai_crdt::register::unique_register::FairRegister;
    pub use moirai_crdt::register::po_register::PORegister;
    pub use moirai_macros::HashSet;
    pub use moirai_crdt::register::to_register::TORegister;
    pub use moirai_crdt::flag::dw_flag::DWFlag;
    pub use moirai_crdt::register::mv_register::MVRegister;
    pub use moirai_crdt::set::aw_set::AWSet;
    pub use moirai_crdt::set::rw_set::RWSet;
}
__classifiers::record!(
    Class { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, qualified_name :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::LwwRegister <
    std::string::String > >> => Option < Option < std::string::String > >, author :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::FairRegister <
    std::string::String > >> => Option < Option < std::string::String > >, stereotype :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::PORegister <
    std::string::String > >> => Option < __classifiers::HashSet < std::string::String >
    >, layer : __classifiers::OptionLog < __classifiers::VecLog <
    __classifiers::TORegister < std::string::String > >> => Option < Option <
    std::string::String > >, is_abstract : __classifiers::OptionLog <
    __classifiers::VecLog < __classifiers::DWFlag >> => Option < bool >, visibility :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::MVRegister <
    Visibility > >> => Option < __classifiers::HashSet < Visibility > >, tags :
    __classifiers::VecLog < __classifiers::AWSet < std::string::String >> =>
    __classifiers::HashSet < std::string::String >, invariants : __classifiers::VecLog <
    __classifiers::RWSet < std::string::String >> => __classifiers::HashSet <
    std::string::String >, }
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
    Feature { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, typ :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::MVRegister <
    PrimitiveType > >> => Option < __classifiers::HashSet < PrimitiveType > >, visibility
    : __classifiers::OptionLog < __classifiers::VecLog < __classifiers::TORegister <
    Visibility > >> => Option < Option < Visibility > >, }
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
    Relation { label : __classifiers::OptionLog < __classifiers::VecLog <
    __classifiers::MVRegister < std::string::String > >> => Option <
    __classifiers::HashSet < std::string::String > >, typ : __classifiers::OptionLog <
    __classifiers::VecLog < __classifiers::TORegister < RelationType > >> => Option <
    Option < RelationType > >, }
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
