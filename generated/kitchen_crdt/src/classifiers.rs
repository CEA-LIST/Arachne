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
    pub use moirai_macros::HashSet;
    pub use moirai_crdt::list::nested_list::NestedListLog;
    pub use moirai_crdt::bag::aw_bag::AWBagLog;
    pub use moirai_macros::HashMap;
    pub use moirai_crdt::set::aw_set::AWSet;
}
__classifiers::record!(
    Foo { my_string : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, my_int :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::Counter < i32 > >>
    => Option < i32 >, my_boolean : __classifiers::OptionLog < __classifiers::VecLog <
    __classifiers::EWFlag >> => Option < bool >, my_char : __classifiers::OptionLog <
    __classifiers::VecLog < __classifiers::MVRegister < char > >> => Option <
    __classifiers::HashSet < char > >, my_long : __classifiers::OptionLog <
    __classifiers::VecLog < __classifiers::Counter < i64 > >> => Option < i64 >, my_float
    : __classifiers::OptionLog < __classifiers::VecLog < __classifiers::Counter < f32 >
    >> => Option < f32 >, my_double : __classifiers::OptionLog < __classifiers::VecLog <
    __classifiers::Counter < f64 > >> => Option < f64 >, my_byte :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::Counter < i8 > >>
    => Option < i8 >, my_short : __classifiers::OptionLog < __classifiers::VecLog <
    __classifiers::Counter < i16 > >> => Option < i16 >, bounds0inf :
    __classifiers::GraphLog < __classifiers::List < i16 >> => Vec < i16 >, bounds1inf :
    __classifiers::GraphLog < __classifiers::List < i16 >> => Vec < i16 >, bounds01 :
    __classifiers::OptionLog < __classifiers::VecLog < __classifiers::Counter < i16 > >>
    => Option < i16 >, bounds11 : __classifiers::VecLog < __classifiers::Counter < i16 >
    > => i16, boundsninf : __classifiers::GraphLog < __classifiers::List < i16 >> => Vec
    < i16 >, boundsnm : __classifiers::GraphLog < __classifiers::List < i16 >> => Vec <
    i16 >, simple_list : __classifiers::NestedListLog < __classifiers::VecLog <
    __classifiers::EWFlag >> => Vec < bool >, unique_list : __classifiers::GraphLog <
    __classifiers::List < i16 >> => Vec < i16 >, bag : __classifiers::AWBagLog < i16 > =>
    __classifiers::HashMap < i16, usize >, set : __classifiers::VecLog <
    __classifiers::AWSet < i16 >> => __classifiers::HashSet < i16 >, }
);
__classifiers::record!(
    Bar { health : __classifiers::OptionLog < __classifiers::VecLog <
    __classifiers::Counter < i32 > >> => Option < i32 >, }
);
__classifiers::union!(AbstractKind = Baz(Baz, BazLog => BazValue));
__classifiers::record!(
    Abstract { name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, }
);
__classifiers::record!(Baz { abstract_super : AbstractLog => AbstractValue, });
