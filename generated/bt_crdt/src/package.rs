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
pub enum Behaviortree {
    Root(crate::classifiers::Root),
    AddReference(__package::Refs),
    RemoveReference(__package::Refs),
}
#[derive(Debug)]
pub enum BehaviortreeRejection {
    Root(<crate::classifiers::RootLog as __package::IsLog>::Rejection),
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
impl std::fmt::Display for BehaviortreeRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Root(error) => write!(f, "{}: {}", "Root", error),
            Self::AddReference(error) => write!(f, "AddReference: {}", error),
            Self::RemoveReference(error) => write!(f, "RemoveReference: {}", error),
        }
    }
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BehaviortreeValue {
    pub root: crate::classifiers::RootValue,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub refs: __package::DiGraph<__package::Instance, __package::Ref>,
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BehaviortreeLog {
    root_log: crate::classifiers::RootLog,
    reference_manager_log: __package::VecLog<
        __package::ReferenceManager<__package::FairPolicy>,
    >,
}
impl BehaviortreeLog {
    pub fn root_log(&self) -> &crate::classifiers::RootLog {
        &self.root_log
    }
    pub fn reference_manager_log(
        &self,
    ) -> &__package::VecLog<__package::ReferenceManager<__package::FairPolicy>> {
        &self.reference_manager_log
    }
}
impl __package::IsLog for BehaviortreeLog {
    type Command = Behaviortree;
    type Op = Behaviortree;
    type Rejection = BehaviortreeRejection;
    fn prepare(&self, command: Self::Command) -> Self::Op {
        command
    }
    fn is_enabled(&self, op: &Self::Op) -> Result<(), Self::Rejection> {
        match op {
            Behaviortree::Root(o) => {
                self.root_log.is_enabled(o).map_err(BehaviortreeRejection::Root)
            }
            Behaviortree::AddReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::AddArc(o.clone()))
                    .map_err(BehaviortreeRejection::AddReference)
            }
            Behaviortree::RemoveReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::RemoveArc(o.clone()))
                    .map_err(BehaviortreeRejection::RemoveReference)
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
            let mut ctx = __package::EffectContext::root(
                "behaviortree",
                Some(&mut sink),
            );
            match event.op().clone() {
                Behaviortree::Root(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "root",
                        |ctx| {
                            self.root_log.effect(child_event, ctx);
                        },
                    );
                }
                Behaviortree::AddReference(o) => {
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
                Behaviortree::RemoveReference(o) => {
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
        self.root_log.stabilize(version);
        self.reference_manager_log.stabilize(version);
    }
    fn redundant_by_parent(&mut self, version: &__package::Version, conservative: bool) {
        self.root_log.redundant_by_parent(version, conservative);
        self.reference_manager_log.redundant_by_parent(version, conservative);
    }
    fn is_default(&self) -> bool {
        self.reference_manager_log.is_default() && self.root_log.is_default()
    }
}
impl __package::EvalNested<__package::Read<BehaviortreeValue>> for BehaviortreeLog {
    fn execute_query(
        &self,
        _q: &__package::Read<BehaviortreeValue>,
    ) -> <__package::Read<BehaviortreeValue> as __package::QueryOperation>::Response {
        BehaviortreeValue {
            root: self.root_log.execute_query(&__package::Read::new()),
            refs: self.reference_manager_log.execute_query(&__package::Read::new()),
        }
    }
}
/// Auto-generated [`QueryableLog`] impl — enables `GET /api/state` in `GenericNode`.
///
/// Calls `replica.query(Read::new())` to evaluate the full CRDT state via
/// the `EvalNested<Read<Value>>` chain, then serializes the result to JSON.
impl moirai_network::query::QueryableLog for BehaviortreeLog {
    fn query_state_json(
        replica: &moirai_protocol::replica::Replica<
            Self,
            moirai_protocol::broadcast::tcsb::Tcsb<Behaviortree>,
        >,
    ) -> serde_json::Value {
        use moirai_protocol::replica::IsReplica;
        let value: BehaviortreeValue = replica.query(&__package::Read::new());
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
impl __package::EvalNested<ReadAsEcore> for BehaviortreeLog {
    fn execute_query(
        &self,
        _q: &ReadAsEcore,
    ) -> <ReadAsEcore as __package::QueryOperation>::Response {
        let mut document_root = xml_builder::XMLElement::new("xmi:XMI");
        document_root.add_attribute("xmi:version", "2.0");
        document_root.add_attribute("xmlns:xmi", "http://www.omg.org/XMI");
        document_root
            .add_attribute("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance");
        document_root
            .add_attribute("xmlns:behaviortree", "http://www.example.org/behaviortree");
        document_root
            .add_child(xml_builder::XMLElement::new("behaviortree:Root"))
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
