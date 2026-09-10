//! Converted character data and raw ASCII syntax are distinct in custom encodings.
//!
//! A multibyte sequence that converts to `<` is data, while an actual ASCII `<`
//! starts markup. The scanner view uses ordinary non-ASCII characters with the
//! converted character's XML name class. Sparse metadata restores the converted
//! scalar at semantic boundaries and retains its original encoded spelling.
//! Offsets identify replacements, so genuine occurrences of the representative
//! Unicode characters are preserved without escaping or sentinel collisions.

use std::ops::Deref;

use oriole_storage::{AllocError, Allocator, String, Vec};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Conversion {
    offset: usize,
    scalar: char,
    bytes: [u8; 4],
    length: u8,
    public_valid: bool,
}

impl Conversion {
    fn lexical(self) -> char {
        if self.length == 1 {
            self.scalar
        } else {
            representative(self.scalar)
        }
    }
}

pub(crate) fn representative(scalar: char) -> char {
    if !scalar.is_ascii() {
        scalar
    } else if crate::names::is_name_start(scalar) {
        '\u{c0}'
    } else if crate::names::is_name_char(scalar) {
        '\u{b7}'
    } else {
        '\u{e000}'
    }
}

/// A converted sequence's position and original spelling within an element name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NameEncoding {
    offset: usize,
    bytes: [u8; 4],
    length: u8,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Slice<'a> {
    text: &'a str,
    conversions: &'a [Conversion],
    offset: usize,
}

impl<'a> Slice<'a> {
    pub(crate) fn as_str(self) -> &'a str {
        self.text
    }

    pub(crate) fn plain(text: &'a str) -> Self {
        Self {
            text,
            conversions: &[],
            offset: 0,
        }
    }

    pub(crate) fn has_ascii_aliases(self) -> bool {
        self.conversions.iter().any(|entry| entry.scalar.is_ascii())
    }

    /// Public identifiers inspect the encoded bytes of custom characters.
    pub(crate) fn invalid_public_character(self) -> Option<usize> {
        fn valid(scalar: char) -> bool {
            scalar.is_ascii_alphanumeric() || " \r\n-'()+,./:=?;!*#@$_%".contains(scalar)
        }
        let mut conversions = self.conversions.iter().peekable();
        for (offset, scalar) in self.text.char_indices() {
            let valid = if conversions
                .peek()
                .is_some_and(|entry| entry.offset == self.offset + offset)
            {
                let entry = conversions.next().expect("converted public character");
                entry.public_valid
            } else {
                valid(scalar)
            };
            if !valid {
                return Some(offset);
            }
        }
        None
    }

    /// Semantic characters paired with whether they came from raw ASCII bytes.
    pub(crate) fn decoded_chars(self) -> impl Iterator<Item = (char, bool)> + 'a {
        let mut conversions = self.conversions.iter().peekable();
        self.text.char_indices().map(move |(offset, scalar)| {
            if conversions
                .peek()
                .is_some_and(|entry| entry.offset == self.offset + offset)
            {
                (
                    conversions.next().expect("converted character").scalar,
                    false,
                )
            } else {
                (scalar, scalar.is_ascii())
            }
        })
    }

    /// Preserve provenance when a string parser returns a borrowed subslice.
    #[inline]
    pub(crate) fn for_slice(self, text: &'a str) -> Self {
        if text.is_empty() || self.conversions.is_empty() {
            Self::plain(text)
        } else {
            self.converted_subslice(text)
        }
    }

    /// Rebase sparse conversions only when the source has encoded characters.
    fn converted_subslice(self, text: &'a str) -> Self {
        let start = (text.as_ptr() as usize)
            .checked_sub(self.text.as_ptr() as usize)
            .expect("lexical subslice starts inside its parent");
        let end = start
            .checked_add(text.len())
            .expect("lexical subslice length");
        assert!(
            end <= self.text.len(),
            "lexical subslice ends inside its parent"
        );
        let offset = self.offset + start;
        let first = self
            .conversions
            .partition_point(|entry| entry.offset < offset);
        let last = self
            .conversions
            .partition_point(|entry| entry.offset < offset + text.len());
        Self {
            text,
            conversions: &self.conversions[first..last],
            offset,
        }
    }

    pub(crate) fn decode(self, allocator: Allocator) -> Result<String, AllocError> {
        if !self.has_ascii_aliases() {
            return String::try_from_str_in(self.text, allocator);
        }
        let mut output = String::try_with_capacity_in(self.text.len(), allocator)?;
        let mut cursor = 0;
        for conversion in self.conversions {
            let start = conversion.offset - self.offset;
            output.try_push_str(&self.text[cursor..start])?;
            output.try_push(conversion.scalar)?;
            cursor = start + conversion.lexical().len_utf8();
        }
        output.try_push_str(&self.text[cursor..])?;
        Ok(output)
    }

    /// Decode physical line endings without normalizing converted ASCII aliases.
    pub(crate) fn normalized(self, allocator: Allocator) -> Result<String, crate::Error> {
        if !self.has_ascii_aliases() {
            return crate::normalize_newlines(self.text, allocator);
        }
        let mut output = String::try_with_capacity_in(self.text.len(), allocator)?;
        let mut previous_cr = false;
        for (scalar, raw_ascii) in self.decoded_chars() {
            if raw_ascii && scalar == '\n' && previous_cr {
                previous_cr = false;
                continue;
            }
            previous_cr = raw_ascii && scalar == '\r';
            output.try_push(if previous_cr { '\n' } else { scalar })?;
        }
        Ok(output)
    }

    pub(crate) fn decoded(self, allocator: Allocator) -> Result<Decoded<'a>, AllocError> {
        if self.has_ascii_aliases() {
            self.decode(allocator).map(Decoded::Owned)
        } else {
            Ok(Decoded::Borrowed(self.text))
        }
    }

    pub(crate) fn name_encoding(
        self,
        allocator: Allocator,
    ) -> Result<Vec<NameEncoding>, AllocError> {
        let mut output = Vec::new_in(allocator);
        output.try_reserve_exact(self.conversions.len())?;
        output.extend(self.conversions.iter().map(|entry| NameEncoding {
            offset: entry.offset - self.offset,
            bytes: entry.bytes,
            length: entry.length,
        }));
        Ok(output)
    }

    pub(crate) fn same_name_encoding(self, expected: &[NameEncoding]) -> bool {
        self.conversions
            .iter()
            .map(|entry| NameEncoding {
                offset: entry.offset - self.offset,
                bytes: entry.bytes,
                length: entry.length,
            })
            .eq(expected.iter().copied())
    }

    pub(crate) fn to_owned(self, allocator: Allocator) -> Result<Buffer, AllocError> {
        let mut conversions = Vec::new_in(allocator);
        conversions.try_reserve_exact(self.conversions.len())?;
        conversions.extend(self.conversions.iter().map(|entry| Conversion {
            offset: entry.offset - self.offset,
            ..*entry
        }));
        Ok(Buffer {
            text: String::try_from_str_in(self.text, allocator)?,
            conversions,
        })
    }
}

impl Deref for Slice<'_> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.text
    }
}

#[derive(Debug)]
pub(crate) enum Decoded<'a> {
    Borrowed(&'a str),
    Owned(String),
}

impl Deref for Decoded<'_> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(text) => text,
            Self::Owned(text) => text,
        }
    }
}

impl Decoded<'_> {
    pub(crate) fn into_owned(self, allocator: Allocator) -> Result<String, AllocError> {
        match self {
            Self::Borrowed(text) => String::try_from_str_in(text, allocator),
            Self::Owned(text) => Ok(text),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Buffer {
    text: String,
    conversions: Vec<Conversion>,
}

impl Buffer {
    pub(crate) fn new_in(allocator: Allocator) -> Self {
        Self {
            text: String::new_in(allocator),
            conversions: Vec::new_in(allocator),
        }
    }

    pub(crate) fn plain(text: String) -> Self {
        let allocator = text.allocator();
        Self {
            text,
            conversions: Vec::new_in(allocator),
        }
    }

    #[inline]
    pub(crate) fn has_conversions(&self) -> bool {
        !self.conversions.is_empty()
    }

    pub(crate) fn view(&self) -> Slice<'_> {
        Slice {
            text: &self.text,
            conversions: &self.conversions,
            offset: 0,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    pub(crate) fn push_str(&mut self, text: &str) -> Result<(), AllocError> {
        self.text.try_push_str(text)
    }

    pub(crate) fn try_push_str(&mut self, text: &str) -> Result<(), AllocError> {
        self.push_str(text)
    }

    pub(crate) fn push(&mut self, scalar: char) -> Result<(), AllocError> {
        self.text.try_push(scalar)
    }

    pub(crate) fn try_push(&mut self, scalar: char) -> Result<(), AllocError> {
        self.push(scalar)
    }

    pub(crate) fn append(&mut self, value: Slice<'_>) -> Result<(), AllocError> {
        if value.conversions.is_empty() {
            return self.text.try_push_str(value.text);
        }
        self.text.try_reserve(value.len())?;
        self.conversions.try_reserve(value.conversions.len())?;
        let base = self.text.len();
        self.text.try_push_str(&value)?;
        self.conversions
            .extend(value.conversions.iter().map(|entry| Conversion {
                offset: base + entry.offset - value.offset,
                ..*entry
            }));
        Ok(())
    }

    pub(crate) fn push_conversion(
        &mut self,
        scalar: char,
        bytes: [u8; 4],
        length: u8,
        public_valid: bool,
    ) -> Result<char, AllocError> {
        let conversion = Conversion {
            offset: self.text.len(),
            scalar,
            bytes,
            length,
            public_valid,
        };
        let lexical = conversion.lexical();
        self.text.try_reserve(lexical.len_utf8())?;
        self.conversions.try_reserve(1)?;
        self.text.try_push(lexical)?;
        self.conversions.push(conversion);
        Ok(lexical)
    }

    /// Publish decoded text after all lexical projections and name checks finish.
    /// Plain text exchanges owners without copying; aliases decode before mutation.
    pub(crate) fn swap_decoded(&mut self, output: &mut String) -> Result<(), AllocError> {
        if self.view().has_ascii_aliases() {
            let decoded = self.view().decode(output.allocator())?;
            self.text = std::mem::replace(output, decoded);
        } else {
            std::mem::swap(&mut self.text, output);
        }
        self.conversions.clear();
        Ok(())
    }

    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.conversions.clear();
    }

    pub(crate) fn truncate(&mut self, length: usize) {
        self.text.truncate(length);
        let count = self
            .conversions
            .partition_point(|entry| entry.offset < length);
        self.conversions.truncate(count);
    }

    pub(crate) fn discard_prefix(&mut self, length: usize) {
        self.text.drain(..length);
        let count = self
            .conversions
            .partition_point(|entry| entry.offset < length);
        self.conversions.drain(..count);
        for entry in &mut self.conversions {
            entry.offset -= length;
        }
    }
}

impl Deref for Buffer {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_metadata_does_not_alias_genuine_representatives() {
        let mut buffer = Buffer::new_in(Allocator::System);
        buffer.push_str("À·\u{e000}").unwrap();
        buffer
            .push_conversion('A', [0x80, b'A', 0, 0], 2, false)
            .unwrap();
        buffer
            .push_conversion('-', [0x80, b'-', 0, 0], 2, false)
            .unwrap();
        buffer
            .push_conversion('<', [0x80, b'<', 0, 0], 2, false)
            .unwrap();
        assert_eq!(
            buffer.view().decode(Allocator::System).unwrap().as_str(),
            "À·\u{e000}A-<"
        );
        let suffix = buffer.view().for_slice(&buffer[7..]);
        let mut copy = Buffer::plain(String::try_from_str_in("prefix", Allocator::System).unwrap());
        copy.append(suffix).unwrap();
        copy.discard_prefix(6);
        assert_eq!(
            copy.view().decode(Allocator::System).unwrap().as_str(),
            "A-<"
        );
        copy.truncate(4);
        assert_eq!(
            copy.view().decode(Allocator::System).unwrap().as_str(),
            "A-"
        );
        copy.clear();
        copy.push_str("À").unwrap();
        assert_eq!(copy.view().decode(Allocator::System).unwrap().as_str(), "À");
    }

    #[test]
    fn owned_slices_rebase_original_name_spelling() {
        let mut source =
            Buffer::plain(String::try_from_str_in("prefix", Allocator::System).unwrap());
        source
            .push_conversion('A', [0x80, 0, 0, 0], 2, false)
            .unwrap();
        source.push('b').unwrap();
        source
            .push_conversion('é', [0x84, 0, 0, 0], 1, false)
            .unwrap();
        let name = source.view().for_slice(&source[6..]);
        let expected = name.name_encoding(Allocator::System).unwrap();
        let copy = name.to_owned(Allocator::System).unwrap();
        source.clear();
        assert!(copy.view().same_name_encoding(&expected));
        assert_eq!(
            copy.view().decode(Allocator::System).unwrap().as_str(),
            "Abé"
        );
        assert!(!Slice::plain("Abé").same_name_encoding(&expected));
    }
}
