use proc_macro2::TokenStream;

use crate::codegen::{import::Import, warnings::Warning};

/// Represents a generated code fragment, including the generated tokens, required imports, and any warnings encountered during generation
pub struct Fragment {
    tokens: TokenStream,
    imports: Vec<Import>,
    warnings: Vec<Warning>,
}

impl Fragment {
    pub fn new(tokens: TokenStream, imports: Vec<Import>, warnings: Vec<Warning>) -> Self {
        Self {
            tokens,
            imports,
            warnings,
        }
    }

    pub fn into(self) -> (TokenStream, Vec<Import>, Vec<Warning>) {
        (self.tokens, self.imports, self.warnings)
    }

    pub fn tokens(&self) -> &TokenStream {
        &self.tokens
    }

    pub fn imports(&self) -> &[Import] {
        &self.imports
    }

    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }
}

/// Trait for generating code fragments from a given input
pub trait Generate {
    fn generate(&self) -> anyhow::Result<Fragment>;
}
