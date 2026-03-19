use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;

use crate::codegen::{generate::Fragment, import::Import, warnings::Warning};

pub const PRIVATE_MOD_PREFIX: &str = "__";

/// Main generator that collects generated fragments, manages imports, and emits warnings
/// One generator per file
pub struct Generator<'a> {
    import_set: HashSet<String>,
    imports: Vec<TokenStream>,
    tokens: Vec<TokenStream>,
    warnings: Vec<Warning>,
    path_mod: &'a str,
}

impl<'a> Generator<'a> {
    pub fn new(path_mod: &'a str) -> Self {
        Self {
            import_set: HashSet::new(),
            tokens: Vec::new(),
            imports: Vec::new(),
            warnings: Vec::new(),
            path_mod,
        }
    }

    pub fn register(&mut self, fragment: Fragment) {
        let (tokens, imports, warnings) = fragment.into();
        // Register imports
        for import in imports {
            self.register_import(import);
        }

        // Collect tokens
        self.tokens.push(tokens);

        // Collect warnings
        self.warnings.extend(warnings);
    }

    /// Register an import on first use. Subsequent uses of the same import are ignored.
    fn register_import(&mut self, import: Import) {
        let import_path = import.path();

        // Only add if not already registered
        if self.import_set.insert(import_path.clone()) {
            let import_stmt = import.to_use_statement();
            self.imports.push(import_stmt);
        }
    }

    /// Emit all warnings to stderr
    pub fn emit_warnings(&self) {
        for warning in &self.warnings {
            warning.emit();
        }
    }

    pub fn build(self) -> TokenStream {
        let path: syn::Path =
            syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, &self.path_mod)).unwrap();
        let imports = &self.imports;
        let tokens = &self.tokens;

        quote! {
            /// Auto-generated code by 🅰🆁🅰🅲🅷🅽🅴 - do not edit directly

            mod #path {
                #(#imports)*
            }

            #(#tokens)*
        }
    }
}
