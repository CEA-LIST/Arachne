//! Ecore's own classes, as the parser builds them in when a metamodel refers to one of them (see
//! `ecore_rs::repr::ecore`).

use ecore_rs::{
    ctx::Ctx,
    repr::{ecore::Typ, idx},
};

/// Ecore's `EObject`, if the metamodel refers to Ecore's classes.
pub fn eobject(ctx: &Ctx) -> Option<idx::Class> {
    ctx.ecore_classes().get(&Typ::EObject).copied()
}

/// True if `class` is Ecore's `EObject`, the class of every object.
///
/// `EObject` has no structural feature and is never generated: a supertype `EObject` adds
/// nothing, and a non-containment reference typed by it refers to an object of any class.
pub fn is_eobject(ctx: &Ctx, class: idx::Class) -> bool {
    eobject(ctx) == Some(class)
}
