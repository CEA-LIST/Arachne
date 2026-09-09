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
pub enum Simpleuml {
    PackageableKind(__package::PackageableKind),
    ClassifierKind(__package::ClassifierKind),
    TTypeKind(__package::TTypeKind),
    ModelElementKind(__package::ModelElementKind),
    AddReference(__package::Refs),
    RemoveReference(__package::Refs),
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SimpleumlValue {
    pub packageable: __package::PackageableKindValue,
    pub classifier: __package::ClassifierKindValue,
    pub t_type: __package::TTypeKindValue,
    pub model_element: __package::ModelElementKindValue,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub refs: <__package::ReferenceManager<
        __package::FairPolicy,
    > as __package::PureCRDT>::Value,
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SimpleumlLog {
    packageable_log: __package::PackageableKindLog,
    classifier_log: __package::ClassifierKindLog,
    t_type_log: __package::TTypeKindLog,
    model_element_log: __package::ModelElementKindLog,
    reference_manager_log: __package::VecLog<
        __package::ReferenceManager<__package::FairPolicy>,
    >,
}
impl SimpleumlLog {
    pub fn packageable_log(&self) -> &__package::PackageableKindLog {
        &self.packageable_log
    }
    pub fn classifier_log(&self) -> &__package::ClassifierKindLog {
        &self.classifier_log
    }
    pub fn t_type_log(&self) -> &__package::TTypeKindLog {
        &self.t_type_log
    }
    pub fn model_element_log(&self) -> &__package::ModelElementKindLog {
        &self.model_element_log
    }
    pub fn reference_manager_log(
        &self,
    ) -> &__package::VecLog<__package::ReferenceManager<__package::FairPolicy>> {
        &self.reference_manager_log
    }
}
impl __package::IsLog for SimpleumlLog {
    type Value = SimpleumlValue;
    type Op = Simpleuml;
    fn is_enabled(&self, op: &Self::Op) -> bool {
        match op {
            Simpleuml::PackageableKind(o) => self.packageable_log.is_enabled(o),
            Simpleuml::ClassifierKind(o) => self.classifier_log.is_enabled(o),
            Simpleuml::TTypeKind(o) => self.t_type_log.is_enabled(o),
            Simpleuml::ModelElementKind(o) => self.model_element_log.is_enabled(o),
            Simpleuml::AddReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::AddArc(o.clone()))
            }
            Simpleuml::RemoveReference(o) => {
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
            Simpleuml::PackageableKind(o) => {
                __package::IsLog::effect(
                    &mut self.packageable_log,
                    __package::Event::unfold(event.clone(), o),
                    __package::ObjectPath::new("simpleuml").field("packageable"),
                    &mut sink,
                    __package::SinkOwnership::Owned,
                )
            }
            Simpleuml::ClassifierKind(o) => {
                __package::IsLog::effect(
                    &mut self.classifier_log,
                    __package::Event::unfold(event.clone(), o),
                    __package::ObjectPath::new("simpleuml").field("classifier"),
                    &mut sink,
                    __package::SinkOwnership::Owned,
                )
            }
            Simpleuml::TTypeKind(o) => {
                __package::IsLog::effect(
                    &mut self.t_type_log,
                    __package::Event::unfold(event.clone(), o),
                    __package::ObjectPath::new("simpleuml").field("t_type"),
                    &mut sink,
                    __package::SinkOwnership::Owned,
                )
            }
            Simpleuml::ModelElementKind(o) => {
                __package::IsLog::effect(
                    &mut self.model_element_log,
                    __package::Event::unfold(event.clone(), o),
                    __package::ObjectPath::new("simpleuml").field("model_element"),
                    &mut sink,
                    __package::SinkOwnership::Owned,
                )
            }
            Simpleuml::AddReference(o) => {
                self.reference_manager_log
                    .effect(
                        __package::Event::unfold(
                            event.clone(),
                            __package::ReferenceManager::AddArc(o),
                        ),
                        __package::ObjectPath::new("simpleuml"),
                        &mut __package::SinkCollector::new(),
                        __package::SinkOwnership::Owned,
                    )
            }
            Simpleuml::RemoveReference(o) => {
                self.reference_manager_log
                    .effect(
                        __package::Event::unfold(
                            event.clone(),
                            __package::ReferenceManager::RemoveArc(o),
                        ),
                        __package::ObjectPath::new("simpleuml"),
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
                                __package::ObjectPath::new("simpleuml"),
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
                            __package::ObjectPath::new("simpleuml"),
                            &mut __package::SinkCollector::new(),
                            __package::SinkOwnership::Owned,
                        );
                }
            }
        }
    }
    fn stabilize(&mut self, version: &__package::Version) {
        self.packageable_log.stabilize(version);
        self.classifier_log.stabilize(version);
        self.t_type_log.stabilize(version);
        self.model_element_log.stabilize(version);
        self.reference_manager_log.stabilize(version);
    }
    fn redundant_by_parent(&mut self, version: &__package::Version, conservative: bool) {
        self.packageable_log.redundant_by_parent(version, conservative);
        self.classifier_log.redundant_by_parent(version, conservative);
        self.t_type_log.redundant_by_parent(version, conservative);
        self.model_element_log.redundant_by_parent(version, conservative);
        self.reference_manager_log.redundant_by_parent(version, conservative);
    }
    fn is_default(&self) -> bool {
        true && self.packageable_log.is_default() && self.classifier_log.is_default()
            && self.t_type_log.is_default() && self.model_element_log.is_default()
    }
}
impl __package::EvalNested<__package::Read<<Self as __package::IsLog>::Value>>
for SimpleumlLog {
    fn execute_query(
        &self,
        _q: __package::Read<<Self as __package::IsLog>::Value>,
    ) -> <__package::Read<
        <Self as __package::IsLog>::Value,
    > as __package::QueryOperation>::Response {
        SimpleumlValue {
            packageable: self.packageable_log.execute_query(__package::Read::new()),
            classifier: self.classifier_log.execute_query(__package::Read::new()),
            t_type: self.t_type_log.execute_query(__package::Read::new()),
            model_element: self.model_element_log.execute_query(__package::Read::new()),
            refs: self.reference_manager_log.execute_query(__package::Read::new()),
        }
    }
}
/// Auto-generated [`QueryableLog`] impl — enables `GET /api/state` in `GenericNode`.
///
/// Calls `replica.query(Read::new())` to evaluate the full CRDT state via
/// the `EvalNested<Read<Value>>` chain, then serializes the result to JSON.
impl moirai_network::query::QueryableLog for SimpleumlLog {
    fn query_state_json(
        replica: &moirai_protocol::replica::Replica<
            Self,
            moirai_protocol::broadcast::tcsb::Tcsb<Simpleuml>,
        >,
    ) -> serde_json::Value {
        use moirai_protocol::replica::IsReplica;
        let value: SimpleumlValue = replica.query(__package::Read::new());
        serde_json::to_value(&value)
            .unwrap_or_else(|e| {
                serde_json::json!({ "error" : format!("serialize: {}", e) })
            })
    }
}
impl __package::InternalizeOp for Simpleuml {
    fn internalize(self, interner: &__package::Interner) -> Self {
        match self {
            Simpleuml::PackageableKind(op) => Simpleuml::PackageableKind(op.clone()),
            Simpleuml::ClassifierKind(op) => Simpleuml::ClassifierKind(op.clone()),
            Simpleuml::TTypeKind(op) => Simpleuml::TTypeKind(op.clone()),
            Simpleuml::ModelElementKind(op) => Simpleuml::ModelElementKind(op.clone()),
            Simpleuml::AddReference(op) => {
                Simpleuml::AddReference(op.internalize(interner))
            }
            Simpleuml::RemoveReference(op) => {
                Simpleuml::RemoveReference(op.internalize(interner))
            }
        }
    }
}
