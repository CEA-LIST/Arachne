use proc_macro2::TokenStream;
use quote::quote;

use crate::codegen::datatype::crdt::{
    Counter, Crdt, Flag, Graph, Map, NestedCrdt, Register, Set, SimpleCrdt,
};

const PROTOCOL_PREFIX: &str = "moirai_protocol";
const CRDT_PREFIX: &str = "moirai_crdt";
const MACROS_PREFIX: &str = "moirai_macros";

#[derive(Clone, Debug)]
pub enum Import {
    Log(Log),
    Crdt(Crdt),
    Macros(Macros),
}

impl Import {
    pub fn path(&self) -> String {
        match self {
            Import::Log(log) => log.path(),
            Import::Crdt(crdt) => crdt.path(),
            Import::Macros(macros) => macros.path(),
        }
    }

    pub fn to_use_statement(&self) -> TokenStream {
        match self {
            Import::Log(log) => log.to_use_statement(),
            Import::Crdt(crdt) => crdt.to_use_statement(),
            Import::Macros(macros) => macros.to_use_statement(),
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
                SimpleCrdt::Counter(counter) => match counter {
                    Counter::Counter => {
                        format!("{}::counter::simple_counter::Counter", CRDT_PREFIX)
                    }
                    Counter::ResettableCounter => {
                        format!("{}::counter::resettable_counter::Counter", CRDT_PREFIX)
                    }
                },
                SimpleCrdt::Flag(flag) => match flag {
                    Flag::EWFlag => format!("{}::flag::ew_flag::EWFlag", CRDT_PREFIX),
                    Flag::DWFlag => format!("{}::flag::dw_flag::DWFlag", CRDT_PREFIX),
                },
                SimpleCrdt::Graph(graph) => match graph {
                    Graph::AWMultigraph => {
                        format!("{}::graph::aw_multigraph::AWMultigraph", CRDT_PREFIX)
                    }
                },
                SimpleCrdt::List => format!("{}::list::eg_walker::List", CRDT_PREFIX),
                SimpleCrdt::Register(register) => match register {
                    Register::MultiValue => {
                        format!("{}::register::mv_register::MVRegister", CRDT_PREFIX)
                    }
                    Register::LastWriterWins => {
                        format!("{}::register::lww_register::LWWRegister", CRDT_PREFIX)
                    }
                    Register::PartiallyOrdered => {
                        format!("{}::register::po_register::PORegister", CRDT_PREFIX)
                    }
                    Register::TotallyOrdered => {
                        format!("{}::register::to_register::TORegister", CRDT_PREFIX)
                    }
                },
                SimpleCrdt::Set(set) => match set {
                    Set::AWSet => format!("{}::set::aw_set::AWSet", CRDT_PREFIX),
                    Set::RWSet => format!("{}::set::rw_set::RWSet", CRDT_PREFIX),
                },
            },
            Crdt::Nested(crdt) => match crdt {
                NestedCrdt::Map(map) => match map {
                    Map::UWMap => format!("{}::map::uw_map::UWMap", CRDT_PREFIX),
                },
                NestedCrdt::List => format!("{}::list::nested_list::NestedList", CRDT_PREFIX),
                NestedCrdt::Graph => format!("{}::graph::uw_multigraph::UWMultigraph", CRDT_PREFIX),
            },
        }
    }
}

impl Crdt {
    /// Get the type name (e.g., "EWFlag", "Counter", "MVRegister")
    pub fn type_name(&self) -> &str {
        match self {
            Crdt::Simple(crdt) => match crdt {
                SimpleCrdt::Counter(_) => "Counter",
                SimpleCrdt::Flag(flag) => match flag {
                    Flag::EWFlag => "EWFlag",
                    Flag::DWFlag => "DWFlag",
                },
                SimpleCrdt::Graph(graph) => match graph {
                    Graph::AWMultigraph => "AWMultigraph",
                },
                SimpleCrdt::List => "List",
                SimpleCrdt::Register(register) => match register {
                    Register::MultiValue => "MVRegister",
                    Register::LastWriterWins => "LWWRegister",
                    Register::PartiallyOrdered => "PORegister",
                    Register::TotallyOrdered => "TORegister",
                },
                SimpleCrdt::Set(set) => match set {
                    Set::AWSet => "AWSet",
                    Set::RWSet => "RWSet",
                },
            },
            Crdt::Nested(crdt) => match crdt {
                NestedCrdt::Map(map) => match map {
                    Map::UWMap => "UWMap",
                },
                NestedCrdt::List => "NestedList",
                NestedCrdt::Graph => "UWMultigraph",
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
}

impl ToUseStatement for Macros {
    fn path(&self) -> String {
        match self {
            Macros::Record => format!("{}::record", MACROS_PREFIX),
            Macros::Union => format!("{}::union", MACROS_PREFIX),
        }
    }
}
