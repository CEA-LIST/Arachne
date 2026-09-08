/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_protocol::state::event_graph::EventGraph;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_crdt::counter::resettable_counter::Counter;
    pub use moirai_crdt::flag::ew_flag::EWFlag;
    pub use moirai_crdt::register::mv_register::MVRegister;
    pub use moirai_crdt::list::nested_list::NestedListLog;
    pub use moirai_crdt::bag::aw_bag::AWBagLog;
}
__classifiers::record!(
    Foo { my_string : __classifiers::EventGraph < __classifiers::List < char > >, my_int
    : __classifiers::VecLog < __classifiers::Counter < i32 > >, my_boolean :
    __classifiers::VecLog < __classifiers::EWFlag >, my_char : __classifiers::VecLog <
    __classifiers::MVRegister < char > >, my_long : __classifiers::VecLog <
    __classifiers::Counter < i64 > >, my_float : __classifiers::VecLog <
    __classifiers::Counter < f32 > >, my_double : __classifiers::VecLog <
    __classifiers::Counter < f64 > >, my_byte : __classifiers::VecLog <
    __classifiers::Counter < u8 > >, my_short : __classifiers::VecLog <
    __classifiers::Counter < i16 > >, bounds0inf : __classifiers::NestedListLog <
    __classifiers::VecLog < __classifiers::Counter < i16 > >>, bounds1inf :
    __classifiers::NestedListLog < __classifiers::VecLog < __classifiers::Counter < i16 >
    >>, bounds01 : __classifiers::VecLog < __classifiers::Counter < i16 > >, bounds11 :
    __classifiers::VecLog < __classifiers::Counter < i16 > >, boundsninf :
    __classifiers::NestedListLog < __classifiers::VecLog < __classifiers::Counter < i16 >
    >>, boundsnm : __classifiers::NestedListLog < __classifiers::VecLog <
    __classifiers::Counter < i16 > >>, simple_list : __classifiers::NestedListLog <
    __classifiers::VecLog < __classifiers::EWFlag >>, unique_list :
    __classifiers::NestedListLog < __classifiers::VecLog < __classifiers::Counter < i16 >
    >>, bag : __classifiers::AWBagLog < i16 >, set : __classifiers::AWBagLog < i16 >, }
);
__classifiers::record!(
    Bar { health : __classifiers::VecLog < __classifiers::Counter < i32 > >, }
);
__classifiers::union!(AbstractKind = Baz(Baz, BazLog));
__classifiers::record!(
    Abstract { name : __classifiers::EventGraph < __classifiers::List < char > >, }
);
__classifiers::record!(Baz { abstract_super : AbstractLog, });
