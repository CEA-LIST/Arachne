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
            Crdt::Simple(simple) => simple.name(),
            Crdt::Nested(nested) => nested.name(),
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
            SimpleCrdt::Primitive(primitive) => primitive.name(),
            SimpleCrdt::Collection(collection) => collection.name(),
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
            Primitive::Counter(counter) => counter.name(),
            Primitive::Flag(flag) => flag.name(),
            Primitive::Register(register) => register.name(),
            Primitive::List => "List",
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
            Collection::Set(set) => set.name(),
            Collection::Graph(graph) => graph.name(),
            Collection::Bag(bag) => bag.name(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Counter {
    Counter,
    #[default]
    ResettableCounter,
}

impl Named for Counter {
    fn name(&self) -> &str {
        match self {
            Counter::Counter => "Counter",
            Counter::ResettableCounter => "Counter",
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
            Flag::EWFlag => "EWFlag",
            Flag::DWFlag => "DWFlag",
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
            Bag::AWBag => "AWBagLog",
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
            Graph::AWMultigraph => "AWMultigraph",
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
            Map::UWMap => "UWMap",
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
            Register::MultiValue => "MVRegister",
            Register::LastWriterWins => "LwwRegister",
            Register::Fair => "FairRegister",
            Register::PartiallyOrdered => "PORegister",
            Register::TotallyOrdered => "TORegister",
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
            Set::AWSet => "AWSet",
            Set::RWSet => "RWSet",
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
            NestedCrdt::Map(map) => map.name(),
            NestedCrdt::List => "List",
            NestedCrdt::Graph => "Graph",
            NestedCrdt::Optional => "Optional",
        }
    }
}
