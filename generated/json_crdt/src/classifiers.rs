/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_crdt::list::nested_list::NestedListLog;
    pub use moirai_crdt::list::nested_list::NestedList;
    pub use moirai_protocol::state::log::BoxedLog;
    pub use moirai_crdt::map::uw_map::UWMapLog;
    pub use moirai_crdt::map::uw_map::UWMap;
    pub use moirai_protocol::state::graph_log::GraphLog;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_crdt::counter::resettable_counter::Counter;
    pub use moirai_crdt::flag::ew_flag::EWFlag;
}
type JsonArray = __classifiers::NestedList<Box<JsonKind>>;
type JsonArrayLog = __classifiers::NestedListLog<__classifiers::BoxedLog<JsonKindLog>>;
type JsonObject = __classifiers::UWMap<std::string::String, Box<JsonKind>>;
type JsonObjectLog = __classifiers::UWMapLog<std::string::String, JsonKindLog>;
type JsonString = __classifiers::List<char>;
type JsonStringLog = __classifiers::GraphLog<__classifiers::List<char>>;
type JsonNumber = __classifiers::Counter<f64>;
type JsonNumberLog = __classifiers::VecLog<__classifiers::Counter<f64>>;
type JsonBoolean = __classifiers::EWFlag;
type JsonBooleanLog = __classifiers::VecLog<__classifiers::EWFlag>;
__classifiers::union!(
    JsonKind = Array(JsonArray, JsonArrayLog) | Object(JsonObject, JsonObjectLog) |
    String(JsonString, JsonStringLog) | Number(JsonNumber, JsonNumberLog) |
    Boolean(JsonBoolean, JsonBooleanLog)
);
