/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __package {
    pub use moirai_protocol::crdt::query::Read;
    pub use moirai_protocol::crdt::eval::EvalNested;
    pub use moirai_protocol::state::log::IsLog;
    pub use moirai_protocol::clock::version_vector::Version;
    pub use moirai_protocol::event::Event;
    pub use moirai_protocol::crdt::query::QueryOperation;
    pub use moirai_protocol::state::object_path::ObjectPath;
    pub use moirai_protocol::state::sink::SinkEffect;
    pub use moirai_protocol::state::sink::SinkOwnership;
    pub use moirai_protocol::utils::intern_str::Interner;
    pub use moirai_protocol::utils::intern_str::InternalizeOp;
    pub use moirai_protocol::state::sink::SinkCollector;
    pub use moirai_protocol::state::po_log::POLog;
    pub use crate::classifiers::*;
}
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Json {
    JsonKind(__package::JsonKind),
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JsonValue {
    pub json: __package::JsonKindValue,
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JsonLog {
    json_log: __package::JsonKindLog,
}
impl JsonLog {
    pub fn json_log(&self) -> &__package::JsonKindLog {
        &self.json_log
    }
}
impl __package::IsLog for JsonLog {
    type Value = JsonValue;
    type Op = Json;
    fn is_enabled(&self, op: &Self::Op) -> bool {
        match op {
            Json::JsonKind(o) => self.json_log.is_enabled(o),
        }
    }
    fn effect(
        &mut self,
        event: __package::Event<Self::Op>,
        _path: __package::ObjectPath,
        _sink: &mut __package::SinkCollector,
        _ownership: __package::SinkOwnership,
    ) {
        match event.op().clone() {
            Json::JsonKind(o) => {
                self.json_log
                    .effect(
                        __package::Event::unfold(event.clone(), o),
                        __package::ObjectPath::new("json"),
                        &mut __package::SinkCollector::new(),
                        __package::SinkOwnership::Owned,
                    )
            }
        }
    }
    fn stabilize(&mut self, version: &__package::Version) {
        self.json_log.stabilize(version);
    }
    fn redundant_by_parent(&mut self, version: &__package::Version, conservative: bool) {
        self.json_log.redundant_by_parent(version, conservative);
    }
    fn is_default(&self) -> bool {
        true && self.json_log.is_default()
    }
}
impl __package::EvalNested<__package::Read<<Self as __package::IsLog>::Value>>
for JsonLog {
    fn execute_query(
        &self,
        _q: __package::Read<<Self as __package::IsLog>::Value>,
    ) -> <__package::Read<
        <Self as __package::IsLog>::Value,
    > as __package::QueryOperation>::Response {
        JsonValue {
            json: self.json_log.execute_query(__package::Read::new()),
        }
    }
}
/// Auto-generated [`QueryableLog`] impl — enables `GET /api/state` in `GenericNode`.
///
/// Calls `replica.query(Read::new())` to evaluate the full CRDT state via
/// the `EvalNested<Read<Value>>` chain, then serializes the result to JSON.
impl moirai_network::query::QueryableLog for JsonLog {
    fn query_state_json(
        replica: &moirai_protocol::replica::Replica<
            Self,
            moirai_protocol::broadcast::tcsb::Tcsb<Json>,
        >,
    ) -> serde_json::Value {
        use moirai_protocol::replica::IsReplica;
        let value: JsonValue = replica.query(__package::Read::new());
        serde_json::to_value(&value)
            .unwrap_or_else(|e| {
                serde_json::json!({ "error" : format!("serialize: {}", e) })
            })
    }
}
impl __package::InternalizeOp for Json {
    fn internalize(self, interner: &__package::Interner) -> Self {
        match self {
            Json::JsonKind(op) => Json::JsonKind(op.clone()),
        }
    }
}
