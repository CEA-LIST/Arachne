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
pub enum Test {
    Foo(__package::Foo),
    Bar(__package::Bar),
    Baz(__package::Baz),
    AddReference(__package::Refs),
    RemoveReference(__package::Refs),
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestValue {
    pub foo: __package::FooValue,
    pub bar: __package::BarValue,
    pub baz: __package::BazValue,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub refs: <__package::ReferenceManager<
        __package::FairPolicy,
    > as __package::PureCRDT>::Value,
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestLog {
    foo_log: __package::FooLog,
    bar_log: __package::BarLog,
    baz_log: __package::BazLog,
    reference_manager_log: __package::VecLog<
        __package::ReferenceManager<__package::FairPolicy>,
    >,
}
impl TestLog {
    pub fn foo_log(&self) -> &__package::FooLog {
        &self.foo_log
    }
    pub fn bar_log(&self) -> &__package::BarLog {
        &self.bar_log
    }
    pub fn baz_log(&self) -> &__package::BazLog {
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
    fn is_enabled(&self, op: &Self::Op) -> bool {
        match op {
            Test::Foo(o) => self.foo_log.is_enabled(o),
            Test::Bar(o) => self.bar_log.is_enabled(o),
            Test::Baz(o) => self.baz_log.is_enabled(o),
            Test::AddReference(o) => {
                self.reference_manager_log
                    .is_enabled(&__package::ReferenceManager::AddArc(o.clone()))
            }
            Test::RemoveReference(o) => {
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
            Test::Foo(o) => {
                __package::IsLog::effect(
                    &mut self.foo_log,
                    __package::Event::unfold(event.clone(), o),
                    __package::ObjectPath::new("test").field("foo"),
                    &mut sink,
                    __package::SinkOwnership::Owned,
                )
            }
            Test::Bar(o) => {
                __package::IsLog::effect(
                    &mut self.bar_log,
                    __package::Event::unfold(event.clone(), o),
                    __package::ObjectPath::new("test").field("bar"),
                    &mut sink,
                    __package::SinkOwnership::Owned,
                )
            }
            Test::Baz(o) => {
                __package::IsLog::effect(
                    &mut self.baz_log,
                    __package::Event::unfold(event.clone(), o),
                    __package::ObjectPath::new("test").field("baz"),
                    &mut sink,
                    __package::SinkOwnership::Owned,
                )
            }
            Test::AddReference(o) => {
                self.reference_manager_log
                    .effect(
                        __package::Event::unfold(
                            event.clone(),
                            __package::ReferenceManager::AddArc(o),
                        ),
                        __package::ObjectPath::new("test"),
                        &mut __package::SinkCollector::new(),
                        __package::SinkOwnership::Owned,
                    )
            }
            Test::RemoveReference(o) => {
                self.reference_manager_log
                    .effect(
                        __package::Event::unfold(
                            event.clone(),
                            __package::ReferenceManager::RemoveArc(o),
                        ),
                        __package::ObjectPath::new("test"),
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
                                __package::ObjectPath::new("test"),
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
                            __package::ObjectPath::new("test"),
                            &mut __package::SinkCollector::new(),
                            __package::SinkOwnership::Owned,
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
        true && self.foo_log.is_default() && self.bar_log.is_default()
            && self.baz_log.is_default()
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
