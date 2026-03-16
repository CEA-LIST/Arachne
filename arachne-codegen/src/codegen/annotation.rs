use ecore_rs::repr::{Class, Structural, annot::Val, structural::Typ as StructuralTyp};

use crate::codegen::datatype::crdt::{Counter, Flag, Primitive, Register, Set};

const SEMANTICS_SOURCE: &str = "urn:arachne:semantics";
const DATATYPE_KEY: &str = "datatype";
const KEY_FEATURE_KEY: &str = "key-feature";
const VALUE_FEATURE_KEY: &str = "value-feature";
const REPRESENTATION_SOURCE: &str = "urn:arachne:representation";
const KIND_KEY: &str = "kind";
const FIELD_KEY: &str = "field";

#[derive(Clone, Debug)]
pub enum DatatypeOverride {
    Primitive(Primitive),
    Set(Set),
}

#[derive(Clone, Debug)]
pub struct UwMapSpec {
    pub key_feature: String,
    pub value_feature: String,
}

pub fn datatype_override(feature: &Structural) -> Option<DatatypeOverride> {
    let value = feature
        .annotations()
        .iter()
        .find(|annot| annot.source() == SEMANTICS_SOURCE)
        .and_then(|annot| annot.details().get(DATATYPE_KEY))?;

    parse_datatype_override(feature.kind, value)
}

pub fn uw_map_spec(feature: &Structural) -> Option<UwMapSpec> {
    let annot = feature
        .annotations()
        .iter()
        .find(|annot| annot.source() == SEMANTICS_SOURCE)?;
    let datatype = annot.details().get(DATATYPE_KEY)?;
    if !datatype.trim().eq_ignore_ascii_case("uw-map") {
        return None;
    }

    Some(UwMapSpec {
        key_feature: annot
            .details()
            .get(KEY_FEATURE_KEY)
            .cloned()
            .unwrap_or_else(|| "key".to_string()),
        value_feature: annot
            .details()
            .get(VALUE_FEATURE_KEY)
            .cloned()
            .unwrap_or_else(|| "value".to_string()),
    })
}

pub fn transparent_field(class: &Class) -> Option<String> {
    let annot = class
        .annotations()
        .iter()
        .find(|annot| annot.source() == REPRESENTATION_SOURCE)?;
    match annot.details().get(KIND_KEY)?.as_str() {
        "transparent" => annot.details().get(FIELD_KEY).cloned(),
        _ => None,
    }
}

fn parse_datatype_override(kind: StructuralTyp, value: &Val) -> Option<DatatypeOverride> {
    let normalized = value.trim().to_ascii_lowercase();

    let primitive = match normalized.as_str() {
        "resettable-counter" => Some(Primitive::Counter(Counter::ResettableCounter)),
        "ew-flag" => Some(Primitive::Flag(Flag::EWFlag)),
        "dw-flag" => Some(Primitive::Flag(Flag::DWFlag)),
        "mv-register" => Some(Primitive::Register(Register::MultiValue)),
        "lww-register" => Some(Primitive::Register(Register::LastWriterWins)),
        "fair-register" => Some(Primitive::Register(Register::Fair)),
        "po-register" | "partial-order-register" => {
            Some(Primitive::Register(Register::PartiallyOrdered))
        }
        "to-register" | "total-order-register" => {
            Some(Primitive::Register(Register::TotallyOrdered))
        }
        "list" if kind == StructuralTyp::EAttribute => Some(Primitive::List),
        _ => None,
    };

    if let Some(primitive) = primitive {
        return Some(DatatypeOverride::Primitive(primitive));
    }

    match normalized.as_str() {
        "aw-set" => Some(DatatypeOverride::Set(Set::AWSet)),
        "rw-set" => Some(DatatypeOverride::Set(Set::RWSet)),
        _ => None,
    }
}
