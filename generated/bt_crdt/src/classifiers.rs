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
    Root { behaviortrees : __classifiers::NestedListLog < BehaviorTreeLog >, main :
    BehaviorTreeLog, }
);
__classifiers::record!(
    BehaviorTree { id : __classifiers::GraphLog < __classifiers::List < char > >, child :
    __classifiers::BoxedLog < TreeNodeKindLog >, blackboard : BlackboardLog, }
);
__classifiers::union!(
    TreeNodeKind = ExecutionNode(ExecutionNodeKind, ExecutionNodeKindLog) |
    Decorator(DecoratorKind, DecoratorKindLog) | ControlNode(ControlNodeKind,
    ControlNodeKindLog) | SubTree(SubTree, SubTreeLog)
);
__classifiers::record!(
    TreeNode { id : __classifiers::GraphLog < __classifiers::List < char > >, name :
    __classifiers::OptionLog < __classifiers::GraphLog < __classifiers::List < char > >>,
    }
);
__classifiers::record!(
    Blackboard { entries : __classifiers::NestedListLog < BlackboardEntryLog >, }
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
    ExecutionNodeKind = Action(ActionKind, ActionKindLog) | Condition(ConditionKind,
    ConditionKindLog)
);
__classifiers::record!(
    ExecutionNode { tree_node_super : TreeNodeLog, outflowports :
    __classifiers::NestedListLog < OutFlowPortLog >, inflowports :
    __classifiers::NestedListLog < InFlowPortLog >, }
);
__classifiers::union!(
    DataFlowPortKind = OutFlowPort(OutFlowPort, OutFlowPortLog) | InFlowPort(InFlowPort,
    InFlowPortLog)
);
__classifiers::record!(DataFlowPort {});
__classifiers::record!(OutFlowPort { data_flow_port_super : DataFlowPortLog, });
__classifiers::record!(InFlowPort { data_flow_port_super : DataFlowPortLog, });
__classifiers::union!(DecoratorKind = Inverter(Inverter, InverterLog));
__classifiers::record!(
    Decorator { tree_node_super : TreeNodeLog, child : __classifiers::BoxedLog <
    TreeNodeKindLog >, }
);
__classifiers::union!(
    ControlNodeKind = Sequence(Sequence, SequenceLog) | Fallback(Fallback, FallbackLog)
);
__classifiers::record!(
    ControlNode { tree_node_super : TreeNodeLog, children : __classifiers::NestedListLog
    < __classifiers::BoxedLog < TreeNodeKindLog > >, }
);
__classifiers::record!(Sequence { control_node_super : ControlNodeLog, });
__classifiers::record!(Fallback { control_node_super : ControlNodeLog, });
__classifiers::union!(
    ActionKind = OpenDoor(OpenDoor, OpenDoorLog) | EnterRoom(EnterRoom, EnterRoomLog) |
    CloseDoor(CloseDoor, CloseDoorLog)
);
__classifiers::record!(Action { execution_node_super : ExecutionNodeLog, });
__classifiers::union!(ConditionKind = IsDoorOpen(IsDoorOpen, IsDoorOpenLog));
__classifiers::record!(Condition { execution_node_super : ExecutionNodeLog, });
__classifiers::record!(
    BlackboardEntry { key : __classifiers::GraphLog < __classifiers::List < char > >,
    value : __classifiers::GraphLog < __classifiers::List < char > >, }
);
__classifiers::record!(Inverter { decorator_super : DecoratorLog, });
__classifiers::record!(IsDoorOpen { condition_super : ConditionLog, });
__classifiers::record!(OpenDoor { action_super : ActionLog, });
__classifiers::record!(EnterRoom { action_super : ActionLog, });
__classifiers::record!(CloseDoor { action_super : ActionLog, });
__classifiers::record!(
    SubTree { tree_node_super : TreeNodeLog, tree : BehaviorTreeLog, }
);
