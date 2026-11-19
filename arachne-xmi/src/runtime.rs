use std::{collections::HashMap, fmt::Display, hash::Hash};

use quick_xml::{
    Writer as XmlWriter,
    events::{BytesDecl, BytesEnd, BytesStart, Event},
};

/// Deterministically ordered XML attributes with support for multi-valued
/// Ecore structural features.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attributes(Vec<(String, String)>);

impl Attributes {
    /// Starts an attribute set with the stable XMI identifier for an object.
    pub fn for_object(path: &impl Display) -> Self {
        let mut attributes = Self::default();
        attributes.push_value("xmi:id", path_id(path));
        attributes
    }

    /// Adds a single XML attribute, merging it with an existing attribute of
    /// the same name as a space-separated Ecore value.
    pub fn push_value(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.push_values(name, [value.into()], true);
    }

    /// Adds an Ecore attribute or reference represented by zero or more
    /// space-separated values.
    pub fn push_values<I, S>(&mut self, name: impl Into<String>, values: I, force: bool)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut values = values.into_iter().map(Into::into).collect::<Vec<_>>();
        values.retain(|value| force || !value.is_empty());
        if values.is_empty() && !force {
            return;
        }

        let name = name.into();
        let value = values.join(" ");
        if let Some((_, existing)) = self
            .0
            .iter_mut()
            .find(|(existing_name, _)| *existing_name == name)
        {
            if !existing.is_empty() && !value.is_empty() {
                existing.push(' ');
            }
            existing.push_str(&value);
        } else {
            self.0.push((name, value));
        }
    }

    fn sorted(&self) -> Vec<(&str, &str)> {
        let mut attributes = self
            .0
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        attributes.sort_unstable_by_key(|(name, _)| *name);
        attributes
    }
}

/// Streaming XMI writer backed by `quick-xml`.
pub struct Writer {
    inner: XmlWriter<Vec<u8>>,
}

impl Writer {
    /// Creates an XMI 2.0 document and opens its `xmi:XMI` document element.
    pub fn new(namespace_prefix: &str, namespace_uri: &str) -> Self {
        let mut writer = Self {
            inner: XmlWriter::new_with_indent(Vec::new(), b' ', 2),
        };
        writer.write(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)));

        let mut attributes = Attributes::default();
        attributes.push_value("xmi:version", "2.0");
        attributes.push_value("xmlns:xmi", "http://www.omg.org/XMI");
        attributes.push_value("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance");
        attributes.push_value(format!("xmlns:{namespace_prefix}"), namespace_uri);
        writer.start_element("xmi:XMI", &attributes);
        writer
    }

    /// Writes one nested element. XML framing and escaping are handled by the
    /// runtime; the closure only visits its children.
    pub fn element(
        &mut self,
        name: &str,
        attributes: &Attributes,
        children: impl FnOnce(&mut Self),
    ) {
        self.start_element(name, attributes);
        children(self);
        self.write(Event::End(BytesEnd::new(name)));
    }

    /// Closes the XMI document and returns its UTF-8 bytes.
    pub fn finish(mut self) -> Vec<u8> {
        self.write(Event::End(BytesEnd::new("xmi:XMI")));
        let mut bytes = self.inner.into_inner();
        bytes.push(b'\n');
        bytes
    }

    fn start_element(&mut self, name: &str, attributes: &Attributes) {
        let mut start = BytesStart::new(name);
        for attribute in attributes.sorted() {
            start.push_attribute(attribute);
        }
        self.write(Event::Start(start));
    }

    fn write(&mut self, event: Event<'_>) {
        self.inner
            .write_event(event)
            .expect("writing XMI to an in-memory buffer cannot fail");
    }
}

/// Produces a deterministic XML `NCName`-compatible identifier from an object
/// path. Hex encoding makes the mapping injective for UTF-8 path strings.
pub fn path_id(path: &impl Display) -> String {
    let raw = path.to_string();
    let mut id = String::with_capacity(raw.len() * 2 + 8);
    id.push_str("arachne_");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in raw.bytes() {
        id.push(HEX[(byte >> 4) as usize] as char);
        id.push(HEX[(byte & 0x0f) as usize] as char);
    }
    id
}

/// Precomputed links from replicated object paths and Ecore features to XMI
/// object references.
#[derive(Clone, Debug)]
pub struct ReferenceIndex<K> {
    values: HashMap<K, HashMap<&'static str, Vec<String>>>,
}

impl<K> Default for ReferenceIndex<K> {
    fn default() -> Self {
        Self {
            values: HashMap::new(),
        }
    }
}

impl<K> ReferenceIndex<K>
where
    K: Eq + Hash,
{
    /// Adds a link, keeping each feature's targets sorted and deduplicated.
    pub fn insert(&mut self, source: K, feature: &'static str, target: &impl Display) {
        let target = format!("#{}", path_id(target));
        let values = self
            .values
            .entry(source)
            .or_default()
            .entry(feature)
            .or_default();
        if let Err(position) = values.binary_search(&target) {
            values.insert(position, target);
        }
    }

    /// Returns the XMI links for one replicated object feature.
    pub fn values(&self, source: &K, feature: &str) -> Vec<String> {
        self.values
            .get(source)
            .and_then(|features| features.get(feature))
            .cloned()
            .unwrap_or_default()
    }
}
