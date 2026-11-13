/////////////////////////////////////////////////////////////////////////////
//  Original implementation: Red Iron Team (OCamlPro), Arthur Carcano.
//  Upstream code is licensed under LGPL-3.0-or-later.
//
//  Modifications Copyright (c) 2025, 2026 CEA LIST.
//  CEA modifications are made available under the Apache License, Version 2.0.
//
//  SPDX-License-Identifier: LGPL-3.0-or-later AND Apache-2.0
//
//  Contributors:
//  Red Iron Team (OCamlPro), Arthur Carcano - Original ecore.rs implementation
//  Léo Olivier (CEA LIST) - Arachne adaptations and extensions
//
/////////////////////////////////////////////////////////////////////////////

prelude! {
    repr::*,
}

pub trait HasAnnots {
    fn annotations(&self) -> &Annots;
    fn annotations_mut(&mut self) -> &mut Annots;
}

pub trait HasStructural {
    fn structural(&self) -> &[repr::Structural];
    fn structural_mut(&mut self) -> &mut Vec<repr::Structural>;
    fn structural_push(&mut self, s: impl Into<repr::Structural>) {
        self.structural_mut().push(s.into())
    }
}
