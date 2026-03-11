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

use crate::generated::{
    Action, ActionFeat, BehaviorTree, Blackboard, Condition, ConditionFeat, ExecutionNode,
    ExecutionNodeFeat, IsDoorOpen, OpenDoor, Root, RootLog, RootValue, TreeNode,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryId(pub EventId);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataFlowPortId(pub EventId);

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
    Root(Root),
    Reference(Refs), // Ref(ReferenceManager<LwwPolicy>),
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
            Model::Root(o) => self.root_log.is_enabled(o),
            Model::Reference(o) => self
                .reference_manager_log
                .is_enabled(&ReferenceManager::AddArc(o.clone())),
        }
    }

    // The function must intercept operations that could create/delete a new vertex in the reference manager,
    // and update the reference manager log accordingly.
    // For example, if a new BlackboardEntry is created, a new EntryId vertex should be created in the reference manager.
    fn effect(&mut self, event: Event<Self::Op>) {
        match &event.op() {
            Model::Root(Root::Main(BehaviorTree::Blackboard(Blackboard::Entries(
                List::Insert { .. },
            )))) => {
                let entry_id = EntryId(event.id().clone());
                let new_vertex = ReferenceManager::<LwwPolicy>::AddVertex {
                    id: Instance::EntryId(entry_id),
                };
                self.reference_manager_log
                    .effect(Event::unfold(event.clone(), new_vertex));
            }
            Model::Root(Root::Main(BehaviorTree::Blackboard(Blackboard::Entries(
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
            Model::Root(Root::Main(BehaviorTree::Child(child))) => {
                if let TreeNode::ExecutionNode(execution_node) = child.as_ref()
                    && let Some(new_vertex) =
                        created_vertex_from_execution_node(event.id(), execution_node)
                {
                    self.reference_manager_log
                        .effect(Event::unfold(event.clone(), new_vertex));
                }
            }
            _ => {}
        }

        match event.op().clone() {
            Model::Root(root) => self.root_log.effect(Event::unfold(event, root)),
            Model::Reference(refs) => self
                .reference_manager_log
                .effect(Event::unfold(event, ReferenceManager::AddArc(refs))),
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

fn created_vertex_from_execution_node(
    event_id: &EventId,
    execution_node: &ExecutionNode,
) -> Option<ReferenceManager<LwwPolicy>> {
    let created_data_flow_port = match execution_node {
        ExecutionNode::Action(Action::OpenDoor(OpenDoor::ActionFeat(
            ActionFeat::ExecutionNodeFeat(ExecutionNodeFeat::Inflowports(List::Insert { .. })),
        )))
        | ExecutionNode::Action(Action::OpenDoor(OpenDoor::ActionFeat(
            ActionFeat::ExecutionNodeFeat(ExecutionNodeFeat::Outflowports(List::Insert { .. })),
        )))
        | ExecutionNode::Condition(Condition::IsDoorOpen(IsDoorOpen::ConditionFeat(
            ConditionFeat::ExecutionNodeFeat(ExecutionNodeFeat::Inflowports(List::Insert {
                ..
            })),
        )))
        | ExecutionNode::Condition(Condition::IsDoorOpen(IsDoorOpen::ConditionFeat(
            ConditionFeat::ExecutionNodeFeat(ExecutionNodeFeat::Outflowports(List::Insert {
                ..
            })),
        ))) => true,
        _ => false,
    };

    if created_data_flow_port {
        Some(ReferenceManager::<LwwPolicy>::AddVertex {
            id: Instance::DataFlowPortId(DataFlowPortId(event_id.clone())),
        })
    } else {
        None
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
