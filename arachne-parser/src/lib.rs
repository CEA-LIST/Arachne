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

#![forbid(unsafe_code)]

#[macro_use]
pub mod prelude;

pub mod ctx;
pub mod parser;
pub mod repr;
pub mod traits;
