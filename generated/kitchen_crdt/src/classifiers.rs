/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_protocol::state::graph_log::GraphLog;
    pub use moirai_crdt::option::OptionLog;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_crdt::counter::resettable_counter::Counter;
    pub use moirai_crdt::flag::ew_flag::EWFlag;
    pub use moirai_crdt::register::mv_register::MVRegister;
    pub use moirai_crdt::list::nested_list::NestedListLog;
    pub use moirai_crdt::bag::aw_bag::AWBagLog;
    pub use moirai_crdt::set::aw_set::AWSet;
}
__classifiers::record!(
    Foo { my_string : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >>, my_int : __classifiers::OptionLog <
    __classifiers::VecLog < __classifiers::Counter < i32 > >>, my_boolean :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::EWFlag >>, my_char
    : __classifiers::OptionLog < __classifiers::VecLog < __classifiers::MVRegister < char
    > >>, my_long : __classifiers::OptionLog < __classifiers::VecLog <
    __classifiers::Counter < i64 > >>, my_float : __classifiers::OptionLog <
    __classifiers::VecLog < __classifiers::Counter < f32 > >>, my_double :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::Counter < f64 > >>,
    my_byte : __classifiers::OptionLog < __classifiers::VecLog < __classifiers::Counter <
    i8 > >>, my_short : __classifiers::OptionLog < __classifiers::VecLog <
    __classifiers::Counter < i16 > >>, bounds0inf : __classifiers::GraphLog <
    __classifiers::List < i16 >>, bounds1inf : __classifiers::GraphLog <
    __classifiers::List < i16 >>, bounds01 : __classifiers::OptionLog <
    __classifiers::VecLog < __classifiers::Counter < i16 > >>, bounds11 :
    __classifiers::VecLog < __classifiers::Counter < i16 > >, boundsninf :
    __classifiers::GraphLog < __classifiers::List < i16 >>, boundsnm :
    __classifiers::GraphLog < __classifiers::List < i16 >>, simple_list :
    __classifiers::NestedListLog < __classifiers::VecLog < __classifiers::EWFlag >>,
    unique_list : __classifiers::GraphLog < __classifiers::List < i16 >>, bag :
    __classifiers::AWBagLog < i16 >, set : __classifiers::VecLog < __classifiers::AWSet <
    i16 >>, }
);
__classifiers::record!(
    Bar { health : __classifiers::OptionLog < __classifiers::VecLog <
    __classifiers::Counter < i32 > >>, }
);
__classifiers::union!(AbstractKind = Baz(Baz, BazLog));
__classifiers::record!(
    Abstract { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >>, }
);
__classifiers::record!(Baz { abstract_super : AbstractLog, });
