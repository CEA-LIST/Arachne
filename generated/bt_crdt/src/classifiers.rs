/// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly
mod __classifiers {
    pub use moirai_macros::record;
    pub use moirai_macros::union;
    pub use moirai_crdt::list::nested_list::NestedListLog;
    pub use moirai_protocol::state::graph_log::GraphLog;
    pub use moirai_crdt::list::eg_walker::List;
    pub use moirai_protocol::state::log::BoxedLog;
    pub use moirai_crdt::option::OptionLog;
}
__classifiers::record!(
    Root { behaviortrees : __classifiers::NestedListLog < BehaviorTreeLog > => Vec <
    BehaviorTreeValue >, main : BehaviorTreeLog => BehaviorTreeValue, }
);
__classifiers::record!(
    BehaviorTree { id : __classifiers::GraphLog < __classifiers::List < char > > => Vec <
    char >, child : __classifiers::BoxedLog < TreeNodeKindLog > => Box <
    TreeNodeKindValue >, blackboard : BlackboardLog => BlackboardValue, }
);
__classifiers::union!(
    TreeNodeKind = ExecutionNode(ExecutionNodeKind, ExecutionNodeKindLog =>
    ExecutionNodeKindValue) | Decorator(DecoratorKind, DecoratorKindLog =>
    DecoratorKindValue) | ControlNode(ControlNodeKind, ControlNodeKindLog =>
    ControlNodeKindValue) | SubTree(SubTree, SubTreeLog => SubTreeValue)
);
__classifiers::record!(
    TreeNode { id : __classifiers::GraphLog < __classifiers::List < char > > => Vec <
    char >, name : __classifiers::OptionLog < __classifiers::GraphLog <
    __classifiers::List < char > >> => Option < Vec < char > >, }
);
__classifiers::record!(
    Blackboard { entries : __classifiers::NestedListLog < BlackboardEntryLog > => Vec <
    BlackboardEntryValue >, }
);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Status {
    #[default]
    Running,
    Success,
    Failure,
}
__classifiers::union!(
    ExecutionNodeKind = Action(ActionKind, ActionKindLog => ActionKindValue) |
    Condition(ConditionKind, ConditionKindLog => ConditionKindValue)
);
__classifiers::record!(
    ExecutionNode { tree_node_super : TreeNodeLog => TreeNodeValue, outflowports :
    __classifiers::NestedListLog < OutFlowPortLog > => Vec < OutFlowPortValue >,
    inflowports : __classifiers::NestedListLog < InFlowPortLog > => Vec < InFlowPortValue
    >, }
);
__classifiers::union!(
    DataFlowPortKind = OutFlowPort(OutFlowPort, OutFlowPortLog => OutFlowPortValue) |
    InFlowPort(InFlowPort, InFlowPortLog => InFlowPortValue)
);
__classifiers::record!(DataFlowPort {});
__classifiers::record!(
    OutFlowPort { data_flow_port_super : DataFlowPortLog => DataFlowPortValue, }
);
__classifiers::record!(
    InFlowPort { data_flow_port_super : DataFlowPortLog => DataFlowPortValue, }
);
__classifiers::union!(DecoratorKind = Inverter(Inverter, InverterLog => InverterValue));
__classifiers::record!(
    Decorator { tree_node_super : TreeNodeLog => TreeNodeValue, child :
    __classifiers::BoxedLog < TreeNodeKindLog > => Box < TreeNodeKindValue >, }
);
__classifiers::union!(
    ControlNodeKind = Sequence(Sequence, SequenceLog => SequenceValue) |
    Fallback(Fallback, FallbackLog => FallbackValue)
);
__classifiers::record!(
    ControlNode { tree_node_super : TreeNodeLog => TreeNodeValue, children :
    __classifiers::NestedListLog < __classifiers::BoxedLog < TreeNodeKindLog > > => Vec <
    Box < TreeNodeKindValue > >, }
);
__classifiers::record!(
    Sequence { control_node_super : ControlNodeLog => ControlNodeValue, }
);
__classifiers::record!(
    Fallback { control_node_super : ControlNodeLog => ControlNodeValue, }
);
__classifiers::union!(
    ActionKind = OpenDoor(OpenDoor, OpenDoorLog => OpenDoorValue) | EnterRoom(EnterRoom,
    EnterRoomLog => EnterRoomValue) | CloseDoor(CloseDoor, CloseDoorLog =>
    CloseDoorValue)
);
__classifiers::record!(
    Action { execution_node_super : ExecutionNodeLog => ExecutionNodeValue, }
);
__classifiers::union!(
    ConditionKind = IsDoorOpen(IsDoorOpen, IsDoorOpenLog => IsDoorOpenValue)
);
__classifiers::record!(
    Condition { execution_node_super : ExecutionNodeLog => ExecutionNodeValue, }
);
__classifiers::record!(
    BlackboardEntry { key : __classifiers::GraphLog < __classifiers::List < char > > =>
    Vec < char >, value : __classifiers::GraphLog < __classifiers::List < char > > => Vec
    < char >, }
);
__classifiers::record!(Inverter { decorator_super : DecoratorLog => DecoratorValue, });
__classifiers::record!(IsDoorOpen { condition_super : ConditionLog => ConditionValue, });
__classifiers::record!(OpenDoor { action_super : ActionLog => ActionValue, });
__classifiers::record!(EnterRoom { action_super : ActionLog => ActionValue, });
__classifiers::record!(CloseDoor { action_super : ActionLog => ActionValue, });
__classifiers::record!(
    SubTree { tree_node_super : TreeNodeLog => TreeNodeValue, tree : BehaviorTreeLog =>
    BehaviorTreeValue, }
);
