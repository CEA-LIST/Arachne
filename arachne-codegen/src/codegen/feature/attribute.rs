use ecore_rs::{ctx::Ctx, repr::Structural};

use crate::{
    CLASSIFIERS_PATH_MOD,
    codegen::{
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
    },
};

use super::plan::FeaturePlan;

pub struct AttributeGenerator<'a> {
    attribute: &'a Structural,
    ctx: &'a Ctx,
}

impl<'a> AttributeGenerator<'a> {
    pub fn new(attribute: &'a Structural, ctx: &'a Ctx) -> Self {
        Self { attribute, ctx }
    }
}

impl Generate for AttributeGenerator<'_> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let path = syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD))?;
        FeaturePlan::attribute(self.ctx, self.attribute)?.into_field(self.ctx, &path)
    }
}
