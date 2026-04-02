use std::{fs, path::Path};

use ecore_rs::ctx::Ctx;

use crate::error::Result;

/// Parser for Ecore metamodels
pub struct EcoreParser {
    pub ctx: Ctx,
}

impl EcoreParser {
    /// Parses an Ecore metamodel from a file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)?;
        Self::from_string(&content)
    }

    /// Parses an Ecore metamodel from a string
    pub fn from_string(content: &str) -> Result<Self> {
        let ctx = Ctx::parse(content)?;
        Ok(Self { ctx })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::EcoreParser;

    #[test]
    fn rejects_unknown_external_datatype() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="test"
    nsURI="http://example.org/test"
    nsPrefix="test">
    <eClassifiers xsi:type="ecore:EClass" name="Node">
        <eStructuralFeatures xsi:type="ecore:EAttribute"
            name="label"
            eType="ecore:EDataType ../../org.eclipse.uml2.types/model/Types.ecore#//String"/>
    </eClassifiers>
</ecore:EPackage>"##;

        assert!(EcoreParser::from_string(ecore).is_err());
    }

    #[test]
    fn rejects_interpackage_links() {
        let ecore = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="root"
    nsURI="http://example.org/root"
    nsPrefix="root">
    <eClassifiers xsi:type="ecore:EClass" name="A">
        <eStructuralFeatures xsi:type="ecore:EReference" name="toB" eType="#//sub/B"/>
    </eClassifiers>
    <ecore:EPackage name="sub" nsURI="http://example.org/root/sub" nsPrefix="sub">
        <eClassifiers xsi:type="ecore:EClass" name="B"/>
    </ecore:EPackage>
</ecore:EPackage>"##;

        assert!(EcoreParser::from_string(ecore).is_err());
    }

    #[test]
    fn preserves_subclasses_for_forward_referenced_supertypes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/Company.ecore");
        let parser = EcoreParser::from_file(path).expect("company.ecore should parse");
        let subunit = parser
            .ctx
            .classes()
            .iter()
            .find(|class| class.name() == "Subunit")
            .expect("Subunit should exist");

        let subclass_names: Vec<_> = subunit
            .sub()
            .iter()
            .map(|idx| parser.ctx.classes()[**idx].name().to_string())
            .collect();

        assert_eq!(subclass_names, vec!["Dept".to_string(), "Employee".to_string()]);
    }
}
