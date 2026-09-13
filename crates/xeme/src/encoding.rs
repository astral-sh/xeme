use crate::{DeclarationContext, Error, ErrorKind, Position, ScanMode};
use xeme_storage::{Allocator, Box, String, Vec, try_box, try_extend_from_slice};

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
struct Detection {
    requested: Option<String>,
    declaration_checked: usize,
    unknown_name: Option<String>,
    encoding_error_position: Option<Position>,
}

impl Detection {
    fn detect(
        &mut self,
        bytes: &[u8],
        allocator: Allocator,
        final_input: bool,
        max_token: usize,
        external_content: bool,
        declaration_context: DeclarationContext,
    ) -> Result<Option<(Encoding, usize)>, Error> {
        // Latin-1 content can commit ordinary bytes immediately; only a leading
        // zero or markup opener can still select a UTF-16 signature.
        if external_content
            && self.requested.as_deref().and_then(Encoding::named) == Some(Encoding::Latin1)
            && bytes.first().is_some_and(|byte| !matches!(byte, 0 | b'<'))
        {
            return Ok(Some((Encoding::Latin1, 0)));
        }
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
        let requested = self.requested.as_deref().map(|name| {
            // Expat uses big-endian UTF-16 until a signature selects its order.
            if name.eq_ignore_ascii_case("UTF-16") {
                Some(Encoding::Utf16Be)
            } else {
                Encoding::named(name)
            }
        });
        if requested == Some(None) {
            self.unknown_name = self.requested.as_ref().map(String::try_clone).transpose()?;
            return Err(Error::bare(
                ErrorKind::UnknownEncoding,
                "unsupported input encoding",
            ));
        }
        let requested = requested.flatten();
        let content_latin1 = external_content && requested == Some(Encoding::Latin1);
        let content_utf16 =
            external_content && matches!(requested, Some(Encoding::Utf16Le | Encoding::Utf16Be));
        // Unicode signatures select the initial decoder even with a built-in
        // protocol encoding. External content preserves signatures that can be
        // ordinary data in its explicitly selected Latin-1 or UTF-16 encoding.
        let (detected, skip) =
            if bytes.starts_with(&[0xef, 0xbb, 0xbf]) && !content_latin1 && !content_utf16 {
                (Some(Encoding::Utf8), 3)
            } else if bytes.starts_with(&[0xff, 0xfe]) && !content_latin1 {
                (Some(Encoding::Utf16Le), 2)
            } else if bytes.starts_with(&[0xfe, 0xff]) && !content_latin1 {
                (Some(Encoding::Utf16Be), 2)
            } else if bytes.first() == Some(&0)
                && bytes.len() >= 2
                && !(external_content && requested == Some(Encoding::Utf16Le))
            {
                (Some(Encoding::Utf16Be), 0)
            } else if bytes.get(1) == Some(&0)
                && (!external_content || (bytes[0] == b'<' && requested != Some(Encoding::Utf16Be)))
            {
                (Some(Encoding::Utf16Le), 0)
            } else {
                (None, 0)
            };
        if let Some(requested) = requested {
            return Ok(Some((detected.unwrap_or(requested), skip)));
        }
        let sniffed = detected.unwrap_or(Encoding::Utf8);
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
            // Enforce the complete token bound before inspecting or copying an
            // encoding name, including when the closing delimiter just arrived.
            if end.map_or(content.len(), |end| end + 2) > max_token {
                return Err(Error::bare(
                    ErrorKind::LimitExceeded,
                    "XML declaration byte limit exceeded",
                ));
            }
            let Some(end) = end else {
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
                        if encoding.is_none()
                            && !mismatch
                            && declaration
                                .bytes()
                                .all(|byte| required_ascii(i32::from(byte)))
                            && crate::malformed_ascii_declaration(
                                &declaration[5..],
                                declaration_context,
                            )
                        {
                            // These bytes are invariant under every custom map.
                            // Let ordinary token parsing publish the grammar error before any
                            // unknown-encoding callback; never decode a valid
                            // unknown encoding by assuming UTF-8.
                            return Ok(Some((Encoding::Utf8, skip)));
                        }
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
                            self.unknown_name = Some(String::try_from_str_in(name, allocator)?);
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
}

#[derive(Debug)]
pub(crate) struct Decoder {
    allocator: Allocator,
    encoding: Option<Encoding>,
    detection: Detection,
    pending: Vec<u8>,
    pending_cursor: usize,
    custom_map: Option<Box<[i32; 256]>>,
    conversion: Option<([u8; 4], u8)>,
}
impl Decoder {
    pub(crate) fn new(requested: Option<&str>, allocator: Allocator) -> Result<Self, Error> {
        Ok(Self {
            allocator,
            encoding: None,
            detection: Detection {
                requested: requested
                    .map(|name| String::try_from_str_in(name, allocator))
                    .transpose()?,
                declaration_checked: 0,
                unknown_name: None,
                encoding_error_position: None,
            },
            pending: Vec::new_in(allocator),
            pending_cursor: 0,
            custom_map: None,
            conversion: None,
        })
    }
    /// Replace only the protocol name after the adapter has stopped parsing.
    pub(crate) fn set_completed_encoding(&mut self, requested: Option<&str>) -> Result<(), Error> {
        let requested = requested
            .map(|name| String::try_from_str_in(name, self.allocator))
            .transpose()?;
        self.detection.requested = requested;
        Ok(())
    }

    /// A complete BOM can precede enough input to choose the declaration's
    /// encoding. Report only that recognized prefix for consumed-byte accounting.
    pub(crate) fn pending_bom_len(&self, external_content: bool) -> usize {
        self.bom_len(&self.pending, external_content)
    }

    fn bom_len(&self, bytes: &[u8], external_content: bool) -> usize {
        if self.encoding.is_some()
            // An explicit unknown protocol encoding must first be resolved by
            // its handler; these bytes have not yet been recognized as a BOM.
            || self.detection.requested.as_deref().is_some_and(|name| {
                Encoding::named(name).is_none() && !name.eq_ignore_ascii_case("UTF-16")
            })
            || (external_content
                && self.detection.requested.as_deref().and_then(Encoding::named) == Some(Encoding::Latin1))
        {
            return 0;
        }
        if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
            if external_content
                && self.detection.requested.as_deref().is_some_and(|name| {
                    name.eq_ignore_ascii_case("UTF-16")
                        || matches!(
                            Encoding::named(name),
                            Some(Encoding::Utf16Le | Encoding::Utf16Be)
                        )
                })
            {
                0
            } else {
                3
            }
        } else if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
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
        declaration_context: DeclarationContext,
    ) -> Result<(), Error> {
        if self.encoding == Some(Encoding::Utf8)
            && self.conversion.is_none()
            && self.pending.is_empty()
            && self.pending_cursor == 0
            && self.pending.capacity() >= bytes.len()
            && let Ok(text) = std::str::from_utf8(bytes)
        {
            // The ordinary pending append cannot allocate in this state. Copy
            // directly into the source, retaining its existing growth policy.
            if let Err(error) = source
                .text
                .try_push_str_with_minimum(text, max_token.min(1024))
            {
                // Preserve the ordinary failure state. Empty length and the
                // checked capacity make this restoration allocation-free.
                try_extend_from_slice(&mut self.pending, bytes)?;
                return Err(error.into());
            }
            return Ok(());
        }
        try_extend_from_slice(&mut self.pending, bytes)?;
        if self.conversion.is_some() {
            return Ok(());
        }
        if self.encoding.is_none() {
            let Some((encoding, skip)) = self.detection.detect(
                &self.pending,
                self.allocator,
                final_input,
                max_token,
                external_content,
                declaration_context,
            )?
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

    /// A parameter child is read once its protocol encoding is known, even for
    /// empty non-final input that does not complete byte-order detection.
    pub(crate) fn protocol_encoding_ready(&self) -> bool {
        self.encoding.is_some()
            || self.detection.requested.as_deref().is_none_or(|name| {
                Encoding::named(name).is_some() || name.eq_ignore_ascii_case("UTF-16")
            })
    }

    pub(crate) fn unknown_encoding(&self) -> Option<&str> {
        self.detection.unknown_name.as_deref()
    }
    pub(crate) fn encoding_error_position(&self) -> Option<Position> {
        self.detection.encoding_error_position
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
            .detection
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
        if self.detection.requested.is_some() && self.pending.starts_with(&[0xef, 0xbb, 0xbf]) {
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
            .zip(self.detection.unknown_name.as_deref())
            .is_some_and(|(requested, name)| requested.eq_ignore_ascii_case(name))
        {
            (
                self.detection
                    .unknown_name
                    .as_ref()
                    .map_or(0, |name| name.len()),
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
            .detection
            .requested
            .as_deref()
            .zip(parent.detection.unknown_name.as_deref())
            .is_some_and(|(name, parent)| name.eq_ignore_ascii_case(parent))
        {
            self.custom_map = parent
                .custom_map
                .as_ref()
                .map(|map| try_box(**map, self.allocator))
                .transpose()?;
            self.detection.unknown_name = parent
                .detection
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
            .detection
            .unknown_name
            .as_deref()
            .is_some_and(|custom| custom.eq_ignore_ascii_case(name))
        {
            return Ok(());
        }
        // An explicitly supplied encoding takes precedence over the declaration.
        if self.detection.requested.is_some() {
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

/// Adapter raw context uses the native Source allocation until conversion or a
/// non-UTF-8 feed requires the ordinary separate representation.
#[derive(Debug)]
pub(crate) struct InputContext {
    raw: Vec<u8>,
    start: usize,
    physical_start: usize,
    native: bool,
}

impl InputContext {
    pub(crate) fn new(allocator: Allocator, decoder: &Decoder, source: &mut Source) -> Self {
        let native = decoder
            .encoding
            .is_none_or(|encoding| encoding == Encoding::Utf8)
            && decoder.pending.is_empty()
            && decoder.pending_cursor == 0
            && decoder.conversion.is_none();
        source.retain_native_input = native;
        Self {
            raw: Vec::new_in(allocator),
            start: 0,
            physical_start: 0,
            native,
        }
    }

    pub(crate) fn view<'a>(&'a self, source: &'a Source) -> (&'a [u8], usize) {
        let bytes = if self.native {
            &source.text.as_bytes()[self.start - self.physical_start..]
        } else {
            &self.raw
        };
        (bytes, self.start)
    }

    /// Publish raw input before decoder errors or callbacks. Native appends also
    /// supply decoded storage, but detection keeps those bytes hidden by cursor.
    pub(crate) fn preserve(
        &mut self,
        decoder: &mut Decoder,
        source: &mut Source,
        input: &[u8],
        retain_from: usize,
        minimum: usize,
    ) -> Result<bool, xeme_storage::AllocError> {
        let discard = retain_from
            .saturating_sub(self.start)
            .min(self.view(source).0.len());
        self.start += discard;
        if self.native {
            let physical = source
                .text
                .floor_char_boundary(self.start - self.physical_start);
            debug_assert!(physical <= source.cursor);
            source.text.discard_prefix(physical);
            source.cursor -= physical;
            self.physical_start += physical;
            if let Ok(text) = std::str::from_utf8(input) {
                source.text.try_push_str_with_minimum(text, minimum)?;
                if decoder.encoding.is_none() {
                    source.cursor = source.text.len();
                }
                return Ok(true);
            }
            self.separate(decoder, source, input, minimum)?;
        } else {
            self.raw.drain(..discard);
            if !input.is_empty() && self.raw.capacity() == 0 {
                self.raw.try_reserve(input.len().max(minimum))?;
            }
            try_extend_from_slice(&mut self.raw, input)?;
        }
        Ok(false)
    }

    /// Commit a one-way fallback only after its raw owner and any undecoded
    /// staging bytes have been allocated. Known UTF-8 Source bytes stay intact.
    fn separate(
        &mut self,
        decoder: &mut Decoder,
        source: &mut Source,
        input: &[u8],
        minimum: usize,
    ) -> Result<(), xeme_storage::AllocError> {
        let history = self.view(source).0;
        let length = history
            .len()
            .checked_add(input.len())
            .ok_or(xeme_storage::AllocError::CapacityOverflow)?;
        let mut raw = Vec::new_in(decoder.allocator);
        if length != 0 {
            raw.try_reserve(length.max(minimum))?;
        }
        try_extend_from_slice(&mut raw, history)?;
        try_extend_from_slice(&mut raw, input)?;
        if decoder.encoding.is_none() {
            // Source contains only detection staging: pending must own every
            // earlier byte before that hidden view is cleared.
            try_extend_from_slice(&mut decoder.pending, source.text.as_bytes())?;
            source.text.clear();
            source.cursor = 0;
        }
        self.raw = raw;
        self.native = false;
        source.retain_native_input = false;
        Ok(())
    }

    pub(crate) fn decode(
        &mut self,
        decoder: &mut Decoder,
        source: &mut Source,
        final_input: bool,
        max_token: usize,
        external_content: bool,
        declaration_context: DeclarationContext,
    ) -> Result<(), Error> {
        debug_assert!(self.native);
        if decoder.encoding == Some(Encoding::Utf8) {
            debug_assert!(decoder.pending.is_empty() && decoder.conversion.is_none());
            return Ok(());
        }
        let detected = decoder.detection.detect(
            source.text.as_bytes(),
            decoder.allocator,
            final_input,
            max_token,
            external_content,
            declaration_context,
        );
        match detected {
            Ok(None) => Ok(()),
            Ok(Some((Encoding::Utf8, skip))) => {
                decoder.encoding = Some(Encoding::Utf8);
                source.encoding = Encoding::Utf8;
                source.raw_index = skip;
                source.column = usize::from(skip != 0);
                source.cursor = skip;
                Ok(())
            }
            result => {
                self.separate(decoder, source, &[], max_token.min(1024))?;
                let Some((encoding, skip)) = result? else {
                    unreachable!()
                };
                // Reuse the completed detection result. In particular, do not
                // rescan a declaration after its progress cursor advanced.
                decoder.encoding = Some(encoding);
                source.encoding = encoding;
                source.raw_index = skip;
                source.column = usize::from(skip != 0);
                decoder.pending.drain(..skip);
                decoder.feed(
                    &[],
                    final_input,
                    source,
                    max_token,
                    external_content,
                    declaration_context,
                )
            }
        }
    }

    pub(crate) fn pending_bom_len(
        &self,
        decoder: &Decoder,
        source: &Source,
        external_content: bool,
    ) -> usize {
        if self.native {
            decoder.bom_len(source.text.as_bytes(), external_content)
        } else {
            decoder.pending_bom_len(external_content)
        }
    }
}

/// Original-input coordinates are bounded by `Parser::input_bytes_remaining`.
/// Internal entities own a finite UTF-8 buffer and return the referring source's
/// anchor without adding replacement offsets. Compaction changes buffer offsets,
/// never the cumulative original-byte index or one-based line number.
#[derive(Debug)]
pub(crate) struct Source {
    pub(crate) text: crate::lexical::Buffer,
    cursor: usize,
    retain_native_input: bool,
    encoding: Encoding,
    raw_index: usize,
    accounted_raw: usize,
    raw_widths: Vec<u8>,
    decoded_end: Position,
    decoded_end_cr: bool,
    line: usize,
    column: usize,
    previous_cr: bool,
    anchor: Option<Position>,
    pub(crate) initial_depth: usize,
    pub(crate) entity_name: Option<String>,
    pub(crate) dtd_fragment: bool,
    scan: Scan,
    deferred_size: usize,
    name_rules: crate::NameRules,
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
            retain_native_input: false,
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
                .map_err(xeme_storage::AllocError::from)?;
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
    pub(crate) fn position(&self, count: usize) -> Position {
        if let Some(anchor) = self.anchor {
            return anchor;
        }
        Position {
            byte_index: self.raw_index,
            line: self.line,
            column: self.column,
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
        let mut position = self.position(offset);
        position.byte_index += position.byte_count;
        position.byte_count = 0;
        let mut previous_cr = self.previous_cr;
        advance_position(
            &self.remaining()[..offset],
            &mut position.line,
            &mut position.column,
            &mut previous_cr,
        );
        position.byte_count = self.raw_len(offset, count);
        position
    }

    pub(crate) fn position_cursor(&self) -> PositionCursor {
        PositionCursor {
            offset: 0,
            position: self.position(0),
            previous_cr: self.previous_cr,
        }
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
        self.raw_index += self.raw_len(0, count);
        let text = &self.text[self.cursor..self.cursor + count];
        advance_position(
            text,
            &mut self.line,
            &mut self.column,
            &mut self.previous_cr,
        );
        self.finish_consume(count);
    }

    /// Consume a nonempty native ASCII tag proven to contain no line breaks.
    /// The caller must complete semantic work and accounting before this mutation.
    pub(crate) fn consume_ascii_tag(&mut self, count: usize) {
        debug_assert!(self.native_utf8_byte_index().is_some() && !self.has_conversions());
        debug_assert!(
            count > 0
                && self.remaining()[..count]
                    .bytes()
                    .all(|byte| byte.is_ascii() && !matches!(byte, b'\r' | b'\n'))
        );
        self.raw_index += self.raw_len(0, count);
        self.column += count;
        self.previous_cr = false;
        self.finish_consume(count);
    }

    /// Commit native text whose scanner already computed the eager position delta.
    pub(crate) fn consume_text(&mut self, plan: crate::text::TextPlan) {
        debug_assert!(self.native_utf8_byte_index().is_some() && !self.has_conversions());
        self.raw_index += self.raw_len(0, plan.end);
        plan.advance_position(&mut self.line, &mut self.column, &mut self.previous_cr);
        self.finish_consume(plan.end);
    }

    /// Retire consumed input and scanning state after coordinates are committed.
    fn finish_consume(&mut self, count: usize) {
        self.cursor += count;
        self.scan = Scan::default();
        self.deferred_size = 0;
        if !self.retain_native_input
            && self.cursor >= 64 * 1024
            && self.cursor >= self.text.len() / 2
        {
            self.text.discard_prefix(self.cursor);
            if self.encoding == Encoding::MultiByte {
                self.raw_widths.drain(..self.cursor);
            }
            self.cursor = 0;
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
    fn warmed_utf8_feeds_match_buffered_splits_and_errors() {
        for bytes in [
            b"".as_slice(),
            b"plain ASCII\r\n<r/>",
            b"raw\0\x01\x7f<&",
            "\n\u{feff}é雪😀<r/>".as_bytes(),
            b"prefix\xfftail",
            b"prefix\xf0\x9f",
        ] {
            for split in 0..=bytes.len() {
                for warm in [false, true] {
                    let make = || {
                        let mut decoder = Decoder::new(None, Allocator::System).unwrap();
                        decoder.encoding = Some(Encoding::Utf8);
                        if warm {
                            decoder.append_pending(&[0; 64]).unwrap();
                            decoder.pending.clear();
                        }
                        let mut source =
                            Source::new(Allocator::System, crate::NameRules::default());
                        source.text.try_push_str("head\r").unwrap();
                        source.consume(5);
                        (decoder, source)
                    };
                    let (mut direct, mut direct_source) = make();
                    let (mut buffered, mut buffered_source) = make();
                    for (part, final_input) in [(&bytes[..split], false), (&bytes[split..], true)] {
                        // A nonempty pending buffer selects the unchanged path.
                        buffered.append_pending(part).unwrap();
                        let expected = buffered.feed(
                            &[],
                            final_input,
                            &mut buffered_source,
                            64,
                            false,
                            DeclarationContext::Document,
                        );
                        let actual = direct.feed(
                            part,
                            final_input,
                            &mut direct_source,
                            64,
                            false,
                            DeclarationContext::Document,
                        );
                        assert_eq!(actual, expected, "{bytes:?}, split {split}, warm {warm}");
                        assert_eq!(direct.pending, buffered.pending);
                        assert_eq!(direct.pending_cursor, buffered.pending_cursor);
                        assert_eq!(direct.encoding, buffered.encoding);
                        assert_eq!(direct_source.text.as_str(), buffered_source.text.as_str());
                        assert_eq!(direct_source.position(0), buffered_source.position(0));
                        assert_eq!(direct_source.end_position(), buffered_source.end_position());
                        assert_eq!(direct_source.accounted_raw, buffered_source.accounted_raw);
                        if actual.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn warmed_utf8_source_failure_retains_pending_without_growth() {
        use xeme_storage::{AllocationTracker, with_tracking};

        for prefix in ["", "old"] {
            let tracker = AllocationTracker::try_new_in(Allocator::System).unwrap();
            with_tracking(&tracker, || {
                let allocator = Allocator::TrackedSystem;
                let mut decoder = Decoder::new(None, allocator).unwrap();
                decoder.encoding = Some(Encoding::Utf8);
                let bytes = [b'x'; 2048];
                decoder.append_pending(&bytes).unwrap();
                decoder.pending.clear();
                let pending_pointer = decoder.pending.as_ptr();
                let pending_capacity = decoder.pending.capacity();
                let mut source = Source::new(allocator, crate::NameRules::default());
                source.text.try_push_str(prefix).unwrap();
                let position = source.position(0);
                let live = tracker.live_bytes();
                // Reject the next backing allocation, including Source's first
                // block or growth. Restoring pending must not request one.
                tracker.set_activation_threshold(0);
                let error = decoder
                    .feed(
                        &bytes,
                        true,
                        &mut source,
                        1024,
                        false,
                        DeclarationContext::Document,
                    )
                    .unwrap_err();
                assert_eq!(error, Error::bare(ErrorKind::NoMemory, "out of memory"));
                assert_eq!(decoder.pending.as_slice(), bytes.as_slice());
                assert_eq!(decoder.pending_cursor, 0);
                assert_eq!(decoder.pending.as_ptr(), pending_pointer);
                assert_eq!(decoder.pending.capacity(), pending_capacity);
                assert_eq!(source.text.as_str(), prefix);
                assert_eq!(source.position(0), position);
                assert_eq!(tracker.live_bytes(), live);
                tracker.set_activation_threshold(u64::MAX);
                // The retained bytes are available to the ordinary decoder path.
                decoder
                    .feed(
                        &[],
                        true,
                        &mut source,
                        1024,
                        false,
                        DeclarationContext::Document,
                    )
                    .unwrap();
                assert!(decoder.pending.is_empty());
                assert_eq!(source.text.len(), prefix.len() + bytes.len());
                assert_eq!(
                    &source.text.as_str()[prefix.len()..],
                    std::str::from_utf8(&bytes).unwrap()
                );
            });
            assert_eq!(tracker.live_bytes(), 0);
        }
    }

    #[test]
    fn proven_ascii_tag_matches_eager_coordinates_and_compaction() {
        for tag in ["<r>", "<r/>", "<abcdefgh>", "<p:r/>"] {
            for prefix in [0, 65_530, 65_533] {
                for previous_cr in [false, true] {
                    let make_source = || {
                        let mut source =
                            Source::new(Allocator::System, crate::NameRules::default());
                        source.text.try_push_str(&"x".repeat(prefix)).unwrap();
                        source.text.try_push_str(tag).unwrap();
                        source.text.try_push_str("\nTAIL").unwrap();
                        source.cursor = prefix;
                        source.raw_index = prefix;
                        source.line = 7;
                        source.column = 11;
                        source.previous_cr = previous_cr;
                        source.scan.mode = Some(ScanMode::Tag);
                        source.deferred_size = 12;
                        source
                    };
                    let mut eager = make_source();
                    let mut proven = make_source();
                    eager.consume(tag.len());
                    proven.consume_ascii_tag(tag.len());
                    assert_eq!(proven.position(0), eager.position(0));
                    assert_eq!(proven.previous_cr, eager.previous_cr);
                    assert_eq!(proven.remaining(), eager.remaining());
                    assert_eq!(proven.cursor, eager.cursor);
                    assert_eq!(proven.cursor == 0, prefix + tag.len() >= 65_536);
                    assert!(proven.scan.mode.is_none());
                    assert_eq!(proven.deferred_size, 0);
                    // A prior CR must not absorb the LF after an intervening tag.
                    eager.consume(1);
                    proven.consume(1);
                    assert_eq!(proven.position(0), eager.position(0));
                    assert_eq!(
                        (proven.line, proven.column, proven.previous_cr),
                        (8, 0, false)
                    );
                }
            }
        }
    }

    #[test]
    fn planned_text_matches_eager_consumption_and_compaction() {
        for text in [
            "".to_string(),
            "\n".to_string(),
            "\nabc\t\n\nend\u{7f}".to_string(),
            "\nxxxxxx\nxxxxxxxx<tail>\n\n".to_string(),
            "xxxxxxx\nxxxxxxx\n\nxxxxxx\nxxxxxxxx<tail>".to_string(),
            format!("a\n{}<tail>", "x".repeat(65_534)),
            format!("{}<tail>", "\nxxxxxx\nxxxxxxxx".repeat(4096)),
        ] {
            for previous_cr in [false, true] {
                let make_source = || {
                    let mut source = Source::new(Allocator::System, crate::NameRules::default());
                    source.text.try_push_str(&text).unwrap();
                    source.line = 7;
                    source.column = 11;
                    source.previous_cr = previous_cr;
                    source.scan.mode = Some(ScanMode::Tag);
                    source.deferred_size = 12;
                    source
                };
                let mut original = make_source();
                let mut planned = make_source();
                let plan = crate::text::TextPlan::scan(&text).unwrap();
                original.consume(plan.end);
                planned.consume_text(plan);
                assert_eq!(planned.position(0), original.position(0));
                assert_eq!(planned.previous_cr, original.previous_cr);
                assert_eq!(planned.remaining(), original.remaining());
                assert_eq!(planned.cursor, original.cursor);
                assert!(planned.scan.mode.is_none());
                assert_eq!(planned.deferred_size, 0);
                if plan.end == 65_536 {
                    assert_eq!(planned.cursor, 0);
                    assert_eq!(planned.remaining(), "<tail>");
                    let expected = if text.starts_with("a\n") {
                        (8, 65_534)
                    } else {
                        (8199 - usize::from(previous_cr), 8)
                    };
                    assert_eq!((planned.line, planned.column), expected);
                }
            }
        }

        // Outer plans keep long noncoalesced lines intact and preserve an
        // incoming CR, following LF, and the ordinary compaction boundary.
        for length in [1, 15, 16, 17, 65_535, 65_536, 65_537] {
            let whitespace: std::string::String = " \t".chars().cycle().take(length).collect();
            let text = format!("{whitespace}\n<tail>");
            for previous_cr in [false, true] {
                let make_source = || {
                    let mut source = Source::new(Allocator::System, crate::NameRules::default());
                    source.text.try_push_str(&text).unwrap();
                    source.line = 7;
                    source.column = 11;
                    source.previous_cr = previous_cr;
                    source.scan.mode = Some(ScanMode::Tag);
                    source.deferred_size = 12;
                    source
                };
                let mut original = make_source();
                let mut planned = make_source();
                let plan = crate::text::TextPlan::scan_outer_whitespace(&text).unwrap();
                assert_eq!(plan.end, length);
                original.consume(length);
                planned.consume_text(plan);
                assert_eq!(planned.position(0), original.position(0));
                assert_eq!(planned.previous_cr, original.previous_cr);
                assert_eq!(planned.remaining(), original.remaining());
                assert_eq!(planned.cursor, original.cursor);
                assert!(planned.scan.mode.is_none());
                assert_eq!(planned.deferred_size, 0);
                assert_eq!((planned.line, planned.column), (7, 11 + length));
                original.consume(1);
                planned.consume(1);
                assert_eq!(planned.position(0), original.position(0));
                assert_eq!(
                    (planned.line, planned.column, planned.previous_cr),
                    (8, 0, false)
                );
            }
        }
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
