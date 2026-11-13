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

use ecore_rs::repr::builtin::Typ as EcoreType;
use proc_macro2::TokenStream;
use quote::quote;

use crate::codegen::datatype::crdt::{Counter, Primitive};

pub trait ToCrdt {
    fn to_crdt_container(&self) -> Primitive;
    fn to_rust_type(&self) -> Option<TokenStream>;
}

impl ToCrdt for EcoreType {
    fn to_crdt_container(&self) -> Primitive {
        match self {
            Self::EByte
            | Self::EShort
            | Self::EInt
            | Self::ELong
            | Self::EFloat
            | Self::EDouble => Primitive::Counter(Counter::default()),
            Self::EBoolean => Primitive::Flag(Default::default()),
            Self::EChar => Primitive::Register(Default::default()),
            Self::EString => Primitive::List,
            // We do not support object types for CRDTs, and don't expect to encounter them
            Self::Object => unimplemented!(),
        }
    }

    fn to_rust_type(&self) -> Option<TokenStream> {
        match self {
            Self::EByte => Some(quote! { i8 }),
            Self::EShort => Some(quote! { i16 }),
            Self::EInt => Some(quote! { i32 }),
            Self::ELong => Some(quote! { i64 }),
            Self::EFloat => Some(quote! { f32 }),
            Self::EDouble => Some(quote! { f64 }),
            Self::EChar => Some(quote! {char }),
            Self::EString => Some(quote! { std::string::String }),
            Self::EBoolean => Some(quote! { bool }),
            Self::Object => unimplemented!(),
        }
    }
}
