/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_protocol::state::graph_log::GraphLog;
    pub use moirai_crdt::option::OptionLog;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_crdt::list::nested_list::NestedListLog;
}
__classifiers::record!(
    Board { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, cards :
    __classifiers::NestedListLog < CardLog > => Vec < CardValue >, lanes :
    __classifiers::NestedListLog < LaneLog > => Vec < LaneValue >, pinned :
    __classifiers::OptionLog < CardLog > => Option < CardValue >, }
);
__classifiers::record!(
    Card { title : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, note :
    __classifiers::OptionLog < __classifiers::GraphLog < __classifiers::List < char > >>
    => Option < Vec < char > >, }
);
__classifiers::record!(
    Lane { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, cards :
    __classifiers::NestedListLog < CardLog > => Vec < CardValue >, }
);
