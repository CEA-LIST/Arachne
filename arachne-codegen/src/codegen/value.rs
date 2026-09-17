//! The value type every emitted log reads as.
//!
//! Moirai v0.7 took the `Value` associated type off `IsLog`, and its `union!`
//! asks for a value type per variant (`Variant(Op, Log => Value)`). A variant's
//! value can only be named where the record it belongs to has a *nominal* value
//! type, and `record!`'s bare form does not give one: it writes a generic
//! `FooValue<NameValue, ...>` whose parameters a recursive model can never
//! close — `bt_crdt`'s BehaviorTree → TreeNodeKind → Decorator → TreeNodeKind
//! would need its own value type to instantiate its own value type. So every
//! `record!` the generator writes uses the explicit `field: Log => Value` form,
//! and every site that builds a log type computes its read-out beside it.
//!
//! The mapping is the one Moirai's `EvalNested`/`Eval` impls declare, leaf by
//! leaf and wrapper by wrapper; this module is the single place that states it.

use proc_macro2::TokenStream;
use quote::quote;

use crate::codegen::{
    datatype::crdt::{Primitive, Register, Set},
    import::Import,
};

/// `moirai_macros` re-exports the `rustc_hash` aliases publicly, and
/// `moirai-macros` and `moirai-crdt` pin the same `rustc-hash` 1.1, so the
/// alias named here is the very type `AWSet`, `MVRegister` and `UWMapLog` read
/// as. `moirai-crdt`'s own `HashMap`/`HashSet` are private, and
/// `moirai-protocol`'s are over `rustc-hash` 2, which is a different type.
const HASH_MAP: &str = "moirai_macros::HashMap";
const HASH_SET: &str = "moirai_macros::HashSet";

/// A value type and whatever the generated module has to import to name it.
#[derive(Clone, Debug)]
pub struct Value {
    tokens: TokenStream,
    imports: Vec<Import>,
}

impl Value {
    /// A value type that needs no import of its own.
    pub fn plain(tokens: TokenStream) -> Self {
        Self {
            tokens,
            imports: Vec::new(),
        }
    }

    pub fn into_parts(self) -> (TokenStream, Vec<Import>) {
        (self.tokens, self.imports)
    }

    /// The read-out of an `OptionLog` over this one.
    pub fn optional(self) -> Self {
        self.wrap(|inner| quote! { Option<#inner> })
    }

    /// The read-out of a `NestedListLog` over this one.
    pub fn sequence(self) -> Self {
        self.wrap(|inner| quote! { Vec<#inner> })
    }

    /// The read-out of a `BoxedLog` over this one, which keeps the
    /// indirection a recursive model needs.
    pub fn boxed(self) -> Self {
        self.wrap(|inner| quote! { Box<#inner> })
    }

    /// The read-out of a `BoxedLog` over this one when `boxed` is set, and
    /// this one otherwise.
    pub fn boxed_if(self, boxed: bool) -> Self {
        if boxed { self.boxed() } else { self }
    }

    /// The read-out of a `UWMapLog<K, L>` whose value log reads as this one.
    pub fn keyed_by(mut self, key: &TokenStream, path: &syn::Path) -> Self {
        let inner = self.tokens;
        self.tokens = quote! { #path::HashMap<#key, #inner> };
        self.imports.push(Import::Custom(HASH_MAP));
        self
    }

    fn wrap(mut self, f: impl FnOnce(&TokenStream) -> TokenStream) -> Self {
        self.tokens = f(&self.tokens);
        self
    }
}

/// The read-out of a leaf CRDT, the operation type a `VecLog` or a `GraphLog`
/// holds.
///
/// `rust_ty` is the attribute's declared Rust type; the flag leaves ignore it,
/// and `Primitive::List` is always `List<char>` at this site — the
/// unique-and-ordered many-valued shape builds its own `List<T>` and asks for
/// a sequence of the element type instead.
pub fn leaf(primitive: &Primitive, rust_ty: Option<&TokenStream>, path: &syn::Path) -> Value {
    match primitive {
        Primitive::Counter(_) => {
            let rust_ty = rust_ty.expect("a counter has a declared Rust type");
            Value::plain(quote! { #rust_ty })
        }
        Primitive::Flag(_) => Value::plain(quote! { bool }),
        Primitive::Register(register) => {
            let rust_ty = rust_ty.expect("a register has a declared Rust type");
            self::register(register, &quote! { #rust_ty }, path)
        }
        // `GraphLog<List<char>>` reads as a `Vec<char>`. `List<char>` also
        // evaluates a `Read<String>`, but the equivalence oracles project the
        // read-out from the character sequence, so that is the one named here.
        Primitive::List => Value::plain(quote! { Vec<char> }),
    }
}

/// The read-out of one register over `inner`.
pub fn register(register: &Register, inner: &TokenStream, path: &syn::Path) -> Value {
    match register {
        // A multi-value and a partially ordered register both keep every
        // concurrent write.
        Register::MultiValue | Register::PartiallyOrdered => set_of(inner, path),
        // A register that breaks every tie holds at most one value.
        Register::LastWriterWins | Register::Fair | Register::TotallyOrdered => {
            Value::plain(quote! { Option<#inner> })
        }
    }
}

/// The read-out of a set CRDT over `element`. Add-wins and remove-wins differ
/// on what survives a concurrent remove, not on what a read gives back.
pub fn set(_set: &Set, element: &TokenStream, path: &syn::Path) -> Value {
    set_of(element, path)
}

/// The read-out of an `AWBagLog<V>`: every value with its multiplicity.
pub fn bag(element: &TokenStream, path: &syn::Path) -> Value {
    Value {
        tokens: quote! { #path::HashMap<#element, usize> },
        imports: vec![Import::Custom(HASH_MAP)],
    }
}

fn set_of(element: &TokenStream, path: &syn::Path) -> Value {
    Value {
        tokens: quote! { #path::HashSet<#element> },
        imports: vec![Import::Custom(HASH_SET)],
    }
}
