use crate::{ErrorKind, Position, ScanMode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Latin1,
    Ascii,
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
            Self::Latin1 => text.chars().count(),
            Self::Utf16Le | Self::Utf16Be => text.chars().map(char::len_utf16).sum::<usize>() * 2,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Decoder {
    encoding: Option<Encoding>,
    requested: Option<String>,
    pending: Vec<u8>,
    declaration_checked: usize,
}
impl Decoder {
    pub(crate) fn new(requested: Option<&str>) -> Self {
        Self {
            encoding: None,
            requested: requested.map(str::to_owned),
            pending: Vec::new(),
            declaration_checked: 0,
        }
    }
    pub(crate) fn feed(
        &mut self,
        bytes: &[u8],
        final_input: bool,
        source: &mut Source,
        max_token: usize,
    ) -> Result<(), (ErrorKind, &'static str)> {
        self.pending.extend_from_slice(bytes);
        if self.encoding.is_none() {
            let Some((encoding, skip)) = self.detect(final_input, max_token)? else {
                return Ok(());
            };
            self.encoding = Some(encoding);
            source.encoding = encoding;
            source.raw_index = skip;
            source.column = usize::from(skip != 0);
            self.pending.drain(..skip);
        }
        let encoding = self.encoding.expect("encoding was detected");
        let mut consumed = 0;
        match encoding {
            Encoding::Utf8 => match std::str::from_utf8(&self.pending) {
                Ok(text) => {
                    source.text.push_str(text);
                    consumed = self.pending.len();
                }
                Err(error) => {
                    consumed = error.valid_up_to();
                    source.text.push_str(
                        std::str::from_utf8(&self.pending[..consumed])
                            .expect("validated UTF-8 prefix"),
                    );
                    if error.error_len().is_some() {
                        return Err((ErrorKind::InvalidToken, "invalid UTF-8"));
                    }
                }
            },
            Encoding::Ascii | Encoding::Latin1 => {
                let end = if encoding == Encoding::Ascii {
                    self.pending
                        .iter()
                        .position(|byte| !byte.is_ascii())
                        .unwrap_or(self.pending.len())
                } else {
                    self.pending.len()
                };
                source
                    .text
                    .extend(self.pending[..end].iter().map(|byte| char::from(*byte)));
                if end < self.pending.len() {
                    return Err((ErrorKind::InvalidToken, "non-ASCII byte in ASCII document"));
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
                            return Err((ErrorKind::InvalidToken, "unpaired UTF-16 surrogate"));
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
                    source.text.push(
                        char::from_u32(codepoint)
                            .ok_or((ErrorKind::InvalidToken, "unpaired UTF-16 surrogate"))?,
                    );
                    consumed += width;
                }
            }
        }
        self.pending.drain(..consumed);
        if final_input && !self.pending.is_empty() {
            return Err((ErrorKind::PartialCharacter, "incomplete encoded character"));
        }
        Ok(())
    }

    fn detect(
        &mut self,
        final_input: bool,
        max_token: usize,
    ) -> Result<Option<(Encoding, usize)>, (ErrorKind, &'static str)> {
        let bytes = &self.pending;
        if bytes.len() < 4 && !final_input {
            return Ok(None);
        }
        let (sniffed, skip) = if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
            (Encoding::Utf8, 3)
        } else if bytes.starts_with(&[0xff, 0xfe]) {
            (Encoding::Utf16Le, 2)
        } else if bytes.starts_with(&[0xfe, 0xff]) {
            (Encoding::Utf16Be, 2)
        } else if bytes.starts_with(&[b'<', 0]) {
            (Encoding::Utf16Le, 0)
        } else if bytes.starts_with(&[0, b'<']) {
            (Encoding::Utf16Be, 0)
        } else {
            (Encoding::Utf8, 0)
        };
        if let Some(requested) = &self.requested {
            let requested = if requested.eq_ignore_ascii_case("UTF-16") {
                match sniffed {
                    Encoding::Utf16Le | Encoding::Utf16Be => sniffed,
                    _ => {
                        return Err((
                            ErrorKind::IncorrectEncoding,
                            "UTF-16 input requires a byte order mark or declaration",
                        ));
                    }
                }
            } else {
                Encoding::named(requested)
                    .ok_or((ErrorKind::UnknownEncoding, "unsupported input encoding"))?
            };
            if skip > 0 && sniffed != requested {
                return Err((
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
                    return Err((
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
                        let encoding = Encoding::named(name)
                            .ok_or((ErrorKind::UnknownEncoding, "unsupported declared encoding"))?;
                        if matches!(encoding, Encoding::Utf16Le | Encoding::Utf16Be)
                            || (skip > 0 && encoding != Encoding::Utf8)
                        {
                            return Err((
                                ErrorKind::IncorrectEncoding,
                                "declared encoding conflicts with input bytes",
                            ));
                        }
                        return Ok(Some((encoding, skip)));
                    }
                }
            }
        }
        Ok(Some((Encoding::Utf8, skip)))
    }

    pub(crate) fn check_declaration(&self, name: &str) -> Result<(), (ErrorKind, &'static str)> {
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
        let declared = Encoding::named(name)
            .ok_or((ErrorKind::UnknownEncoding, "unsupported declared encoding"))?;
        if declared != actual {
            return Err((
                ErrorKind::IncorrectEncoding,
                "declared encoding conflicts with detected encoding",
            ));
        }
        Ok(())
    }
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

#[derive(Debug)]
pub(crate) struct Source {
    pub(crate) text: String,
    cursor: usize,
    encoding: Encoding,
    raw_index: usize,
    line: usize,
    column: usize,
    previous_cr: bool,
    anchor: Option<Position>,
    pub(crate) initial_depth: usize,
    pub(crate) entity_name: Option<String>,
    scan: Scan,
}
impl Source {
    pub(crate) fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            encoding: Encoding::Utf8,
            raw_index: 0,
            line: 1,
            column: 0,
            previous_cr: false,
            anchor: None,
            initial_depth: 0,
            entity_name: None,
            scan: Scan::default(),
        }
    }
    pub(crate) fn entity(
        text: String,
        name: String,
        position: Position,
        initial_depth: usize,
    ) -> Self {
        Self {
            text,
            anchor: Some(position),
            initial_depth,
            entity_name: Some(name),
            ..Self::new()
        }
    }
    pub(crate) fn remaining(&self) -> &str {
        &self.text[self.cursor..]
    }
    pub(crate) fn position(&self, count: usize) -> Position {
        if let Some(anchor) = self.anchor {
            return anchor;
        }
        Position {
            byte_index: self.raw_index,
            line: self.line,
            column: self.column,
            byte_count: self.encoding.raw_len(&self.remaining()[..count]),
        }
    }
    pub(crate) fn end_position(&self) -> Position {
        let mut position = self.position(self.remaining().len());
        position.byte_index += position.byte_count;
        position.byte_count = 0;
        let mut previous_cr = self.previous_cr;
        for c in self.remaining().chars() {
            match c {
                '\r' => {
                    position.line += 1;
                    position.column = 0;
                    previous_cr = true;
                }
                '\n' => {
                    if !previous_cr {
                        position.line += 1;
                    }
                    position.column = 0;
                    previous_cr = false;
                }
                _ => {
                    position.column += 1;
                    previous_cr = false;
                }
            }
        }
        position
    }
    pub(crate) fn scan_reference(&mut self, limit: usize) -> Result<Option<usize>, ErrorKind> {
        let bytes = &self.text.as_bytes()[self.cursor..];
        for index in self.scan.checked.max(1)..bytes.len() {
            if index >= limit {
                return Err(ErrorKind::LimitExceeded);
            }
            match bytes[index] {
                b';' => return Ok(Some(index)),
                b'<' | b'&' | b'\'' | b'"' | b' ' | b'\t' | b'\r' | b'\n' | 0 => {
                    return Err(ErrorKind::InvalidToken);
                }
                _ => {}
            }
        }
        self.scan.checked = bytes.len();
        if bytes.len() > limit {
            Err(ErrorKind::LimitExceeded)
        } else {
            Ok(None)
        }
    }
    pub(crate) fn consume(&mut self, count: usize) {
        let text = &self.text[self.cursor..self.cursor + count];
        self.raw_index += self.encoding.raw_len(text);
        for character in text.chars() {
            match character {
                '\r' => {
                    self.line += 1;
                    self.column = 0;
                    self.previous_cr = true;
                }
                '\n' => {
                    if !self.previous_cr {
                        self.line += 1;
                    }
                    self.column = 0;
                    self.previous_cr = false;
                }
                _ => {
                    self.column += 1;
                    self.previous_cr = false;
                }
            }
        }
        self.cursor += count;
        self.scan = Scan::default();
        if self.cursor >= 64 * 1024 && self.cursor >= self.text.len() / 2 {
            self.text.drain(..self.cursor);
            self.cursor = 0;
        }
    }
    pub(crate) fn scan_token(
        &mut self,
        mode: ScanMode,
        limit: usize,
    ) -> Result<Option<usize>, ErrorKind> {
        let bytes = &self.text.as_bytes()[self.cursor..];
        if self.scan.mode != Some(mode) {
            self.scan = Scan {
                mode: Some(mode),
                ..Scan::default()
            };
        }
        let scan = &mut self.scan;
        match mode {
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
                        Err(ErrorKind::LimitExceeded)
                    };
                }
                scan.checked = bytes.len().saturating_sub(terminator.len() - 1);
            }
            ScanMode::Tag | ScanMode::Doctype => {
                let mut index = scan.checked;
                while index < bytes.len() {
                    if index >= limit {
                        return Err(ErrorKind::LimitExceeded);
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
                            b'[' if mode == ScanMode::Doctype => scan.brackets += 1,
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
            Err(ErrorKind::LimitExceeded)
        } else {
            Ok(None)
        }
    }
}
