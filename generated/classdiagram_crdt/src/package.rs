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
    pub use moirai_crdt::policy::FairPolicy;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_protocol::crdt::pure_crdt::PureCRDT;
    pub use crate::references::*;
}
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Classdiagram {
    Class(crate::classifiers::Class),
    Feature(crate::classifiers::Feature),
    Relation(crate::classifiers::Relation),
    AddReference(__package::Refs),
    RemoveReference(__package::Refs),
}
#[derive(Debug)]
pub enum ClassdiagramRejection {
    Class(<crate::classifiers::ClassLog as __package::IsLog>::Rejection),
    Feature(<crate::classifiers::FeatureLog as __package::IsLog>::Rejection),
    Relation(<crate::classifiers::RelationLog as __package::IsLog>::Rejection),
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
impl std::fmt::Display for ClassdiagramRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Class(error) => write!(f, "{}: {}", "Class", error),
            Self::Feature(error) => write!(f, "{}: {}", "Feature", error),
            Self::Relation(error) => write!(f, "{}: {}", "Relation", error),
            Self::AddReference(error) => write!(f, "AddReference: {}", error),
            Self::RemoveReference(error) => write!(f, "RemoveReference: {}", error),
        }
    }
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClassdiagramValue {
    pub class: crate::classifiers::ClassValue,
    pub feature: crate::classifiers::FeatureValue,
    pub relation: crate::classifiers::RelationValue,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub refs: <__package::ReferenceManager<
        __package::FairPolicy,
    > as __package::PureCRDT>::Value,
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClassdiagramLog {
    class_log: crate::classifiers::ClassLog,
    feature_log: crate::classifiers::FeatureLog,
    relation_log: crate::classifiers::RelationLog,
    reference_manager_log: __package::VecLog<
        __package::ReferenceManager<__package::FairPolicy>,
    >,
}
impl ClassdiagramLog {
    pub fn class_log(&self) -> &crate::classifiers::ClassLog {
        &self.class_log
    }
    pub fn feature_log(&self) -> &crate::classifiers::FeatureLog {
        &self.feature_log
    }
    pub fn relation_log(&self) -> &crate::classifiers::RelationLog {
        &self.relation_log
    }
    pub fn reference_manager_log(
        &self,
    ) -> &__package::VecLog<__package::ReferenceManager<__package::FairPolicy>> {
        &self.reference_manager_log
    }
}
impl __package::IsLog for ClassdiagramLog {
    type Value = ClassdiagramValue;
    type Op = Classdiagram;
    type Rejection = ClassdiagramRejection;
    fn is_enabled(&self, op: &Self::Op) -> Result<(), Self::Rejection> {
        match op {
            Classdiagram::Class(o) => {
                self.class_log.is_enabled(o).map_err(ClassdiagramRejection::Class)
            }
            Classdiagram::Feature(o) => {
                self.feature_log.is_enabled(o).map_err(ClassdiagramRejection::Feature)
            }
            Classdiagram::Relation(o) => {
                self.relation_log.is_enabled(o).map_err(ClassdiagramRejection::Relation)
            }
            Classdiagram::AddReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::AddArc(o.clone()))
                    .map_err(ClassdiagramRejection::AddReference)
            }
            Classdiagram::RemoveReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::RemoveArc(o.clone()))
                    .map_err(ClassdiagramRejection::RemoveReference)
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
                "classdiagram",
                Some(&mut sink),
            );
            match event.op().clone() {
                Classdiagram::Class(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "class",
                        |ctx| {
                            self.class_log.effect(child_event, ctx);
                        },
                    );
                }
                Classdiagram::Feature(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "feature",
                        |ctx| {
                            self.feature_log.effect(child_event, ctx);
                        },
                    );
                }
                Classdiagram::Relation(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "relation",
                        |ctx| {
                            self.relation_log.effect(child_event, ctx);
                        },
                    );
                }
                Classdiagram::AddReference(o) => {
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
                Classdiagram::RemoveReference(o) => {
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
        self.class_log.stabilize(version);
        self.feature_log.stabilize(version);
        self.relation_log.stabilize(version);
        self.reference_manager_log.stabilize(version);
    }
    fn redundant_by_parent(&mut self, version: &__package::Version, conservative: bool) {
        self.class_log.redundant_by_parent(version, conservative);
        self.feature_log.redundant_by_parent(version, conservative);
        self.relation_log.redundant_by_parent(version, conservative);
        self.reference_manager_log.redundant_by_parent(version, conservative);
    }
    fn is_default(&self) -> bool {
        self.reference_manager_log.is_default() && self.class_log.is_default()
            && self.feature_log.is_default() && self.relation_log.is_default()
    }
}
impl __package::EvalNested<__package::Read<<Self as __package::IsLog>::Value>>
for ClassdiagramLog {
    fn execute_query(
        &self,
        _q: __package::Read<<Self as __package::IsLog>::Value>,
    ) -> <__package::Read<
        <Self as __package::IsLog>::Value,
    > as __package::QueryOperation>::Response {
        ClassdiagramValue {
            class: self.class_log.execute_query(__package::Read::new()),
            feature: self.feature_log.execute_query(__package::Read::new()),
            relation: self.relation_log.execute_query(__package::Read::new()),
            refs: self.reference_manager_log.execute_query(__package::Read::new()),
        }
    }
}
/// Auto-generated [`QueryableLog`] impl — enables `GET /api/state` in `GenericNode`.
///
/// Calls `replica.query(Read::new())` to evaluate the full CRDT state via
/// the `EvalNested<Read<Value>>` chain, then serializes the result to JSON.
impl moirai_network::query::QueryableLog for ClassdiagramLog {
    fn query_state_json(
        replica: &moirai_protocol::replica::Replica<
            Self,
            moirai_protocol::broadcast::tcsb::Tcsb<Classdiagram>,
        >,
    ) -> serde_json::Value {
        use moirai_protocol::replica::IsReplica;
        let value: ClassdiagramValue = replica.query(__package::Read::new());
        serde_json::to_value(&value)
            .unwrap_or_else(|e| {
                serde_json::json!({ "error" : format!("serialize: {}", e) })
            })
    }
}
impl __package::InternalizeOp for Classdiagram {
    fn internalize(self, interner: &__package::Interner) -> Self {
        match self {
            Classdiagram::Class(op) => Classdiagram::Class(op.clone()),
            Classdiagram::Feature(op) => Classdiagram::Feature(op.clone()),
            Classdiagram::Relation(op) => Classdiagram::Relation(op.clone()),
            Classdiagram::AddReference(op) => {
                Classdiagram::AddReference(op.internalize(interner))
            }
            Classdiagram::RemoveReference(op) => {
                Classdiagram::RemoveReference(op.internalize(interner))
            }
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
impl __package::EvalNested<ReadAsEcore> for ClassdiagramLog {
    fn execute_query(
        &self,
        _q: ReadAsEcore,
    ) -> <ReadAsEcore as __package::QueryOperation>::Response {
        let mut document_root = xml_builder::XMLElement::new("xmi:XMI");
        document_root.add_attribute("xmi:version", "2.0");
        document_root.add_attribute("xmlns:xmi", "http://www.omg.org/XMI");
        document_root
            .add_attribute("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance");
        document_root
            .add_attribute("xmlns:classdiagram", "http://www.example.org/classdiagram");
        document_root
            .add_child(xml_builder::XMLElement::new("classdiagram:Class"))
            .expect("adding a root object to the XMI document should not fail");
        document_root
            .add_child(xml_builder::XMLElement::new("classdiagram:Feature"))
            .expect("adding a root object to the XMI document should not fail");
        document_root
            .add_child(xml_builder::XMLElement::new("classdiagram:Relation"))
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
