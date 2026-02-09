use ecore_rs::ctx::Ctx;
use std::fs;
use std::path::Path;

use crate::error::Result;

/// Parser for Ecore metamodels
pub struct EcoreParser {
    pub ctx: Ctx,
}

impl EcoreParser {
    /// Parses an Ecore metamodel from a file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)?;
        Self::from_string(&content)
    }

    /// Parses an Ecore metamodel from a string
    pub fn from_string(content: &str) -> Result<Self> {
        let ctx = Ctx::parse(content)?;
        Ok(Self { ctx })
    }
}
