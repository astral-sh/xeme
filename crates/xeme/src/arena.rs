//! Detached callback storage. Only owned buffers and scalar offsets leave the core.

use std::ops::Range;

use xeme_storage::{AllocError, Allocator, String, Vec, try_extend_from_slice};

use crate::{Position, RawAttribute, RecyclingToken};

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
/// Start-tag slices include a trailing NUL; text is length-delimited. The frame
/// does not borrow the parser. A C adapter that retains the original input can
/// receive text as byte offsets into that input. The parser retains the raw token.
#[doc(hidden)]
#[derive(Debug)]
pub struct AdapterFrame {
    pub(crate) generation: RecyclingToken,
    bytes: Vec<u8>,
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
            bytes: Vec::new_in(allocator),
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
            self.bytes = Vec::new_in(*self.bytes.allocator());
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
            try_extend_from_slice(&mut self.bytes, text.as_bytes())?;
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
        if required > self.bytes.capacity() - self.bytes.len()
            && self.bytes.capacity() > MAX_ARENA_BYTES / 2
        {
            // Text can leave a non-power-of-two capacity. If doubling it would
            // exceed the arena budget, reserve the full bound once instead.
            self.bytes
                .try_reserve_exact(MAX_ARENA_BYTES - self.bytes.len())?;
        } else {
            self.bytes.try_reserve(required)?;
        }
        let start = self.bytes.len();
        try_extend_from_slice(&mut self.bytes, text.as_bytes())?;
        try_extend_from_slice(&mut self.bytes, b"\0")?;
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

    /// Use a literal span only when its bytes and the later name already fit.
    /// Cold paths retain value-before-name growth and its allocation error order.
    pub(crate) fn fits_literal_attributes(&self, rest: &str, name: &str) -> bool {
        debug_assert!(!self.active && self.bytes.is_empty() && self.attributes.is_empty());
        rest.len()
            .checked_add(name.len())
            .and_then(|bytes| bytes.checked_add(1))
            .is_some_and(|bytes| bytes <= MAX_ARENA_BYTES && bytes <= self.bytes.capacity())
    }

    /// Copy validated literal fields together, patching only ASCII delimiters.
    /// The caller has checked duplicates, reserved records, and proved byte capacity.
    pub(crate) fn push_literal_attributes(
        &mut self,
        rest: &str,
        attributes: &[RawAttribute],
    ) -> Result<(), AllocError> {
        debug_assert!(!self.active && self.bytes.is_empty() && self.attributes.is_empty());
        debug_assert!(rest.len() <= self.bytes.capacity() && rest.len() <= MAX_ARENA_BYTES);
        debug_assert!(attributes.len() <= self.attributes.capacity());
        // The capacity check inside this existing bulk helper cannot grow here.
        try_extend_from_slice(&mut self.bytes, rest.as_bytes())?;
        for attribute in attributes {
            debug_assert!(rest.as_bytes()[attribute.name_end].is_ascii());
            debug_assert!(matches!(rest.as_bytes()[attribute.value_end], b'\'' | b'"'));
            self.bytes[attribute.name_end] = 0;
            self.bytes[attribute.value_end] = 0;
            self.attributes.push(ArenaAttribute {
                name: attribute.name_start..attribute.name_end + 1,
                value: attribute.value_start..attribute.value_end + 1,
            });
            self.callback_bytes += attribute.name_end - attribute.name_start + attribute.value_end
                - attribute.value_start;
        }
        debug_assert!(self.callback_bytes <= rest.len());
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

    /// Whether the frame contains text, including a range in the original input.
    pub(crate) fn is_text(&self) -> bool {
        self.active
            && matches!(
                self.payload,
                Payload::InlineText | Payload::HeapText | Payload::NativeText { .. }
            )
    }

    /// Absolute byte offset and length of text in the original input.
    /// The C adapter must validate this range against its input before each callback.
    #[doc(hidden)]
    #[must_use]
    pub fn native_text_range_for_c(&self) -> Option<(usize, usize)> {
        if self.active {
            self.prepared_native_text_range()
        } else {
            None
        }
    }

    /// Return the text range so the parser can retain the raw token before delivery.
    pub(crate) fn prepared_native_text_range(&self) -> Option<(usize, usize)> {
        if let Payload::NativeText { start } = self.payload {
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
            Payload::HeapText => Some(self.bytes.as_slice()),
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
        &self.bytes.as_slice()[self.name.clone()]
    }

    /// Attribute name/value bytes including each final NUL, in source order.
    pub fn attributes(&self) -> impl ExactSizeIterator<Item = (&[u8], &[u8])> {
        assert!(
            !matches!(self.payload, Payload::NativeText { .. }),
            "C-context Text has no attributes"
        );
        self.attributes.iter().map(|attribute| {
            (
                &self.bytes.as_slice()[attribute.name.clone()],
                &self.bytes.as_slice()[attribute.value.clone()],
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
    fn warmed_literal_spans_keep_byte_ranges_and_capacity() {
        let mut parser = Parser::new(Config::default());
        let mut frame = parser.adapter_frame();
        let rest = "\tπ = '😀>\"'\r\n empty=\"\" tail = \"λ ' >\"\t";
        let name = "élément";
        let mut attributes = Vec::new_in(Allocator::System);
        crate::parse_raw_attributes(
            rest,
            false,
            &mut attributes,
            MAX_ARENA_ATTRIBUTES,
            crate::NameRules::FifthEdition,
        )
        .unwrap();
        assert!(!frame.fits_literal_attributes(rest, name));
        let required = rest.len() + name.len() + 1;
        frame.prepare_text(&"x".repeat(required - 1)).unwrap();
        frame.clear();
        assert!(!frame.fits_literal_attributes(rest, name));
        frame.prepare_text(&"x".repeat(required)).unwrap();
        frame.clear();
        frame.prepare(attributes.len()).unwrap();
        assert!(frame.fits_literal_attributes(rest, name));
        assert!(!frame.fits_literal_attributes(&"x".repeat(MAX_ARENA_BYTES), name));
        let pointer = frame.bytes.as_ptr();
        let capacity = frame.bytes.capacity();
        let records = frame.attributes.as_ptr();
        frame.push_literal_attributes(rest, &attributes).unwrap();
        frame.set_name(name).unwrap();
        frame.publish(Position::default());
        assert_eq!(frame.bytes.as_ptr(), pointer);
        assert_eq!(frame.bytes.capacity(), capacity);
        assert_eq!(frame.attributes.as_ptr(), records);
        assert_eq!(frame.bytes.len(), required);
        assert_eq!(frame.name_bytes(), "élément\0".as_bytes());
        let expected = [
            ("π\0".as_bytes(), "😀>\"\0".as_bytes()),
            (b"empty\0".as_slice(), b"\0".as_slice()),
            (b"tail\0".as_slice(), "λ ' >\0".as_bytes()),
        ];
        assert!(frame.attributes().eq(expected));
        assert_eq!(
            frame.callback_bytes(),
            name.len()
                + expected
                    .iter()
                    .map(|(n, v)| n.len() + v.len() - 2)
                    .sum::<usize>()
        );
        let mut copied = rest.as_bytes().to_vec();
        for attribute in &attributes {
            copied[attribute.name_end] = 0;
            copied[attribute.value_end] = 0;
        }
        assert_eq!(&frame.bytes[..rest.len()], copied.as_slice());
        parser.finish_adapter_frame(frame);
    }

    #[test]
    fn warmed_parser_start_uses_the_literal_span() {
        let warm = "<n a='abcdefghijklmnopqrstuvwxyz' b='abcdefghijklmnopqrstuvwxyz' c='abcdefghijklmnopqrstuvwxyz'/>";
        let rest = " a='v' empty='' π='λ' ";
        let target = format!("<n{rest}/>");
        let input = format!("<r>{warm}{warm}{target}</r>");
        let mut parser = Parser::new(Config::default());
        let mut frame = parser.adapter_frame();
        parser.feed(input.as_bytes(), true).unwrap();
        let mut warmed = None;
        let mut observed = false;
        loop {
            let mut event = None;
            let Some(token) = parser
                .next_event_for_adapter_into(&mut event, &mut frame)
                .unwrap()
            else {
                break;
            };
            if let Some(name) = frame.take_end_name() {
                parser.recycle_end_element(token, name);
            } else if frame.is_active() {
                if parser.current_raw() == Some(warm) {
                    warmed = Some((frame.bytes.as_ptr(), frame.bytes.capacity()));
                } else if parser.current_raw() == Some(target.as_str()) {
                    assert_eq!(
                        (frame.bytes.as_ptr(), frame.bytes.capacity()),
                        warmed.unwrap()
                    );
                    // Packed per-field appends cannot produce this source-span
                    // length and element-name offset, even with identical events.
                    assert_eq!(frame.bytes.len(), rest.len() + 2);
                    assert_eq!(frame.name, rest.len()..rest.len() + 2);
                    assert_eq!(frame.bytes[0], b' ');
                    assert_eq!(frame.name_bytes(), b"n\0");
                    assert!(frame.attributes().eq([
                        (b"a\0".as_slice(), b"v\0".as_slice()),
                        (b"empty\0".as_slice(), b"\0".as_slice()),
                        ("π\0".as_bytes(), "λ\0".as_bytes()),
                    ]));
                    observed = true;
                }
            } else if let Some(event) = event {
                match event.kind {
                    crate::EventKind::StartElement { name, attributes } => {
                        parser.recycle_start_element(token, name, attributes)
                    }
                    crate::EventKind::EndElement { name } => {
                        parser.recycle_end_element(token, name)
                    }
                    _ => {}
                }
            }
        }
        assert!(observed);
        parser.finish_adapter_frame(frame);
    }

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
        let pointer = frame.bytes.as_slice().as_ptr();
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
        assert_eq!(frame.bytes.as_slice().as_ptr(), pointer);
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

    #[test]
    fn direct_start_keeps_the_warmed_text_arena_owner() {
        let mut parser = Parser::new(Config::default());
        parser.enable_input_context();
        parser.feed(b"<r><w a='v' b='v'/>", false).unwrap();
        while parser.next_event().unwrap().is_some() {}
        let text = "x".repeat(3000);
        parser.feed(text.as_bytes(), false).unwrap();
        let mut frame = parser.adapter_frame();
        let mut event = None;
        parser
            .next_event_for_c_text_context_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        assert!(frame.native_text_range_for_c().is_some());
        let owner = frame.bytes.as_ptr();
        let capacity = frame.bytes.capacity();
        assert_eq!(capacity, 3000);
        let tag = "<n a='value' b='more'>";
        parser.feed(tag.as_bytes(), false).unwrap();
        parser
            .next_event_for_c_text_context_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        assert!(event.is_none() && frame.is_active());
        assert!(parser.native_raw.is_some());
        assert_eq!(parser.current_raw(), Some(tag));
        assert_eq!(frame.bytes.as_ptr(), owner);
        assert_eq!(frame.bytes.capacity(), capacity);
        assert_eq!(frame.name_bytes(), b"n\0");
        assert_eq!(
            frame.attributes().collect::<std::vec::Vec<_>>(),
            [
                (b"a\0".as_slice(), b"value\0".as_slice()),
                (b"b\0".as_slice(), b"more\0".as_slice()),
            ]
        );
        let retained =
            frame.bytes.capacity() + frame.attributes.capacity() * size_of::<ArenaAttribute>();
        assert!(retained <= RETAINED_ARENA_BYTES);
        parser.feed(text.as_bytes(), false).unwrap();
        parser
            .next_event_for_c_text_context_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        assert!(frame.native_text_range_for_c().is_some());
        assert_eq!(frame.bytes.as_ptr(), owner);
        assert_eq!(frame.bytes.capacity(), capacity);
        parser.finish_adapter_frame(frame);
    }

    #[test]
    fn start_after_text_stays_within_the_reserved_frame_capacity() {
        let warm_attributes = (0..MAX_ARENA_ATTRIBUTES)
            .map(|index| format!(" a{index}=''"))
            .collect::<std::string::String>();
        let attributes = (0..MAX_ARENA_ATTRIBUTES)
            .map(|index| format!(" a{index}='{}'", "v".repeat(22)))
            .collect::<std::string::String>();
        let tag = format!("<n{attributes}/>");
        assert!(tag.len() <= MAX_ARENA_BYTES);
        let text = "x".repeat(3000);
        let xml = format!("<r><w{warm_attributes}/><w{warm_attributes}/>{text}{tag}</r>");
        let mut parser = Parser::new(Config::default());
        parser.feed(xml.as_bytes(), true).unwrap();
        let mut frame = parser.adapter_frame();
        let mut event = None;
        // The first attribute tag warms lexical storage; the second warms the
        // detached frame's full attribute capacity.
        for _ in 0..4 {
            parser
                .next_event_for_adapter_into(&mut event, &mut frame)
                .unwrap()
                .unwrap();
        }
        assert!(frame.is_active() && event.is_none());
        assert_eq!(frame.attributes.len(), MAX_ARENA_ATTRIBUTES);
        assert_eq!(frame.attributes.capacity(), MAX_ARENA_ATTRIBUTES);
        // Deliver the empty End, then Text with an exact 3000-byte reservation.
        for _ in 0..2 {
            parser
                .next_event_for_adapter_into(&mut event, &mut frame)
                .unwrap()
                .unwrap();
        }
        assert_eq!(frame.text_bytes(), Some(text.as_bytes()));
        assert_eq!(frame.bytes.capacity(), 3000);
        parser
            .next_event_for_adapter_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        assert!(frame.is_active() && event.is_none());
        assert_eq!(frame.name_bytes(), b"n\0");
        assert_eq!(frame.bytes.len(), 3476);
        assert_eq!(frame.attributes.len(), MAX_ARENA_ATTRIBUTES);
        let retained =
            frame.bytes.capacity() + frame.attributes.capacity() * size_of::<ArenaAttribute>();
        assert!(
            retained <= RETAINED_ARENA_BYTES,
            "active frame retains {retained} bytes against its {RETAINED_ARENA_BYTES}-byte reservation"
        );
        parser.finish_adapter_frame(frame);
    }
}
