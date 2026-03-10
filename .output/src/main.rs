// use moirai_crdt::list::eg_walker::List as SimpleList;
// use moirai_crdt::list::nested_list::List;
// use moirai_macros::typed_graph::Arc;
// use moirai_protocol::{
//     broadcast::tcsb::Tcsb,
//     crdt::query::Read,
//     replica::{IsReplica, Replica},
// };

// use crate::{
//     generated::{BehaviorTree, Blackboard, BlackboardEntry, Decorator, Inverter, Root, TreeNode},
//     model::{Model, ModelLog},
// };

mod generated;
mod model;

fn main() {
    // let mut replica_a = Replica::<ModelLog, Tcsb<Model>>::new("a".to_string());
    // replica_a.send(Model::Create(Root::New)).unwrap();
    // replica_a
    //     .send(Model::Create(Root::Main(BehaviorTree::Blackboard(
    //         Blackboard::Entries(List::Insert {
    //             pos: 0,
    //             value: BlackboardEntry::Key(SimpleList::Insert {
    //                 pos: 0,
    //                 content: 'a',
    //             }),
    //         }),
    //     ))))
    //     .unwrap();
    // replica_a
    //     .send(Model::Create(Root::Main(BehaviorTree::Child(
    //         TreeNode::Decorator(Decorator::Inverter(Inverter::New)),
    //     ))))
    //     .unwrap();
    // replica_a
    //     .send(Model::Create(Root::Main(BehaviorTree::Blackboard(
    //         Blackboard::Entries(List::Delete { pos: 0 }),
    //     ))))
    //     .unwrap();
    // let (_, graph) = replica_a.query(Read::new());
    // graph.node_weights().for_each(|n| {
    //     println!("Node {:?}", n);
    // });

    // replica_a
    //     .send(Model::Ref(ReferenceManager::AddArc(Refs::Entry(Arc {
    //         source: todo!(),
    //         target: todo!(),
    //         kind: EntryEdge,
    //     }))))
    //     .unwrap();

    // println!("{:#?}", replica_a.query(Read::new()));
}
