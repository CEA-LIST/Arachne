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

pub trait Named {
    fn name(&self) -> &str;
}

#[derive(Clone, Debug)]
pub enum Crdt {
    Simple(SimpleCrdt),
    Nested(NestedCrdt),
}

impl Named for Crdt {
    fn name(&self) -> &str {
        match self {
            Self::Simple(simple) => simple.name(),
            Self::Nested(nested) => nested.name(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum SimpleCrdt {
    Primitive(Primitive),
    Collection(Collection),
}

impl Named for SimpleCrdt {
    fn name(&self) -> &str {
        match self {
            Self::Primitive(primitive) => primitive.name(),
            Self::Collection(collection) => collection.name(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Primitive {
    Counter(Counter),
    Flag(Flag),
    Register(Register),
    List,
}

impl Named for Primitive {
    fn name(&self) -> &str {
        match self {
            Self::Counter(counter) => counter.name(),
            Self::Flag(flag) => flag.name(),
            Self::Register(register) => register.name(),
            Self::List => "List",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Collection {
    Set(Set),
    Graph(Graph),
    Bag(Bag),
}

impl Named for Collection {
    fn name(&self) -> &str {
        match self {
            Self::Set(set) => set.name(),
            Self::Graph(graph) => graph.name(),
            Self::Bag(bag) => bag.name(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Counter {
    #[default]
    ResettableCounter,
}

impl Named for Counter {
    fn name(&self) -> &str {
        match self {
            Self::ResettableCounter => "Counter",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Flag {
    #[default]
    EWFlag,
    DWFlag,
}

impl Named for Flag {
    fn name(&self) -> &str {
        match self {
            Self::EWFlag => "EWFlag",
            Self::DWFlag => "DWFlag",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Bag {
    #[default]
    AWBag,
}

impl Named for Bag {
    fn name(&self) -> &str {
        match self {
            Self::AWBag => "AWBagLog",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Graph {
    #[default]
    AWMultigraph,
}

impl Named for Graph {
    fn name(&self) -> &str {
        match self {
            Self::AWMultigraph => "Graph",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Map {
    #[default]
    UWMap,
}

impl Named for Map {
    fn name(&self) -> &str {
        match self {
            Self::UWMap => "UWMap",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Register {
    #[default]
    MultiValue,
    LastWriterWins,
    Fair,
    PartiallyOrdered,
    TotallyOrdered,
}

impl Named for Register {
    fn name(&self) -> &str {
        match self {
            Self::MultiValue => "MVRegister",
            Self::LastWriterWins => "LwwRegister",
            Self::Fair => "FairRegister",
            Self::PartiallyOrdered => "PORegister",
            Self::TotallyOrdered => "TORegister",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Set {
    #[default]
    AWSet,
    RWSet,
}

impl Named for Set {
    fn name(&self) -> &str {
        match self {
            Self::AWSet => "AWSet",
            Self::RWSet => "RWSet",
        }
    }
}

#[derive(Clone, Debug)]
pub enum NestedCrdt {
    Map(Map),
    List,
    Graph,
    Optional,
}

impl Named for NestedCrdt {
    fn name(&self) -> &str {
        match self {
            Self::Map(map) => map.name(),
            Self::List => "List",
            Self::Graph => "Graph",
            Self::Optional => "Optional",
        }
    }
}
