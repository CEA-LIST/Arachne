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
    pub use moirai_crdt::policy::FairPolicy;
    pub use moirai_protocol::state::po_log::VecLog;
    pub use moirai_protocol::crdt::pure_crdt::PureCRDT;
    pub use crate::references::*;
}
pub type ReferenceManagerLog = __package::POLog<
    __package::ReferenceManager<__package::FairPolicy>,
    __package::ReferenceManagerState<__package::FairPolicy>,
>;
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Classdiagram {
    Class(__package::Class),
    Feature(__package::Feature),
    Relation(__package::Relation),
    AddReference(__package::Refs),
    RemoveReference(__package::Refs),
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClassdiagramValue {
    pub class: __package::ClassValue,
    pub feature: __package::FeatureValue,
    pub relation: __package::RelationValue,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub refs: <__package::ReferenceManager<
        __package::FairPolicy,
    > as __package::PureCRDT>::Value,
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClassdiagramLog {
    class_log: __package::ClassLog,
    feature_log: __package::FeatureLog,
    relation_log: __package::RelationLog,
    reference_manager_log: __package::VecLog<
        __package::ReferenceManager<__package::FairPolicy>,
    >,
}
impl ClassdiagramLog {
    pub fn class_log(&self) -> &__package::ClassLog {
        &self.class_log
    }
    pub fn feature_log(&self) -> &__package::FeatureLog {
        &self.feature_log
    }
    pub fn relation_log(&self) -> &__package::RelationLog {
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
    fn is_enabled(&self, op: &Self::Op) -> bool {
        match op {
            Classdiagram::Class(o) => self.class_log.is_enabled(o),
            Classdiagram::Feature(o) => self.feature_log.is_enabled(o),
            Classdiagram::Relation(o) => self.relation_log.is_enabled(o),
            Classdiagram::AddReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::AddArc(o.clone()))
            }
            Classdiagram::RemoveReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::RemoveArc(o.clone()))
            }
        }
    }
    fn effect(
        &mut self,
        event: __package::Event<Self::Op>,
        _path: __package::ObjectPath,
        _sink: &mut __package::SinkCollector,
        _ownership: __package::SinkOwnership,
    ) {
        let mut sink = __package::SinkCollector::new();
        match event.op().clone() {
            Classdiagram::Class(o) => {
                __package::IsLog::effect(
                    &mut self.class_log,
                    __package::Event::unfold(event.clone(), o),
                    __package::ObjectPath::new("classdiagram").field("class"),
                    &mut sink,
                    __package::SinkOwnership::Owned,
                )
            }
            Classdiagram::Feature(o) => {
                __package::IsLog::effect(
                    &mut self.feature_log,
                    __package::Event::unfold(event.clone(), o),
                    __package::ObjectPath::new("classdiagram").field("feature"),
                    &mut sink,
                    __package::SinkOwnership::Owned,
                )
            }
            Classdiagram::Relation(o) => {
                __package::IsLog::effect(
                    &mut self.relation_log,
                    __package::Event::unfold(event.clone(), o),
                    __package::ObjectPath::new("classdiagram").field("relation"),
                    &mut sink,
                    __package::SinkOwnership::Owned,
                )
            }
            Classdiagram::AddReference(o) => {
                self.reference_manager_log
                    .effect(
                        __package::Event::unfold(
                            event.clone(),
                            __package::ReferenceManager::AddArc(o),
                        ),
                        __package::ObjectPath::new("classdiagram"),
                        &mut __package::SinkCollector::new(),
                        __package::SinkOwnership::Owned,
                    )
            }
            Classdiagram::RemoveReference(o) => {
                self.reference_manager_log
                    .effect(
                        __package::Event::unfold(
                            event.clone(),
                            __package::ReferenceManager::RemoveArc(o),
                        ),
                        __package::ObjectPath::new("classdiagram"),
                        &mut __package::SinkCollector::new(),
                        __package::SinkOwnership::Owned,
                    )
            }
        }
        for sink in sink.into_sinks() {
            match sink.effect() {
                __package::SinkEffect::Create | __package::SinkEffect::Update => {
                    let vertex_ops = __package::instance_from_path(sink.path())
                        .map(|instance| __package::ReferenceManager::AddVertex {
                            id: instance,
                        });
                    if let Some(o) = vertex_ops {
                        self.reference_manager_log
                            .effect(
                                __package::Event::unfold(event.clone(), o),
                                __package::ObjectPath::new("classdiagram"),
                                &mut __package::SinkCollector::new(),
                                __package::SinkOwnership::Owned,
                            );
                    }
                }
                __package::SinkEffect::Delete => {
                    self.reference_manager_log
                        .effect(
                            __package::Event::unfold(
                                event.clone(),
                                __package::ReferenceManager::DeleteSubtree {
                                    prefix: sink.path().clone(),
                                },
                            ),
                            __package::ObjectPath::new("classdiagram"),
                            &mut __package::SinkCollector::new(),
                            __package::SinkOwnership::Owned,
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
        true && self.class_log.is_default() && self.feature_log.is_default()
            && self.relation_log.is_default()
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
