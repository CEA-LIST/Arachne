use moirai_crdt::list::eg_walker::List as SimpleList;
use moirai_crdt::list::nested_list::List;
use moirai_macros::typed_graph::Arc;
use moirai_protocol::{
    broadcast::tcsb::Tcsb,
    crdt::query::Read,
    replica::{IsReplica, Replica},
};

use crate::{
    generated::{
        Action, ActionFeat, BehaviorTree, Blackboard, BlackboardEntry, DataFlowPortFeat,
        ExecutionNode, ExecutionNodeFeat, OpenDoor, OutFlowPort, Root, TreeNode,
    },
    model::{EntryEdge, Model, ModelLog, Refs},
};

mod generated;
mod model;

fn main() {
    let mut replica_a = Replica::<ModelLog, Tcsb<Model>>::new("a".to_string());
    replica_a.send(Model::Root(Root::New)).unwrap();
    replica_a
        .send(Model::Root(Root::Main(BehaviorTree::Blackboard(
            Blackboard::Entries(List::Insert {
                pos: 0,
                value: BlackboardEntry::Key(SimpleList::Insert {
                    pos: 0,
                    content: 'a',
                }),
            }),
        ))))
        .unwrap();
    replica_a
        .send(Model::Root(Root::Main(BehaviorTree::Child(Box::new(
            TreeNode::ExecutionNode(ExecutionNode::Action(Action::OpenDoor(
                OpenDoor::ActionFeat(ActionFeat::ExecutionNodeFeat(
                    ExecutionNodeFeat::Outflowports(List::Insert {
                        pos: 0,
                        value: OutFlowPort::DataFlowPortFeat(DataFlowPortFeat::New),
                    }),
                )),
            ))),
        )))))
        .unwrap();

    // replica_a
    //     .send(Model::Create(Root::Main(BehaviorTree::Blackboard(
    //         Blackboard::Entries(List::Delete { pos: 0 }),
    //     ))))
    //     .unwrap();

    println!("{:?}", replica_a.query(Read::new()));

    let (_, graph) = replica_a.query(Read::new());
    let entry_id = graph
        .node_weights()
        .find_map(|n| match n {
            model::Instance::EntryId(entry_id) => Some(entry_id.clone()),
            model::Instance::DataFlowPortId(_) => None,
        })
        .unwrap();
    let data_flow_port_id = graph
        .node_weights()
        .find_map(|n| {
            if let model::Instance::DataFlowPortId(data_flow_port_id) = n {
                Some(data_flow_port_id.clone())
            } else {
                None
            }
        })
        .unwrap();

    replica_a
        .send(Model::Reference(Refs::Entry(Arc {
            source: data_flow_port_id.clone(),
            target: entry_id.clone(),
            kind: EntryEdge,
        })))
        .unwrap();

    println!(
        "{:?}",
        petgraph::dot::Dot::with_config(&replica_a.query(Read::new()).1, &[])
    );
}
