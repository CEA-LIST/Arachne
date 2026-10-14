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
    pub use moirai_protocol::state::sink::SinkCollector;
    pub use moirai_protocol::crdt::policy::FairPolicy;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use petgraph::graph::DiGraph;
    pub use crate::references::*;
}
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Si05keyed {
    Board(crate::classifiers::Board),
    AddReference(__package::Refs),
    RemoveReference(__package::Refs),
}
#[derive(Debug)]
pub enum Si05keyedRejection {
    Board(<crate::classifiers::BoardLog as __package::IsLog>::Rejection),
    AddReference(
        <__package::VecLog<
            __package::ReferenceManager<__package::FairPolicy>,
        > as __package::IsLog>::Rejection,
    ),
    RemoveReference(
        <__package::VecLog<
            __package::ReferenceManager<__package::FairPolicy>,
        > as __package::IsLog>::Rejection,
    ),
}
impl std::fmt::Display for Si05keyedRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Board(error) => write!(f, "{}: {}", "Board", error),
            Self::AddReference(error) => write!(f, "AddReference: {}", error),
            Self::RemoveReference(error) => write!(f, "RemoveReference: {}", error),
        }
    }
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Si05keyedValue {
    pub board: crate::classifiers::BoardValue,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub refs: __package::DiGraph<__package::Instance, __package::Ref>,
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Si05keyedLog {
    board_log: crate::classifiers::BoardLog,
    reference_manager_log: __package::VecLog<
        __package::ReferenceManager<__package::FairPolicy>,
    >,
}
impl Si05keyedLog {
    pub fn board_log(&self) -> &crate::classifiers::BoardLog {
        &self.board_log
    }
    pub fn reference_manager_log(
        &self,
    ) -> &__package::VecLog<__package::ReferenceManager<__package::FairPolicy>> {
        &self.reference_manager_log
    }
}
impl __package::IsLog for Si05keyedLog {
    type Command = Si05keyed;
    type Op = Si05keyed;
    type Rejection = Si05keyedRejection;
    fn prepare(&self, command: Self::Command) -> Self::Op {
        command
    }
    fn is_enabled(&self, op: &Self::Op) -> Result<(), Self::Rejection> {
        match op {
            Si05keyed::Board(o) => {
                self.board_log.is_enabled(o).map_err(Si05keyedRejection::Board)
            }
            Si05keyed::AddReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::AddArc(o.clone()))
                    .map_err(Si05keyedRejection::AddReference)
            }
            Si05keyed::RemoveReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::RemoveArc(o.clone()))
                    .map_err(Si05keyedRejection::RemoveReference)
            }
        }
    }
    fn effect(
        &mut self,
        event: __package::ProtocolEvent<Self::Op>,
        _ctx: &mut __package::EffectContext<'_>,
    ) {
        let mut sink = __package::SinkCollector::new();
        {
            let mut ctx = __package::EffectContext::root("si05keyed", Some(&mut sink));
            match event.op().clone() {
                Si05keyed::Board(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "board",
                        |ctx| {
                            self.board_log.effect(child_event, ctx);
                        },
                    );
                }
                Si05keyed::AddReference(o) => {
                    let mut ctx = __package::EffectContext::silent();
                    self.reference_manager_log
                        .effect(
                            __package::ProtocolEvent::unfold(
                                event.clone(),
                                __package::ReferenceManager::AddArc(o),
                            ),
                            &mut ctx,
                        );
                }
                Si05keyed::RemoveReference(o) => {
                    let mut ctx = __package::EffectContext::silent();
                    self.reference_manager_log
                        .effect(
                            __package::ProtocolEvent::unfold(
                                event.clone(),
                                __package::ReferenceManager::RemoveArc(o),
                            ),
                            &mut ctx,
                        );
                }
            }
        }
        let mut reference_effect_disambiguator = 0u32;
        for sink in sink.into_sinks() {
            match sink.effect() {
                __package::SinkEffect::Create | __package::SinkEffect::Update => {
                    let vertex_ops = sink
                        .kind()
                        .and_then(|kind| __package::instance_from_sink_kind(
                            kind,
                            sink.path(),
                        ))
                        .map(|instance| __package::ReferenceManager::AddVertex {
                            id: instance,
                        });
                    if let Some(o) = vertex_ops {
                        reference_effect_disambiguator += 1;
                        let mut ctx = __package::EffectContext::silent();
                        self.reference_manager_log
                            .effect(
                                __package::ProtocolEvent::unfold_with_disambiguator(
                                    event.clone(),
                                    reference_effect_disambiguator,
                                    o,
                                ),
                                &mut ctx,
                            );
                    }
                }
                __package::SinkEffect::Delete => {
                    reference_effect_disambiguator += 1;
                    let mut ctx = __package::EffectContext::silent();
                    self.reference_manager_log
                        .effect(
                            __package::ProtocolEvent::unfold_with_disambiguator(
                                event.clone(),
                                reference_effect_disambiguator,
                                __package::ReferenceManager::DeleteSubtree {
                                    prefix: sink.path().clone(),
                                },
                            ),
                            &mut ctx,
                        );
                }
            }
        }
    }
    fn stabilize(&mut self, version: &__package::Version) {
        self.board_log.stabilize(version);
        self.reference_manager_log.stabilize(version);
    }
    fn redundant_by_parent(&mut self, version: &__package::Version, conservative: bool) {
        self.board_log.redundant_by_parent(version, conservative);
        self.reference_manager_log.redundant_by_parent(version, conservative);
    }
    fn is_default(&self) -> bool {
        self.reference_manager_log.is_default() && self.board_log.is_default()
    }
}
impl __package::EvalNested<__package::Read<Si05keyedValue>> for Si05keyedLog {
    fn execute_query(
        &self,
        _q: &__package::Read<Si05keyedValue>,
    ) -> <__package::Read<Si05keyedValue> as __package::QueryOperation>::Response {
        Si05keyedValue {
            board: self.board_log.execute_query(&__package::Read::new()),
            refs: self.reference_manager_log.execute_query(&__package::Read::new()),
        }
    }
}
/// Auto-generated [`QueryableLog`] impl — enables `GET /api/state` in `GenericNode`.
///
/// Calls `replica.query(Read::new())` to evaluate the full CRDT state via
/// the `EvalNested<Read<Value>>` chain, then serializes the result to JSON.
impl moirai_network::query::QueryableLog for Si05keyedLog {
    fn query_state_json(
        replica: &moirai_protocol::replica::Replica<
            Self,
            moirai_protocol::broadcast::tcsb::Tcsb<Si05keyed>,
        >,
    ) -> serde_json::Value {
        use moirai_protocol::replica::IsReplica;
        let value: Si05keyedValue = replica.query(&__package::Read::new());
        serde_json::to_value(&value)
            .unwrap_or_else(|e| {
                serde_json::json!({ "error" : format!("serialize: {}", e) })
            })
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
impl __package::EvalNested<ReadAsEcore> for Si05keyedLog {
    fn execute_query(
        &self,
        _q: &ReadAsEcore,
    ) -> <ReadAsEcore as __package::QueryOperation>::Response {
        let mut document_root = xml_builder::XMLElement::new("xmi:XMI");
        document_root.add_attribute("xmi:version", "2.0");
        document_root.add_attribute("xmlns:xmi", "http://www.omg.org/XMI");
        document_root
            .add_attribute("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance");
        document_root.add_attribute("xmlns:keyed", "http://cea.fr/crdt/si05/keyed");
        document_root
            .add_child(xml_builder::XMLElement::new("keyed:Board"))
            .expect("adding a root object to the XMI document should not fail");
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
