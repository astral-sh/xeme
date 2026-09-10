//! Detached callback storage. Only owned buffers and scalar offsets leave the core.

use std::ops::Range;

use oriole_storage::{AllocError, Allocator, String, Vec};

use crate::{Position, RecyclingToken};

pub(crate) const MAX_ARENA_BYTES: usize = 4 * 1024;
pub(crate) const MAX_ARENA_ATTRIBUTES: usize = 128;
pub(crate) const RETAINED_ARENA_BYTES: usize =
    MAX_ARENA_BYTES + MAX_ARENA_ATTRIBUTES * size_of::<ArenaAttribute>();

#[derive(Debug)]
struct ArenaAttribute {
    name: Range<usize>,
    value: Range<usize>,
}

/// Owned callback storage for one native literal start tag.
///
/// Returned byte slices include a trailing NUL. Offsets remain private and no
/// parser borrow escapes. The original raw token remains owned by the parser.
#[doc(hidden)]
#[derive(Debug)]
pub struct AdapterFrame {
    pub(crate) generation: RecyclingToken,
    bytes: String,
    attributes: Vec<ArenaAttribute>,
    name: Range<usize>,
    position: Position,
    callback_bytes: usize,
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
            active: false,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.active = false;
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

    /// Name bytes including the final NUL; valid until this owned frame is reused.
    #[must_use]
    pub fn name_bytes(&self) -> &[u8] {
        &self.bytes.as_bytes()[self.name.clone()]
    }

    /// Attribute name/value bytes including each final NUL, in source order.
    pub fn attributes(&self) -> impl ExactSizeIterator<Item = (&[u8], &[u8])> {
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
