use ecore_rs::{
    ctx::Ctx,
    repr::{Class, Operation},
};
use quote::quote;

use crate::codegen::{
    generate::{Fragment, Generate},
    warnings::Warning,
};

pub struct OperationGenerator<'a> {
    operation: &'a Operation,
    source_class: &'a Class,
    _ctx: &'a Ctx,
}

impl<'a> OperationGenerator<'a> {
    pub fn new(operation: &'a Operation, source_class: &'a Class, ctx: &'a Ctx) -> Self {
        Self {
            operation,
            source_class,
            _ctx: ctx,
        }
    }
}

impl<'a> Generate for OperationGenerator<'a> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        Ok(Fragment::new(
            quote! {},
            vec![],
            vec![Warning::OperationNotSupported(format!(
                "{}::{}",
                self.source_class.name(),
                self.operation.name()
            ))],
        ))
    }
}
