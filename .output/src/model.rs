use moirai_crdt::{list::nested_list::List, policy::LwwPolicy};
use moirai_macros::typed_graph;
use moirai_protocol::{
    clock::version_vector::Version,
    crdt::{
        eval::EvalNested,
        pure_crdt::PureCRDT,
        query::{QueryOperation, Read},
    },
    event::{Event, id::EventId},
    state::{log::IsLog, po_log::VecLog},
};

use crate::generated::{BehaviorTree, Blackboard, Root, RootLog, RootValue};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryId(pub EventId);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataFlowPortId(pub EventId);

// impl ValueGenerator for DataFlowPortId {
//     type Config = ();

//     fn generate(rng: &mut impl rand::RngCore, _config: &Self::Config) -> Self {
//         todo!()
//     }
// }

// impl ValueGenerator for EntryId {
//     type Config = ();

//     fn generate(rng: &mut impl rand::RngCore, _config: &Self::Config) -> Self {
//         todo!()
//     }
// }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryEdge;

typed_graph! {
    graph: ReferenceManager,
    vertex: Instance,
    edge: Ref,
    arcs_type: Refs,
    vertices { EntryId, DataFlowPortId },
    connections {
        Entry: DataFlowPortId -> EntryId (EntryEdge) [0, 1],
    }
}

#[derive(Debug, Clone)]
pub enum Model {
    Create(Root),
    Ref(ReferenceManager<LwwPolicy>),
}

#[derive(Debug, Clone, Default)]
pub struct ModelLog {
    pub root_log: RootLog,
    pub reference_manager_log: VecLog<ReferenceManager<LwwPolicy>>,
}

impl IsLog for ModelLog {
    type Value = (RootValue, <ReferenceManager<LwwPolicy> as PureCRDT>::Value);
    type Op = Model;

    fn is_enabled(&self, op: &Self::Op) -> bool {
        match op {
            Model::Create(o) => self.root_log.is_enabled(o),
            Model::Ref(o) => self.reference_manager_log.is_enabled(o),
        }
    }

    // The function must intercept operations that could create/delete a new vertex in the reference manager,
    // and update the reference manager log accordingly.
    // For example, if a new BlackboardEntry is created, a new EntryId vertex should be created in the reference manager.
    fn effect(&mut self, event: Event<Self::Op>) {
        match &event.op() {
            Model::Create(Root::Main(BehaviorTree::Blackboard(Blackboard::Entries(
                List::Insert { .. },
            )))) => {
                let entry_id = EntryId(event.id().clone());
                let new_vertex = ReferenceManager::<LwwPolicy>::AddVertex {
                    id: Instance::EntryId(entry_id),
                };
                self.reference_manager_log
                    .effect(Event::unfold(event.clone(), new_vertex));
            }
            Model::Create(Root::Main(BehaviorTree::Blackboard(Blackboard::Entries(
                List::Delete { pos },
            )))) => {
                let positions = self
                    .root_log
                    .main
                    .blackboard
                    .entries
                    .position
                    .execute_query(Read::new());
                let event_id = positions[*pos].clone();
                let remove_vertex = ReferenceManager::<LwwPolicy>::RemoveVertex {
                    id: Instance::EntryId(EntryId(event_id)),
                };
                self.reference_manager_log
                    .effect(Event::unfold(event.clone(), remove_vertex));
            }
            _ => {}
        }

        match event.op().clone() {
            Model::Create(root) => self.root_log.effect(Event::unfold(event, root)),
            Model::Ref(ref_manager) => self
                .reference_manager_log
                .effect(Event::unfold(event, ref_manager)),
        }
    }

    fn stabilize(&mut self, version: &Version) {
        self.root_log.stabilize(version);
        self.reference_manager_log.stabilize(version);
    }

    fn redundant_by_parent(&mut self, version: &Version, conservative: bool) {
        self.root_log.redundant_by_parent(version, conservative);
        self.reference_manager_log
            .redundant_by_parent(version, conservative);
    }

    fn is_default(&self) -> bool {
        self.root_log.is_default()
    }
}

impl EvalNested<Read<<Self as IsLog>::Value>> for ModelLog {
    fn execute_query(
        &self,
        _q: Read<<Self as IsLog>::Value>,
    ) -> <Read<<Self as IsLog>::Value> as QueryOperation>::Response {
        let root = self.root_log.execute_query(Read::new());
        let reference_manager = self.reference_manager_log.execute_query(Read::new());
        (root, reference_manager)
    }
}
