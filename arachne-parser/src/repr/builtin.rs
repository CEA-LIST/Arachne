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

//! Builtin Ecore classes/types.

use std::str::FromStr;

prelude! {
    repr::*,
}

pub type Builder = fn(Path, idx::Class) -> Class;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Typ {
    Object,
    EByte,
    EShort,
    EInt,
    ELong,
    EFloat,
    EDouble,
    EBoolean,
    EChar,
    EString,

    // Opaque Ecore datatypes, only meaningful for operation signatures
    EDiagnosticChain,
    EMap,
    EJavaObject,

    // Ecore Classes
    EObject,
    EModelElement,
    ENamedElement,
    ETypedElement,
    EStructuralFeature,
    EAttribute,
    EReference,
    EOperation,
    EParameter,
    EClassifier,
    EClass,
    EDataType,
    EEnum,
    EEnumLiteral,
    EPackage,
    EFactory,
    EAnnotation,
    EStringToStringMapEntry,
    EGenericType,
    ETypeParameter,
}

impl FromStr for Typ {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Object" => Ok(Self::Object),
            "EByte" => Ok(Self::EByte),
            "EShort" => Ok(Self::EShort),
            "EInt" => Ok(Self::EInt),
            "ELong" => Ok(Self::ELong),
            "EFloat" => Ok(Self::EFloat),
            "EDouble" => Ok(Self::EDouble),
            "EBoolean" => Ok(Self::EBoolean),
            "EChar" => Ok(Self::EChar),
            "EString" => Ok(Self::EString),
            "EDiagnosticChain" => Ok(Self::EDiagnosticChain),
            "EMap" => Ok(Self::EMap),
            "EJavaObject" => Ok(Self::EJavaObject),

            "EObject" => Ok(Self::EObject),
            "EModelElement" => Ok(Self::EModelElement),
            "ENamedElement" => Ok(Self::ENamedElement),
            "ETypedElement" => Ok(Self::ETypedElement),
            "EStructuralFeature" => Ok(Self::EStructuralFeature),
            "EAttribute" => Ok(Self::EAttribute),
            "EReference" => Ok(Self::EReference),
            "EOperation" => Ok(Self::EOperation),
            "EParameter" => Ok(Self::EParameter),
            "EClassifier" => Ok(Self::EClassifier),
            "EClass" => Ok(Self::EClass),
            "EDataType" => Ok(Self::EDataType),
            "EEnum" => Ok(Self::EEnum),
            "EEnumLiteral" => Ok(Self::EEnumLiteral),
            "EPackage" => Ok(Self::EPackage),
            "EFactory" => Ok(Self::EFactory),
            "EAnnotation" => Ok(Self::EAnnotation),
            "EStringToStringMapEntry" => Ok(Self::EStringToStringMapEntry),
            "EGenericType" => Ok(Self::EGenericType),
            "ETypeParameter" => Ok(Self::ETypeParameter),
            _ => Err(()),
        }
    }
}

impl AsRef<Typ> for Typ {
    fn as_ref(&self) -> &Typ {
        self
    }
}

impl Display for Typ {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use Typ::*;
        match self {
            Object => "Object".fmt(fmt),
            EByte => "EByte".fmt(fmt),
            EShort => "EShort".fmt(fmt),
            EInt => "EInt".fmt(fmt),
            ELong => "ELong".fmt(fmt),
            EFloat => "EFloat".fmt(fmt),
            EDouble => "EDouble".fmt(fmt),
            EBoolean => "EBoolean".fmt(fmt),
            EChar => "EChar".fmt(fmt),
            EString => "EString".fmt(fmt),
            EDiagnosticChain => "EDiagnosticChain".fmt(fmt),
            EMap => "EMap".fmt(fmt),
            EJavaObject => "EJavaObject".fmt(fmt),

            EObject => "EObject".fmt(fmt),
            EModelElement => "EModelElement".fmt(fmt),
            ENamedElement => "ENamedElement".fmt(fmt),
            ETypedElement => "ETypedElement".fmt(fmt),
            EStructuralFeature => "EStructuralFeature".fmt(fmt),
            EAttribute => "EAttribute".fmt(fmt),
            EReference => "EReference".fmt(fmt),
            EOperation => "EOperation".fmt(fmt),
            EParameter => "EParameter".fmt(fmt),
            EClassifier => "EClassifier".fmt(fmt),
            EClass => "EClass".fmt(fmt),
            EDataType => "EDataType".fmt(fmt),
            EEnum => "EEnum".fmt(fmt),
            EEnumLiteral => "EEnumLiteral".fmt(fmt),
            EPackage => "EPackage".fmt(fmt),
            EFactory => "EFactory".fmt(fmt),
            EAnnotation => "EAnnotation".fmt(fmt),
            EStringToStringMapEntry => "EStringToStringMapEntry".fmt(fmt),
            EGenericType => "EGenericType".fmt(fmt),
            ETypeParameter => "ETypeParameter".fmt(fmt),
        }
    }
}

impl Typ {
    pub(crate) const ECORE_BUILTIN: &'static str = "ecore:Builtin";
    const ECORE_URL: &'static str = "http://www.eclipse.org/emf/2002/Ecore#//";
    const ECORE_PLATFORM_URL: &'static str = "platform:/plugin/org.eclipse.emf.ecore/model/Ecore.ecore#//";
    const UML_TYPES_URL: &'static str = "platform:/plugin/org.eclipse.uml2.types/model/Types.ecore#//";

    /// Rewrites platform URLs of well-known datatypes to the canonical Ecore URL.
    fn normalize_etype_url(s: &str) -> String {
        if let Some(name) = s.strip_prefix(Self::ECORE_PLATFORM_URL) {
            return format!("{}{}", Self::ECORE_URL, name);
        }
        let mapped = match s.strip_prefix(Self::UML_TYPES_URL) {
            Some("Boolean") => "EBoolean",
            Some("String") => "EString",
            // `UnlimitedNatural` uses -1 for unbounded, which fits a signed integer
            Some("Integer" | "UnlimitedNatural") => "EInt",
            Some("Real") => "EDouble",
            _ => return s.to_string(),
        };
        format!("{}{}", Self::ECORE_URL, mapped)
    }
}

macro_rules! builtin_builders {
    ( $(
        $builder_name:ident = $variant:ident
    ),* $(,)? ) => {
        impl Typ {
            $(
                fn $builder_name() -> (Typ, Builder) {
                    (Typ::$variant, |path, idx| {
                        Class::new(
                            idx,
                            path,
                            Typ::ECORE_BUILTIN,
                            stringify!($variant),
                            None as Option<String>,
                            Some(false),
                            Some(false),
                        )
                    })
                }
            )*

            ///  Lists all builtin types and their corresponding class builder.
            pub fn builders() -> impl IntoIterator<Item = (Typ, Builder)> {
                [
                    $( Self::$builder_name() , )*
                ].into_iter()
            }

            /// Parses a builtin type URL appearing after a `ecore:EDataType` in an `eType` attribute.
            pub fn parse_etype_url(s: impl AsRef<str>) -> Res<Self> {
                let normalized = Self::normalize_etype_url(s.as_ref());
                let s = normalized.as_str();
                match s {
                    $(
                        concat!("http://www.eclipse.org/emf/2002/Ecore#//", stringify!($variant)) =>
                            Ok(Self::$variant),
                    )*
                    _ => bail!("unknown builtit class URL `{}`", s),
                }
            }
        }
    };
}

builtin_builders! {
    build_object = Object,
    build_byte = EByte,
    build_short = EShort,
    build_int = EInt,
    build_long = ELong,
    build_float = EFloat,
    build_double = EDouble,
    build_bool = EBoolean,
    build_char = EChar,
    build_string = EString,
    build_ediagnosticchain = EDiagnosticChain,
    build_emap = EMap,
    build_ejavaobject = EJavaObject,

    build_eobject = EObject,
    build_emodelelement = EModelElement,
    build_enamedelement = ENamedElement,
    build_etypedelement = ETypedElement,
    build_estructuralfeature = EStructuralFeature,
    build_eattribute = EAttribute,
    build_ereference = EReference,
    build_eoperation = EOperation,
    build_eparameter = EParameter,
    build_eclassifier = EClassifier,
    build_eclass = EClass,
    build_edatatype = EDataType,
    build_eenum = EEnum,
    build_eenumliteral = EEnumLiteral,
    build_epackage = EPackage,
    build_efactory = EFactory,
    build_eannotation = EAnnotation,
    build_estringtostringmapentry = EStringToStringMapEntry,
    build_egenerictype = EGenericType,
    build_etypeparameter = ETypeParameter,
}

impl Typ {
    /// Recognizes a builtin `ecore` datatype from its `eType` description.
    ///
    /// Returns `None` if `s` does not start with `ecore:EDataType`, otherwise returns the type if it is
    /// recognized from the URL part and an error if not.
    ///
    /// If you know you're parsing a builtin type, like if you already parsed the `ecore:EDataType`
    /// header of the `eType`, use [`Self::parse_etype_url`] instead.
    pub fn try_parse_etype(s: impl AsRef<str>) -> Res<Option<Self>> {
        let s = s.as_ref();
        let Some(rest) = ["ecore:EDataType", "ecore:EClass"]
            .into_iter()
            .find_map(|pref| s.strip_prefix(pref))
        else {
            return Ok(None);
        };
        Self::parse_etype_url(rest.trim()).map(Some)
    }
}
