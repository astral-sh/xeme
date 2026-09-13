//! Borrowed namespace spellings for a literal start tag with unchanged bindings.

use xeme_storage::{HashMap, String};

use crate::{Config, RawAttribute, arena};

#[derive(Clone, Copy)]
pub(crate) struct Name<'a> {
    pub(crate) uri: Option<&'a str>,
    pub(crate) prefix: Option<&'a str>,
    pub(crate) local: &'a str,
}

impl<'a> Name<'a> {
    /// Resolve a QName already validated as an XML Name by the tag scanner.
    fn resolve(
        name: &'a str,
        attribute: bool,
        config: &Config,
        namespaces: &'a HashMap<String, String>,
        default: Option<&'a str>,
    ) -> Option<Self> {
        let (prefix, local, uri) = match name.split_once(':') {
            Some((prefix, local)) => {
                if prefix.is_empty()
                    || !local
                        .chars()
                        .next()
                        .is_some_and(|first| config.name_rules.is_name_start(first))
                    || local.contains(':')
                    || prefix == "xmlns"
                {
                    return None;
                }
                (Some(prefix), local, Some(namespaces.get(prefix)?.as_str()))
            }
            None => (None, name, if attribute { None } else { default }),
        };
        Some(Self { uri, prefix, local })
    }

    /// Keep the original conservative frame bound, including both separators.
    fn extra_bytes(self, separator: char) -> Option<usize> {
        self.uri.map_or(Some(0), |uri| {
            uri.len().checked_add(if separator == '\0' {
                0
            } else {
                2 * separator.len_utf8()
            })
        })
    }

    /// Final pieces share the same serialization used by expanded duplicate keys.
    pub(crate) fn parts(self, separator: &'a str, triplets: bool) -> [&'a str; 5] {
        let prefix = self.prefix.filter(|_| triplets && !separator.is_empty());
        [
            self.uri.unwrap_or(""),
            if self.uri.is_some() { separator } else { "" },
            self.local,
            if prefix.is_some() { separator } else { "" },
            prefix.unwrap_or(""),
        ]
    }

    /// Compare serialized keys, including collisions with a NUL separator.
    pub(crate) fn matches_key(self, separator: &str, key: &[u8]) -> bool {
        key.strip_prefix(self.uri.unwrap_or("").as_bytes())
            .and_then(|tail| tail.strip_prefix(separator.as_bytes()))
            == Some(self.local.as_bytes())
    }
}

pub(crate) struct FramePlan<'a> {
    pub(crate) element: Name<'a>,
    pub(crate) attributes: [Option<Name<'a>>; 8],
    pub(crate) separator: char,
}

impl<'a> FramePlan<'a> {
    /// Check eligibility without allocating, charging work, or changing bindings.
    pub(crate) fn new(
        name: &'a str,
        rest: &'a str,
        raw_attributes: &[RawAttribute],
        token_bytes: usize,
        config: &Config,
        namespaces: &'a HashMap<String, String>,
        default: Option<&'a str>,
    ) -> Option<Self> {
        let separator = config.namespace_separator?;
        let element = Name::resolve(name, false, config, namespaces, default)?;
        let mut bytes = token_bytes.checked_add(element.extra_bytes(separator)?)?;
        let mut attributes = [None; 8];
        for (index, attribute) in raw_attributes.iter().enumerate() {
            let name = attribute.name(rest);
            if name == "xmlns" {
                return None;
            }
            if name.contains(':') {
                if raw_attributes.len() > attributes.len() {
                    return None;
                }
                let name = Name::resolve(name, true, config, namespaces, default)?;
                bytes = bytes.checked_add(name.extra_bytes(separator)?)?;
                attributes[index] = Some(name);
            }
        }
        (bytes <= arena::MAX_ARENA_BYTES).then_some(Self {
            element,
            attributes,
            separator,
        })
    }
}
