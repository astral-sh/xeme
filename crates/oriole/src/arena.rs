//! Detached callback storage. Only owned buffers and scalar offsets leave the core.

use std::ops::Range;

use oriole_storage::{AllocError, Allocator, String, Vec};

use crate::{Position, RecyclingToken};

pub(crate) const MAX_ARENA_BYTES: usize = 4 * 1024;
pub(crate) const INLINE_TEXT_BYTES: usize = 23;
pub(crate) const MAX_ARENA_ATTRIBUTES: usize = 128;
pub(crate) const RETAINED_ARENA_BYTES: usize =
    MAX_ARENA_BYTES + MAX_ARENA_ATTRIBUTES * size_of::<ArenaAttribute>();

#[derive(Debug)]
struct ArenaAttribute {
    name: Range<usize>,
    value: Range<usize>,
}

#[derive(Debug)]
enum Payload {
    Start,
    InlineText,
    HeapText,
    End(String),
    NativeText { start: usize },
}

/// Detached callback storage for literal tags or plain character data.
///
/// Start-tag slices include a trailing NUL; text is length-delimited. No
/// parser borrow escapes. The explicit C host may instead receive a scalar
/// original-input Text range. The raw token remains owned by the parser.
#[doc(hidden)]
#[derive(Debug)]
pub struct AdapterFrame {
    pub(crate) generation: RecyclingToken,
    bytes: String,
    attributes: Vec<ArenaAttribute>,
    name: Range<usize>,
    position: Position,
    callback_bytes: usize,
    inline: [u8; INLINE_TEXT_BYTES],
    payload: Payload,
    pub(crate) active: bool,
}

impl AdapterFrame {
    pub(crate) fn finish(self) -> RecyclingToken {
        drop(self.bytes);
        drop(self.attributes);
        self.generation
    }

    pub(crate) fn new(allocator: Allocator, generation: RecyclingToken) -> Self {
        Self {
            generation,
            bytes: String::new_in(allocator),
            attributes: Vec::new_in(allocator),
            name: 0..0,
            position: Position::default(),
            callback_bytes: 0,
            inline: [0; INLINE_TEXT_BYTES],
            payload: Payload::Start,
            active: false,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.active = false;
        self.payload = Payload::Start;
        self.name = 0..0;
        self.callback_bytes = 0;
        self.position = Position::default();
        if self.bytes.capacity() > MAX_ARENA_BYTES {
            self.bytes = String::new_in(self.bytes.allocator());
        } else {
            self.bytes.clear();
        }
        self.attributes.clear();
    }

    pub(crate) fn prepare(&mut self, count: usize) -> Result<(), AllocError> {
        debug_assert!(!self.active && self.attributes.is_empty());
        debug_assert!(count <= MAX_ARENA_ATTRIBUTES);
        // The old owned-attribute vector also reserves before duplicate/value
        // processing. Arena bytes grow later, in that semantic processing order.
        self.attributes.try_reserve_exact(count)?;
        Ok(())
    }

    /// Prepare owned length-delimited bytes without publishing an event.
    pub(crate) fn prepare_text(&mut self, text: &str) -> Result<(), AllocError> {
        debug_assert!(!self.active && self.bytes.is_empty());
        debug_assert!(text.len() <= MAX_ARENA_BYTES);
        if text.len() <= INLINE_TEXT_BYTES {
            self.inline[..text.len()].copy_from_slice(text.as_bytes());
            self.payload = Payload::InlineText;
        } else {
            // Reserve exactly the bounded span. Amortized growth from a prior
            // start tag could otherwise retain more than the 4 KiB byte limit.
            self.bytes.try_reserve_exact(text.len())?;
            self.bytes.try_push_str(text)?;
            self.payload = Payload::HeapText;
        }
        self.callback_bytes = text.len();
        Ok(())
    }

    /// Retain the ordinary Text reservation but let the C host resolve the bytes.
    pub(crate) fn prepare_native_text(
        &mut self,
        start: usize,
        count: usize,
    ) -> Result<(), AllocError> {
        debug_assert!(!self.active && self.bytes.is_empty());
        debug_assert!(count > 0 && count <= MAX_ARENA_BYTES);
        if count > INLINE_TEXT_BYTES {
            self.bytes.try_reserve_exact(count)?;
        }
        self.payload = Payload::NativeText { start };
        self.callback_bytes = count;
        Ok(())
    }

    fn append(&mut self, text: &str) -> Result<Range<usize>, AllocError> {
        debug_assert!(!text.as_bytes().contains(&0));
        let required = text
            .len()
            .checked_add(1)
            .ok_or(AllocError::CapacityOverflow)?;
        let count = self
            .callback_bytes
            .checked_add(text.len())
            .ok_or(AllocError::CapacityOverflow)?;
        self.bytes.try_reserve(required)?;
        let start = self.bytes.len();
        self.bytes.try_push_str(text)?;
        self.bytes.try_push('\0')?;
        self.callback_bytes = count;
        Ok(start..self.bytes.len())
    }

    pub(crate) fn push_attribute(&mut self, name: &str, value: &str) -> Result<(), AllocError> {
        // Preserve the old literal value-before-name allocation order.
        let value = self.append(value)?;
        let name = self.append(name)?;
        debug_assert!(self.attributes.len() < self.attributes.capacity());
        self.attributes.push(ArenaAttribute { name, value });
        Ok(())
    }

    pub(crate) fn set_name(&mut self, name: &str) -> Result<(), AllocError> {
        self.name = self.append(name)?;
        Ok(())
    }

    /// Detach the opening element's name without displacing the reusable arena.
    pub(crate) fn prepare_end(&mut self, name: String) {
        debug_assert!(!self.active && self.attributes.is_empty());
        self.callback_bytes = name.len();
        self.payload = Payload::End(name);
    }

    pub(crate) fn publish(&mut self, position: Position) {
        self.position = position;
        self.active = true;
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    #[must_use]
    pub fn position(&self) -> Position {
        self.position
    }

    /// Identify Text without requiring an owned-byte projection.
    pub(crate) fn is_text(&self) -> bool {
        self.active
            && matches!(
                self.payload,
                Payload::InlineText | Payload::HeapText | Payload::NativeText { .. }
            )
    }

    /// Absolute original-input range for the explicit C-context host protocol.
    /// The host must validate and resolve this range before each callback.
    #[doc(hidden)]
    #[must_use]
    pub fn native_text_range_for_c(&self) -> Option<(usize, usize)> {
        if self.active
            && let Payload::NativeText { start } = self.payload
        {
            Some((start, self.callback_bytes))
        } else {
            None
        }
    }

    /// Character data bytes, valid through the callback and until frame reuse.
    #[must_use]
    pub fn text_bytes(&self) -> Option<&[u8]> {
        if !self.active {
            return None;
        }
        match self.payload {
            Payload::Start | Payload::End(_) => None,
            Payload::InlineText => Some(&self.inline[..self.callback_bytes]),
            Payload::HeapText => Some(self.bytes.as_bytes()),
            Payload::NativeText { .. } => panic!("C-context Text requires the host range resolver"),
        }
    }

    /// Take the original end-name owner for dispatch and subsequent recycling.
    /// No trailing NUL has been added. This consumes the active End delivery.
    #[must_use]
    pub fn take_end_name(&mut self) -> Option<String> {
        if !self.active || !matches!(self.payload, Payload::End(_)) {
            return None;
        }
        let Payload::End(name) = std::mem::replace(&mut self.payload, Payload::Start) else {
            unreachable!("active End payload was checked above")
        };
        self.active = false;
        Some(name)
    }

    /// Name bytes including the final NUL; valid until this owned frame is reused.
    #[must_use]
    pub fn name_bytes(&self) -> &[u8] {
        assert!(
            !matches!(self.payload, Payload::NativeText { .. }),
            "C-context Text has no name bytes"
        );
        &self.bytes.as_bytes()[self.name.clone()]
    }

    /// Attribute name/value bytes including each final NUL, in source order.
    pub fn attributes(&self) -> impl ExactSizeIterator<Item = (&[u8], &[u8])> {
        assert!(
            !matches!(self.payload, Payload::NativeText { .. }),
            "C-context Text has no attributes"
        );
        self.attributes.iter().map(|attribute| {
            (
                &self.bytes.as_bytes()[attribute.name.clone()],
                &self.bytes.as_bytes()[attribute.value.clone()],
            )
        })
    }

    #[must_use]
    pub fn callback_bytes(&self) -> usize {
        self.callback_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, Parser};

    #[test]
    fn detached_end_keeps_the_warmed_start_arena_and_oversized_owner_separate() {
        let mut parser = Parser::new(Config::default());
        let mut frame = parser.adapter_frame();
        parser
            .feed(b"<r><n a='v'></n><n a='v'></n><n a='v'></n></r>", true)
            .unwrap();
        let mut event = None;
        // The first attribute tag warms the lexical record capacity through
        // the existing owned fallback. Compare two subsequent framed starts.
        for _ in 0..4 {
            parser
                .next_event_for_adapter_into(&mut event, &mut frame)
                .unwrap()
                .unwrap();
        }
        assert!(frame.is_active() && event.is_none());
        assert_eq!(frame.name_bytes(), b"n\0");
        let pointer = frame.bytes.as_bytes().as_ptr();
        let capacity = frame.bytes.capacity();
        let attribute_capacity = frame.attributes.capacity();
        let token = parser
            .next_event_for_adapter_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        let name = frame.take_end_name().unwrap();
        assert_eq!(name.as_str(), "n");
        parser.recycle_end_element(token, name);
        parser
            .next_event_for_adapter_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        assert!(frame.is_active() && event.is_none());
        assert_eq!(frame.name_bytes(), b"n\0");
        assert_eq!(frame.bytes.as_bytes().as_ptr(), pointer);
        assert_eq!(frame.bytes.capacity(), capacity);
        assert_eq!(frame.attributes.capacity(), attribute_capacity);
        parser.finish_adapter_frame(frame);

        let name = "n".repeat(MAX_ARENA_BYTES + 1);
        let mut parser = Parser::new(Config::default());
        parser
            .feed(format!("<{name}></{name}>").as_bytes(), true)
            .unwrap();
        parser.next_event().unwrap().unwrap();
        let mut frame = parser.adapter_frame();
        let token = parser
            .next_event_for_adapter_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        assert_eq!(frame.bytes.capacity(), 0);
        let owner = frame.take_end_name().unwrap();
        assert_eq!(owner.as_str(), name);
        parser.recycle_end_element(token, owner);
        assert!(parser.event_recycling.take_name().is_none());
        parser.finish_adapter_frame(frame);
    }

    #[test]
    fn text_storage_drops_start_terminators_and_stays_within_the_byte_cap() {
        let parser = Parser::new(Config::default());
        let mut frame = parser.adapter_frame();
        frame.prepare_text("abc").unwrap();
        frame.publish(Position::default());
        assert_eq!(frame.bytes.capacity(), 0);
        assert_eq!(frame.text_bytes(), Some(b"abc".as_slice()));
        for len in [24, 31, 1000, 3000, 4095, 4096] {
            let value = "x".repeat(len);
            frame.clear();
            frame.prepare_text(&value).unwrap();
            assert!(!frame.is_active());
            frame.publish(Position::default());
            assert_eq!(frame.text_bytes(), Some(value.as_bytes()));
            assert!(frame.bytes.capacity() <= MAX_ARENA_BYTES);
        }
        frame.clear();
        frame.prepare(0).unwrap();
        frame.set_name("name").unwrap();
        frame.publish(Position::default());
        assert_eq!(frame.name_bytes(), b"name\0");
        frame.clear();
        frame.prepare_text("abcdefghijklmnopqrstuvwxyz").unwrap();
        frame.publish(Position::default());
        assert_eq!(
            frame.text_bytes(),
            Some(b"abcdefghijklmnopqrstuvwxyz".as_slice())
        );
        assert!(frame.bytes.capacity() <= MAX_ARENA_BYTES);
    }
}
