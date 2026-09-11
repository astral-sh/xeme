use crate::{Error, ErrorKind, NativeLocation, Position, ScanMode};
use oriole_storage::{Allocator, Box, String, Vec, try_box, try_extend_from_slice};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Latin1,
    Ascii,
    SingleByte,
    MultiByte,
}
impl Encoding {
    fn named(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("UTF-8") {
            Some(Self::Utf8)
        } else if name.eq_ignore_ascii_case("UTF-16LE") {
            Some(Self::Utf16Le)
        } else if name.eq_ignore_ascii_case("UTF-16BE") {
            Some(Self::Utf16Be)
        } else if name.eq_ignore_ascii_case("ISO-8859-1") {
            Some(Self::Latin1)
        } else if name.eq_ignore_ascii_case("US-ASCII") || name.eq_ignore_ascii_case("ASCII") {
            Some(Self::Ascii)
        } else {
            None
        }
    }
    fn raw_len(self, text: &str) -> usize {
        match self {
            Self::Utf8 | Self::Ascii => text.len(),
            Self::Latin1 | Self::SingleByte => text.chars().count(),
            Self::MultiByte => unreachable!("custom byte widths belong to the source"),
            Self::Utf16Le | Self::Utf16Be => text.chars().map(char::len_utf16).sum::<usize>() * 2,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Decoder {
    allocator: Allocator,
    encoding: Option<Encoding>,
    requested: Option<String>,
    pending: Vec<u8>,
    pending_cursor: usize,
    declaration_checked: usize,
    unknown_name: Option<String>,
    encoding_error_position: Option<Position>,
    custom_map: Option<Box<[i32; 256]>>,
    conversion: Option<([u8; 4], u8)>,
}
impl Decoder {
    pub(crate) fn new(requested: Option<&str>, allocator: Allocator) -> Result<Self, Error> {
        Ok(Self {
            allocator,
            encoding: None,
            requested: requested
                .map(|name| String::try_from_str_in(name, allocator))
                .transpose()?,
            pending: Vec::new_in(allocator),
            pending_cursor: 0,
            declaration_checked: 0,
            unknown_name: None,
            encoding_error_position: None,
            custom_map: None,
            conversion: None,
        })
    }
    /// Replace only the protocol name after the adapter has stopped parsing.
    pub(crate) fn set_completed_encoding(&mut self, requested: Option<&str>) -> Result<(), Error> {
        let requested = requested
            .map(|name| String::try_from_str_in(name, self.allocator))
            .transpose()?;
        self.requested = requested;
        Ok(())
    }

    /// A complete BOM can precede enough input to choose the declaration's
    /// encoding. Report only that recognized prefix for consumed-byte accounting.
    pub(crate) fn pending_bom_len(&self, external_content: bool) -> usize {
        if self.encoding.is_some()
            // An explicit unknown protocol encoding must first be resolved by
            // its handler; these bytes have not yet been recognized as a BOM.
            || self.requested.as_deref().is_some_and(|name| {
                Encoding::named(name).is_none() && !name.eq_ignore_ascii_case("UTF-16")
            })
            || (external_content
                && self.requested.as_deref().and_then(Encoding::named) == Some(Encoding::Latin1))
        {
            return 0;
        }
        if self.pending.starts_with(&[0xef, 0xbb, 0xbf]) {
            3
        } else if self.pending.starts_with(&[0xff, 0xfe]) || self.pending.starts_with(&[0xfe, 0xff])
        {
            2
        } else {
            0
        }
    }

    pub(crate) fn feed(
        &mut self,
        bytes: &[u8],
        final_input: bool,
        source: &mut Source,
        max_token: usize,
        external_content: bool,
    ) -> Result<(), Error> {
        try_extend_from_slice(&mut self.pending, bytes)?;
        if self.conversion.is_some() {
            return Ok(());
        }
        if self.encoding.is_none() {
            let Some((encoding, skip)) = self.detect(final_input, max_token, external_content)?
            else {
                return Ok(());
            };
            self.encoding = Some(encoding);
            source.encoding = encoding;
            source.raw_index = skip;
            source.column = usize::from(skip != 0);
            self.pending.drain(..skip);
        }
        let encoding = self.encoding.expect("encoding was detected");
        let mut consumed = self.pending_cursor;
        match encoding {
            Encoding::Utf8 => match std::str::from_utf8(&self.pending) {
                Ok(text) => {
                    source
                        .text
                        .try_push_str_with_minimum(text, max_token.min(1024))?;
                    consumed = self.pending.len();
                }
                Err(error) => {
                    consumed = error.valid_up_to();
                    source.text.try_push_str_with_minimum(
                        std::str::from_utf8(&self.pending[..consumed])
                            .expect("validated UTF-8 prefix"),
                        max_token.min(1024),
                    )?;
                    if error.error_len().is_some() {
                        return Err(Error::bare(ErrorKind::InvalidToken, "invalid UTF-8"));
                    }
                }
            },
            Encoding::SingleByte | Encoding::MultiByte => {
                let map = self.custom_map.as_ref().expect("custom encoding has a map");
                while consumed < self.pending.len() {
                    let value = map[usize::from(self.pending[consumed])];
                    if (-4..=-2).contains(&value) {
                        let width = (-value) as usize;
                        if self.pending.len() - consumed >= width {
                            let mut sequence = [0; 4];
                            sequence[..width]
                                .copy_from_slice(&self.pending[consumed..consumed + width]);
                            self.conversion = Some((sequence, width as u8));
                        }
                        break;
                    }
                    let character =
                        u32::try_from(value)
                            .ok()
                            .and_then(char::from_u32)
                            .ok_or(Error::bare(
                                ErrorKind::InvalidToken,
                                "undefined byte in custom encoding",
                            ))?;
                    let byte = self.pending[consumed];
                    let spelling = (u32::from(character) != u32::from(byte)).then_some((
                        [byte, 0, 0, 0],
                        custom_public_byte(map, byte, source.name_rules),
                    ));
                    source.push_custom(character, 1, spelling)?;
                    consumed += 1;
                }
            }
            Encoding::Ascii | Encoding::Latin1 => {
                let end = if encoding == Encoding::Ascii {
                    self.pending
                        .iter()
                        .position(|byte| !byte.is_ascii())
                        .unwrap_or(self.pending.len())
                } else {
                    self.pending.len()
                };
                for byte in &self.pending[..end] {
                    source.text.try_push(char::from(*byte))?;
                }
                if end < self.pending.len() {
                    return Err(Error::bare(
                        ErrorKind::InvalidToken,
                        "non-ASCII byte in ASCII document",
                    ));
                }
                consumed = self.pending.len();
            }
            Encoding::Utf16Le | Encoding::Utf16Be => {
                let unit = |bytes: &[u8]| {
                    if encoding == Encoding::Utf16Le {
                        u16::from_le_bytes([bytes[0], bytes[1]])
                    } else {
                        u16::from_be_bytes([bytes[0], bytes[1]])
                    }
                };
                while consumed + 1 < self.pending.len() {
                    let first = unit(&self.pending[consumed..]);
                    let (codepoint, width) = if (0xd800..=0xdbff).contains(&first) {
                        if consumed + 3 >= self.pending.len() {
                            break;
                        }
                        let second = unit(&self.pending[consumed + 2..]);
                        if !(0xdc00..=0xdfff).contains(&second) {
                            return Err(Error::bare(
                                ErrorKind::InvalidToken,
                                "unpaired UTF-16 surrogate",
                            ));
                        }
                        (
                            0x10000
                                + ((u32::from(first) - 0xd800) << 10)
                                + (u32::from(second) - 0xdc00),
                            4,
                        )
                    } else {
                        (u32::from(first), 2)
                    };
                    source
                        .text
                        .try_push(char::from_u32(codepoint).ok_or(Error::bare(
                            ErrorKind::InvalidToken,
                            "unpaired UTF-16 surrogate",
                        ))?)?;
                    consumed += width;
                }
            }
        }
        if encoding == Encoding::MultiByte {
            self.pending_cursor = consumed;
            if consumed == self.pending.len()
                || (consumed >= 64 * 1024 && consumed >= self.pending.len() / 2)
            {
                self.pending.drain(..consumed);
                self.pending_cursor = 0;
            }
        } else {
            self.pending.drain(..consumed);
        }
        if final_input && self.conversion.is_none() && self.pending_cursor < self.pending.len() {
            let kind = if matches!(encoding, Encoding::Utf16Le | Encoding::Utf16Be)
                && self.pending.len() == 1
            {
                ErrorKind::UnclosedToken
            } else {
                ErrorKind::PartialCharacter
            };
            return Err(Error::bare(kind, "incomplete encoded character"));
        }
        Ok(())
    }

    fn detect(
        &mut self,
        final_input: bool,
        max_token: usize,
        external_content: bool,
    ) -> Result<Option<(Encoding, usize)>, Error> {
        // An explicitly labelled external text entity can begin with ordinary
        // Latin-1 bytes that happen to spell a Unicode byte-order mark.
        if external_content
            && self.requested.as_deref().and_then(Encoding::named) == Some(Encoding::Latin1)
        {
            return Ok(Some((Encoding::Latin1, 0)));
        }
        let bytes = &self.pending;
        if bytes.len() < 4
            && !final_input
            && !(bytes.len() >= 2
                && bytes[0].is_ascii()
                && bytes[0] != 0
                && bytes[1].is_ascii()
                && bytes[1] != 0)
        {
            return Ok(None);
        }
        let (sniffed, skip) = if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
            (Encoding::Utf8, 3)
        } else if bytes.starts_with(&[0xff, 0xfe]) {
            (Encoding::Utf16Le, 2)
        } else if bytes.starts_with(&[0xfe, 0xff]) {
            (Encoding::Utf16Be, 2)
        } else if bytes.first() == Some(&0) && bytes.len() >= 2 {
            (Encoding::Utf16Be, 0)
        } else if bytes.get(1) == Some(&0)
            && (!external_content || self.requested.is_some() || bytes[0] == b'<')
        {
            // In external content, a plain ASCII character can be complete
            // before the next byte arrives. Only a markup opener selects
            // unlabelled little-endian UTF-16; document/DTD detection differs.
            (Encoding::Utf16Le, 0)
        } else {
            (Encoding::Utf8, 0)
        };
        if let Some(requested) = &self.requested {
            let requested = if requested.eq_ignore_ascii_case("UTF-16") {
                match sniffed {
                    Encoding::Utf16Le | Encoding::Utf16Be => sniffed,
                    _ => {
                        return Err(Error::bare(
                            ErrorKind::IncorrectEncoding,
                            "UTF-16 input requires a byte order mark or declaration",
                        ));
                    }
                }
            } else {
                let Some(encoding) = Encoding::named(requested) else {
                    self.unknown_name = Some(requested.try_clone()?);
                    return Err(Error::bare(
                        ErrorKind::UnknownEncoding,
                        "unsupported input encoding",
                    ));
                };
                encoding
            };
            if skip > 0 && sniffed != requested {
                return Err(Error::bare(
                    ErrorKind::IncorrectEncoding,
                    "byte order mark conflicts with the requested encoding",
                ));
            }
            return Ok(Some((requested, skip)));
        }
        if sniffed != Encoding::Utf8 {
            return Ok(Some((sniffed, skip)));
        }
        let content = &bytes[skip..];
        if b"<?xml".starts_with(content) && !final_input {
            return Ok(None);
        }
        if content.starts_with(b"<?xml") && content.get(5).is_some_and(u8::is_ascii_whitespace) {
            let start = self.declaration_checked.min(content.len());
            let end = content[start..]
                .windows(2)
                .position(|window| window == b"?>")
                .map(|end| end + start);
            self.declaration_checked = content.len().saturating_sub(1);
            let Some(end) = end else {
                if content.len() > max_token {
                    return Err(Error::bare(
                        ErrorKind::LimitExceeded,
                        "XML declaration byte limit exceeded",
                    ));
                }
                if !final_input {
                    return Ok(None);
                }
                return Ok(Some((Encoding::Utf8, skip)));
            };
            // The declaration is ASCII in every supported ASCII-compatible encoding.
            if let Ok(declaration) = std::str::from_utf8(&content[..end])
                && let Some(start) = declaration.find("encoding")
            {
                let rest = declaration[start + 8..].trim_start_matches(crate::whitespace);
                if let Some(rest) = rest.strip_prefix('=') {
                    let rest = rest.trim_start_matches(crate::whitespace);
                    if let Some(quote @ ('\'' | '"')) = rest.chars().next()
                        && let Some(end) = rest[1..].find(quote)
                    {
                        let name = &rest[1..end + 1];
                        if !crate::valid_encoding_name(name) {
                            return Ok(Some((Encoding::Utf8, skip)));
                        }
                        let encoding = Encoding::named(name);
                        let mismatch = name.eq_ignore_ascii_case("UTF-16")
                            || matches!(encoding, Some(Encoding::Utf16Le | Encoding::Utf16Be));
                        if encoding.is_none() || mismatch {
                            let offset = declaration.len() - rest.len() + 1;
                            let mut position = Position {
                                byte_index: skip + offset,
                                line: 1,
                                column: usize::from(skip != 0),
                                byte_count: 0,
                            };
                            let mut previous_cr = false;
                            for character in declaration[..offset].chars() {
                                match character {
                                    '\r' => {
                                        position.line += 1;
                                        position.column = 0;
                                    }
                                    '\n' if !previous_cr => {
                                        position.line += 1;
                                        position.column = 0;
                                    }
                                    '\n' => {}
                                    _ => position.column += 1,
                                }
                                previous_cr = character == '\r';
                            }
                            self.encoding_error_position = Some(position);
                            if mismatch {
                                return Err(Error::bare(
                                    ErrorKind::IncorrectEncoding,
                                    "declared encoding conflicts with input bytes",
                                ));
                            }
                            self.unknown_name =
                                Some(String::try_from_str_in(name, self.allocator)?);
                            return Err(Error::bare(
                                ErrorKind::UnknownEncoding,
                                "unsupported declared encoding",
                            ));
                        }
                        return Ok(encoding.map(|encoding| (encoding, skip)));
                    }
                }
            }
        }
        Ok(Some((Encoding::Utf8, skip)))
    }

    /// A parameter child is read once its protocol encoding is known, even for
    /// empty non-final input that does not complete byte-order detection.
    pub(crate) fn protocol_encoding_ready(&self) -> bool {
        self.encoding.is_some()
            || self.requested.as_deref().is_none_or(|name| {
                Encoding::named(name).is_some() || name.eq_ignore_ascii_case("UTF-16")
            })
    }

    pub(crate) fn unknown_encoding(&self) -> Option<&str> {
        self.unknown_name.as_deref()
    }
    pub(crate) fn encoding_error_position(&self) -> Option<Position> {
        self.encoding_error_position
    }
    pub(crate) fn append_pending(&mut self, bytes: &[u8]) -> Result<(), Error> {
        try_extend_from_slice(&mut self.pending, bytes)?;
        Ok(())
    }
    pub(crate) fn install_map(
        &mut self,
        name: &str,
        map: [i32; 256],
        allow_multibyte: bool,
        source: &mut Source,
    ) -> Result<(), Error> {
        if self
            .unknown_name
            .as_deref()
            .is_none_or(|unknown| !unknown.eq_ignore_ascii_case(name))
        {
            return Err(Error::bare(
                ErrorKind::UnknownEncoding,
                "encoding map does not match the requested encoding",
            ));
        }
        for (byte, value) in map.iter().copied().enumerate() {
            if required_ascii(byte as i32) && value != byte as i32 {
                return Err(Error::bare(
                    ErrorKind::UnknownEncoding,
                    "custom encoding changes an ASCII markup character",
                ));
            }
            let minimum = if allow_multibyte { -4 } else { -1 };
            if !(minimum..=0xffff).contains(&value)
                || (required_ascii(value) && value != byte as i32)
            {
                return Err(Error::bare(
                    ErrorKind::UnknownEncoding,
                    "custom encoding changes XML syntax or requires multibyte conversion",
                ));
            }
        }
        if self.requested.is_some() && self.pending.starts_with(&[0xef, 0xbb, 0xbf]) {
            return Err(Error::bare(
                ErrorKind::IncorrectEncoding,
                "custom encoding conflicts with byte order mark",
            ));
        }
        let encoding = if map.iter().any(|value| *value < -1) {
            Encoding::MultiByte
        } else {
            Encoding::SingleByte
        };
        self.custom_map = Some(try_box(map, self.allocator)?);
        if self.pending.starts_with(&[0xef, 0xbb, 0xbf]) {
            self.pending.drain(..3);
            source.raw_index = 3;
            source.column = 1;
        }
        self.encoding = Some(encoding);
        source.encoding = encoding;
        if encoding == Encoding::MultiByte {
            source.decoded_end = source.position(0);
        }
        Ok(())
    }
    pub(crate) fn conversion(&self) -> Option<([u8; 4], u8)> {
        self.conversion
    }

    pub(crate) fn resolve_conversion(
        &mut self,
        value: i32,
        source: &mut Source,
    ) -> Result<(), Error> {
        let (bytes, width) = self.conversion.ok_or(Error::bare(
            ErrorKind::InvalidToken,
            "no encoding conversion is pending",
        ))?;
        let character = u32::try_from(value)
            .ok()
            .filter(|value| *value <= 0xffff)
            .and_then(char::from_u32)
            .filter(|character| crate::names::is_xml_char(*character))
            .ok_or(Error::bare(
                ErrorKind::InvalidToken,
                "invalid custom encoding conversion",
            ))?;
        let map = self.custom_map.as_ref().expect("custom encoding has a map");
        let public_valid = bytes[..usize::from(width)]
            .iter()
            .all(|byte| custom_public_byte(map, *byte, source.name_rules));
        source.push_custom(character, width, Some((bytes, public_valid)))?;
        self.pending_cursor += usize::from(width);
        self.conversion = None;
        Ok(())
    }
    /// Payload copied by `inherit_map` for a matching requested encoding.
    pub(crate) fn inherited_map_bytes(&self, requested: Option<&str>) -> (usize, usize) {
        if self.encoding == Some(Encoding::MultiByte) {
            return (0, 0);
        }
        if requested
            .zip(self.unknown_name.as_deref())
            .is_some_and(|(requested, name)| requested.eq_ignore_ascii_case(name))
        {
            (
                self.unknown_name.as_ref().map_or(0, |name| name.len()),
                self.custom_map
                    .as_ref()
                    .map_or(0, |_| size_of::<[i32; 256]>()),
            )
        } else {
            (0, 0)
        }
    }

    pub(crate) fn inherit_map(&mut self, parent: &Self, source: &mut Source) -> Result<(), Error> {
        // A multibyte converter has application-owned state. A child must request
        // its own encoding instance instead of inheriting a borrowed callback.
        if parent.encoding == Some(Encoding::MultiByte) {
            return Ok(());
        }
        if self
            .requested
            .as_deref()
            .zip(parent.unknown_name.as_deref())
            .is_some_and(|(name, parent)| name.eq_ignore_ascii_case(parent))
        {
            self.custom_map = parent
                .custom_map
                .as_ref()
                .map(|map| try_box(**map, self.allocator))
                .transpose()?;
            self.unknown_name = parent
                .unknown_name
                .as_ref()
                .map(|name| String::try_from_str_in(name, self.allocator))
                .transpose()?;
            if self.custom_map.is_some() {
                self.encoding = Some(Encoding::SingleByte);
                source.encoding = Encoding::SingleByte;
            }
        }
        Ok(())
    }
    pub(crate) fn check_declaration(&self, name: &str) -> Result<(), Error> {
        if matches!(
            self.encoding,
            Some(Encoding::SingleByte | Encoding::MultiByte)
        ) && self
            .unknown_name
            .as_deref()
            .is_some_and(|custom| custom.eq_ignore_ascii_case(name))
        {
            return Ok(());
        }
        // An explicitly supplied encoding takes precedence over the declaration.
        if self.requested.is_some() {
            return Ok(());
        }
        let actual = self.encoding.unwrap_or(Encoding::Utf8);
        if name.eq_ignore_ascii_case("UTF-16")
            && matches!(actual, Encoding::Utf16Le | Encoding::Utf16Be)
        {
            return Ok(());
        }
        let declared = Encoding::named(name).ok_or(Error::bare(
            ErrorKind::UnknownEncoding,
            "unsupported declared encoding",
        ))?;
        if declared != actual {
            return Err(Error::bare(
                ErrorKind::IncorrectEncoding,
                "declared encoding conflicts with detected encoding",
            ));
        }
        Ok(())
    }
}

/// Expat's public-ID scanner checks every original byte's type, even bytes
/// inside a converter sequence. Mapped ASCII bytes classified as XML name
/// characters remain allowed; the two ordinary punctuation exceptions use raw
/// `$`/`@` values. Required ASCII syntax is fixed by map installation.
fn custom_public_byte(map: &[i32; 256], byte: u8, name_rules: crate::NameRules) -> bool {
    let scalar = char::from(byte);
    scalar.is_ascii_alphanumeric()
        || " \r\n-'()+,./:=?;!*#@$_%".contains(scalar)
        || (byte.is_ascii()
            && u32::try_from(map[usize::from(byte)])
                .ok()
                .filter(|value| *value > 0x7f)
                .and_then(char::from_u32)
                .is_some_and(|character| name_rules.is_name_char(character)))
}

/// ASCII characters that Expat requires custom encodings to preserve exactly.
/// Ordinary punctuation such as `$`, `@`, and `~` may be remapped; XML markup,
/// whitespace, and ASCII name characters may not acquire alternate byte forms.
fn required_ascii(value: i32) -> bool {
    matches!(value, 9 | 10 | 13)
        || ((32..127).contains(&value)
            && !matches!(value, 36 | 64 | 92 | 94 | 96 | 123 | 125 | 126))
}

#[derive(Debug, Default)]
struct Scan {
    mode: Option<ScanMode>,
    checked: usize,
    quote: u8,
    brackets: usize,
    comment: bool,
    pi: bool,
}

/// Original-input coordinates are bounded by `Parser::input_bytes_remaining`.
/// Internal entities own a finite UTF-8 buffer and return the referring source's
/// anchor without adding replacement offsets. Compaction changes buffer offsets,
/// never the cumulative original-byte index or one-based line number.
#[derive(Debug)]
pub(crate) struct Source {
    pub(crate) text: crate::lexical::Buffer,
    cursor: usize,
    encoding: Encoding,
    raw_index: usize,
    accounted_raw: usize,
    raw_widths: Vec<u8>,
    decoded_end: Position,
    decoded_end_cr: bool,
    line: usize,
    column: usize,
    previous_cr: bool,
    coordinate_cursor: usize,
    committed: Option<NativePoint>,
    prospective: Option<NativePoint>,
    anchor: Option<Position>,
    pub(crate) initial_depth: usize,
    pub(crate) entity_name: Option<String>,
    pub(crate) dtd_fragment: bool,
    scan: Scan,
    deferred_size: usize,
    name_rules: crate::NameRules,
}

/// A native event start survives compaction either as a retained offset or an
/// exact resolved position. Prospective points are never public before commit.
#[derive(Clone, Copy, Debug)]
struct NativePoint {
    identity: NativeLocation,
    coordinates: NativeCoordinates,
}

#[derive(Clone, Copy, Debug)]
enum NativeCoordinates {
    Retained(usize),
    Resolved(Position),
}

/// Coordinates within one source whose consumed prefix remains unchanged.
/// Offsets advance monotonically, so many callbacks never rescan earlier text.
#[derive(Debug)]
pub(crate) struct PositionCursor {
    offset: usize,
    position: Position,
    previous_cr: bool,
}
impl Source {
    pub(crate) fn new(allocator: Allocator, name_rules: crate::NameRules) -> Self {
        Self {
            text: crate::lexical::Buffer::new_in(allocator),
            cursor: 0,
            encoding: Encoding::Utf8,
            raw_index: 0,
            accounted_raw: 0,
            raw_widths: Vec::new_in(allocator),
            decoded_end: Position {
                line: 1,
                ..Position::default()
            },
            decoded_end_cr: false,
            line: 1,
            column: 0,
            previous_cr: false,
            coordinate_cursor: 0,
            committed: None,
            prospective: None,
            anchor: None,
            initial_depth: 0,
            entity_name: None,
            dtd_fragment: false,
            scan: Scan::default(),
            deferred_size: 0,
            name_rules,
        }
    }
    pub(crate) fn entity(
        text: String,
        name: String,
        position: Position,
        initial_depth: usize,
        name_rules: crate::NameRules,
    ) -> Self {
        let allocator = text.allocator();
        Self {
            text: crate::lexical::Buffer::plain(text),
            anchor: Some(position),
            initial_depth,
            entity_name: Some(name),
            ..Self::new(allocator, name_rules)
        }
    }
    pub(crate) fn remaining(&self) -> &str {
        &self.text[self.cursor..]
    }

    /// Identify an unanchored UTF-8 token without projecting custom raw widths.
    pub(crate) fn native_utf8_byte_index(&self) -> Option<usize> {
        (self.encoding == Encoding::Utf8 && self.anchor.is_none()).then_some(self.raw_index)
    }

    #[inline]
    pub(crate) fn has_conversions(&self) -> bool {
        self.text.has_conversions()
    }

    pub(crate) fn lexical_remaining(&self) -> crate::lexical::Slice<'_> {
        self.text.view().for_slice(self.remaining())
    }

    fn push_custom(
        &mut self,
        character: char,
        raw_width: u8,
        converted: Option<([u8; 4], bool)>,
    ) -> Result<(), Error> {
        let lexical = converted
            .filter(|_| raw_width > 1)
            .map_or(character, |_| crate::lexical::representative(character));
        if self.encoding == Encoding::MultiByte {
            let count = lexical.len_utf8();
            self.raw_widths
                .try_reserve(count)
                .map_err(oriole_storage::AllocError::from)?;
            if let Some((bytes, public_valid)) = converted {
                self.text
                    .push_conversion(character, bytes, raw_width, public_valid)?;
            } else {
                self.text.try_push(character)?;
            }
            // Both reservations precede mutation, so allocation failure preserves
            // the alignment between UTF-8 bytes and original encoded widths.
            self.raw_widths.push(raw_width);
            self.raw_widths.extend(std::iter::repeat_n(0, count - 1));
            self.decoded_end.byte_index += usize::from(raw_width);
            match lexical {
                '\r' => {
                    self.decoded_end.line += 1;
                    self.decoded_end.column = 0;
                }
                '\n' => {
                    if !self.decoded_end_cr {
                        self.decoded_end.line += 1;
                    }
                    self.decoded_end.column = 0;
                }
                _ => self.decoded_end.column += 1,
            }
            self.decoded_end_cr = lexical == '\r';
        } else if let Some((bytes, public_valid)) = converted {
            self.text
                .push_conversion(character, bytes, raw_width, public_valid)?;
        } else {
            self.text.try_push(character)?;
        }
        Ok(())
    }

    #[inline]
    fn raw_len(&self, offset: usize, count: usize) -> usize {
        if matches!(self.encoding, Encoding::Utf8 | Encoding::Ascii) {
            // Native UTF-8 and ASCII keep one source byte per stored byte.
            debug_assert!(self.remaining().get(offset..offset + count).is_some());
            return count;
        }
        self.converted_raw_len(offset, count)
    }

    /// Count original encoded bytes in a converted source range.
    fn converted_raw_len(&self, offset: usize, count: usize) -> usize {
        if self.encoding == Encoding::MultiByte {
            self.raw_widths[self.cursor + offset..self.cursor + offset + count]
                .iter()
                .map(|width| usize::from(*width))
                .sum()
        } else {
            self.encoding
                .raw_len(&self.remaining()[offset..offset + count])
        }
    }

    /// Expat emits converted character data through a 1 KiB UTF-8 buffer.
    /// Its native UTF-8 and ASCII paths do not need that conversion buffer.
    pub(crate) fn converted_text_limit(&self) -> usize {
        if matches!(self.encoding, Encoding::Utf8 | Encoding::Ascii) {
            self.remaining().len()
        } else {
            self.remaining().floor_char_boundary(1024)
        }
    }
    /// Original bytes remain eager even when line and column are deferred.
    pub(crate) fn byte_index(&self) -> usize {
        self.anchor
            .map_or(self.raw_index, |anchor| anchor.byte_index)
    }

    fn coordinates_at(&self, offset: usize) -> (usize, usize, bool) {
        assert!(offset >= self.coordinate_cursor && offset <= self.text.len());
        let (mut line, mut column, mut previous_cr) = (self.line, self.column, self.previous_cr);
        if offset == self.coordinate_cursor {
            return (line, column, previous_cr);
        }
        advance_position(
            &self.text[self.coordinate_cursor..offset],
            &mut line,
            &mut column,
            &mut previous_cr,
        );
        (line, column, previous_cr)
    }

    fn advance_coordinates(&mut self, offset: usize) {
        if offset == self.coordinate_cursor {
            return;
        }
        let (line, column, previous_cr) = self.coordinates_at(offset);
        self.line = line;
        self.column = column;
        self.previous_cr = previous_cr;
        self.coordinate_cursor = offset;
    }

    pub(crate) fn position(&self, count: usize) -> Position {
        if let Some(anchor) = self.anchor {
            return anchor;
        }
        let (line, column, _) = self.coordinates_at(self.cursor);
        Position {
            byte_index: self.raw_index,
            line,
            column,
            byte_count: self.raw_len(0, count),
        }
    }
    pub(crate) fn end_position(&self) -> Position {
        if self.encoding == Encoding::MultiByte {
            return self.decoded_end;
        }
        self.position_at(self.remaining().len(), 0)
    }
    pub(crate) fn position_at(&self, offset: usize, count: usize) -> Position {
        if let Some(anchor) = self.anchor {
            return anchor;
        }
        let (line, column, _) = self.coordinates_at(self.cursor + offset);
        Position {
            byte_index: self.raw_index + self.raw_len(0, offset),
            byte_count: self.raw_len(offset, count),
            line,
            column,
        }
    }

    pub(crate) fn position_cursor(&self) -> PositionCursor {
        let (line, column, previous_cr) = self.coordinates_at(self.cursor);
        PositionCursor {
            offset: 0,
            position: self.anchor.unwrap_or(Position {
                byte_index: self.raw_index,
                byte_count: 0,
                line,
                column,
            }),
            previous_cr,
        }
    }

    pub(crate) fn begin_prospective(&mut self, identity: NativeLocation) {
        assert!(self.prospective.is_none());
        assert!(self.native_utf8_byte_index() == Some(identity.byte_index));
        assert!(!self.has_conversions() && identity.byte_count > 0);
        assert!(self.remaining().get(..identity.byte_count).is_some());
        self.prospective = Some(NativePoint {
            identity,
            coordinates: NativeCoordinates::Retained(self.cursor),
        });
    }

    pub(crate) fn commit_prospective(&mut self, identity: NativeLocation) {
        let point = self
            .prospective
            .take()
            .expect("native frame has a prospective start");
        assert_eq!(point.identity, identity);
        self.committed = Some(point);
    }

    pub(crate) fn discard_prospective(&mut self) {
        self.prospective = None;
    }

    pub(crate) fn recover_prospective(&mut self) {
        if self.prospective.is_some() {
            self.materialize_coordinates();
            self.discard_prospective();
        }
    }

    fn point_position(&self, point: NativePoint) -> Position {
        match point.coordinates {
            NativeCoordinates::Resolved(position) => position,
            NativeCoordinates::Retained(offset) => {
                let (line, column, _) = self.coordinates_at(offset);
                Position {
                    byte_index: point.identity.byte_index,
                    byte_count: point.identity.byte_count,
                    line,
                    column,
                }
            }
        }
    }

    pub(crate) fn published_position(&self, identity: NativeLocation) -> Position {
        let point = self.committed.expect("native publication is retained");
        assert_eq!(point.identity, identity);
        self.point_position(point)
    }

    fn resolve_point(&mut self, mut point: NativePoint) -> NativePoint {
        if let NativeCoordinates::Retained(offset) = point.coordinates {
            self.advance_coordinates(offset);
            point.coordinates = NativeCoordinates::Resolved(Position {
                byte_index: point.identity.byte_index,
                byte_count: point.identity.byte_count,
                line: self.line,
                column: self.column,
            });
        }
        point
    }

    pub(crate) fn resolve_published(&mut self, identity: NativeLocation) -> Position {
        let point = self.committed.expect("native publication is retained");
        assert_eq!(point.identity, identity);
        let point = self.resolve_point(point);
        self.committed = Some(point);
        self.point_position(point)
    }

    /// Resolve starts in order before advancing the checkpoint or discarding text.
    pub(crate) fn materialize_coordinates(&mut self) {
        // A retained point exactly at the checkpoint still needs its snapshot.
        // Already resolved points need neither projection nor another writeback.
        if let Some(point) = self.committed.as_ref()
            && matches!(point.coordinates, NativeCoordinates::Retained(_))
        {
            let point = *point;
            self.committed = Some(self.resolve_point(point));
        }
        if let Some(point) = self.prospective.as_ref()
            && matches!(point.coordinates, NativeCoordinates::Retained(_))
        {
            let point = *point;
            self.prospective = Some(self.resolve_point(point));
        }
        self.advance_coordinates(self.cursor);
    }

    /// Advance coordinates in an append-only source view. The caller must not
    /// consume from this source while the cursor is retained. This uses the same
    /// raw widths, XML line rules and entity anchor as `position_at`.
    pub(crate) fn position_from_cursor(
        &self,
        cursor: &mut PositionCursor,
        offset: usize,
        count: usize,
    ) -> Position {
        assert!(offset >= cursor.offset);
        if let Some(anchor) = self.anchor {
            cursor.offset = offset;
            return anchor;
        }
        cursor.position.byte_index += self.raw_len(cursor.offset, offset - cursor.offset);
        advance_position(
            &self.remaining()[cursor.offset..offset],
            &mut cursor.position.line,
            &mut cursor.position.column,
            &mut cursor.previous_cr,
        );
        cursor.offset = offset;
        Position {
            byte_count: self.raw_len(offset, count),
            ..cursor.position
        }
    }
    pub(crate) fn should_defer(&self, limit: usize) -> bool {
        self.deferred_size > 0
            && self.remaining().len() < self.deferred_size.saturating_mul(2)
            && self.remaining().len() <= limit
    }
    pub(crate) fn mark_deferred(&mut self) {
        self.deferred_size = self.remaining().len();
    }
    /// Scan a quoted prolog token without revisiting an incomplete prefix.
    pub(crate) fn scan_prolog_literal(
        &mut self,
        limit: usize,
    ) -> Result<Option<usize>, (ErrorKind, usize)> {
        let text = &self.text[self.cursor..];
        let quote = text.as_bytes()[0];
        let start = self.scan.checked.max(1);
        for (relative, character) in text[start..].char_indices() {
            let index = start + relative;
            if index >= limit {
                return Err((ErrorKind::LimitExceeded, 0));
            }
            if !crate::is_xml_char(character) {
                return Err((ErrorKind::InvalidToken, index));
            }
            if character == char::from(quote) {
                // Retain the closing quote while awaiting its delimiter.
                self.scan.checked = index;
                return Ok(Some(index + 1));
            }
        }
        self.scan.checked = text.len();
        if text.len() > limit {
            Err((ErrorKind::LimitExceeded, 0))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn scan_reference(
        &mut self,
        limit: usize,
    ) -> Result<Option<usize>, (ErrorKind, usize)> {
        let text = &self.text[self.cursor..];
        let numeric = text.starts_with("&#");
        let hexadecimal = numeric && text.as_bytes().get(2) == Some(&b'x');
        let digits_start = if hexadecimal { 3 } else { 2 };
        let start = self.scan.checked.max(1);
        for (relative, character) in text[start..].char_indices() {
            let index = start + relative;
            if index >= limit {
                return Err((ErrorKind::LimitExceeded, 0));
            }
            if character == ';' {
                if index == 1 || (numeric && index == digits_start) {
                    return Err((ErrorKind::InvalidToken, index));
                }
                return Ok(Some(index));
            }
            let valid = if index == 1 {
                numeric || self.name_rules.is_name_start(character)
            } else if numeric {
                (index == 2 && character == 'x')
                    || if hexadecimal {
                        character.is_ascii_hexdigit()
                    } else {
                        character.is_ascii_digit()
                    }
            } else {
                self.name_rules.is_name_char(character)
            };
            if !valid {
                return Err((ErrorKind::InvalidToken, index));
            }
        }
        self.scan.checked = text.len();
        if text.len() > limit {
            Err((ErrorKind::LimitExceeded, 0))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn accounting_bytes(&self, count: usize) -> usize {
        (self.raw_index + self.raw_len(0, count)).saturating_sub(self.accounted_raw)
    }

    pub(crate) fn unaccounted_prefix(&self, raw_bytes: usize) -> usize {
        raw_bytes.saturating_sub(self.accounted_raw)
    }

    /// Record the delta returned by an accounting query without rescanning text.
    pub(crate) fn mark_accounted(&mut self, bytes: usize) {
        self.accounted_raw += bytes;
    }

    pub(crate) fn consume(&mut self, count: usize) {
        let deferred = self.prospective.is_some();
        if !deferred {
            self.materialize_coordinates();
        }
        self.raw_index += self.raw_len(0, count);
        self.cursor += count;
        if !deferred {
            self.advance_coordinates(self.cursor);
        }
        self.scan = Scan::default();
        self.deferred_size = 0;
        if self.cursor >= 64 * 1024 && self.cursor >= self.text.len() / 2 {
            self.materialize_coordinates();
            self.text.discard_prefix(self.cursor);
            if self.encoding == Encoding::MultiByte {
                self.raw_widths.drain(..self.cursor);
            }
            self.cursor = 0;
            self.coordinate_cursor = 0;
        }
    }
    pub(crate) fn scan_token(
        &mut self,
        mode: ScanMode,
        limit: usize,
    ) -> Result<Option<usize>, (ErrorKind, usize)> {
        let bytes = &self.text.as_bytes()[self.cursor..];
        if self.scan.mode != Some(mode) {
            self.scan = Scan {
                mode: Some(mode),
                ..Scan::default()
            };
        }
        let scan = &mut self.scan;
        match mode {
            ScanMode::Tag if bytes.starts_with(b"</") => {
                // End tags contain only a name, optional XML whitespace, and '>'.
                // Resume at a character boundary instead of rescanning long names.
                let start = scan.checked.max(2);
                let mut after_name =
                    start > 2 && matches!(bytes[start - 1], b' ' | b'\t' | b'\r' | b'\n');
                for (relative, character) in self.text[self.cursor + start..].char_indices() {
                    let index = start + relative;
                    if index + character.len_utf8() > limit {
                        return Err((ErrorKind::LimitExceeded, index));
                    }
                    if character == '>' {
                        return Ok(Some(index + 1));
                    }
                    if crate::names::whitespace(character) {
                        after_name = true;
                    } else if after_name || !self.name_rules.is_name_char(character) {
                        return Err((ErrorKind::InvalidToken, index));
                    }
                }
                scan.checked = bytes.len();
            }
            ScanMode::Tag if bytes.first() == Some(&b'<') && bytes.get(1) != Some(&b'!') => {
                return scan_element_tag(bytes, scan, limit);
            }
            ScanMode::Comment | ScanMode::Pi => {
                let terminator: &[u8] = if mode == ScanMode::Comment {
                    b"-->"
                } else {
                    b"?>"
                };
                let start = scan
                    .checked
                    .max(if mode == ScanMode::Comment { 4 } else { 2 });
                if start <= bytes.len()
                    && let Some(relative) = bytes[start..]
                        .windows(terminator.len())
                        .position(|part| part == terminator)
                {
                    let end = start + relative + terminator.len();
                    return if end <= limit {
                        Ok(Some(end))
                    } else {
                        Err((ErrorKind::LimitExceeded, limit))
                    };
                }
                scan.checked = bytes.len().saturating_sub(terminator.len() - 1);
            }
            ScanMode::Tag | ScanMode::Doctype | ScanMode::DtdDeclaration => {
                let element_markup = mode == ScanMode::Tag
                    && bytes.first() == Some(&b'<')
                    && bytes.get(1) != Some(&b'!');
                let mut index = scan.checked;
                while index < bytes.len() {
                    // An element tag cannot contain another literal '<', even
                    // inside an attribute. DTD entity literals can contain it.
                    if element_markup && index != 0 && bytes[index] == b'<' {
                        return Err((ErrorKind::InvalidToken, index));
                    }
                    if index >= limit {
                        return Err((ErrorKind::LimitExceeded, limit));
                    }
                    if scan.comment {
                        if bytes[index..].starts_with(b"-->") {
                            scan.comment = false;
                            index += 3;
                            continue;
                        }
                        if bytes[index] == b'-' && bytes.len() - index < 3 {
                            break;
                        }
                    } else if scan.pi {
                        if bytes[index..].starts_with(b"?>") {
                            scan.pi = false;
                            index += 2;
                            continue;
                        }
                        if bytes[index] == b'?' && bytes.len() - index < 2 {
                            break;
                        }
                    } else if scan.quote != 0 {
                        if bytes[index] == scan.quote {
                            scan.quote = 0;
                        }
                    } else if mode == ScanMode::Doctype && bytes[index] == b'<' && index != 0 {
                        if bytes[index..].starts_with(b"<!--") {
                            scan.comment = true;
                            index += 4;
                            continue;
                        }
                        if bytes[index..].starts_with(b"<?") {
                            scan.pi = true;
                            index += 2;
                            continue;
                        }
                        if bytes.len() - index < 4 {
                            break;
                        }
                    } else {
                        match bytes[index] {
                            b'\'' | b'"' => scan.quote = bytes[index],
                            b'%' if mode == ScanMode::DtdDeclaration => {
                                if index + 1 == bytes.len() {
                                    break;
                                }
                                if !matches!(bytes[index + 1], b' ' | b'\t' | b'\r' | b'\n') {
                                    return Ok(Some(index + 1));
                                }
                            }
                            b'[' if mode == ScanMode::Doctype => return Ok(Some(index + 1)),
                            b']' if mode == ScanMode::Doctype => {
                                scan.brackets = scan.brackets.saturating_sub(1);
                            }
                            b'>' if scan.brackets == 0 => return Ok(Some(index + 1)),
                            _ => {}
                        }
                    }
                    index += 1;
                }
                scan.checked = index;
            }
        }
        if bytes.len() > limit {
            Err((ErrorKind::LimitExceeded, limit))
        } else {
            Ok(None)
        }
    }
}

/// Resume a start tag, skipping ordinary name and attribute-value bytes.
fn scan_element_tag(
    bytes: &[u8],
    scan: &mut Scan,
    limit: usize,
) -> Result<Option<usize>, (ErrorKind, usize)> {
    let mut index = scan.checked;
    if index == 0 {
        if limit == 0 {
            return Err((ErrorKind::LimitExceeded, 0));
        }
        // The opening '<' is the only one permitted in an element tag.
        index = 1;
    }
    // Short tags avoid setting up a byte search. Resume the bulk scan only
    // after this fixed prefix, including when a tag spans input chunks.
    while index < bytes.len().min(64) {
        let byte = bytes[index];
        if byte == b'<' {
            return Err((ErrorKind::InvalidToken, index));
        }
        if index >= limit {
            return Err((ErrorKind::LimitExceeded, limit));
        }
        if scan.quote != 0 {
            if byte == scan.quote {
                scan.quote = 0;
            }
        } else if byte == b'>' {
            return Ok(Some(index + 1));
        } else if matches!(byte, b'\'' | b'"') {
            scan.quote = byte;
        }
        index += 1;
    }
    while index < bytes.len() {
        if bytes[index] == b'<' {
            return Err((ErrorKind::InvalidToken, index));
        }
        if index >= limit {
            return Err((ErrorKind::LimitExceeded, limit));
        }
        // Inspect the byte at the limit: a literal '<' there takes precedence
        // over the limit error. No search needs to read farther than that byte.
        let end = bytes.len().min(limit.saturating_add(1));
        let text = &bytes[index..end];
        let next = if scan.quote != 0 {
            memchr::memchr2(scan.quote, b'<', text)
        } else {
            let syntax = memchr::memchr3(b'>', b'\'', b'"', text);
            memchr::memchr(b'<', &text[..syntax.unwrap_or(text.len())]).or(syntax)
        };
        let Some(next) = next else {
            if end > limit {
                return Err((ErrorKind::LimitExceeded, limit));
            }
            index = end;
            break;
        };
        index += next;
        if bytes[index] == b'<' {
            return Err((ErrorKind::InvalidToken, index));
        }
        if index >= limit {
            return Err((ErrorKind::LimitExceeded, limit));
        }
        if scan.quote != 0 {
            scan.quote = 0;
        } else if bytes[index] == b'>' {
            return Ok(Some(index + 1));
        } else {
            scan.quote = bytes[index];
        }
        index += 1;
    }
    scan.checked = index;
    if bytes.len() > limit {
        Err((ErrorKind::LimitExceeded, limit))
    } else {
        Ok(None)
    }
}

/// Advance XML line and character coordinates over decoded UTF-8.
///
/// A line break discards the preceding column, so only count characters after
/// the last break. Byte searches can skip UTF-8 spans because CR and LF cannot
/// occur inside a multibyte character. Keep CR state across empty input and
/// consumption boundaries so a split CRLF remains one line break.
#[inline(always)]
fn advance_position(text: &str, line: &mut usize, column: &mut usize, previous_cr: &mut bool) {
    // Short tokens count ordinary ASCII words directly, then scan the suffix
    // once instead of separately searching for breaks and counting characters.
    if text.len() <= 128 {
        let mut rest = text;
        while let Some(chunk) = rest.as_bytes().first_chunk::<8>() {
            let word = u64::from_le_bytes(*chunk);
            let high = 0x8080_8080_8080_8080;
            let low = 0x0101_0101_0101_0101;
            let cr = word ^ 0x0d0d_0d0d_0d0d_0d0d;
            let lf = word ^ 0x0a0a_0a0a_0a0a_0a0a;
            // An ASCII word without CR/LF advances exactly eight columns.
            // A possible special byte leaves the complete suffix to the scalar
            // path, so word-borrow false positives only stop this shortcut.
            if (word | (cr.wrapping_sub(low) & !cr) | (lf.wrapping_sub(low) & !lf)) & high != 0 {
                break;
            }
            *column += 8;
            *previous_cr = false;
            rest = &rest[8..];
        }
        for character in rest.chars() {
            match character {
                '\r' => {
                    *line += 1;
                    *column = 0;
                }
                '\n' => {
                    if !*previous_cr {
                        *line += 1;
                    }
                    *column = 0;
                }
                _ => *column += 1,
            }
            *previous_cr = character == '\r';
        }
        return;
    }
    advance_long_position(text, line, column, previous_cr);
}

fn advance_long_position(text: &str, line: &mut usize, column: &mut usize, previous_cr: &mut bool) {
    let mut rest = text;
    while let Some(index) = memchr::memchr2(b'\r', b'\n', rest.as_bytes()) {
        let cr = rest.as_bytes()[index] == b'\r';
        if cr || index != 0 || !*previous_cr {
            *line += 1;
        }
        *column = 0;
        *previous_cr = cr;
        rest = &rest[index + 1..];
    }
    if !rest.is_empty() {
        *column += rest.chars().count();
        *previous_cr = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialization_snapshots_publications_at_the_current_checkpoint() {
        for committed in [false, true] {
            let mut source = Source::new(Allocator::System, crate::NameRules::default());
            source.text.try_push_str("a\r\nb").unwrap();
            let identity = NativeLocation {
                generation: 1,
                byte_index: 0,
                byte_count: 1,
            };
            let expected = source.position(1);
            source.begin_prospective(identity);
            if committed {
                // A deliverable Text prefix can commit before failed accounting
                // advances the source, leaving its start exactly at q == cursor.
                source.commit_prospective(identity);
            }
            source.materialize_coordinates();
            let point = if committed {
                source.committed
            } else {
                source.prospective
            }
            .unwrap();
            assert!(
                matches!(point.coordinates, NativeCoordinates::Resolved(position) if position == expected)
            );
            if !committed {
                source.commit_prospective(identity);
            }
            source.consume(4);
            assert_eq!(source.published_position(identity), expected);
            assert_eq!(
                source.position(0),
                Position {
                    byte_index: 4,
                    byte_count: 0,
                    line: 2,
                    column: 1
                }
            );
        }
    }

    #[test]
    fn lazy_compaction_resolves_both_starts_without_committing_the_candidate() {
        for commit in [false, true] {
            let mut source = Source::new(Allocator::System, crate::NameRules::default());
            source.text.try_push_str(&"x".repeat(65_530)).unwrap();
            source.text.try_push_str("a\r\néz").unwrap();
            source.consume(65_530);
            let a = NativeLocation {
                generation: 1,
                byte_index: 65_530,
                byte_count: 2,
            };
            let expected_a = source.position(2);
            source.begin_prospective(a);
            source.consume(2);
            source.commit_prospective(a);
            assert_eq!(source.coordinate_cursor, 65_530);
            assert_eq!(source.published_position(a), expected_a);
            // The CR is still pending at the checkpoint. Both projection APIs
            // must carry its state into the LF without counting a second line.
            let mut cursor = source.position_cursor();
            assert_eq!(
                source.position_from_cursor(&mut cursor, 1, 2),
                source.position_at(1, 2)
            );
            assert_eq!(cursor.position.line, 2);
            let b = NativeLocation {
                generation: 1,
                byte_index: 65_532,
                byte_count: 4,
            };
            let expected_b = source.position(4);
            source.begin_prospective(b);
            source.consume(4); // Existing 64 KiB compaction discards A and B bytes.
            assert_eq!(source.cursor, 0);
            assert_eq!(source.coordinate_cursor, 0);
            assert!(source.text.is_empty());
            assert_eq!(source.published_position(a), expected_a);
            assert_eq!(
                source.position(0),
                Position {
                    byte_index: 65_536,
                    byte_count: 0,
                    line: 2,
                    column: 2,
                }
            );
            if commit {
                source.commit_prospective(b);
                assert_eq!(source.resolve_published(b), expected_b);
            } else {
                // This is also entry recovery after a caught host panic. B was
                // resolved before compaction but never became a public event.
                source.recover_prospective();
                assert!(source.prospective.is_none());
                assert_eq!(source.resolve_published(a), expected_a);
            }
        }
        eprintln!(
            "lazy layouts Source={} Parser={} AdapterFrame={}",
            size_of::<Source>(),
            size_of::<crate::Parser>(),
            size_of::<crate::AdapterFrame>()
        );
    }

    #[test]
    fn converted_positions_fit_at_the_source_bound() {
        let bound = isize::MAX as usize;
        let text = "é\r\n雪";
        for (encoding, widths) in [
            (Encoding::Utf8, [2_u8, 1, 1, 3]),
            (Encoding::Utf16Le, [2, 2, 2, 2]),
            (Encoding::Utf16Be, [2, 2, 2, 2]),
            (Encoding::SingleByte, [1, 1, 1, 1]),
            (Encoding::MultiByte, [3, 1, 1, 4]),
        ] {
            let raw_bytes = widths
                .iter()
                .map(|width| usize::from(*width))
                .sum::<usize>();
            let mut source = Source::new(Allocator::System, crate::NameRules::default());
            source.encoding = encoding;
            source.raw_index = bound - raw_bytes;
            source.accounted_raw = source.raw_index;
            source.line = source.raw_index + 1;
            source.decoded_end = source.position(0);
            if encoding == Encoding::MultiByte {
                for (character, width) in text.chars().zip(widths) {
                    source.push_custom(character, width, None).unwrap();
                }
            } else {
                source.text.try_push_str(text).unwrap();
            }
            let mut cursor = source.position_cursor();
            let mut raw_offset = bound - raw_bytes;
            for ((offset, character), width) in text.char_indices().zip(widths) {
                let position =
                    source.position_from_cursor(&mut cursor, offset, character.len_utf8());
                assert_eq!(position, source.position_at(offset, character.len_utf8()));
                assert_eq!(position.byte_index, raw_offset);
                assert_eq!(position.byte_count, usize::from(width));
                raw_offset += usize::from(width);
            }
            let end = source.position_from_cursor(&mut cursor, text.len(), 0);
            assert_eq!(end.byte_index, bound);
            assert_eq!(end.line, bound - raw_bytes + 2);
            assert_eq!(end.column, 1);
            if encoding == Encoding::MultiByte {
                assert_eq!(source.decoded_end, end);
            }
            assert_eq!(source.accounting_bytes(text.len()), raw_bytes);
            source.mark_accounted(raw_bytes);
            assert_eq!(source.accounted_raw, bound);
            source.consume(text.len());
            assert_eq!(source.position(0), end);
        }
    }

    #[test]
    fn compaction_and_entity_anchors_keep_bounded_coordinates() {
        let bound = isize::MAX as usize;
        let mut source = Source::new(Allocator::System, crate::NameRules::default());
        source.text.try_push_str(&"\n".repeat(65_537)).unwrap();
        source.raw_index = bound - source.text.len();
        source.accounted_raw = source.raw_index;
        source.line = source.raw_index + 1;
        source.consume(65_536);
        assert_eq!(source.cursor, 0);
        assert_eq!(source.text.as_str(), "\n");
        assert_eq!(source.raw_index, bound - 1);
        source.consume(1);
        assert_eq!(source.raw_index, bound);
        assert_eq!(source.line, bound + 1);

        let anchor = source.position(0);
        let mut entity = Source::entity(
            String::try_from_str_in("é\nx", Allocator::System).unwrap(),
            String::try_from_str_in("e", Allocator::System).unwrap(),
            anchor,
            0,
            crate::NameRules::default(),
        );
        let mut cursor = entity.position_cursor();
        assert_eq!(entity.position_at(2, 1), anchor);
        assert_eq!(entity.position_from_cursor(&mut cursor, 4, 0), anchor);
        entity.consume(4);
        assert_eq!(entity.raw_index, 4);
        assert_eq!(entity.position(0), anchor);
    }

    #[test]
    fn short_position_words_match_scalar_characters_and_cr_state() {
        fn check(text: &str) {
            for initial_cr in [false, true] {
                let mut expected = (17, 29, initial_cr);
                for character in text.chars() {
                    match character {
                        '\r' => {
                            expected.0 += 1;
                            expected.1 = 0;
                        }
                        '\n' => {
                            if !expected.2 {
                                expected.0 += 1;
                            }
                            expected.1 = 0;
                        }
                        _ => expected.1 += 1,
                    }
                    expected.2 = character == '\r';
                }
                let mut actual = (17, 29, initial_cr);
                advance_position(text, &mut actual.0, &mut actual.1, &mut actual.2);
                assert_eq!(actual, expected, "{text:?}, previous CR={initial_cr}");
            }
        }
        for length in [0, 1, 7, 8, 9, 15, 16, 17, 63, 64, 65, 127, 128, 129] {
            check(&"x".repeat(length));
            for offset in 0..length {
                for byte in 0..=127 {
                    let mut text = vec![b'x'; length];
                    text[offset] = byte;
                    check(std::str::from_utf8(&text).unwrap());
                }
                for special in ["\r\n", "\n\r", "\r\r", "\n\n", "é", "雪", "😀"] {
                    check(&format!(
                        "{}{special}{}",
                        "x".repeat(offset),
                        "y".repeat(length - offset)
                    ));
                }
            }
        }
        // Every valid Unicode scalar can occur on either side of a word boundary.
        for character in (0..=0x10ffff).filter_map(char::from_u32) {
            check(&format!("xxxxxxx{character}xxxxxxxx\r\ntail"));
        }
        // Adjacent bytes can carry a subtraction borrow between lanes.
        for first in 0..=127 {
            for second in 0..=127 {
                for offset in 0..8 {
                    let mut text = *b"abcdefghijklmnop";
                    text[offset] = first;
                    text[offset + 1] = second;
                    check(std::str::from_utf8(&text).unwrap());
                }
            }
        }
    }

    #[test]
    fn monotonic_positions_match_source_coordinates_and_anchors() {
        let text = "prefix\r\nπ\r雪\n😀 tail";
        for encoding in [
            Encoding::Utf8,
            Encoding::Utf16Le,
            Encoding::Utf16Be,
            Encoding::SingleByte,
            Encoding::MultiByte,
        ] {
            let mut source = Source::new(Allocator::System, crate::NameRules::default());
            source.text.push_str(text).unwrap();
            source.encoding = encoding;
            source.raw_index = 17;
            source.line = 3;
            source.column = 4;
            source.previous_cr = true;
            if encoding == Encoding::MultiByte {
                for character in text.chars() {
                    try_extend_from_slice(&mut source.raw_widths, &[3]).unwrap();
                    for _ in 1..character.len_utf8() {
                        try_extend_from_slice(&mut source.raw_widths, &[0]).unwrap();
                    }
                }
            }
            let mut cursor = source.position_cursor();
            for (offset, character) in text.char_indices() {
                for count in [0, character.len_utf8()] {
                    assert_eq!(
                        source.position_from_cursor(&mut cursor, offset, count),
                        source.position_at(offset, count)
                    );
                }
            }
            assert_eq!(
                source.position_from_cursor(&mut cursor, text.len(), 0),
                source.position_at(text.len(), 0)
            );
            let anchor = Position {
                byte_index: 99,
                line: 7,
                column: 8,
                byte_count: 4,
            };
            source.anchor = Some(anchor);
            let mut cursor = source.position_cursor();
            for (offset, character) in text.char_indices() {
                assert_eq!(
                    source.position_from_cursor(&mut cursor, offset, character.len_utf8()),
                    anchor
                );
            }
        }
    }

    #[test]
    fn tag_limits_preserve_less_than_precedence_across_feeds() {
        for (text, limit, expected) in [
            ("<r a='x<y'/>", 6, Err((ErrorKind::LimitExceeded, 6))),
            ("<r a='x<y'/>", 7, Err((ErrorKind::InvalidToken, 7))),
            ("<r a='x<y'/>", 8, Err((ErrorKind::InvalidToken, 7))),
            ("<r a='xy'/>", 8, Err((ErrorKind::LimitExceeded, 8))),
            ("<r a='xy'/>", 10, Err((ErrorKind::LimitExceeded, 10))),
            ("<r a='xy'/>", 11, Ok(Some(11))),
            ("<r abc<tail", 6, Err((ErrorKind::InvalidToken, 6))),
        ] {
            for width in 1..=text.len() {
                let mut scan = Scan::default();
                let mut result = Ok(None);
                for end in (width..text.len() + width).step_by(width) {
                    result =
                        scan_element_tag(&text.as_bytes()[..end.min(text.len())], &mut scan, limit);
                    if result != Ok(None) {
                        break;
                    }
                }
                assert_eq!(result, expected, "{text:?}, width {width}, limit {limit}");
            }
        }
    }

    #[test]
    fn tag_prefix_boundary_preserves_limits_inside_quoted_values() {
        for boundary in [31, 32, 33, 63, 64, 65, 127, 128] {
            let text = format!("<r a='{}<tail'/>", "x".repeat(boundary - 6));
            for limit in [boundary - 1, boundary, boundary + 1] {
                let expected = if limit < boundary {
                    Err((ErrorKind::LimitExceeded, limit))
                } else {
                    Err((ErrorKind::InvalidToken, boundary))
                };
                for width in [1, 3, 31, 32, 33, 63, 64, 65, text.len()] {
                    let mut scan = Scan::default();
                    let mut result = Ok(None);
                    for end in (width..text.len() + width).step_by(width) {
                        result = scan_element_tag(
                            &text.as_bytes()[..end.min(text.len())],
                            &mut scan,
                            limit,
                        );
                        if result != Ok(None) {
                            break;
                        }
                    }
                    assert_eq!(
                        result, expected,
                        "boundary {boundary}, width {width}, limit {limit}"
                    );
                }
            }
        }
    }
}
