//! Bounded reuse of attribute storage explicitly returned by an adapter.

use std::sync::atomic::{AtomicU64, Ordering};

use oriole_storage::{AllocError, Allocator, String, Vec};

use crate::Attribute;

const MAX_RECORDS: usize = 128;
const MAX_STRING_BYTES: usize = 4 * 1024;
const MAX_RETAINED_BYTES: usize = 64 * 1024;
static NEXT_PARSER_ID: AtomicU64 = AtomicU64::new(1);

/// Identifies the parser that emitted an event for a recycling adapter.
///
/// This opaque token is consumed on return and cannot be cloned. It prevents
/// accidental cross-parser returns; it does not authenticate mutated event data.
#[derive(Debug)]
pub struct RecyclingToken {
    parser_id: u64,
}

#[derive(Debug)]
pub(crate) struct AttributeRecycling {
    parser_id: u64,
    attributes: Vec<Attribute>,
}

impl AttributeRecycling {
    /// Assign a never-reused generation without allocating an identity object.
    pub(crate) fn new(allocator: Allocator) -> Result<Self, AllocError> {
        let parser_id = NEXT_PARSER_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| AllocError::CapacityOverflow)?;
        Ok(Self {
            parser_id,
            attributes: Vec::new_in(allocator),
        })
    }

    /// Copy only the generation number; no reference or shared-owner clone escapes.
    pub(crate) fn token(&self) -> RecyclingToken {
        RecyclingToken {
            parser_id: self.parser_id,
        }
    }

    /// Detach reusable owners before parsing; errors then drop them normally.
    pub(crate) fn take(&mut self) -> Vec<Attribute> {
        let allocator = *self.attributes.allocator();
        std::mem::replace(&mut self.attributes, Vec::new_in(allocator))
    }

    /// Keep bounded original capacities, dropping oversized owners without growth.
    pub(crate) fn recycle(&mut self, token: RecyclingToken, mut attributes: Vec<Attribute>) {
        if token.parser_id != self.parser_id || attributes.capacity() > MAX_RECORDS {
            return;
        }
        if attributes.capacity() == 0 {
            return;
        }
        let allocator = *self.attributes.allocator();
        let Some(mut remaining) = attributes
            .capacity()
            .checked_mul(size_of::<Attribute>())
            .and_then(|bytes| MAX_RETAINED_BYTES.checked_sub(bytes))
        else {
            return;
        };
        for attribute in &mut attributes {
            for text in [&mut attribute.name, &mut attribute.value] {
                let capacity = text.capacity();
                if capacity <= MAX_STRING_BYTES && capacity <= remaining {
                    remaining -= capacity;
                    text.clear();
                } else {
                    *text = String::new_in(allocator);
                }
            }
        }
        self.attributes = attributes;
    }
}

/// Refill an owner while preserving the spare terminator byte used by C callbacks.
pub(crate) fn copy_attribute_string(output: &mut String, text: &str) -> Result<(), AllocError> {
    let capacity = if text.is_empty() {
        0
    } else {
        text.len()
            .checked_add(1)
            .ok_or(AllocError::CapacityOverflow)?
    };
    output.clear();
    output.try_reserve(capacity)?;
    output.try_push_str(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, EventKind, Parser};

    fn next_attributes(parser: &mut Parser) -> (RecyclingToken, Vec<Attribute>) {
        loop {
            let (event, token) = parser.next_event_for_recycling().unwrap().unwrap();
            if let EventKind::StartElement { attributes, .. } = event.kind
                && !attributes.is_empty()
            {
                return (token, attributes);
            }
        }
    }

    #[test]
    fn returned_attributes_reuse_buffers_without_changing_owned_events() {
        let mut parser = Parser::new(Config::default());
        parser.feed(b"<r><n a='long literal' b='another literal'/><empty/><n a='short' b='value'/><n a='owned'/></r>", true).unwrap();
        let (token, mut attributes) = next_attributes(&mut parser);
        let name = attributes[0].name.as_ptr();
        let value = attributes[0].value.as_ptr();
        let vector = attributes.as_ptr();
        // The C adapter adds terminators before returning these original owners.
        for attribute in &mut attributes {
            attribute.name.try_push('\0').unwrap();
            attribute.value.try_push('\0').unwrap();
        }
        parser.recycle_attributes(token, attributes);
        let (_, attributes) = next_attributes(&mut parser);
        assert_eq!(attributes.as_ptr(), vector);
        assert_eq!(attributes[0].name.as_ptr(), name);
        assert_eq!(attributes[0].value.as_ptr(), value);
        assert_eq!(attributes[0].name, "a");
        assert_eq!(attributes[0].value, "short");
        assert_eq!(attributes[1].value, "value");
        // An event whose owners are retained instead of returned remains stable.
        let (_, next) = next_attributes(&mut parser);
        assert_eq!(next[0].value, "owned");
        assert_eq!(attributes[0].value, "short");
        assert_eq!(attributes[1].value, "value");
    }

    #[test]
    fn foreign_and_replaced_parser_generations_cannot_return_storage() {
        let mut first = Parser::new(Config::default());
        first.feed(b"<r a='value'/>", true).unwrap();
        let (token, attributes) = next_attributes(&mut first);
        let mut second = Parser::new(Config::default());
        second.recycle_attributes(token, attributes);
        assert!(second.attribute_recycling.attributes.is_empty());
        second.feed(b"<r a='value'/>", true).unwrap();
        let (token, attributes) = next_attributes(&mut second);
        // C Reset similarly replaces the core while retaining the C handle address.
        second = Parser::new(Config::default());
        second.recycle_attributes(token, attributes);
        assert!(second.attribute_recycling.attributes.is_empty());
    }

    #[test]
    fn retained_storage_obeys_record_string_and_aggregate_capacity_limits() {
        for (records, value_bytes) in [(1, 8192), (64, 1024), (129, 1)] {
            let value = "x".repeat(value_bytes);
            let mut xml = std::string::String::from("<r");
            for index in 0..records {
                use std::fmt::Write;
                write!(&mut xml, " a{index}='{value}'").unwrap();
            }
            xml.push_str("/>");
            let mut parser = Parser::new(Config::default());
            parser.feed(xml.as_bytes(), true).unwrap();
            let (token, attributes) = next_attributes(&mut parser);
            parser.recycle_attributes(token, attributes);
            let cache = &parser.attribute_recycling.attributes;
            assert!(cache.capacity() <= MAX_RECORDS);
            let mut retained = cache.capacity() * size_of::<Attribute>();
            for attribute in cache {
                for text in [&attribute.name, &attribute.value] {
                    assert!(text.is_empty());
                    assert!(text.capacity() <= MAX_STRING_BYTES);
                    retained += text.capacity();
                }
            }
            assert!(retained <= MAX_RETAINED_BYTES);
            if records > MAX_RECORDS {
                assert_eq!(cache.capacity(), 0);
            } else if value_bytes > MAX_STRING_BYTES {
                assert_eq!(cache[0].value.capacity(), 0);
                assert!(cache[0].name.capacity() > 0);
            }
        }
    }
}
