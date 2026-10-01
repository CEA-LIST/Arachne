//! Shared, deterministic XMI serialization support for Arachne-generated CRDTs.

include!("runtime.rs");

/// Canonical production source copied into standalone generated projects.
#[doc(hidden)]
pub const RUNTIME_SOURCE: &str = include_str!("runtime.rs");

#[cfg(test)]
mod tests {
    use quick_xml::{Reader, events::Event};

    use super::{Attributes, ReferenceIndex, Writer, path_id};

    #[test]
    fn writes_deterministic_escaped_xmi() {
        let mut writer = Writer::new("example", "urn:test&model");
        let mut attributes = Attributes::for_object(&"root/object");
        attributes.push_value("z", "<&\"'>");
        attributes.push_values("links", ["#b", "#a"], false);
        writer.element("example:Root", &attributes, |writer| {
            writer.element("children", &Attributes::for_object(&"root/child"), |_| {});
        });

        let xml = String::from_utf8(writer.finish()).expect("XMI must be UTF-8");
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("xmlns:example=\"urn:test&amp;model\""));
        assert!(xml.contains("z=\"&lt;&amp;&quot;&apos;&gt;\""));
        assert!(xml.contains("links=\"#b #a\""));
        assert!(xml.contains("<children xmi:id=\"arachne_"));

        let mut reader = Reader::from_str(&xml);
        loop {
            if matches!(
                reader
                    .read_event()
                    .expect("the generated XMI must be well-formed"),
                Event::Eof
            ) {
                break;
            }
        }
    }

    #[test]
    fn merges_repeated_feature_values() {
        let mut attributes = Attributes::default();
        attributes.push_values("link", ["#one"], false);
        attributes.push_values("link", ["#two"], false);
        assert_eq!(attributes.0, [("link".into(), "#one #two".into())]);
    }

    #[test]
    fn path_identifier_is_stable_and_distinguishes_paths() {
        assert_eq!(path_id(&"a"), "arachne_61");
        assert_ne!(path_id(&"a/b"), path_id(&"a.b"));
    }

    #[test]
    fn indexes_reference_targets_deterministically() {
        let mut references = ReferenceIndex::default();
        references.insert("source", "feature", &"target-b");
        references.insert("source", "feature", &"target-a");
        references.insert("source", "feature", &"target-b");

        assert_eq!(
            references.values(&"source", "feature"),
            [
                format!("#{}", path_id(&"target-a")),
                format!("#{}", path_id(&"target-b")),
            ]
        );
        assert!(references.values(&"source", "missing").is_empty());
    }
}
