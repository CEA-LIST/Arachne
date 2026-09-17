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
pub enum Simpleuml {
    PackageableKind(crate::classifiers::PackageableKind),
    ClassifierKind(crate::classifiers::ClassifierKind),
    TTypeKind(crate::classifiers::TTypeKind),
    ModelElementKind(crate::classifiers::ModelElementKind),
    AddReference(__package::Refs),
    RemoveReference(__package::Refs),
}
#[derive(Debug)]
pub enum SimpleumlRejection {
    PackageableKind(
        <crate::classifiers::PackageableKindLog as __package::IsLog>::Rejection,
    ),
    ClassifierKind(
        <crate::classifiers::ClassifierKindLog as __package::IsLog>::Rejection,
    ),
    TTypeKind(<crate::classifiers::TTypeKindLog as __package::IsLog>::Rejection),
    ModelElementKind(
        <crate::classifiers::ModelElementKindLog as __package::IsLog>::Rejection,
    ),
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
impl std::fmt::Display for SimpleumlRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PackageableKind(error) => write!(f, "{}: {}", "PackageableKind", error),
            Self::ClassifierKind(error) => write!(f, "{}: {}", "ClassifierKind", error),
            Self::TTypeKind(error) => write!(f, "{}: {}", "TTypeKind", error),
            Self::ModelElementKind(error) => {
                write!(f, "{}: {}", "ModelElementKind", error)
            }
            Self::AddReference(error) => write!(f, "AddReference: {}", error),
            Self::RemoveReference(error) => write!(f, "RemoveReference: {}", error),
        }
    }
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SimpleumlValue {
    pub packageable: crate::classifiers::PackageableKindValue,
    pub classifier: crate::classifiers::ClassifierKindValue,
    pub t_type: crate::classifiers::TTypeKindValue,
    pub model_element: crate::classifiers::ModelElementKindValue,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub refs: <__package::ReferenceManager<
        __package::FairPolicy,
    > as __package::PureCRDT>::Value,
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SimpleumlLog {
    packageable_log: crate::classifiers::PackageableKindLog,
    classifier_log: crate::classifiers::ClassifierKindLog,
    t_type_log: crate::classifiers::TTypeKindLog,
    model_element_log: crate::classifiers::ModelElementKindLog,
    reference_manager_log: __package::VecLog<
        __package::ReferenceManager<__package::FairPolicy>,
    >,
}
impl SimpleumlLog {
    pub fn packageable_log(&self) -> &crate::classifiers::PackageableKindLog {
        &self.packageable_log
    }
    pub fn classifier_log(&self) -> &crate::classifiers::ClassifierKindLog {
        &self.classifier_log
    }
    pub fn t_type_log(&self) -> &crate::classifiers::TTypeKindLog {
        &self.t_type_log
    }
    pub fn model_element_log(&self) -> &crate::classifiers::ModelElementKindLog {
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
    type Rejection = SimpleumlRejection;
    fn is_enabled(&self, op: &Self::Op) -> Result<(), Self::Rejection> {
        match op {
            Simpleuml::PackageableKind(o) => {
                self.packageable_log
                    .is_enabled(o)
                    .map_err(SimpleumlRejection::PackageableKind)
            }
            Simpleuml::ClassifierKind(o) => {
                self.classifier_log
                    .is_enabled(o)
                    .map_err(SimpleumlRejection::ClassifierKind)
            }
            Simpleuml::TTypeKind(o) => {
                self.t_type_log.is_enabled(o).map_err(SimpleumlRejection::TTypeKind)
            }
            Simpleuml::ModelElementKind(o) => {
                self.model_element_log
                    .is_enabled(o)
                    .map_err(SimpleumlRejection::ModelElementKind)
            }
            Simpleuml::AddReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::AddArc(o.clone()))
                    .map_err(SimpleumlRejection::AddReference)
            }
            Simpleuml::RemoveReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::RemoveArc(o.clone()))
                    .map_err(SimpleumlRejection::RemoveReference)
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
            let mut ctx = __package::EffectContext::root("simpleuml", Some(&mut sink));
            match event.op().clone() {
                Simpleuml::PackageableKind(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "packageable",
                        |ctx| {
                            self.packageable_log.effect(child_event, ctx);
                        },
                    );
                }
                Simpleuml::ClassifierKind(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "classifier",
                        |ctx| {
                            self.classifier_log.effect(child_event, ctx);
                        },
                    );
                }
                Simpleuml::TTypeKind(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "t_type",
                        |ctx| {
                            self.t_type_log.effect(child_event, ctx);
                        },
                    );
                }
                Simpleuml::ModelElementKind(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "model_element",
                        |ctx| {
                            self.model_element_log.effect(child_event, ctx);
                        },
                    );
                }
                Simpleuml::AddReference(o) => {
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
                Simpleuml::RemoveReference(o) => {
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
        self.reference_manager_log.is_default() && self.packageable_log.is_default()
            && self.classifier_log.is_default() && self.t_type_log.is_default()
            && self.model_element_log.is_default()
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
impl __package::EvalNested<ReadAsEcore> for SimpleumlLog {
    fn execute_query(
        &self,
        _q: ReadAsEcore,
    ) -> <ReadAsEcore as __package::QueryOperation>::Response {
        let mut document_root = xml_builder::XMLElement::new("xmi:XMI");
        document_root.add_attribute("xmi:version", "2.0");
        document_root.add_attribute("xmlns:xmi", "http://www.omg.org/XMI");
        document_root
            .add_attribute("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance");
        document_root.add_attribute("xmlns:uml", "http:///SimpleUML.ecore");
        match &self.packageable_log.child {
            crate::classifiers::PackageableKindContainer::Unset => {}
            crate::classifiers::PackageableKindContainer::Value(__child) => {
                match __child.as_ref() {
                    crate::classifiers::PackageableKindChild::Package(__child_log) => {
                        match &__child_log.child {
                            crate::classifiers::PackageKindContainer::Unset => {}
                            crate::classifiers::PackageKindContainer::Value(__child) => {
                                match __child.as_ref() {
                                    crate::classifiers::PackageKindChild::Package(_) => {
                                        document_root
                                            .add_child(xml_builder::XMLElement::new("uml:Package"))
                                            .expect(
                                                "adding a root object to the XMI document should not fail",
                                            );
                                    }
                                    crate::classifiers::PackageKindChild::Model(_) => {
                                        document_root
                                            .add_child(xml_builder::XMLElement::new("uml:Model"))
                                            .expect(
                                                "adding a root object to the XMI document should not fail",
                                            );
                                    }
                                }
                            }
                            crate::classifiers::PackageKindContainer::Conflicts(
                                __children,
                            ) => {
                                for __child in __children {
                                    match __child {
                                        crate::classifiers::PackageKindChild::Package(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                        crate::classifiers::PackageKindChild::Model(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    crate::classifiers::PackageableKindChild::TType(__child_log) => {
                        match &__child_log.child {
                            crate::classifiers::TTypeKindContainer::Unset => {}
                            crate::classifiers::TTypeKindContainer::Value(__child) => {
                                match __child.as_ref() {
                                    crate::classifiers::TTypeKindChild::DataType(
                                        __child_log,
                                    ) => {
                                        match &__child_log.child {
                                            crate::classifiers::DataTypeKindContainer::Unset => {}
                                            crate::classifiers::DataTypeKindContainer::Value(
                                                __child,
                                            ) => {
                                                match __child.as_ref() {
                                                    crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                        document_root
                                                            .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                            .expect(
                                                                "adding a root object to the XMI document should not fail",
                                                            );
                                                    }
                                                    crate::classifiers::DataTypeKindChild::Class(_) => {
                                                        document_root
                                                            .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                            .expect(
                                                                "adding a root object to the XMI document should not fail",
                                                            );
                                                    }
                                                }
                                            }
                                            crate::classifiers::DataTypeKindContainer::Conflicts(
                                                __children,
                                            ) => {
                                                for __child in __children {
                                                    match __child {
                                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                        document_root
                                            .add_child(
                                                xml_builder::XMLElement::new("uml:PrimitiveType"),
                                            )
                                            .expect(
                                                "adding a root object to the XMI document should not fail",
                                            );
                                    }
                                    crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                        document_root
                                            .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                            .expect(
                                                "adding a root object to the XMI document should not fail",
                                            );
                                    }
                                }
                            }
                            crate::classifiers::TTypeKindContainer::Conflicts(
                                __children,
                            ) => {
                                for __child in __children {
                                    match __child {
                                        crate::classifiers::TTypeKindChild::DataType(
                                            __child_log,
                                        ) => {
                                            match &__child_log.child {
                                                crate::classifiers::DataTypeKindContainer::Unset => {}
                                                crate::classifiers::DataTypeKindContainer::Value(
                                                    __child,
                                                ) => {
                                                    match __child.as_ref() {
                                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                                crate::classifiers::DataTypeKindContainer::Conflicts(
                                                    __children,
                                                ) => {
                                                    for __child in __children {
                                                        match __child {
                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                            document_root
                                                .add_child(
                                                    xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                )
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                        crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    crate::classifiers::PackageableKindChild::Association(_) => {
                        document_root
                            .add_child(xml_builder::XMLElement::new("uml:Association"))
                            .expect(
                                "adding a root object to the XMI document should not fail",
                            );
                    }
                }
            }
            crate::classifiers::PackageableKindContainer::Conflicts(__children) => {
                for __child in __children {
                    match __child {
                        crate::classifiers::PackageableKindChild::Package(
                            __child_log,
                        ) => {
                            match &__child_log.child {
                                crate::classifiers::PackageKindContainer::Unset => {}
                                crate::classifiers::PackageKindContainer::Value(__child) => {
                                    match __child.as_ref() {
                                        crate::classifiers::PackageKindChild::Package(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                        crate::classifiers::PackageKindChild::Model(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                    }
                                }
                                crate::classifiers::PackageKindContainer::Conflicts(
                                    __children,
                                ) => {
                                    for __child in __children {
                                        match __child {
                                            crate::classifiers::PackageKindChild::Package(_) => {
                                                document_root
                                                    .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                    .expect(
                                                        "adding a root object to the XMI document should not fail",
                                                    );
                                            }
                                            crate::classifiers::PackageKindChild::Model(_) => {
                                                document_root
                                                    .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                    .expect(
                                                        "adding a root object to the XMI document should not fail",
                                                    );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        crate::classifiers::PackageableKindChild::TType(__child_log) => {
                            match &__child_log.child {
                                crate::classifiers::TTypeKindContainer::Unset => {}
                                crate::classifiers::TTypeKindContainer::Value(__child) => {
                                    match __child.as_ref() {
                                        crate::classifiers::TTypeKindChild::DataType(
                                            __child_log,
                                        ) => {
                                            match &__child_log.child {
                                                crate::classifiers::DataTypeKindContainer::Unset => {}
                                                crate::classifiers::DataTypeKindContainer::Value(
                                                    __child,
                                                ) => {
                                                    match __child.as_ref() {
                                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                                crate::classifiers::DataTypeKindContainer::Conflicts(
                                                    __children,
                                                ) => {
                                                    for __child in __children {
                                                        match __child {
                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                            document_root
                                                .add_child(
                                                    xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                )
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                        crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                    }
                                }
                                crate::classifiers::TTypeKindContainer::Conflicts(
                                    __children,
                                ) => {
                                    for __child in __children {
                                        match __child {
                                            crate::classifiers::TTypeKindChild::DataType(
                                                __child_log,
                                            ) => {
                                                match &__child_log.child {
                                                    crate::classifiers::DataTypeKindContainer::Unset => {}
                                                    crate::classifiers::DataTypeKindContainer::Value(
                                                        __child,
                                                    ) => {
                                                        match __child.as_ref() {
                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                    crate::classifiers::DataTypeKindContainer::Conflicts(
                                                        __children,
                                                    ) => {
                                                        for __child in __children {
                                                            match __child {
                                                                crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                    document_root
                                                                        .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                        .expect(
                                                                            "adding a root object to the XMI document should not fail",
                                                                        );
                                                                }
                                                                crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                    document_root
                                                                        .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                        .expect(
                                                                            "adding a root object to the XMI document should not fail",
                                                                        );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                                document_root
                                                    .add_child(
                                                        xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                    )
                                                    .expect(
                                                        "adding a root object to the XMI document should not fail",
                                                    );
                                            }
                                            crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                                document_root
                                                    .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                    .expect(
                                                        "adding a root object to the XMI document should not fail",
                                                    );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        crate::classifiers::PackageableKindChild::Association(_) => {
                            document_root
                                .add_child(xml_builder::XMLElement::new("uml:Association"))
                                .expect(
                                    "adding a root object to the XMI document should not fail",
                                );
                        }
                    }
                }
            }
        }
        match &self.classifier_log.child {
            crate::classifiers::ClassifierKindContainer::Unset => {}
            crate::classifiers::ClassifierKindContainer::Value(__child) => {
                match __child.as_ref() {
                    crate::classifiers::ClassifierKindChild::Package(__child_log) => {
                        match &__child_log.child {
                            crate::classifiers::PackageKindContainer::Unset => {}
                            crate::classifiers::PackageKindContainer::Value(__child) => {
                                match __child.as_ref() {
                                    crate::classifiers::PackageKindChild::Package(_) => {
                                        document_root
                                            .add_child(xml_builder::XMLElement::new("uml:Package"))
                                            .expect(
                                                "adding a root object to the XMI document should not fail",
                                            );
                                    }
                                    crate::classifiers::PackageKindChild::Model(_) => {
                                        document_root
                                            .add_child(xml_builder::XMLElement::new("uml:Model"))
                                            .expect(
                                                "adding a root object to the XMI document should not fail",
                                            );
                                    }
                                }
                            }
                            crate::classifiers::PackageKindContainer::Conflicts(
                                __children,
                            ) => {
                                for __child in __children {
                                    match __child {
                                        crate::classifiers::PackageKindChild::Package(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                        crate::classifiers::PackageKindChild::Model(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    crate::classifiers::ClassifierKindChild::TType(__child_log) => {
                        match &__child_log.child {
                            crate::classifiers::TTypeKindContainer::Unset => {}
                            crate::classifiers::TTypeKindContainer::Value(__child) => {
                                match __child.as_ref() {
                                    crate::classifiers::TTypeKindChild::DataType(
                                        __child_log,
                                    ) => {
                                        match &__child_log.child {
                                            crate::classifiers::DataTypeKindContainer::Unset => {}
                                            crate::classifiers::DataTypeKindContainer::Value(
                                                __child,
                                            ) => {
                                                match __child.as_ref() {
                                                    crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                        document_root
                                                            .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                            .expect(
                                                                "adding a root object to the XMI document should not fail",
                                                            );
                                                    }
                                                    crate::classifiers::DataTypeKindChild::Class(_) => {
                                                        document_root
                                                            .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                            .expect(
                                                                "adding a root object to the XMI document should not fail",
                                                            );
                                                    }
                                                }
                                            }
                                            crate::classifiers::DataTypeKindContainer::Conflicts(
                                                __children,
                                            ) => {
                                                for __child in __children {
                                                    match __child {
                                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                        document_root
                                            .add_child(
                                                xml_builder::XMLElement::new("uml:PrimitiveType"),
                                            )
                                            .expect(
                                                "adding a root object to the XMI document should not fail",
                                            );
                                    }
                                    crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                        document_root
                                            .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                            .expect(
                                                "adding a root object to the XMI document should not fail",
                                            );
                                    }
                                }
                            }
                            crate::classifiers::TTypeKindContainer::Conflicts(
                                __children,
                            ) => {
                                for __child in __children {
                                    match __child {
                                        crate::classifiers::TTypeKindChild::DataType(
                                            __child_log,
                                        ) => {
                                            match &__child_log.child {
                                                crate::classifiers::DataTypeKindContainer::Unset => {}
                                                crate::classifiers::DataTypeKindContainer::Value(
                                                    __child,
                                                ) => {
                                                    match __child.as_ref() {
                                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                                crate::classifiers::DataTypeKindContainer::Conflicts(
                                                    __children,
                                                ) => {
                                                    for __child in __children {
                                                        match __child {
                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                            document_root
                                                .add_child(
                                                    xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                )
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                        crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::classifiers::ClassifierKindContainer::Conflicts(__children) => {
                for __child in __children {
                    match __child {
                        crate::classifiers::ClassifierKindChild::Package(__child_log) => {
                            match &__child_log.child {
                                crate::classifiers::PackageKindContainer::Unset => {}
                                crate::classifiers::PackageKindContainer::Value(__child) => {
                                    match __child.as_ref() {
                                        crate::classifiers::PackageKindChild::Package(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                        crate::classifiers::PackageKindChild::Model(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                    }
                                }
                                crate::classifiers::PackageKindContainer::Conflicts(
                                    __children,
                                ) => {
                                    for __child in __children {
                                        match __child {
                                            crate::classifiers::PackageKindChild::Package(_) => {
                                                document_root
                                                    .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                    .expect(
                                                        "adding a root object to the XMI document should not fail",
                                                    );
                                            }
                                            crate::classifiers::PackageKindChild::Model(_) => {
                                                document_root
                                                    .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                    .expect(
                                                        "adding a root object to the XMI document should not fail",
                                                    );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        crate::classifiers::ClassifierKindChild::TType(__child_log) => {
                            match &__child_log.child {
                                crate::classifiers::TTypeKindContainer::Unset => {}
                                crate::classifiers::TTypeKindContainer::Value(__child) => {
                                    match __child.as_ref() {
                                        crate::classifiers::TTypeKindChild::DataType(
                                            __child_log,
                                        ) => {
                                            match &__child_log.child {
                                                crate::classifiers::DataTypeKindContainer::Unset => {}
                                                crate::classifiers::DataTypeKindContainer::Value(
                                                    __child,
                                                ) => {
                                                    match __child.as_ref() {
                                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                                crate::classifiers::DataTypeKindContainer::Conflicts(
                                                    __children,
                                                ) => {
                                                    for __child in __children {
                                                        match __child {
                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                            document_root
                                                .add_child(
                                                    xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                )
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                        crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                    }
                                }
                                crate::classifiers::TTypeKindContainer::Conflicts(
                                    __children,
                                ) => {
                                    for __child in __children {
                                        match __child {
                                            crate::classifiers::TTypeKindChild::DataType(
                                                __child_log,
                                            ) => {
                                                match &__child_log.child {
                                                    crate::classifiers::DataTypeKindContainer::Unset => {}
                                                    crate::classifiers::DataTypeKindContainer::Value(
                                                        __child,
                                                    ) => {
                                                        match __child.as_ref() {
                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                    crate::classifiers::DataTypeKindContainer::Conflicts(
                                                        __children,
                                                    ) => {
                                                        for __child in __children {
                                                            match __child {
                                                                crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                    document_root
                                                                        .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                        .expect(
                                                                            "adding a root object to the XMI document should not fail",
                                                                        );
                                                                }
                                                                crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                    document_root
                                                                        .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                        .expect(
                                                                            "adding a root object to the XMI document should not fail",
                                                                        );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                                document_root
                                                    .add_child(
                                                        xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                    )
                                                    .expect(
                                                        "adding a root object to the XMI document should not fail",
                                                    );
                                            }
                                            crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                                document_root
                                                    .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                    .expect(
                                                        "adding a root object to the XMI document should not fail",
                                                    );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        match &self.t_type_log.child {
            crate::classifiers::TTypeKindContainer::Unset => {}
            crate::classifiers::TTypeKindContainer::Value(__child) => {
                match __child.as_ref() {
                    crate::classifiers::TTypeKindChild::DataType(__child_log) => {
                        match &__child_log.child {
                            crate::classifiers::DataTypeKindContainer::Unset => {}
                            crate::classifiers::DataTypeKindContainer::Value(__child) => {
                                match __child.as_ref() {
                                    crate::classifiers::DataTypeKindChild::DataType(_) => {
                                        document_root
                                            .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                            .expect(
                                                "adding a root object to the XMI document should not fail",
                                            );
                                    }
                                    crate::classifiers::DataTypeKindChild::Class(_) => {
                                        document_root
                                            .add_child(xml_builder::XMLElement::new("uml:Class"))
                                            .expect(
                                                "adding a root object to the XMI document should not fail",
                                            );
                                    }
                                }
                            }
                            crate::classifiers::DataTypeKindContainer::Conflicts(
                                __children,
                            ) => {
                                for __child in __children {
                                    match __child {
                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                        document_root
                            .add_child(xml_builder::XMLElement::new("uml:PrimitiveType"))
                            .expect(
                                "adding a root object to the XMI document should not fail",
                            );
                    }
                    crate::classifiers::TTypeKindChild::Enumeration(_) => {
                        document_root
                            .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                            .expect(
                                "adding a root object to the XMI document should not fail",
                            );
                    }
                }
            }
            crate::classifiers::TTypeKindContainer::Conflicts(__children) => {
                for __child in __children {
                    match __child {
                        crate::classifiers::TTypeKindChild::DataType(__child_log) => {
                            match &__child_log.child {
                                crate::classifiers::DataTypeKindContainer::Unset => {}
                                crate::classifiers::DataTypeKindContainer::Value(
                                    __child,
                                ) => {
                                    match __child.as_ref() {
                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                            document_root
                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                .expect(
                                                    "adding a root object to the XMI document should not fail",
                                                );
                                        }
                                    }
                                }
                                crate::classifiers::DataTypeKindContainer::Conflicts(
                                    __children,
                                ) => {
                                    for __child in __children {
                                        match __child {
                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                document_root
                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                    .expect(
                                                        "adding a root object to the XMI document should not fail",
                                                    );
                                            }
                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                document_root
                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                    .expect(
                                                        "adding a root object to the XMI document should not fail",
                                                    );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                            document_root
                                .add_child(
                                    xml_builder::XMLElement::new("uml:PrimitiveType"),
                                )
                                .expect(
                                    "adding a root object to the XMI document should not fail",
                                );
                        }
                        crate::classifiers::TTypeKindChild::Enumeration(_) => {
                            document_root
                                .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                .expect(
                                    "adding a root object to the XMI document should not fail",
                                );
                        }
                    }
                }
            }
        }
        match &self.model_element_log.child {
            crate::classifiers::ModelElementKindContainer::Unset => {}
            crate::classifiers::ModelElementKindContainer::Value(__child) => {
                match __child.as_ref() {
                    crate::classifiers::ModelElementKindChild::Classifier(
                        __child_log,
                    ) => {
                        match &__child_log.child {
                            crate::classifiers::ClassifierKindContainer::Unset => {}
                            crate::classifiers::ClassifierKindContainer::Value(
                                __child,
                            ) => {
                                match __child.as_ref() {
                                    crate::classifiers::ClassifierKindChild::Package(
                                        __child_log,
                                    ) => {
                                        match &__child_log.child {
                                            crate::classifiers::PackageKindContainer::Unset => {}
                                            crate::classifiers::PackageKindContainer::Value(__child) => {
                                                match __child.as_ref() {
                                                    crate::classifiers::PackageKindChild::Package(_) => {
                                                        document_root
                                                            .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                            .expect(
                                                                "adding a root object to the XMI document should not fail",
                                                            );
                                                    }
                                                    crate::classifiers::PackageKindChild::Model(_) => {
                                                        document_root
                                                            .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                            .expect(
                                                                "adding a root object to the XMI document should not fail",
                                                            );
                                                    }
                                                }
                                            }
                                            crate::classifiers::PackageKindContainer::Conflicts(
                                                __children,
                                            ) => {
                                                for __child in __children {
                                                    match __child {
                                                        crate::classifiers::PackageKindChild::Package(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::PackageKindChild::Model(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    crate::classifiers::ClassifierKindChild::TType(
                                        __child_log,
                                    ) => {
                                        match &__child_log.child {
                                            crate::classifiers::TTypeKindContainer::Unset => {}
                                            crate::classifiers::TTypeKindContainer::Value(__child) => {
                                                match __child.as_ref() {
                                                    crate::classifiers::TTypeKindChild::DataType(
                                                        __child_log,
                                                    ) => {
                                                        match &__child_log.child {
                                                            crate::classifiers::DataTypeKindContainer::Unset => {}
                                                            crate::classifiers::DataTypeKindContainer::Value(
                                                                __child,
                                                            ) => {
                                                                match __child.as_ref() {
                                                                    crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                        document_root
                                                                            .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                            .expect(
                                                                                "adding a root object to the XMI document should not fail",
                                                                            );
                                                                    }
                                                                    crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                        document_root
                                                                            .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                            .expect(
                                                                                "adding a root object to the XMI document should not fail",
                                                                            );
                                                                    }
                                                                }
                                                            }
                                                            crate::classifiers::DataTypeKindContainer::Conflicts(
                                                                __children,
                                                            ) => {
                                                                for __child in __children {
                                                                    match __child {
                                                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                            document_root
                                                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                .expect(
                                                                                    "adding a root object to the XMI document should not fail",
                                                                                );
                                                                        }
                                                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                            document_root
                                                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                .expect(
                                                                                    "adding a root object to the XMI document should not fail",
                                                                                );
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                                        document_root
                                                            .add_child(
                                                                xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                            )
                                                            .expect(
                                                                "adding a root object to the XMI document should not fail",
                                                            );
                                                    }
                                                    crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                                        document_root
                                                            .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                            .expect(
                                                                "adding a root object to the XMI document should not fail",
                                                            );
                                                    }
                                                }
                                            }
                                            crate::classifiers::TTypeKindContainer::Conflicts(
                                                __children,
                                            ) => {
                                                for __child in __children {
                                                    match __child {
                                                        crate::classifiers::TTypeKindChild::DataType(
                                                            __child_log,
                                                        ) => {
                                                            match &__child_log.child {
                                                                crate::classifiers::DataTypeKindContainer::Unset => {}
                                                                crate::classifiers::DataTypeKindContainer::Value(
                                                                    __child,
                                                                ) => {
                                                                    match __child.as_ref() {
                                                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                            document_root
                                                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                .expect(
                                                                                    "adding a root object to the XMI document should not fail",
                                                                                );
                                                                        }
                                                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                            document_root
                                                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                .expect(
                                                                                    "adding a root object to the XMI document should not fail",
                                                                                );
                                                                        }
                                                                    }
                                                                }
                                                                crate::classifiers::DataTypeKindContainer::Conflicts(
                                                                    __children,
                                                                ) => {
                                                                    for __child in __children {
                                                                        match __child {
                                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                                            document_root
                                                                .add_child(
                                                                    xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                                )
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            crate::classifiers::ClassifierKindContainer::Conflicts(
                                __children,
                            ) => {
                                for __child in __children {
                                    match __child {
                                        crate::classifiers::ClassifierKindChild::Package(
                                            __child_log,
                                        ) => {
                                            match &__child_log.child {
                                                crate::classifiers::PackageKindContainer::Unset => {}
                                                crate::classifiers::PackageKindContainer::Value(__child) => {
                                                    match __child.as_ref() {
                                                        crate::classifiers::PackageKindChild::Package(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::PackageKindChild::Model(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                                crate::classifiers::PackageKindContainer::Conflicts(
                                                    __children,
                                                ) => {
                                                    for __child in __children {
                                                        match __child {
                                                            crate::classifiers::PackageKindChild::Package(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::PackageKindChild::Model(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        crate::classifiers::ClassifierKindChild::TType(
                                            __child_log,
                                        ) => {
                                            match &__child_log.child {
                                                crate::classifiers::TTypeKindContainer::Unset => {}
                                                crate::classifiers::TTypeKindContainer::Value(__child) => {
                                                    match __child.as_ref() {
                                                        crate::classifiers::TTypeKindChild::DataType(
                                                            __child_log,
                                                        ) => {
                                                            match &__child_log.child {
                                                                crate::classifiers::DataTypeKindContainer::Unset => {}
                                                                crate::classifiers::DataTypeKindContainer::Value(
                                                                    __child,
                                                                ) => {
                                                                    match __child.as_ref() {
                                                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                            document_root
                                                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                .expect(
                                                                                    "adding a root object to the XMI document should not fail",
                                                                                );
                                                                        }
                                                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                            document_root
                                                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                .expect(
                                                                                    "adding a root object to the XMI document should not fail",
                                                                                );
                                                                        }
                                                                    }
                                                                }
                                                                crate::classifiers::DataTypeKindContainer::Conflicts(
                                                                    __children,
                                                                ) => {
                                                                    for __child in __children {
                                                                        match __child {
                                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                                            document_root
                                                                .add_child(
                                                                    xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                                )
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                                crate::classifiers::TTypeKindContainer::Conflicts(
                                                    __children,
                                                ) => {
                                                    for __child in __children {
                                                        match __child {
                                                            crate::classifiers::TTypeKindChild::DataType(
                                                                __child_log,
                                                            ) => {
                                                                match &__child_log.child {
                                                                    crate::classifiers::DataTypeKindContainer::Unset => {}
                                                                    crate::classifiers::DataTypeKindContainer::Value(
                                                                        __child,
                                                                    ) => {
                                                                        match __child.as_ref() {
                                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                        }
                                                                    }
                                                                    crate::classifiers::DataTypeKindContainer::Conflicts(
                                                                        __children,
                                                                    ) => {
                                                                        for __child in __children {
                                                                            match __child {
                                                                                crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                                    document_root
                                                                                        .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                        .expect(
                                                                                            "adding a root object to the XMI document should not fail",
                                                                                        );
                                                                                }
                                                                                crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                                    document_root
                                                                                        .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                        .expect(
                                                                                            "adding a root object to the XMI document should not fail",
                                                                                        );
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                                                document_root
                                                                    .add_child(
                                                                        xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                                    )
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    crate::classifiers::ModelElementKindChild::Property(_) => {
                        document_root
                            .add_child(xml_builder::XMLElement::new("uml:Property"))
                            .expect(
                                "adding a root object to the XMI document should not fail",
                            );
                    }
                    crate::classifiers::ModelElementKindChild::Association(_) => {
                        document_root
                            .add_child(xml_builder::XMLElement::new("uml:Association"))
                            .expect(
                                "adding a root object to the XMI document should not fail",
                            );
                    }
                }
            }
            crate::classifiers::ModelElementKindContainer::Conflicts(__children) => {
                for __child in __children {
                    match __child {
                        crate::classifiers::ModelElementKindChild::Classifier(
                            __child_log,
                        ) => {
                            match &__child_log.child {
                                crate::classifiers::ClassifierKindContainer::Unset => {}
                                crate::classifiers::ClassifierKindContainer::Value(
                                    __child,
                                ) => {
                                    match __child.as_ref() {
                                        crate::classifiers::ClassifierKindChild::Package(
                                            __child_log,
                                        ) => {
                                            match &__child_log.child {
                                                crate::classifiers::PackageKindContainer::Unset => {}
                                                crate::classifiers::PackageKindContainer::Value(__child) => {
                                                    match __child.as_ref() {
                                                        crate::classifiers::PackageKindChild::Package(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::PackageKindChild::Model(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                                crate::classifiers::PackageKindContainer::Conflicts(
                                                    __children,
                                                ) => {
                                                    for __child in __children {
                                                        match __child {
                                                            crate::classifiers::PackageKindChild::Package(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::PackageKindChild::Model(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        crate::classifiers::ClassifierKindChild::TType(
                                            __child_log,
                                        ) => {
                                            match &__child_log.child {
                                                crate::classifiers::TTypeKindContainer::Unset => {}
                                                crate::classifiers::TTypeKindContainer::Value(__child) => {
                                                    match __child.as_ref() {
                                                        crate::classifiers::TTypeKindChild::DataType(
                                                            __child_log,
                                                        ) => {
                                                            match &__child_log.child {
                                                                crate::classifiers::DataTypeKindContainer::Unset => {}
                                                                crate::classifiers::DataTypeKindContainer::Value(
                                                                    __child,
                                                                ) => {
                                                                    match __child.as_ref() {
                                                                        crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                            document_root
                                                                                .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                .expect(
                                                                                    "adding a root object to the XMI document should not fail",
                                                                                );
                                                                        }
                                                                        crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                            document_root
                                                                                .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                .expect(
                                                                                    "adding a root object to the XMI document should not fail",
                                                                                );
                                                                        }
                                                                    }
                                                                }
                                                                crate::classifiers::DataTypeKindContainer::Conflicts(
                                                                    __children,
                                                                ) => {
                                                                    for __child in __children {
                                                                        match __child {
                                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                                            document_root
                                                                .add_child(
                                                                    xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                                )
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                        crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                                            document_root
                                                                .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                                .expect(
                                                                    "adding a root object to the XMI document should not fail",
                                                                );
                                                        }
                                                    }
                                                }
                                                crate::classifiers::TTypeKindContainer::Conflicts(
                                                    __children,
                                                ) => {
                                                    for __child in __children {
                                                        match __child {
                                                            crate::classifiers::TTypeKindChild::DataType(
                                                                __child_log,
                                                            ) => {
                                                                match &__child_log.child {
                                                                    crate::classifiers::DataTypeKindContainer::Unset => {}
                                                                    crate::classifiers::DataTypeKindContainer::Value(
                                                                        __child,
                                                                    ) => {
                                                                        match __child.as_ref() {
                                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                        }
                                                                    }
                                                                    crate::classifiers::DataTypeKindContainer::Conflicts(
                                                                        __children,
                                                                    ) => {
                                                                        for __child in __children {
                                                                            match __child {
                                                                                crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                                    document_root
                                                                                        .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                        .expect(
                                                                                            "adding a root object to the XMI document should not fail",
                                                                                        );
                                                                                }
                                                                                crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                                    document_root
                                                                                        .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                        .expect(
                                                                                            "adding a root object to the XMI document should not fail",
                                                                                        );
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                                                document_root
                                                                    .add_child(
                                                                        xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                                    )
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                crate::classifiers::ClassifierKindContainer::Conflicts(
                                    __children,
                                ) => {
                                    for __child in __children {
                                        match __child {
                                            crate::classifiers::ClassifierKindChild::Package(
                                                __child_log,
                                            ) => {
                                                match &__child_log.child {
                                                    crate::classifiers::PackageKindContainer::Unset => {}
                                                    crate::classifiers::PackageKindContainer::Value(__child) => {
                                                        match __child.as_ref() {
                                                            crate::classifiers::PackageKindChild::Package(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::PackageKindChild::Model(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                    crate::classifiers::PackageKindContainer::Conflicts(
                                                        __children,
                                                    ) => {
                                                        for __child in __children {
                                                            match __child {
                                                                crate::classifiers::PackageKindChild::Package(_) => {
                                                                    document_root
                                                                        .add_child(xml_builder::XMLElement::new("uml:Package"))
                                                                        .expect(
                                                                            "adding a root object to the XMI document should not fail",
                                                                        );
                                                                }
                                                                crate::classifiers::PackageKindChild::Model(_) => {
                                                                    document_root
                                                                        .add_child(xml_builder::XMLElement::new("uml:Model"))
                                                                        .expect(
                                                                            "adding a root object to the XMI document should not fail",
                                                                        );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            crate::classifiers::ClassifierKindChild::TType(
                                                __child_log,
                                            ) => {
                                                match &__child_log.child {
                                                    crate::classifiers::TTypeKindContainer::Unset => {}
                                                    crate::classifiers::TTypeKindContainer::Value(__child) => {
                                                        match __child.as_ref() {
                                                            crate::classifiers::TTypeKindChild::DataType(
                                                                __child_log,
                                                            ) => {
                                                                match &__child_log.child {
                                                                    crate::classifiers::DataTypeKindContainer::Unset => {}
                                                                    crate::classifiers::DataTypeKindContainer::Value(
                                                                        __child,
                                                                    ) => {
                                                                        match __child.as_ref() {
                                                                            crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                            crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                                document_root
                                                                                    .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                    .expect(
                                                                                        "adding a root object to the XMI document should not fail",
                                                                                    );
                                                                            }
                                                                        }
                                                                    }
                                                                    crate::classifiers::DataTypeKindContainer::Conflicts(
                                                                        __children,
                                                                    ) => {
                                                                        for __child in __children {
                                                                            match __child {
                                                                                crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                                    document_root
                                                                                        .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                        .expect(
                                                                                            "adding a root object to the XMI document should not fail",
                                                                                        );
                                                                                }
                                                                                crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                                    document_root
                                                                                        .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                        .expect(
                                                                                            "adding a root object to the XMI document should not fail",
                                                                                        );
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                                                document_root
                                                                    .add_child(
                                                                        xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                                    )
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                            crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                                                document_root
                                                                    .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                                    .expect(
                                                                        "adding a root object to the XMI document should not fail",
                                                                    );
                                                            }
                                                        }
                                                    }
                                                    crate::classifiers::TTypeKindContainer::Conflicts(
                                                        __children,
                                                    ) => {
                                                        for __child in __children {
                                                            match __child {
                                                                crate::classifiers::TTypeKindChild::DataType(
                                                                    __child_log,
                                                                ) => {
                                                                    match &__child_log.child {
                                                                        crate::classifiers::DataTypeKindContainer::Unset => {}
                                                                        crate::classifiers::DataTypeKindContainer::Value(
                                                                            __child,
                                                                        ) => {
                                                                            match __child.as_ref() {
                                                                                crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                                    document_root
                                                                                        .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                        .expect(
                                                                                            "adding a root object to the XMI document should not fail",
                                                                                        );
                                                                                }
                                                                                crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                                    document_root
                                                                                        .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                        .expect(
                                                                                            "adding a root object to the XMI document should not fail",
                                                                                        );
                                                                                }
                                                                            }
                                                                        }
                                                                        crate::classifiers::DataTypeKindContainer::Conflicts(
                                                                            __children,
                                                                        ) => {
                                                                            for __child in __children {
                                                                                match __child {
                                                                                    crate::classifiers::DataTypeKindChild::DataType(_) => {
                                                                                        document_root
                                                                                            .add_child(xml_builder::XMLElement::new("uml:DataType"))
                                                                                            .expect(
                                                                                                "adding a root object to the XMI document should not fail",
                                                                                            );
                                                                                    }
                                                                                    crate::classifiers::DataTypeKindChild::Class(_) => {
                                                                                        document_root
                                                                                            .add_child(xml_builder::XMLElement::new("uml:Class"))
                                                                                            .expect(
                                                                                                "adding a root object to the XMI document should not fail",
                                                                                            );
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                crate::classifiers::TTypeKindChild::PrimitiveType(_) => {
                                                                    document_root
                                                                        .add_child(
                                                                            xml_builder::XMLElement::new("uml:PrimitiveType"),
                                                                        )
                                                                        .expect(
                                                                            "adding a root object to the XMI document should not fail",
                                                                        );
                                                                }
                                                                crate::classifiers::TTypeKindChild::Enumeration(_) => {
                                                                    document_root
                                                                        .add_child(xml_builder::XMLElement::new("uml:Enumeration"))
                                                                        .expect(
                                                                            "adding a root object to the XMI document should not fail",
                                                                        );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        crate::classifiers::ModelElementKindChild::Property(_) => {
                            document_root
                                .add_child(xml_builder::XMLElement::new("uml:Property"))
                                .expect(
                                    "adding a root object to the XMI document should not fail",
                                );
                        }
                        crate::classifiers::ModelElementKindChild::Association(_) => {
                            document_root
                                .add_child(xml_builder::XMLElement::new("uml:Association"))
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
