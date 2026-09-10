//! Bounded reuse of event storage explicitly returned by an adapter.

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
pub(crate) struct EventRecycling {
    parser_id: u64,
    attributes: Vec<Attribute>,
    attribute_bytes: usize,
    adapter_bytes: usize,
    names: [String; 2],
}

impl EventRecycling {
    /// Assign a never-reused generation without allocating an identity object.
    pub(crate) fn new(allocator: Allocator) -> Result<Self, AllocError> {
        let parser_id = NEXT_PARSER_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| AllocError::CapacityOverflow)?;
        Ok(Self {
            parser_id,
            attributes: Vec::new_in(allocator),
            attribute_bytes: 0,
            adapter_bytes: 0,
            names: [String::new_in(allocator), String::new_in(allocator)],
        })
    }

    /// Copy only the generation number; no reference or shared-owner clone escapes.
    pub(crate) fn token(&self) -> RecyclingToken {
        RecyclingToken {
            parser_id: self.parser_id,
        }
    }

    pub(crate) fn accepts(&self, token: &RecyclingToken) -> bool {
        token.parser_id == self.parser_id
    }

    /// An adapter's detached arena shares the existing aggregate cache budget.
    /// Reserve only when its first eligible event is prepared, without allocating.
    pub(crate) fn reserve_adapter(&mut self, token: &RecyclingToken, bytes: usize) {
        assert!(self.accepts(token));
        assert!(bytes <= MAX_RETAINED_BYTES - 2 * MAX_STRING_BYTES);
        self.adapter_bytes = bytes;
        if self.attribute_bytes + self.names[0].capacity() + self.names[1].capacity()
            > MAX_RETAINED_BYTES - bytes
        {
            self.attributes = Vec::new_in(*self.attributes.allocator());
            self.attribute_bytes = 0;
        }
    }

    pub(crate) fn release_adapter(&mut self, token: &RecyclingToken) {
        if self.accepts(token) {
            self.adapter_bytes = 0;
        }
    }

    /// Detach reusable owners before parsing; errors then drop them normally.
    pub(crate) fn take(&mut self) -> Vec<Attribute> {
        let allocator = *self.attributes.allocator();
        self.attribute_bytes = 0;
        std::mem::replace(&mut self.attributes, Vec::new_in(allocator))
    }

    /// Detach one original name owner without retaining a borrow into the cache.
    pub(crate) fn take_name(&mut self) -> Option<String> {
        let index = usize::from(self.names[0].capacity() == 0);
        if self.names[index].capacity() == 0 {
            return None;
        }
        let allocator = self.names[index].allocator();
        Some(std::mem::replace(
            &mut self.names[index],
            String::new_in(allocator),
        ))
    }

    /// Keep bounded original capacities, dropping oversized owners without growth.
    pub(crate) fn recycle(&mut self, token: RecyclingToken, attributes: Vec<Attribute>) {
        if token.parser_id != self.parser_id {
            return;
        }
        self.retain_attributes(attributes);
    }

    /// Return both start-event owners under the same generation token.
    pub(crate) fn recycle_start(
        &mut self,
        token: RecyclingToken,
        name: String,
        attributes: Vec<Attribute>,
    ) {
        if token.parser_id == self.parser_id {
            self.retain_name(name);
            self.retain_attributes(attributes);
        }
    }

    /// Return an end-event name after its callback has finished.
    pub(crate) fn recycle_end(&mut self, token: RecyclingToken, name: String) {
        if token.parser_id == self.parser_id {
            self.retain_name(name);
        }
    }

    fn retain_name(&mut self, mut name: String) {
        let index = usize::from(self.names[1].capacity() < self.names[0].capacity());
        let capacity = name.capacity();
        if capacity <= self.names[index].capacity() || capacity > MAX_STRING_BYTES {
            return;
        }
        let remaining = MAX_RETAINED_BYTES
            - self.adapter_bytes
            - self.attribute_bytes
            - self.names[1 - index].capacity();
        if capacity <= remaining {
            name.clear();
            self.names[index] = name;
        }
    }

    fn retain_attributes(&mut self, mut attributes: Vec<Attribute>) {
        if attributes.capacity() > MAX_RECORDS {
            return;
        }
        if attributes.capacity() == 0 {
            return;
        }
        let allocator = *self.attributes.allocator();
        let budget = MAX_RETAINED_BYTES
            - self.adapter_bytes
            - self.names[0].capacity()
            - self.names[1].capacity();
        let Some(mut remaining) = attributes
            .capacity()
            .checked_mul(size_of::<Attribute>())
            .and_then(|bytes| budget.checked_sub(bytes))
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
        self.attribute_bytes = budget - remaining;
        self.attributes = attributes;
    }
}

/// Preserve ordinary allocation behavior unless an adapter returned a name owner.
pub(crate) fn copy_name(
    text: &str,
    reusable: Option<String>,
    allocator: Allocator,
) -> Result<String, AllocError> {
    if let Some(mut output) = reusable {
        copy_attribute_string(&mut output, text)?;
        Ok(output)
    } else {
        String::try_from_str_in(text, allocator)
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

    #[test]
    fn arena_and_owned_fallbacks_share_the_retained_capacity_budget() {
        fn check(parser: &Parser) {
            let cache = &parser.event_recycling;
            let attributes = cache.attributes.capacity() * size_of::<Attribute>()
                + cache
                    .attributes
                    .iter()
                    .map(|a| a.name.capacity() + a.value.capacity())
                    .sum::<usize>();
            assert_eq!(attributes, cache.attribute_bytes);
            assert!(
                attributes
                    + cache.adapter_bytes
                    + cache.names.iter().map(String::capacity).sum::<usize>()
                    <= MAX_RETAINED_BYTES
            );
        }
        let mut xml = std::string::String::from("<r><warm");
        for index in 0..64 {
            use std::fmt::Write;
            write!(&mut xml, " a{index}='{}'", "x".repeat(1024)).unwrap();
        }
        xml.push_str("/>abcdefghijklmnopqrstuvwxyz0123456789<n a='v'/>x<fallback a='&amp;'/>abcdefghijklmnopqrstuvwxyz0123456789<n a='again'/></r>");
        let mut parser = Parser::new(Config::default());
        parser.feed(xml.as_bytes(), true).unwrap();
        parser.next_event().unwrap().unwrap();
        let (event, token) = parser.next_event_for_recycling().unwrap().unwrap();
        let EventKind::StartElement { name, attributes } = event.kind else {
            panic!("warm start")
        };
        parser.recycle_start_element(token, name, attributes);
        check(&parser);
        assert!(
            parser.event_recycling.attribute_bytes
                > MAX_RETAINED_BYTES - crate::arena::RETAINED_ARENA_BYTES
        );
        assert_eq!(parser.event_recycling.adapter_bytes, 0);
        let mut frame = parser.adapter_frame();
        let mut frames = 0;
        let mut texts = 0;
        let mut fallbacks = 0;
        loop {
            let mut event = None;
            let Some(token) = parser
                .next_event_for_adapter_into(&mut event, &mut frame)
                .unwrap()
            else {
                break;
            };
            if frame.is_active() {
                if frame.text_bytes().is_some() {
                    texts += 1;
                } else {
                    frames += 1;
                }
                assert_eq!(
                    parser.event_recycling.adapter_bytes,
                    crate::arena::RETAINED_ARENA_BYTES
                );
            } else {
                match event.unwrap().kind {
                    EventKind::StartElement { name, attributes } => {
                        fallbacks += 1;
                        parser.recycle_start_element(token, name, attributes);
                    }
                    EventKind::EndElement { name } => parser.recycle_end_element(token, name),
                    _ => {}
                }
            }
            check(&parser);
        }
        assert_eq!((frames, fallbacks, texts), (2, 1, 3));
        parser.finish_adapter_frame(frame);
        assert_eq!(parser.event_recycling.adapter_bytes, 0);
        check(&parser);
    }

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
        assert!(second.event_recycling.attributes.is_empty());
        second.feed(b"<r a='value'/>", true).unwrap();
        let (token, attributes) = next_attributes(&mut second);
        // C Reset similarly replaces the core while retaining the C handle address.
        second = Parser::new(Config::default());
        second.recycle_attributes(token, attributes);
        assert!(second.event_recycling.attributes.is_empty());
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
            let cache = &parser.event_recycling.attributes;
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

    #[test]
    fn returned_names_reuse_both_owners_and_retained_events_stay_owned() {
        let mut parser = Parser::new(Config::default());
        parser
            .feed(b"<root><first_long/><second/></root>", true)
            .unwrap();
        let (root, _) = parser.next_event_for_recycling().unwrap().unwrap();
        let (event, token) = parser.next_event_for_recycling().unwrap().unwrap();
        let EventKind::StartElement { name, attributes } = event.kind else {
            panic!("expected first start");
        };
        let start_owner = name.as_ptr();
        parser.recycle_start_element(token, name, attributes);
        let (event, token) = parser.next_event_for_recycling().unwrap().unwrap();
        let EventKind::EndElement { name } = event.kind else {
            panic!("expected first end");
        };
        let end_owner = name.as_ptr();
        assert_ne!(start_owner, end_owner);
        parser.recycle_end_element(token, name);
        let (second, _) = parser.next_event_for_recycling().unwrap().unwrap();
        let EventKind::StartElement { name, .. } = &second.kind else {
            panic!("expected second start");
        };
        assert!([start_owner, end_owner].contains(&name.as_ptr()));
        let (event, token) = parser.next_event_for_recycling().unwrap().unwrap();
        let EventKind::EndElement { name } = event.kind else {
            panic!("expected second end");
        };
        assert!([start_owner, end_owner].contains(&name.as_ptr()));
        parser.recycle_end_element(token, name);
        while parser.next_event().unwrap().is_some() {}
        drop(parser);
        assert!(matches!(root.kind, EventKind::StartElement { name, .. } if name == "root"));
        assert!(matches!(second.kind, EventKind::StartElement { name, .. } if name == "second"));
    }

    #[test]
    fn foreign_and_reset_generations_cannot_return_names() {
        for (reset, end) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut first = Parser::new(Config::default());
            first.feed(b"<first a='value'/>", true).unwrap();
            if end {
                first.next_event().unwrap();
            }
            let mut output = None;
            let token = first
                .next_event_for_recycling_into(&mut output)
                .unwrap()
                .unwrap();
            let event = output.unwrap();
            let mut second = if reset {
                first = Parser::new(Config::default());
                first
            } else {
                Parser::new(Config::default())
            };
            match event.kind {
                EventKind::StartElement { name, attributes } => {
                    second.recycle_start_element(token, name, attributes);
                }
                EventKind::EndElement { name } => second.recycle_end_element(token, name),
                _ => panic!("expected an element event"),
            }
            assert!(second.event_recycling.attributes.is_empty());
            assert!(
                second
                    .event_recycling
                    .names
                    .iter()
                    .all(|name| name.capacity() == 0)
            );
        }
    }

    #[test]
    fn name_capacities_share_the_attribute_retention_budget() {
        for name_len in [MAX_STRING_BYTES - 1, MAX_STRING_BYTES] {
            let mut xml = format!("<{}", "n".repeat(name_len));
            for index in 0..64 {
                use std::fmt::Write;
                write!(&mut xml, " a{index}='{}'", "x".repeat(1024)).unwrap();
            }
            xml.push_str("/>");
            let mut parser = Parser::new(Config::default());
            parser.feed(xml.as_bytes(), true).unwrap();
            let (event, token) = parser.next_event_for_recycling().unwrap().unwrap();
            let EventKind::StartElement { name, attributes } = event.kind else {
                panic!("expected start");
            };
            parser.recycle_start_element(token, name, attributes);
            let (event, token) = parser.next_event_for_recycling().unwrap().unwrap();
            let EventKind::EndElement { name } = event.kind else {
                panic!("expected end");
            };
            parser.recycle_end_element(token, name);
            let cache = &parser.event_recycling;
            let attribute_bytes = cache.attributes.capacity() * size_of::<Attribute>()
                + cache
                    .attributes
                    .iter()
                    .map(|attribute| attribute.name.capacity() + attribute.value.capacity())
                    .sum::<usize>();
            assert_eq!(cache.attribute_bytes, attribute_bytes);
            assert!(
                cache
                    .names
                    .iter()
                    .all(|name| name.capacity() <= MAX_STRING_BYTES)
            );
            assert!(
                attribute_bytes + cache.names.iter().map(String::capacity).sum::<usize>()
                    <= MAX_RETAINED_BYTES
            );
            if name_len == MAX_STRING_BYTES {
                assert!(cache.names.iter().all(|name| name.capacity() == 0));
            }
        }
    }
}
