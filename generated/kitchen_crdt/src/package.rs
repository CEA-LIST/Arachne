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
pub enum Test {
    Foo(crate::classifiers::Foo),
    Bar(crate::classifiers::Bar),
    Baz(crate::classifiers::Baz),
    AddReference(__package::Refs),
    RemoveReference(__package::Refs),
}
#[derive(Debug)]
pub enum TestRejection {
    Foo(<crate::classifiers::FooLog as __package::IsLog>::Rejection),
    Bar(<crate::classifiers::BarLog as __package::IsLog>::Rejection),
    Baz(<crate::classifiers::BazLog as __package::IsLog>::Rejection),
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
impl std::fmt::Display for TestRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Foo(error) => write!(f, "{}: {}", "Foo", error),
            Self::Bar(error) => write!(f, "{}: {}", "Bar", error),
            Self::Baz(error) => write!(f, "{}: {}", "Baz", error),
            Self::AddReference(error) => write!(f, "AddReference: {}", error),
            Self::RemoveReference(error) => write!(f, "RemoveReference: {}", error),
        }
    }
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestValue {
    pub foo: crate::classifiers::FooValue,
    pub bar: crate::classifiers::BarValue,
    pub baz: crate::classifiers::BazValue,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub refs: <__package::ReferenceManager<
        __package::FairPolicy,
    > as __package::PureCRDT>::Value,
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestLog {
    foo_log: crate::classifiers::FooLog,
    bar_log: crate::classifiers::BarLog,
    baz_log: crate::classifiers::BazLog,
    reference_manager_log: __package::VecLog<
        __package::ReferenceManager<__package::FairPolicy>,
    >,
}
impl TestLog {
    pub fn foo_log(&self) -> &crate::classifiers::FooLog {
        &self.foo_log
    }
    pub fn bar_log(&self) -> &crate::classifiers::BarLog {
        &self.bar_log
    }
    pub fn baz_log(&self) -> &crate::classifiers::BazLog {
        &self.baz_log
    }
    pub fn reference_manager_log(
        &self,
    ) -> &__package::VecLog<__package::ReferenceManager<__package::FairPolicy>> {
        &self.reference_manager_log
    }
}
impl __package::IsLog for TestLog {
    type Value = TestValue;
    type Op = Test;
    type Rejection = TestRejection;
    fn is_enabled(&self, op: &Self::Op) -> Result<(), Self::Rejection> {
        match op {
            Test::Foo(o) => self.foo_log.is_enabled(o).map_err(TestRejection::Foo),
            Test::Bar(o) => self.bar_log.is_enabled(o).map_err(TestRejection::Bar),
            Test::Baz(o) => self.baz_log.is_enabled(o).map_err(TestRejection::Baz),
            Test::AddReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::AddArc(o.clone()))
                    .map_err(TestRejection::AddReference)
            }
            Test::RemoveReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::RemoveArc(o.clone()))
                    .map_err(TestRejection::RemoveReference)
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
            let mut ctx = __package::EffectContext::root("test", Some(&mut sink));
            match event.op().clone() {
                Test::Foo(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "foo",
                        |ctx| {
                            self.foo_log.effect(child_event, ctx);
                        },
                    );
                }
                Test::Bar(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "bar",
                        |ctx| {
                            self.bar_log.effect(child_event, ctx);
                        },
                    );
                }
                Test::Baz(o) => {
                    let child_event = __package::ProtocolEvent::unfold(event.clone(), o);
                    ctx.with_field(
                        "baz",
                        |ctx| {
                            self.baz_log.effect(child_event, ctx);
                        },
                    );
                }
                Test::AddReference(o) => {
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
                Test::RemoveReference(o) => {
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
        self.foo_log.stabilize(version);
        self.bar_log.stabilize(version);
        self.baz_log.stabilize(version);
        self.reference_manager_log.stabilize(version);
    }
    fn redundant_by_parent(&mut self, version: &__package::Version, conservative: bool) {
        self.foo_log.redundant_by_parent(version, conservative);
        self.bar_log.redundant_by_parent(version, conservative);
        self.baz_log.redundant_by_parent(version, conservative);
        self.reference_manager_log.redundant_by_parent(version, conservative);
    }
    fn is_default(&self) -> bool {
        self.reference_manager_log.is_default() && self.foo_log.is_default()
            && self.bar_log.is_default() && self.baz_log.is_default()
    }
}
impl __package::EvalNested<__package::Read<<Self as __package::IsLog>::Value>>
for TestLog {
    fn execute_query(
        &self,
        _q: __package::Read<<Self as __package::IsLog>::Value>,
    ) -> <__package::Read<
        <Self as __package::IsLog>::Value,
    > as __package::QueryOperation>::Response {
        TestValue {
            foo: self.foo_log.execute_query(__package::Read::new()),
            bar: self.bar_log.execute_query(__package::Read::new()),
            baz: self.baz_log.execute_query(__package::Read::new()),
            refs: self.reference_manager_log.execute_query(__package::Read::new()),
        }
    }
}
/// Auto-generated [`QueryableLog`] impl — enables `GET /api/state` in `GenericNode`.
///
/// Calls `replica.query(Read::new())` to evaluate the full CRDT state via
/// the `EvalNested<Read<Value>>` chain, then serializes the result to JSON.
impl moirai_network::query::QueryableLog for TestLog {
    fn query_state_json(
        replica: &moirai_protocol::replica::Replica<
            Self,
            moirai_protocol::broadcast::tcsb::Tcsb<Test>,
        >,
    ) -> serde_json::Value {
        use moirai_protocol::replica::IsReplica;
        let value: TestValue = replica.query(__package::Read::new());
        serde_json::to_value(&value)
            .unwrap_or_else(|e| {
                serde_json::json!({ "error" : format!("serialize: {}", e) })
            })
    }
}
impl __package::InternalizeOp for Test {
    fn internalize(self, interner: &__package::Interner) -> Self {
        match self {
            Test::Foo(op) => Test::Foo(op.clone()),
            Test::Bar(op) => Test::Bar(op.clone()),
            Test::Baz(op) => Test::Baz(op.clone()),
            Test::AddReference(op) => Test::AddReference(op.internalize(interner)),
            Test::RemoveReference(op) => Test::RemoveReference(op.internalize(interner)),
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
impl __package::EvalNested<ReadAsEcore> for TestLog {
    fn execute_query(
        &self,
        _q: ReadAsEcore,
    ) -> <ReadAsEcore as __package::QueryOperation>::Response {
        let mut document_root = xml_builder::XMLElement::new("xmi:XMI");
        document_root.add_attribute("xmi:version", "2.0");
        document_root.add_attribute("xmlns:xmi", "http://www.omg.org/XMI");
        document_root
            .add_attribute("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance");
        document_root.add_attribute("xmlns:test", "http://www.example.org/test");
        document_root
            .add_child(xml_builder::XMLElement::new("test:Foo"))
            .expect("adding a root object to the XMI document should not fail");
        document_root
            .add_child(xml_builder::XMLElement::new("test:Bar"))
            .expect("adding a root object to the XMI document should not fail");
        document_root
            .add_child(xml_builder::XMLElement::new("test:Baz"))
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
