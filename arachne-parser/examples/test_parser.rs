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

ecore_rs::prelude! {}

pub const CONTENT: &str = include_str!("../rsc/ExampleEcore.ecore");

fn main() {
    simple_logger::SimpleLogger::new()
        .env()
        .without_timestamps()
        .init()
        .expect("failed to init logger /(-_-)\\");
    let ctx = Ctx::parse(CONTENT).unwrap_or_else(|err| {
        println!("an error occurred:");
        for line in err.to_string().lines() {
            println!("- {}", line)
        }
        panic!("run failed")
    });
    println!("|==| done \\(*o*)/");
    for line in ctx.to_pretty_string().lines() {
        println!("| {}", line);
    }
    println!("|==|")
}
