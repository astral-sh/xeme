//! UTF-8 and C string buffers with fallible growth and explicit allocator ownership.

use std::borrow::Borrow;
use std::ffi::{CStr, c_char};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, RangeBounds};

use crate::{AllocError, Allocator, Vec, try_extend_from_slice};

pub struct String {
    bytes: Vec<u8>,
}
impl String {
    #[must_use]
    pub fn new_in(allocator: Allocator) -> Self {
        Self {
            bytes: Vec::new_in(allocator),
        }
    }
    pub fn try_with_capacity_in(capacity: usize, allocator: Allocator) -> Result<Self, AllocError> {
        let mut result = Self::new_in(allocator);
        result.bytes.try_reserve_exact(capacity)?;
        Ok(result)
    }
    pub fn try_from_str_in(text: &str, allocator: Allocator) -> Result<Self, AllocError> {
        // Keep room for C callback terminators without growing each owned name
        // and attribute again. Empty Rust strings remain allocation-free.
        let capacity = if text.is_empty() {
            0
        } else {
            text.len()
                .checked_add(1)
                .ok_or(AllocError::CapacityOverflow)?
        };
        let mut result = Self::try_with_capacity_in(capacity, allocator)?;
        try_extend_from_slice(&mut result.bytes, text.as_bytes())?;
        Ok(result)
    }
    pub fn try_clone(&self) -> Result<Self, AllocError> {
        Self::try_from_str_in(self, self.allocator())
    }
    #[must_use]
    pub fn allocator(&self) -> Allocator {
        *self.bytes.allocator()
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        // SAFETY: All constructors accept &str; mutators preserve character
        // boundaries, and the byte buffer is never exposed mutably.
        unsafe { std::str::from_utf8_unchecked(&self.bytes) }
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn try_push_str(&mut self, text: &str) -> Result<(), AllocError> {
        try_extend_from_slice(&mut self.bytes, text.as_bytes())
    }
    pub fn try_push(&mut self, character: char) -> Result<(), AllocError> {
        self.try_push_str(character.encode_utf8(&mut [0; 4]))
    }
    pub fn push_str(&mut self, text: &str) -> Result<(), AllocError> {
        self.try_push_str(text)
    }
    pub fn push(&mut self, character: char) -> Result<(), AllocError> {
        self.try_push(character)
    }
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), AllocError> {
        self.bytes.try_reserve(additional).map_err(Into::into)
    }
    pub fn clear(&mut self) {
        self.bytes.clear();
    }
    pub fn truncate(&mut self, length: usize) {
        if length < self.len() {
            assert!(self.is_char_boundary(length));
            self.bytes.truncate(length);
        }
    }
    /// Remove a byte range after checking both UTF-8 boundaries.
    pub fn drain<R: RangeBounds<usize>>(&mut self, range: R) {
        use std::ops::Bound;
        let start = match range.start_bound() {
            Bound::Included(n) => *n,
            Bound::Excluded(n) => n.checked_add(1).expect("range overflow"),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(n) => n.checked_add(1).expect("range overflow"),
            Bound::Excluded(n) => *n,
            Bound::Unbounded => self.len(),
        };
        assert!(start <= end && self.is_char_boundary(start) && self.is_char_boundary(end));
        self.bytes.drain(start..end);
    }
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
impl Deref for String {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<str> for String {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl Borrow<str> for String {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
impl fmt::Display for String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}
impl fmt::Debug for String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}
impl fmt::Write for String {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.try_push_str(text).map_err(|_| fmt::Error)
    }
}
impl PartialEq for String {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
impl Eq for String {}
impl PartialOrd for String {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for String {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}
impl Hash for String {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}
impl PartialEq<str> for String {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}
impl PartialEq<&str> for String {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}
impl PartialEq<String> for str {
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}
impl PartialEq<String> for &str {
    fn eq(&self, other: &String) -> bool {
        *self == other.as_str()
    }
}
impl PartialEq<std::string::String> for String {
    fn eq(&self, other: &std::string::String) -> bool {
        self.as_str() == other.as_str()
    }
}

/// A NUL-terminated buffer whose allocation retains its originating allocator.
pub struct CString {
    bytes: Vec<u8>,
}
impl CString {
    pub fn try_from_cstr_in(text: &CStr, allocator: Allocator) -> Result<Self, AllocError> {
        let mut bytes = Vec::new_in(allocator);
        bytes.try_reserve_exact(text.to_bytes_with_nul().len())?;
        try_extend_from_slice(&mut bytes, text.to_bytes_with_nul())?;
        Ok(Self { bytes })
    }
    pub fn try_from_string(text: String) -> Result<Self, AllocError> {
        if text.as_bytes().contains(&0) {
            return Err(AllocError::InteriorNul);
        }
        let mut bytes = text.into_bytes();
        bytes.try_reserve(1)?;
        bytes.push(0);
        Ok(Self { bytes })
    }
    pub fn try_from_str_in(text: &str, allocator: Allocator) -> Result<Self, AllocError> {
        Self::try_from_string(String::try_from_str_in(text, allocator)?)
    }
    pub fn try_clone(&self) -> Result<Self, AllocError> {
        Self::try_from_cstr_in(self.as_c_str(), *self.bytes.allocator())
    }
    #[must_use]
    pub fn as_ptr(&self) -> *const c_char {
        self.bytes.as_ptr().cast()
    }
    #[must_use]
    pub fn as_c_str(&self) -> &CStr {
        // SAFETY: Constructors ensure exactly one terminal NUL and expose no mutation.
        unsafe { CStr::from_bytes_with_nul_unchecked(&self.bytes) }
    }
    #[must_use]
    pub fn allocator(&self) -> Allocator {
        *self.bytes.allocator()
    }
}
impl Deref for CString {
    type Target = CStr;
    fn deref(&self) -> &CStr {
        self.as_c_str()
    }
}
impl fmt::Debug for CString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_c_str().fmt(f)
    }
}

/// Immutable UTF-8 character data with inline storage for short values.
///
/// Heap values keep the selected allocator. Inline values allocate no memory and
/// expose no growth operation; copying them never needs an allocator. Pointers
/// into an inline value remain valid only while that value remains in place.
pub struct Text {
    buffer: TextBuffer,
}

enum TextBuffer {
    Inline { bytes: [u8; 23], len: u8 },
    Heap(String),
}

impl Text {
    /// Copy a UTF-8 value, allocating through `allocator` only beyond 23 bytes.
    #[inline]
    pub fn try_from_str_in(text: &str, allocator: Allocator) -> Result<Self, AllocError> {
        if text.len() <= 23 {
            let mut bytes = [0; 23];
            bytes[..text.len()].copy_from_slice(text.as_bytes());
            Ok(Self {
                buffer: TextBuffer::Inline {
                    bytes,
                    len: text.len() as u8,
                },
            })
        } else {
            String::try_from_str_in(text, allocator).map(Self::from)
        }
    }

    /// Clone heap storage fallibly using its original allocator.
    pub fn try_clone(&self) -> Result<Self, AllocError> {
        match &self.buffer {
            TextBuffer::Inline { bytes, len } => Ok(Self {
                buffer: TextBuffer::Inline {
                    bytes: *bytes,
                    len: *len,
                },
            }),
            TextBuffer::Heap(text) => text.try_clone().map(Self::from),
        }
    }

    #[must_use]
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        match &self.buffer {
            TextBuffer::Inline { bytes, len } => &bytes[..usize::from(*len)],
            TextBuffer::Heap(text) => text.as_bytes(),
        }
    }

    #[must_use]
    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: Both variants originate from UTF-8 strings. The inline length
        // is the complete input's byte length, and Text exposes no mutation.
        unsafe { std::str::from_utf8_unchecked(self.as_bytes()) }
    }
}

impl From<String> for Text {
    fn from(text: String) -> Self {
        Self {
            buffer: TextBuffer::Heap(text),
        }
    }
}
impl Deref for Text {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}
impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
impl Eq for Text {}
impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}
impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}
