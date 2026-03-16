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
            EcoreType::EByte
            | EcoreType::EShort
            | EcoreType::EInt
            | EcoreType::ELong
            | EcoreType::EFloat
            | EcoreType::EDouble => Primitive::Counter(Counter::default()),
            EcoreType::EBoolean => Primitive::Flag(Default::default()),
            EcoreType::EChar => Primitive::Register(Default::default()),
            EcoreType::EString => Primitive::List,
            EcoreType::Object => unimplemented!(),
        }
    }

    fn to_rust_type(&self) -> Option<TokenStream> {
        match self {
            EcoreType::EByte => Some(quote! { u8 }),
            EcoreType::EShort => Some(quote! { i16 }),
            EcoreType::EInt => Some(quote! { i32 }),
            EcoreType::ELong => Some(quote! { i64 }),
            EcoreType::EFloat => Some(quote! { f32 }),
            EcoreType::EDouble => Some(quote! { f64 }),
            EcoreType::EChar => Some(quote! {char }),
            EcoreType::EString => Some(quote! { std::string::String }),
            EcoreType::EBoolean => Some(quote! { bool }),
            EcoreType::Object => unimplemented!(),
        }
    }
}
