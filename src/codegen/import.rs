use proc_macro2::TokenStream;
use quote::quote;

const PROTOCOL_PREFIX: &str = "moirai_protocol";
const CRDT_PREFIX: &str = "moirai_crdt";
const MACROS_PREFIX: &str = "moirai_macros";

pub enum Import {
    Log(LogImport),
    Crdt(CrdtImport),
    Macros(MacrosImport),
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

pub enum CrdtImport {
    Counter,
    ResettableCounter,
    EWFlag,
    DWFlag,
}

impl ToUseStatement for CrdtImport {
    fn path(&self) -> String {
        match self {
            CrdtImport::Counter => format!("{}::counter::simple_counter::Counter", CRDT_PREFIX),
            CrdtImport::ResettableCounter => {
                format!("{}::counter::resettable_counter::Counter", CRDT_PREFIX)
            }
            CrdtImport::EWFlag => format!("{}::flag::ew_flag::EWFlag", CRDT_PREFIX),
            CrdtImport::DWFlag => format!("{}::flag::dw_flag::DWFlag", CRDT_PREFIX),
        }
    }
}

pub enum LogImport {
    VecLog,
}

impl ToUseStatement for LogImport {
    fn path(&self) -> String {
        match self {
            LogImport::VecLog => format!("{}::state::po_log::VecLog", PROTOCOL_PREFIX),
        }
    }
}

pub enum MacrosImport {
    Record,
    Union,
}

impl ToUseStatement for MacrosImport {
    fn path(&self) -> String {
        match self {
            MacrosImport::Record => format!("{}::record", MACROS_PREFIX),
            MacrosImport::Union => format!("{}::union", MACROS_PREFIX),
        }
    }
}
