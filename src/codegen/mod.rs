//! Code generation module for CRDT types

pub mod class;
pub mod record;

use proc_macro2::TokenStream;

/// Main code generator for Ecore to CRDT transformation
pub struct Generator {
    tokens: Vec<TokenStream>,
}

impl Generator {
    /// Creates a new generator
    pub fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    pub fn concat(&mut self, other: TokenStream) {
        self.tokens.push(other);
    }

    pub fn build(self) -> TokenStream {
        let mut combined = TokenStream::new();
        for token in self.tokens {
            combined.extend(token);
        }
        combined
    }
}

impl Default for Generator {
    fn default() -> Self {
        Self::new()
    }
}
