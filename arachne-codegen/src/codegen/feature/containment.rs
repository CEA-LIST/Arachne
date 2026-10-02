use ecore_rs::{
    ctx::Ctx,
    repr::{Structural, idx},
};
use proc_macro2::TokenStream;

use crate::{
    CLASSIFIERS_PATH_MOD,
    codegen::{
        classifier::is_uninhabited_polymorphic_class,
        cycles::CycleAnalysis,
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
    },
};

use super::plan::FeaturePlan;

pub struct ContainmentGenerator<'a> {
    reference: &'a Structural,
    source_class: idx::Class,
    ctx: &'a Ctx,
    cycle_analysis: &'a CycleAnalysis,
}

impl<'a> ContainmentGenerator<'a> {
    pub fn new(
        reference: &'a Structural,
        source_class: idx::Class,
        ctx: &'a Ctx,
        cycle_analysis: &'a CycleAnalysis,
    ) -> Self {
        Self {
            reference,
            source_class,
            ctx,
            cycle_analysis,
        }
    }
}

impl Generate for ContainmentGenerator<'_> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        if self
            .reference
            .typ
            .is_some_and(|idx| is_uninhabited_polymorphic_class(self.ctx, self.ctx.class(idx)))
        {
            return Ok(Fragment::new(TokenStream::new(), vec![], vec![]));
        }
        let path = syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD))?;
        FeaturePlan::resolve(
            self.ctx,
            self.source_class,
            self.reference,
            self.cycle_analysis,
        )?
        .into_field(self.ctx, &path)
    }
}
