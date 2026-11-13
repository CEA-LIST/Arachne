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

ecore_rs::prelude! {}

pub const CONTENT: &str = include_str!("../rsc/aas.ecore");

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
    println!("|==|");
}
