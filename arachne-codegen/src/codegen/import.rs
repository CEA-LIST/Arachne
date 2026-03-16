use proc_macro2::TokenStream;
use quote::quote;

use crate::codegen::datatype::crdt::{
    Bag, Collection, Counter, Crdt, Flag, Graph, Map, NestedCrdt, Primitive, Register, Set,
    SimpleCrdt,
};

const PROTOCOL_PREFIX: &str = "moirai_protocol";
const CRDT_PREFIX: &str = "moirai_crdt";
const MACROS_PREFIX: &str = "moirai_macros";

#[derive(Clone, Debug)]
pub enum Import {
    Log(Log),
    Crdt(Crdt),
    CrdtOp(CrdtOp),
    Macros(Macros),
    Protocol(Protocol),
    Custom(String),
}

impl Import {
    pub fn path(&self) -> String {
        match self {
            Import::Log(log) => log.path(),
            Import::Crdt(crdt) => crdt.path(),
            Import::Macros(macros) => macros.path(),
            Import::Protocol(protocol) => protocol.path(),
            Import::Custom(path) => path.clone(),
            Import::CrdtOp(op) => op.path(),
        }
    }

    pub fn to_use_statement(&self) -> TokenStream {
        match self {
            Import::Log(log) => log.to_use_statement(),
            Import::Crdt(crdt) => crdt.to_use_statement(),
            Import::Macros(macros) => macros.to_use_statement(),
            Import::Protocol(protocol) => protocol.to_use_statement(),
            Import::Custom(path) => {
                let path_tokens: TokenStream = path.parse().unwrap();
                quote! {
                    pub use #path_tokens;
                }
            }
            Import::CrdtOp(op) => op.to_use_statement(),
        }
    }
}

trait ToUseStatement {
    fn path(&self) -> String;
    fn to_use_statement(&self) -> TokenStream {
        let path = self.path();
        let path_tokens: TokenStream = path.parse().unwrap();
        quote! {
            pub use #path_tokens;
        }
    }
}

impl ToUseStatement for Crdt {
    fn path(&self) -> String {
        match self {
            Crdt::Simple(crdt) => match crdt {
                SimpleCrdt::Primitive(primitive) => match primitive {
                    Primitive::Counter(counter) => match counter {
                        Counter::Counter => {
                            format!("{}::counter::simple_counter::Counter", CRDT_PREFIX)
                        }
                        Counter::ResettableCounter => {
                            format!("{}::counter::resettable_counter::Counter", CRDT_PREFIX)
                        }
                    },
                    Primitive::Flag(flag) => match flag {
                        Flag::EWFlag => format!("{}::flag::ew_flag::EWFlag", CRDT_PREFIX),
                        Flag::DWFlag => format!("{}::flag::dw_flag::DWFlag", CRDT_PREFIX),
                    },
                    Primitive::Register(register) => match register {
                        Register::MultiValue => {
                            format!("{}::register::mv_register::MVRegister", CRDT_PREFIX)
                        }
                        Register::Fair => {
                            format!("{}::register::unique_register::FairRegister", CRDT_PREFIX)
                        }
                        Register::LastWriterWins => {
                            format!("{}::register::unique_register::LwwRegister", CRDT_PREFIX)
                        }
                        Register::PartiallyOrdered => {
                            format!("{}::register::po_register::PORegister", CRDT_PREFIX)
                        }
                        Register::TotallyOrdered => {
                            format!("{}::register::to_register::TORegister", CRDT_PREFIX)
                        }
                    },
                    Primitive::List => format!("{}::list::eg_walker::List", CRDT_PREFIX),
                },
                SimpleCrdt::Collection(collection) => match collection {
                    Collection::Set(set) => match set {
                        Set::AWSet => format!("{}::set::aw_set::AWSet", CRDT_PREFIX),
                        Set::RWSet => format!("{}::set::rw_set::RWSet", CRDT_PREFIX),
                    },
                    Collection::Graph(graph) => match graph {
                        Graph::AWMultigraph => {
                            format!("{}::graph::aw_multigraph::AWMultigraph", CRDT_PREFIX)
                        }
                    },
                    Collection::Bag(bag) => match bag {
                        Bag::AWBag => format!("{}::bag::aw_bag::AWBagLog", CRDT_PREFIX),
                    },
                },
            },
            Crdt::Nested(crdt) => match crdt {
                NestedCrdt::Map(map) => match map {
                    Map::UWMap => format!("{}::map::uw_map::UWMapLog", CRDT_PREFIX),
                },
                NestedCrdt::List => format!("{}::list::nested_list::ListLog", CRDT_PREFIX),
                NestedCrdt::Graph => format!("{}::graph::uw_multigraph::UWMultigraph", CRDT_PREFIX),
                NestedCrdt::Optional => format!("{}::option::OptionLog", CRDT_PREFIX),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub enum Log {
    VecLog,
    EventGraph,
}

impl ToUseStatement for Log {
    fn path(&self) -> String {
        match self {
            Log::VecLog => format!("{}::state::po_log::VecLog", PROTOCOL_PREFIX),
            Log::EventGraph => format!("{}::state::event_graph::EventGraph", PROTOCOL_PREFIX),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Macros {
    Record,
    Union,
    TypedGraph,
}

impl ToUseStatement for Macros {
    fn path(&self) -> String {
        match self {
            Macros::Record => format!("{}::record", MACROS_PREFIX),
            Macros::Union => format!("{}::union", MACROS_PREFIX),
            Macros::TypedGraph => format!("{}::typed_graph", MACROS_PREFIX),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Protocol {
    EventId,
    Read,
    EvalNested,
    IsLog,
    Version,
    LwwPolicy,
    Event,
    PureCRDT,
    QueryOperation,
}

impl ToUseStatement for Protocol {
    fn path(&self) -> String {
        match self {
            Protocol::EventId => format!("{}::event::id::EventId", PROTOCOL_PREFIX),
            Protocol::Read => format!("{}::crdt::query::Read", PROTOCOL_PREFIX),
            Protocol::EvalNested => {
                format!("{}::crdt::eval::EvalNested", PROTOCOL_PREFIX)
            }
            Protocol::IsLog => format!("{}::state::log::IsLog", PROTOCOL_PREFIX),
            Protocol::Version => format!("{}::clock::version_vector::Version", PROTOCOL_PREFIX),
            Protocol::LwwPolicy => format!("{}::policy::LwwPolicy", CRDT_PREFIX),
            Protocol::Event => format!("{}::event::Event", PROTOCOL_PREFIX),
            Protocol::QueryOperation => format!("{}::crdt::query::QueryOperation", PROTOCOL_PREFIX),
            Protocol::PureCRDT => format!("{}::crdt::pure_crdt::PureCRDT", PROTOCOL_PREFIX),
        }
    }
}

#[derive(Clone, Debug)]
pub enum CrdtOp {
    Nested(NestedCrdtOp),
}

#[derive(Clone, Debug)]
pub enum NestedCrdtOp {
    ListOp,
    MapOp,
}

impl ToUseStatement for CrdtOp {
    fn path(&self) -> String {
        match self {
            CrdtOp::Nested(nested_op) => match nested_op {
                NestedCrdtOp::ListOp => format!("{}::list::nested_list::List", CRDT_PREFIX),
                NestedCrdtOp::MapOp => format!("{}::map::uw_map::UWMap", CRDT_PREFIX),
            },
        }
    }
}
