#[derive(Clone, Debug)]
pub enum Crdt {
    Simple(SimpleCrdt),
    Nested(NestedCrdt),
}

#[derive(Clone, Debug)]
pub enum SimpleCrdt {
    Counter(Counter),
    Flag(Flag),
    Graph(Graph),
    List,
    Register(Register),
    Set(Set),
}

#[derive(Clone, Debug, Default)]
pub enum Counter {
    Counter,
    #[default]
    ResettableCounter,
}

#[derive(Clone, Debug, Default)]
pub enum Flag {
    #[default]
    EWFlag,
    DWFlag,
}

#[derive(Clone, Debug, Default)]
pub enum Graph {
    #[default]
    AWMultigraph,
}

#[derive(Clone, Debug, Default)]
pub enum Map {
    #[default]
    UWMap,
}

#[derive(Clone, Debug, Default)]
pub enum Register {
    #[default]
    MultiValue,
    LastWriterWins,
    PartiallyOrdered,
    TotallyOrdered,
}

#[derive(Clone, Debug, Default)]
pub enum Set {
    #[default]
    AWSet,
    RWSet,
}

#[derive(Clone, Debug)]
pub enum NestedCrdt {
    Map(Map),
    List,
    Graph,
}
