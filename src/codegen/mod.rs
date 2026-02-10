pub mod class;
pub mod import;
pub mod record;

use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashSet;

use import::Import;

const GEN_MOD: &'static str = "__generated";

pub struct Generator {
    import_set: HashSet<String>,
    imports: Vec<TokenStream>,
    tokens: Vec<TokenStream>,
}

impl Generator {
    pub fn new() -> Self {
        Self {
            import_set: HashSet::new(),
            tokens: Vec::new(),
            imports: Vec::new(),
        }
    }

    pub fn concat(&mut self, other: TokenStream) {
        self.tokens.push(other);
    }

    /// Register an import on first use. Subsequent uses of the same import are ignored.
    pub fn register_import(&mut self, import: Import) {
        let import_path = import.path();

        // Only add if not already registered
        if self.import_set.insert(import_path.clone()) {
            let import_stmt = import.to_use_statement();
            self.imports.push(import_stmt);
        }
    }

    pub fn build(self) -> TokenStream {
        let gen_mod: syn::Path = syn::parse_str(GEN_MOD).unwrap();
        let imports = &self.imports;
        let tokens = &self.tokens;

        quote! {
            mod #gen_mod {
                #(#imports)*
            }
            use #gen_mod::*;

            #(#tokens)*
        }
    }
}

impl Default for Generator {
    fn default() -> Self {
        Self::new()
    }
}
