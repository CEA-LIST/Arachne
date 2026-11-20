use ecore_rs::repr::Structural;

use super::plan::FeaturePlan;
use crate::{
    CLASSIFIERS_PATH_MOD,
    codegen::{
        analysis::ModelIndex,
        generate::{Fragment, Generate},
        generator::PRIVATE_MOD_PREFIX,
    },
};

pub struct AttributeGenerator<'a> {
    attribute: &'a Structural,
    ctx: &'a ModelIndex<'a>,
}

impl<'a> AttributeGenerator<'a> {
    pub fn new(attribute: &'a Structural, ctx: &'a ModelIndex<'a>) -> Self {
        Self { attribute, ctx }
    }
}

impl Generate for AttributeGenerator<'_> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let path = syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD))?;
        FeaturePlan::attribute(self.ctx, self.attribute)?.into_field(self.ctx, &path)
    }
}
