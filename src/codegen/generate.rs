use proc_macro2::TokenStream;

use crate::codegen::{import::Import, warnings::Warning};

pub struct Fragment {
    pub tokens: TokenStream,
    pub imports: Vec<Import>,
    pub warnings: Vec<Warning>,
}

impl Fragment {
    pub fn new(tokens: TokenStream, imports: Vec<Import>, warnings: Vec<Warning>) -> Self {
        Self {
            tokens,
            imports,
            warnings,
        }
    }
}

pub trait Generate {
    fn generate(&self) -> anyhow::Result<Fragment>;
}
