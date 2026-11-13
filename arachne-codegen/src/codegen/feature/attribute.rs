/////////////////////////////////////////////////////////////////////////////
//  Copyright (c) 2025, 2026 CEA LIST.
//
//  This program and the accompanying materials
//  are made available under the terms of the Apache License, Version 2.0
//  which accompanies this distribution, and is available at
//  https://www.apache.org/licenses/LICENSE-2.0
//
//  SPDX-License-Identifier: Apache-2.0
//
//  Contributors:
//  Léo Olivier -  (CEA LIST) - Initial implementation
//
/////////////////////////////////////////////////////////////////////////////

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
    pub const fn new(attribute: &'a Structural, ctx: &'a ModelIndex<'a>) -> Self {
        Self { attribute, ctx }
    }
}

impl Generate for AttributeGenerator<'_> {
    fn generate(&self) -> anyhow::Result<Fragment> {
        let path = syn::parse_str(&format!("{}{}", PRIVATE_MOD_PREFIX, CLASSIFIERS_PATH_MOD))?;
        FeaturePlan::attribute(self.ctx, self.attribute)?.into_field(self.ctx, &path)
    }
}
