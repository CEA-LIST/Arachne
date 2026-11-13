/////////////////////////////////////////////////////////////////////////////
//  Copyright (c) 2025, 2026 CEA LIST.
//
//  This program and the accompanying materials
//  are made available under the terms of the Apache License, Version 2.0
//  which accompanies this distribution, and is available at
//  https://www.apache.org/licenses/LICENSE-2.0
//
//  SPDX-License-Identifier: Apache-2.0
//
//  Contributors:
//  Léo Olivier -  (CEA LIST) - Initial implementation
//
/////////////////////////////////////////////////////////////////////////////

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
    Custom(&'static str),
}

impl Import {
    pub fn path(&self) -> String {
        match self {
            Self::Log(log) => log.path(),
            Self::Crdt(crdt) => crdt.path(),
            Self::Macros(macros) => macros.path(),
            Self::Protocol(protocol) => protocol.path(),
            Self::Custom(path) => path.to_string(),
            Self::CrdtOp(op) => op.path(),
        }
    }

    pub fn to_use_statement(&self) -> TokenStream {
        match self {
            Self::Log(log) => log.to_use_statement(),
            Self::Crdt(crdt) => crdt.to_use_statement(),
            Self::Macros(macros) => macros.to_use_statement(),
            Self::Protocol(protocol) => protocol.to_use_statement(),
            Self::Custom(path) => {
                let path_tokens: TokenStream = path.parse().unwrap();
                quote! {
                    pub use #path_tokens;
                }
            }
            Self::CrdtOp(op) => op.to_use_statement(),
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
            Self::Simple(crdt) => match crdt {
                SimpleCrdt::Primitive(primitive) => match primitive {
                    Primitive::Counter(counter) => match counter {
                        Counter::ResettableCounter => {
                            format!("{}::counter::Counter", CRDT_PREFIX)
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
                            format!("{}::graph::aw_multidigraph::Graph", CRDT_PREFIX)
                        }
                    },
                    Collection::Bag(bag) => match bag {
                        Bag::AWBag => format!("{}::bag::aw_bag::AWBagLog", CRDT_PREFIX),
                    },
                },
            },
            Self::Nested(crdt) => match crdt {
                NestedCrdt::Map(map) => match map {
                    Map::UWMap => format!("{}::map::uw_map::UWMapLog", CRDT_PREFIX),
                },
                NestedCrdt::List => format!("{}::list::nested_list::NestedListLog", CRDT_PREFIX),
                NestedCrdt::Graph => format!("{}::graph::uw_multidigraph::UWGraphLog", CRDT_PREFIX),
                NestedCrdt::Optional => format!("{}::option::OptionLog", CRDT_PREFIX),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub enum Log {
    Vec,
    Graph,
    PartiallyOrdered,
}

impl ToUseStatement for Log {
    fn path(&self) -> String {
        match self {
            Self::Vec => format!("{}::state::po_log::VecLog", PROTOCOL_PREFIX),
            Self::Graph => format!("{}::state::po_log::GraphLog", PROTOCOL_PREFIX),
            Self::PartiallyOrdered => format!("{}::state::po_log::POLog", PROTOCOL_PREFIX),
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
            Self::Record => format!("{}::record", MACROS_PREFIX),
            Self::Union => format!("{}::union", MACROS_PREFIX),
            Self::TypedGraph => format!("{}::typed_graph", MACROS_PREFIX),
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
    Frontier,
    ReplicaIdx,
    Policy,
    LwwPolicy,
    FairPolicy,
    Event,
    ReplicatedDataType,
    QueryOperation,
    Sink,
    SinkEffect,
    SinkCollector,
    EffectContext,
    PathSegment,
    ObjectPath,
}

impl ToUseStatement for Protocol {
    fn path(&self) -> String {
        match self {
            Self::EventId => format!("{}::event::id::EventId", PROTOCOL_PREFIX),
            Self::Read => format!("{}::data_type::query::Read", PROTOCOL_PREFIX),
            Self::EvalNested => {
                format!("{}::state::log::EvalNested", PROTOCOL_PREFIX)
            }
            Self::IsLog => format!("{}::state::log::IsLog", PROTOCOL_PREFIX),
            Self::Version => format!("{}::clock::version_vector::Version", PROTOCOL_PREFIX),
            Self::Frontier => format!(
                "{}::clock::causal_frontier::CausalFrontier",
                PROTOCOL_PREFIX
            ),
            Self::ReplicaIdx => format!("{}::replica::ReplicaIdx", PROTOCOL_PREFIX),
            Self::LwwPolicy => {
                format!("{}::data_type::policy::LwwPolicy", PROTOCOL_PREFIX)
            }
            Self::FairPolicy => {
                format!("{}::data_type::policy::FairPolicy", PROTOCOL_PREFIX)
            }
            Self::Event => format!("{}::event::Event", PROTOCOL_PREFIX),
            Self::QueryOperation => {
                format!("{}::data_type::query::QueryOperation", PROTOCOL_PREFIX)
            }
            Self::ReplicatedDataType => format!(
                "{}::data_type::replicated::ReplicatedDataType",
                PROTOCOL_PREFIX
            ),
            Self::SinkCollector => format!("{}::state::sink::SinkCollector", PROTOCOL_PREFIX),
            Self::SinkEffect => format!("{}::state::sink::SinkEffect", PROTOCOL_PREFIX),
            Self::EffectContext => {
                format!("{}::state::effect_context::EffectContext", PROTOCOL_PREFIX)
            }
            Self::Sink => format!("{}::state::sink::Sink", PROTOCOL_PREFIX),
            Self::PathSegment => format!(
                "{}::state::object_path::PathSegment::{{Field, ListElement, MapEntry, Variant}}",
                PROTOCOL_PREFIX
            ),
            Self::ObjectPath => format!("{}::state::object_path::ObjectPath", PROTOCOL_PREFIX),
            Self::Policy => format!("{}::data_type::policy::Policy", PROTOCOL_PREFIX),
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
            Self::Nested(nested_op) => match nested_op {
                NestedCrdtOp::ListOp => format!("{}::list::update::NestedListUpdate", CRDT_PREFIX),
                NestedCrdtOp::MapOp => format!("{}::map::update::MapUpdate", CRDT_PREFIX),
            },
        }
    }
}
