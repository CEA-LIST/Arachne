/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __package {
    pub use moirai_protocol::crdt::query::Read;
    pub use moirai_protocol::crdt::eval::EvalNested;
    pub use moirai_protocol::state::log::IsLog;
    pub use moirai_protocol::clock::version_vector::Version;
    pub use moirai_protocol::event::Event as ProtocolEvent;
    pub use moirai_protocol::crdt::query::QueryOperation;
    pub use moirai_protocol::state::sink::SinkEffect;
    pub use moirai_protocol::state::effect_context::EffectContext;
    pub use moirai_protocol::broadcast::internalizer::Interner;
    pub use moirai_protocol::broadcast::internalizer::InternalizeOp;
    pub use moirai_protocol::state::sink::SinkCollector;
}
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Json {
    JsonKind(crate::classifiers::JsonKind),
}
#[derive(Debug)]
pub enum JsonRejection {
    JsonKind(<crate::classifiers::JsonKindLog as __package::IsLog>::Rejection),
}
impl std::fmt::Display for JsonRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JsonKind(error) => write!(f, "{}: {}", "JsonKind", error),
        }
    }
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JsonValue {
    pub json: crate::classifiers::JsonKindValue,
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JsonLog {
    json_log: crate::classifiers::JsonKindLog,
}
impl JsonLog {
    pub fn json_log(&self) -> &crate::classifiers::JsonKindLog {
        &self.json_log
    }
}
impl __package::IsLog for JsonLog {
    type Value = JsonValue;
    type Op = Json;
    type Rejection = JsonRejection;
    fn is_enabled(&self, op: &Self::Op) -> Result<(), Self::Rejection> {
        match op {
            Json::JsonKind(o) => {
                self.json_log.is_enabled(o).map_err(JsonRejection::JsonKind)
            }
        }
    }
    fn effect(
        &mut self,
        event: __package::ProtocolEvent<Self::Op>,
        _ctx: &mut __package::EffectContext<'_>,
    ) {
        let mut ctx = __package::EffectContext::root("json", None);
        match event.op().clone() {
            Json::JsonKind(o) => {
                let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                ctx.with_field(
                    "json",
                    |ctx| {
                        self.json_log.effect(child_event, ctx);
                    },
                );
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
/// Serializes the current model state as XMI conforming to the source Ecore metamodel.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReadAsEcore;
impl __package::QueryOperation for ReadAsEcore {
    type Response = Vec<u8>;
}
impl ReadAsEcore {
    pub fn new() -> Self {
        Self
    }
}
impl __package::EvalNested<ReadAsEcore> for JsonLog {
    fn execute_query(
        &self,
        _q: ReadAsEcore,
    ) -> <ReadAsEcore as __package::QueryOperation>::Response {
        let mut document_root = xml_builder::XMLElement::new("xmi:XMI");
        document_root.add_attribute("xmi:version", "2.0");
        document_root.add_attribute("xmlns:xmi", "http://www.omg.org/XMI");
        document_root
            .add_attribute("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance");
        document_root.add_attribute("xmlns:json", "http://www.example.org/json");
        match &self.json_log.child {
            crate::classifiers::JsonKindContainer::Unset => {}
            crate::classifiers::JsonKindContainer::Value(__child) => {
                match __child.as_ref() {
                    crate::classifiers::JsonKindChild::Array(_) => {
                        document_root
                            .add_child(xml_builder::XMLElement::new("json:Array"))
                            .expect(
                                "adding a root object to the XMI document should not fail",
                            );
                    }
                    crate::classifiers::JsonKindChild::Object(_) => {
                        document_root
                            .add_child(xml_builder::XMLElement::new("json:Object"))
                            .expect(
                                "adding a root object to the XMI document should not fail",
                            );
                    }
                    crate::classifiers::JsonKindChild::String(_) => {
                        document_root
                            .add_child(xml_builder::XMLElement::new("json:String"))
                            .expect(
                                "adding a root object to the XMI document should not fail",
                            );
                    }
                    crate::classifiers::JsonKindChild::Number(_) => {
                        document_root
                            .add_child(xml_builder::XMLElement::new("json:Number"))
                            .expect(
                                "adding a root object to the XMI document should not fail",
                            );
                    }
                    crate::classifiers::JsonKindChild::Boolean(_) => {
                        document_root
                            .add_child(xml_builder::XMLElement::new("json:Boolean"))
                            .expect(
                                "adding a root object to the XMI document should not fail",
                            );
                    }
                }
            }
            crate::classifiers::JsonKindContainer::Conflicts(__children) => {
                for __child in __children {
                    match __child {
                        crate::classifiers::JsonKindChild::Array(_) => {
                            document_root
                                .add_child(xml_builder::XMLElement::new("json:Array"))
                                .expect(
                                    "adding a root object to the XMI document should not fail",
                                );
                        }
                        crate::classifiers::JsonKindChild::Object(_) => {
                            document_root
                                .add_child(xml_builder::XMLElement::new("json:Object"))
                                .expect(
                                    "adding a root object to the XMI document should not fail",
                                );
                        }
                        crate::classifiers::JsonKindChild::String(_) => {
                            document_root
                                .add_child(xml_builder::XMLElement::new("json:String"))
                                .expect(
                                    "adding a root object to the XMI document should not fail",
                                );
                        }
                        crate::classifiers::JsonKindChild::Number(_) => {
                            document_root
                                .add_child(xml_builder::XMLElement::new("json:Number"))
                                .expect(
                                    "adding a root object to the XMI document should not fail",
                                );
                        }
                        crate::classifiers::JsonKindChild::Boolean(_) => {
                            document_root
                                .add_child(xml_builder::XMLElement::new("json:Boolean"))
                                .expect(
                                    "adding a root object to the XMI document should not fail",
                                );
                        }
                    }
                }
            }
        }
        let mut xml = xml_builder::XMLBuilder::new()
            .version(xml_builder::XMLVersion::XML1_0)
            .encoding("UTF-8".into())
            .build();
        xml.set_root_element(document_root);
        let mut writer = Vec::new();
        xml.generate(&mut writer)
            .expect("writing model XMI to an in-memory buffer should not fail");
        writer
    }
}
