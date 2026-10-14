/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_protocol::state::graph_log::GraphLog;
    pub use moirai_crdt::option::OptionLog;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_crdt::list::nested_list::NestedListLog;
    pub use moirai_crdt::map::uw_map::UWMapLog;
    pub use moirai_macros::HashMap;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_crdt::counter::resettable_counter::Counter;
}
__classifiers::record!(
    Board { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, cards :
    __classifiers::NestedListLog < CardLog > => Vec < CardValue >, slots :
    __classifiers::UWMapLog < std::string::String, CardLog > => __classifiers::HashMap <
    std::string::String, CardValue >, }
);
__classifiers::record!(
    Card { title : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, rank :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::Counter < i32 > >>
    => Option < i32 >, }
);
