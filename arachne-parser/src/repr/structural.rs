prelude!(repr::bounds::Bounds);
use crate::repr::{Annot, Annots};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typ {
    EReference,
    EAttribute,
}

impl Display for Typ {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EReference => "EReference".fmt(f),
            Self::EAttribute => "EAttribute".fmt(f),
        }
    }
}

impl Typ {
    pub fn from_xsi_type(s: impl AsRef<str>) -> Res<Self> {
        let s = s.as_ref();
        match s {
            "ecore:EAttribute" => Ok(Self::EAttribute),
            "ecore:EReference" => Ok(Self::EReference),
            _ => bail!("unexpected structural feature `xsi:type` value `{}`", s),
        }
    }

    pub fn parse_bounds(self, lbound: Option<&str>, ubound: Option<&str>) -> Res<Bounds> {
        let lbound = match (lbound, self) {
            (Some(lbound), _) => lbound,
            (None, Self::EReference) => "0",
            (None, Self::EAttribute) => "1",
        };
        Bounds::from_str(Some(lbound), ubound)
    }
}

#[derive(Debug, Clone)]
pub struct Structural {
    pub name: String,
    pub kind: Typ,
    pub typ: Option<idx::Class>,
    pub typ_path: Option<String>,
    /// Upper and lower bounds of the feature. Default is `0..1` for references and `1..1` for attributes.
    pub bounds: Bounds,
    /// Indicates whether the feature is a containment reference. Default is false.
    pub containment: bool,
    /// Indicates whether the feature is an ID. Default is false.
    pub is_id: bool,
    /// Indicates whether the feature value is ordered. Default is true.
    pub ordered: Option<bool>,
    /// Indicates whether the feature value may be modified. Default is true.
    pub changeable: Option<bool>,
    /// Indicates whether the feature value is transient. Default is false.
    pub volatile: Option<bool>,
    /// Indicates whether the feature value is transient (not persisted). Default is false.
    pub transient: Option<bool>,
    /// Indicates whether the feature value is derived from other features. Default is false.
    pub derived: Option<bool>,
    /// Indicates whether the feature value is unique. Default is true.
    pub unique: Option<bool>,
    pub annotations: Annots,
}
impl Structural {
    pub fn new(name: impl Into<String>, kind: Typ, typ: idx::Class, bounds: Bounds) -> Self {
        Self {
            name: name.into(),
            kind,
            typ: Some(typ),
            typ_path: None,
            bounds,
            containment: false,
            is_id: false,
            ordered: None,
            changeable: None,
            volatile: None,
            transient: None,
            derived: None,
            unique: None,
            annotations: Annots::with_capacity(2),
        }
    }

    pub fn with_external(
        name: impl Into<String>,
        kind: Typ,
        typ_path: impl Into<String>,
        bounds: Bounds,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            typ: None,
            typ_path: Some(typ_path.into()),
            bounds,
            containment: false,
            is_id: false,
            ordered: None,
            changeable: None,
            volatile: None,
            transient: None,
            derived: None,
            unique: None,
            annotations: Annots::with_capacity(2),
        }
    }

    pub fn set_containment(&mut self, flag: bool) {
        self.containment = flag
    }
    pub fn try_set_containment(&mut self, flag: Option<bool>) {
        if let Some(flag) = flag {
            self.set_containment(flag)
        }
    }
    pub fn set_is_id(&mut self, flag: bool) {
        self.is_id = flag
    }
    pub fn try_set_is_id(&mut self, flag: Option<bool>) {
        if let Some(flag) = flag {
            self.set_is_id(flag)
        }
    }
    pub fn set_ordered(&mut self, flag: bool) {
        self.ordered = Some(flag);
    }
    pub fn try_set_ordered(&mut self, flag: Option<bool>) {
        if let Some(flag) = flag {
            self.set_ordered(flag)
        }
    }
    pub fn set_changeable(&mut self, flag: bool) {
        self.changeable = Some(flag);
    }
    pub fn try_set_changeable(&mut self, flag: Option<bool>) {
        if let Some(flag) = flag {
            self.set_changeable(flag)
        }
    }
    pub fn set_volatile(&mut self, flag: bool) {
        self.volatile = Some(flag);
    }
    pub fn try_set_volatile(&mut self, flag: Option<bool>) {
        if let Some(flag) = flag {
            self.set_volatile(flag)
        }
    }
    pub fn set_transient(&mut self, flag: bool) {
        self.transient = Some(flag);
    }
    pub fn try_set_transient(&mut self, flag: Option<bool>) {
        if let Some(flag) = flag {
            self.set_transient(flag)
        }
    }
    pub fn set_derived(&mut self, flag: bool) {
        self.derived = Some(flag);
    }
    pub fn try_set_derived(&mut self, flag: Option<bool>) {
        if let Some(flag) = flag {
            self.set_derived(flag)
        }
    }
    pub fn set_unique(&mut self, flag: bool) {
        self.unique = Some(flag);
    }
    pub fn try_set_unique(&mut self, flag: Option<bool>) {
        if let Some(flag) = flag {
            self.set_unique(flag)
        }
    }
    pub fn annotations(&self) -> &Annots {
        &self.annotations
    }
    pub fn add_annotation(&mut self, annot: Annot) {
        self.annotations.push(annot)
    }
}
