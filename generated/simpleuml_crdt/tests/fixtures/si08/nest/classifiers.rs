/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_crdt::list::nested_list::NestedListLog;
    pub use moirai_crdt::option::OptionLog;
    pub use moirai_crdt::map::uw_map::UWMapLog;
    pub use moirai_macros::HashMap;
    pub use moirai_protocol::state::graph_log::GraphLog;
    pub use moirai_crdt::list::eg_walker::List;
}
__classifiers::record!(
    Board { cards : __classifiers::NestedListLog < CardLog > => Vec < CardValue >, pinned
    : __classifiers::OptionLog < FrameLog > => Option < FrameValue >, frames :
    __classifiers::UWMapLog < std::string::String, FrameLog > => __classifiers::HashMap <
    std::string::String, FrameValue >, }
);
__classifiers::record!(
    Card { title : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, }
);
__classifiers::record!(
    Frame { label : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, head : CardLog =>
    CardValue, extra : __classifiers::OptionLog < CardLog > => Option < CardValue >,
    slots : __classifiers::UWMapLog < std::string::String, CardLog > =>
    __classifiers::HashMap < std::string::String, CardValue >, }
);
