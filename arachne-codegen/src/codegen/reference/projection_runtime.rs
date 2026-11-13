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

use moirai_protocol::{
    clock::{causal_frontier::CausalFrontier, version_vector::Version},
    data_type::query::QueryOperation,
    event::Event,
    state::{
        effect_context::EffectContext,
        log::{EvalNested, IsLog},
    },
};

/// Preserves the identity of an EClass whose record was projected away.
///
/// Commands, operations, queries and storage remain those of `L`. The adapter
/// emits a typed lifecycle event at the projected object's path and forwards
/// child effects below the projected feature. Thus a map entry and its value
/// cannot overwrite each other's sink event or share an XMI object identifier.
/// The const IDs select generated metadata; they add no per-instance storage.
/// Fuzz generators and footprint implementations belong to application code.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "test_utils", derive(deepsize::DeepSizeOf))]
pub struct ProjectedLog<L: IsLog, const CLASS: usize, const FIELD: usize>(L);

// Preserve log inspection APIs (children and positions) used
// by XMI traversal without changing the stored or returned value types.
impl<L: IsLog, const C: usize, const F: usize> std::ops::Deref for ProjectedLog<L, C, F> {
    type Target = L;

    fn deref(&self) -> &L {
        &self.0
    }
}

impl<L: IsLog, const C: usize, const F: usize> IsLog for ProjectedLog<L, C, F> {
    type Command = L::Command;
    type Update = L::Update;
    type Rejection = L::Rejection;

    fn new() -> Self {
        Self(L::new())
    }

    fn prepare(&self, command: Self::Command) -> Self::Update {
        self.0.prepare(command)
    }

    fn is_enabled(&self, op: &Self::Update) -> Result<(), Self::Rejection> {
        self.0.is_enabled(op)
    }

    fn effect(&mut self, event: Event<Self::Update>, ctx: &mut EffectContext<'_>) {
        let (kind, field) = super::projection_metadata(C, F);
        if self.0.is_default() {
            ctx.create_typed(kind);
        } else {
            ctx.update_typed(kind);
        }
        // In particular, clearing the projected map deletes entries below the
        // field, not the enclosing transparent object's reference vertex.
        ctx.with_field(field, |ctx| self.0.effect(event, ctx));
    }

    fn stabilize(&mut self, frontier: &CausalFrontier) {
        self.0.stabilize(frontier);
    }

    fn redundant_by_parent(&mut self, version: &Version, conservative: bool) {
        self.0.redundant_by_parent(version, conservative);
    }

    fn is_default(&self) -> bool {
        self.0.is_default()
    }
}

impl<L, Q, const C: usize, const F: usize> EvalNested<Q> for ProjectedLog<L, C, F>
where
    L: IsLog + EvalNested<Q>,
    Q: QueryOperation,
{
    fn execute_query(&self, query: &Q) -> Q::Response {
        self.0.execute_query(query)
    }
}
