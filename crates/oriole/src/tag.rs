//! Resumable lexical boundaries shared by complete and incremental tag parsing.

use oriole_storage::Vec;

use crate::names::is_xml_char;
use crate::{ErrorKind, NameRules, RawAttribute};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Space,
    Name,
    Equals,
    Quote,
    Value(u8),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AttributeScanner {
    phase: Phase,
    offset: usize,
    separated: bool,
    name_start: usize,
    name_end: usize,
    value_start: usize,
    count: usize,
    literal_ascii: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Failure {
    pub(crate) kind: ErrorKind,
    pub(crate) message: &'static str,
    pub(crate) offset: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Step {
    Attribute(RawAttribute),
    End,
    TagEnd { end: usize, empty: bool },
    Incomplete,
}

impl AttributeScanner {
    pub(crate) fn new(offset: usize) -> Self {
        Self {
            phase: Phase::Space,
            offset,
            separated: false,
            name_start: 0,
            name_end: 0,
            value_start: 0,
            count: 0,
            literal_ascii: true,
        }
    }

    /// Resume over an append-only UTF-8 input. Attribute ranges own no references.
    /// `tag_end` permits the closing syntax excluded by complete attribute views.
    pub(crate) fn next(
        &mut self,
        text: &str,
        final_input: bool,
        tag_end: bool,
        allow_refs: bool,
        limit: usize,
        rules: NameRules,
    ) -> Result<Step, Failure> {
        self.next_impl::<false>(text, final_input, tag_end, allow_refs, limit, rules)
    }

    /// Resume native tag planning while checking ordinary ASCII values in place.
    fn next_literal(
        &mut self,
        text: &str,
        limit: usize,
        rules: NameRules,
    ) -> Result<Step, Failure> {
        self.next_impl::<true>(text, false, true, true, limit, rules)
    }

    /// Keep general attribute scanning separate from the native value proof.
    fn next_impl<const LITERAL: bool>(
        &mut self,
        text: &str,
        final_input: bool,
        tag_end: bool,
        allow_refs: bool,
        limit: usize,
        rules: NameRules,
    ) -> Result<Step, Failure> {
        let fail = |kind, message, offset| Failure {
            kind,
            message,
            offset,
        };
        loop {
            match self.phase {
                Phase::Space => {
                    let start = self.offset;
                    self.skip_space(text);
                    self.separated |= self.offset != start;
                    if self.offset == text.len() {
                        return Ok(if final_input {
                            Step::End
                        } else {
                            Step::Incomplete
                        });
                    }
                    if tag_end {
                        match text.as_bytes()[self.offset] {
                            b'>' => {
                                return Ok(Step::TagEnd {
                                    end: self.offset + 1,
                                    empty: false,
                                });
                            }
                            b'/' if text.as_bytes().get(self.offset + 1) == Some(&b'>') => {
                                return Ok(Step::TagEnd {
                                    end: self.offset + 2,
                                    empty: true,
                                });
                            }
                            b'/' if self.offset + 1 == text.len() && !final_input => {
                                return Ok(Step::Incomplete);
                            }
                            _ => {}
                        }
                    }
                    if !self.separated {
                        return Err(fail(
                            ErrorKind::InvalidToken,
                            "attributes must be separated by whitespace",
                            self.offset,
                        ));
                    }
                    if self.count >= limit {
                        return Err(fail(
                            ErrorKind::LimitExceeded,
                            "attribute count limit exceeded",
                            self.offset,
                        ));
                    }
                    self.name_start = self.offset;
                    self.phase = Phase::Name;
                }
                Phase::Name => {
                    if self.offset == self.name_start {
                        let Some(first) = text[self.offset..].chars().next() else {
                            return Ok(Step::Incomplete);
                        };
                        if !rules.is_name_start(first) {
                            return Err(fail(
                                ErrorKind::InvalidToken,
                                "invalid attribute name",
                                self.offset,
                            ));
                        }
                        self.offset += first.len_utf8();
                    }
                    self.offset = name_end(text, self.offset, rules);
                    if self.offset == text.len() && !final_input {
                        return Ok(Step::Incomplete);
                    }
                    self.name_end = self.offset;
                    self.phase = Phase::Equals;
                }
                Phase::Equals => {
                    self.skip_space(text);
                    if self.offset == text.len() && !final_input {
                        return Ok(Step::Incomplete);
                    }
                    if text.as_bytes().get(self.offset) != Some(&b'=') {
                        return Err(fail(
                            ErrorKind::InvalidToken,
                            "attribute is missing equals sign",
                            self.offset,
                        ));
                    }
                    self.offset += 1;
                    self.phase = Phase::Quote;
                }
                Phase::Quote => {
                    self.skip_space(text);
                    if self.offset == text.len() && !final_input {
                        return Ok(Step::Incomplete);
                    }
                    let Some(&quote @ (b'\'' | b'"')) = text.as_bytes().get(self.offset) else {
                        return Err(fail(
                            ErrorKind::InvalidToken,
                            "attribute value must be quoted",
                            self.offset,
                        ));
                    };
                    self.offset += 1;
                    self.value_start = self.offset;
                    if LITERAL {
                        self.literal_ascii = true;
                    }
                    self.phase = Phase::Value(quote);
                }
                Phase::Value(quote) => {
                    let suffix = &text.as_bytes()[self.offset..];
                    let next = if LITERAL {
                        literal_value_end(suffix, quote, &mut self.literal_ascii)
                    } else if tag_end {
                        memchr::memchr2(quote, b'<', suffix)
                    } else {
                        memchr::memchr(quote, suffix)
                    };
                    let Some(relative) = next else {
                        self.offset = text.len();
                        if final_input {
                            return Err(fail(
                                ErrorKind::UnclosedToken,
                                "unclosed attribute value",
                                self.value_start,
                            ));
                        }
                        return Ok(Step::Incomplete);
                    };
                    let end = self.offset + relative;
                    if text.as_bytes()[end] == b'<' {
                        return Err(fail(
                            ErrorKind::InvalidToken,
                            "invalid character in attribute value",
                            end,
                        ));
                    }
                    if !LITERAL {
                        let value = &text[self.value_start..end];
                        // Complete views reject '<' before '&' even when '&' occurs
                        // earlier. Keep that precedence when input arrives in parts.
                        if let Some(offset) = value
                            .find('<')
                            .or_else(|| (!allow_refs).then(|| value.find('&')).flatten())
                        {
                            return Err(fail(
                                ErrorKind::InvalidToken,
                                "invalid character in attribute value",
                                self.value_start + offset,
                            ));
                        }
                    }
                    let attribute = RawAttribute {
                        name_start: self.name_start,
                        name_end: self.name_end,
                        value_start: self.value_start,
                        value_end: end,
                    };
                    self.offset = end + 1;
                    self.count += 1;
                    self.separated = false;
                    self.phase = Phase::Space;
                    return Ok(Step::Attribute(attribute));
                }
            }
        }
    }

    fn skip_space(&mut self, text: &str) {
        while text
            .as_bytes()
            .get(self.offset)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.offset += 1;
        }
    }
}

/// Find the closing quote or '<', recording whether all value bytes are literal ASCII.
/// Special values keep the original completed-value predicate and error ordering.
fn literal_value_end(bytes: &[u8], quote: u8, literal_ascii: &mut bool) -> Option<usize> {
    if !*literal_ascii {
        return memchr::memchr2(quote, b'<', bytes);
    }
    let ones = 0x0101_0101_0101_0101;
    let high = 0x8080_8080_8080_8080;
    let quotes = u64::from(quote) * ones;
    let mut offset = 0;
    loop {
        if let Some(chunk) = bytes[offset..].first_chunk::<8>() {
            let word = u64::from_le_bytes(*chunk);
            let control = word.wrapping_sub(0x2020_2020_2020_2020) & !word;
            let quote = word ^ quotes;
            let less = word ^ 0x3c3c_3c3c_3c3c_3c3c;
            let amp = word ^ 0x2626_2626_2626_2626;
            let special = word
                | control
                | (quote.wrapping_sub(ones) & !quote)
                | (less.wrapping_sub(ones) & !less)
                | (amp.wrapping_sub(ones) & !amp);
            // A clear mask proves all eight bytes are ordinary ASCII. A set
            // mask only selects scalar handling; no propagated borrow is an offset.
            if special & high == 0 {
                offset += 8;
                continue;
            }
        }
        let byte = *bytes.get(offset)?;
        if byte == quote || byte == b'<' {
            return Some(offset);
        }
        if byte < b' ' || byte == b'&' || !byte.is_ascii() {
            *literal_ascii = false;
            return memchr::memchr2(quote, b'<', &bytes[offset..])
                .map(|relative| offset + relative);
        }
        offset += 1;
    }
}

/// Find a name's next boundary; callers validate the first character separately.
pub(crate) fn name_end(text: &str, offset: usize, rules: NameRules) -> usize {
    text[offset..]
        .char_indices()
        .find(|(_, c)| !rules.is_name_char(*c))
        .map_or(text.len(), |(index, _)| offset + index)
}

/// Check literal attribute eligibility without separate normalization and XML scans.
fn is_literal_value(text: &str) -> bool {
    let mut rest = text;
    while let Some(chunk) = rest.as_bytes().first_chunk::<8>() {
        let word = u64::from_le_bytes(*chunk);
        let high = 0x8080_8080_8080_8080;
        let control = word.wrapping_sub(0x2020_2020_2020_2020) & !word & high;
        let ampersand = word ^ 0x2626_2626_2626_2626;
        let reference = ampersand.wrapping_sub(0x0101_0101_0101_0101) & !ampersand & high;
        // Each nonzero mask implies an actual control byte or ampersand.
        // Borrow propagation can mark later bytes, but cannot create a match.
        if control | reference != 0 {
            return false;
        }
        if word & high != 0 {
            break;
        }
        rest = &rest[8..];
    }
    // Only ASCII words were skipped, so this remains a UTF-8 boundary.
    // XML's allowed TAB/CR/LF still require attribute normalization.
    rest.chars()
        .all(|character| character >= ' ' && character != '&' && is_xml_char(character))
}

/// Scalar progress plus offset records supplied by the parser's existing cache.
/// Failed or cold plans fall back once; they never allocate while input arrives.
#[derive(Debug, Default)]
pub(crate) struct TagScanner {
    source_index: Option<usize>,
    offset: usize,
    name_end: Option<usize>,
    attributes: Option<AttributeScanner>,
    disabled: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Planned {
    Complete { end: usize, name_end: usize },
    Incomplete,
    Fallback,
}

impl TagScanner {
    /// Record only into already allocated capacity. Syntax failures are left to
    /// the original token boundary/error path, preserving its error precedence.
    pub(crate) fn scan(
        &mut self,
        text: &str,
        source_index: usize,
        token_limit: usize,
        attribute_limit: usize,
        rules: NameRules,
        records: &mut Vec<RawAttribute>,
    ) -> Planned {
        if self.source_index != Some(source_index) {
            *self = Self {
                source_index: Some(source_index),
                offset: 1,
                ..Self::default()
            };
            records.clear();
        }
        if self.disabled {
            return Planned::Fallback;
        }
        let visible = text.floor_char_boundary(text.len().min(token_limit.saturating_add(1)));
        let input = &text[..visible];
        let result = self.advance(input, token_limit, attribute_limit, rules, records);
        let result = if matches!(result, Planned::Incomplete) && text.len() > token_limit {
            Planned::Fallback
        } else {
            result
        };
        if matches!(result, Planned::Fallback) {
            self.disabled = true;
        }
        result
    }

    fn advance(
        &mut self,
        text: &str,
        token_limit: usize,
        attribute_limit: usize,
        rules: NameRules,
        records: &mut Vec<RawAttribute>,
    ) -> Planned {
        if self.name_end.is_none() {
            if self.offset == 1 {
                let Some(first) = text.get(1..).and_then(|rest| rest.chars().next()) else {
                    return Planned::Incomplete;
                };
                if !rules.is_name_start(first) {
                    return Planned::Fallback;
                }
                self.offset += first.len_utf8();
            }
            self.offset = name_end(text, self.offset, rules);
            if self.offset == text.len() {
                return Planned::Incomplete;
            }
            self.name_end = Some(self.offset);
            self.attributes = Some(AttributeScanner::new(self.offset));
        }
        let name_end = self.name_end.unwrap();
        let scanner = self.attributes.as_mut().unwrap();
        loop {
            match scanner.next_literal(text, attribute_limit, rules) {
                Ok(Step::Attribute(mut attribute)) => {
                    let value = attribute.value(text);
                    if records.len() == records.capacity()
                        || (!scanner.literal_ascii && !is_literal_value(value))
                    {
                        return Planned::Fallback;
                    }
                    attribute.name_start -= name_end;
                    attribute.name_end -= name_end;
                    attribute.value_start -= name_end;
                    attribute.value_end -= name_end;
                    // Capacity was checked above. No allocation is possible.
                    records.push(attribute);
                }
                Ok(Step::TagEnd { end, empty }) => {
                    debug_assert_eq!(empty, text[..end].ends_with("/>"));
                    return if end <= token_limit {
                        Planned::Complete { end, name_end }
                    } else {
                        Planned::Fallback
                    };
                }
                Ok(Step::Incomplete) => return Planned::Incomplete,
                Ok(Step::End) | Err(_) => return Planned::Fallback,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Error, Position, invalid_xml_char, parse_raw_attributes, take_name, try_push, whitespace,
    };
    use oriole_storage::Allocator;

    type Outcome = (
        std::vec::Vec<[usize; 4]>,
        Option<(ErrorKind, &'static str, usize)>,
    );

    #[test]
    fn literal_values_match_separate_checks_at_word_and_unicode_boundaries() {
        fn check(text: &str) {
            let expected = !text
                .bytes()
                .any(|byte| matches!(byte, b'&' | b'\t' | b'\r' | b'\n'))
                && invalid_xml_char(text).is_none();
            assert_eq!(is_literal_value(text), expected, "{text:?}");
            for quote in *b"'\"" {
                let expected_end = text.bytes().position(|byte| byte == quote || byte == b'<');
                let mut literal_ascii = true;
                assert_eq!(
                    literal_value_end(text.as_bytes(), quote, &mut literal_ascii),
                    expected_end,
                    "{text:?} quote={quote}"
                );
                let prefix = &text[..expected_end.unwrap_or(text.len())];
                assert_eq!(
                    literal_ascii,
                    prefix
                        .bytes()
                        .all(|byte| byte.is_ascii() && byte >= b' ' && byte != b'&'),
                    "{text:?} quote={quote}"
                );
            }
        }

        check("");
        // Adjacent ASCII pairs exercise both word masks and propagated borrows.
        for offset in 0..8 {
            for first in 0..128 {
                for second in 0..128 {
                    let mut bytes = [b'x'; 24];
                    bytes[offset] = first;
                    bytes[offset + 1] = second;
                    check(std::str::from_utf8(&bytes).unwrap());
                }
            }
        }
        let mut text = std::string::String::new();
        for scalar in 0..=0x10ffff {
            if let Some(character) = char::from_u32(scalar) {
                text.clear();
                text.push_str("0123456");
                text.push(character);
                text.push_str("abcdefghi");
                check(&text);
            }
        }
        for offset in 0..16 {
            for value in ["é&", "é\t", "é\r\n", "é\0", "é😀", "\u{fffe}", "\u{ffff}"] {
                for tail in [0, 7, 8, 9, 128, 4096] {
                    check(&format!(
                        "{}{value}{}",
                        "x".repeat(offset),
                        "x".repeat(tail)
                    ));
                }
            }
        }
    }

    #[test]
    fn fused_values_preserve_steps_and_eligibility_across_incremental_feeds() {
        for quote in ['\'', '"'] {
            for prefix in [0, 7, 8, 9, 15, 16, 17, 128] {
                for special in [
                    "", "&", "<", "&before<", "\t", "\r\n", "\0", "é", "😀", "\u{fffe}", "'", "\"",
                ] {
                    let input = format!(
                        " a={quote}{}{special}abcdefghij{quote} b='next'>",
                        "x".repeat(prefix)
                    );
                    let mut general = AttributeScanner::new(0);
                    let mut fused = AttributeScanner::new(0);
                    'feeds: for end in (0..=input.len()).filter(|end| input.is_char_boundary(*end))
                    {
                        // The second visit adds no input, including inside values.
                        for _ in 0..2 {
                            loop {
                                let expected = general.next(
                                    &input[..end],
                                    false,
                                    true,
                                    true,
                                    2,
                                    NameRules::default(),
                                );
                                let observed =
                                    fused.next_literal(&input[..end], 2, NameRules::default());
                                match (expected, observed) {
                                    (Ok(Step::Attribute(a)), Ok(Step::Attribute(b))) => {
                                        assert_eq!(
                                            [a.name_start, a.name_end, a.value_start, a.value_end],
                                            [b.name_start, b.name_end, b.value_start, b.value_end],
                                            "{input:?} end={end}"
                                        );
                                        let value = a.value(&input);
                                        assert_eq!(
                                            fused.literal_ascii,
                                            value.bytes().all(|byte| {
                                                byte.is_ascii() && byte >= b' ' && byte != b'&'
                                            }),
                                            "{input:?} end={end}"
                                        );
                                    }
                                    (Ok(Step::Incomplete), Ok(Step::Incomplete)) => break,
                                    (
                                        Ok(Step::TagEnd { end: a, empty: ae }),
                                        Ok(Step::TagEnd { end: b, empty: be }),
                                    ) => {
                                        assert_eq!((a, ae), (b, be));
                                        break 'feeds;
                                    }
                                    (Err(a), Err(b)) => {
                                        assert_eq!(a, b, "{input:?} end={end}");
                                        break 'feeds;
                                    }
                                    (a, b) => panic!("{input:?} end={end}: {a:?} != {b:?}"),
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fused_plans_preserve_unicode_eligibility_and_special_value_timing() {
        let input = "<r a=\"é😀\" b=\"ascii\">";
        for split in (0..input.len()).filter(|split| input.is_char_boundary(*split)) {
            let mut records = Vec::new_in(Allocator::System);
            records.try_reserve_exact(2).unwrap();
            let mut scanner = TagScanner::default();
            assert!(matches!(
                scanner.scan(
                    &input[..split],
                    0,
                    100,
                    2,
                    NameRules::default(),
                    &mut records
                ),
                Planned::Incomplete
            ));
            assert!(matches!(
                scanner.scan(input, 0, 100, 2, NameRules::default(), &mut records),
                Planned::Complete { end, name_end: 2 } if end == input.len()
            ));
            assert_eq!(records.len(), 2);
            assert_eq!(records[0].value(&input[2..]), "é😀");
            assert_eq!(records[1].value(&input[2..]), "ascii");
        }

        let prefix = "<r a=\"x&tail";
        for delimiter in ['"', '<'] {
            let mut records = Vec::new_in(Allocator::System);
            records.try_reserve_exact(1).unwrap();
            let mut scanner = TagScanner::default();
            for end in 0..=prefix.len() {
                assert!(matches!(
                    scanner.scan(
                        &prefix[..end],
                        0,
                        100,
                        1,
                        NameRules::default(),
                        &mut records
                    ),
                    Planned::Incomplete
                ));
            }
            assert!(matches!(
                scanner.scan(
                    &format!("{prefix}{delimiter}"),
                    0,
                    100,
                    1,
                    NameRules::default(),
                    &mut records
                ),
                Planned::Fallback
            ));
            assert!(records.is_empty());
        }
    }

    fn outcome(text: &str, refs: bool, limit: usize, legacy: bool) -> Outcome {
        let mut attrs = Vec::new_in(Allocator::System);
        let result = if legacy {
            legacy_parse_raw_attributes(text, refs, &mut attrs, limit, NameRules::default())
        } else {
            parse_raw_attributes(text, refs, &mut attrs, limit, NameRules::default())
        };
        (
            attrs
                .iter()
                .map(|a| [a.name_start, a.name_end, a.value_start, a.value_end])
                .collect(),
            result
                .err()
                .map(|e| (e.kind, e.message, e.position.byte_index)),
        )
    }

    #[test]
    fn complete_grammar_matches_original_short_alphabet_and_error_prefix() {
        let alphabet = b" a='\"&</\t";
        for len in 0..=4 {
            for mut word in 0..alphabet.len().pow(len) {
                let mut bytes = vec![0; len as usize];
                for byte in &mut bytes {
                    *byte = alphabet[word % alphabet.len()];
                    word /= alphabet.len();
                }
                let text = std::str::from_utf8(&bytes).unwrap();
                for refs in [false, true] {
                    for limit in [0, 1, 3] {
                        assert_eq!(
                            outcome(text, refs, limit, false),
                            outcome(text, refs, limit, true),
                            "{text:?} refs={refs} limit={limit}"
                        );
                    }
                }
            }
        }
        for text in [
            " a='x' b=",
            " a='&x<'",
            " a='&x' b='<'",
            " é = 'α😀' :x=\"z\"",
            " a='x'\r\nb='y'",
            " a='x' a='y'",
            " a='x' b='unterminated",
        ] {
            for refs in [false, true] {
                for limit in [0, 1, 2, 3] {
                    assert_eq!(
                        outcome(text, refs, limit, false),
                        outcome(text, refs, limit, true),
                        "{text:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn planned_boundaries_and_records_match_original_across_every_split() {
        use crate::ScanMode;
        use crate::encoding::Source;
        let mut inputs: std::vec::Vec<std::string::String> = [
            "<root>",
            "<r/>",
            "<r a='x' b=\"y\"/>",
            "<é α='😀' b='a > b' />",
            "<r a='x' b='y' c='z'/>",
            "<r a='x'bad='y'>",
            "<r a='x<' >",
            "<r a='x&y' />",
            "<r a='\0' >",
            "<r a='x' / >",
            "<r a='unterminated",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        for name in ["r", "p:r", ":r", "1r", "r\u{901}", "\u{901}", "é", "😀"] {
            for tail in [
                "",
                " a=\"v\"",
                " a=\"x<\"",
                " a=\"x&y\"",
                " a=\"\0\"",
                " a=\"v\"bad=\"x\"",
                " a=\"v\" a=\"x\"",
                " a=\"unterminated",
                " /",
                "/ ",
            ] {
                inputs.push(format!("<{name}{tail}>"));
            }
        }
        for input in inputs {
            for limit in 0..=input.len() + 1 {
                for split in 0..=input.len() {
                    if !input.is_char_boundary(split) {
                        continue;
                    }
                    for capacity in [0, 1, 4] {
                        let mut records = Vec::new_in(Allocator::System);
                        records.try_reserve_exact(capacity).unwrap();
                        let original_capacity = records.capacity();
                        let original_pointer = records.as_ptr();
                        let mut plan = TagScanner::default();
                        let mut source = Source::new(Allocator::System, NameRules::default());
                        let mut before = 0;
                        for end in [split, input.len()] {
                            source.text.try_push_str(&input[before..end]).unwrap();
                            before = end;
                            let observed = plan.scan(
                                &input[..end],
                                0,
                                limit,
                                10,
                                NameRules::default(),
                                &mut records,
                            );
                            assert_eq!(records.capacity(), original_capacity);
                            assert_eq!(records.as_ptr(), original_pointer);
                            let reference = source.scan_token(ScanMode::Tag, limit);
                            match observed {
                                Planned::Complete {
                                    end: tag_end,
                                    name_end,
                                } => {
                                    assert_eq!(
                                        reference,
                                        Ok(Some(tag_end)),
                                        "{input:?} split={split} limit={limit}"
                                    );
                                    assert!(invalid_xml_char(&input[..tag_end]).is_none());
                                    let body_end = tag_end
                                        - if input[..tag_end].ends_with("/>") {
                                            2
                                        } else {
                                            1
                                        };
                                    let mut expected = Vec::new_in(Allocator::System);
                                    legacy_parse_raw_attributes(
                                        &input[name_end..body_end],
                                        true,
                                        &mut expected,
                                        10,
                                        NameRules::default(),
                                    )
                                    .unwrap();
                                    assert_eq!(
                                        records
                                            .iter()
                                            .map(|a| [
                                                a.name_start,
                                                a.name_end,
                                                a.value_start,
                                                a.value_end
                                            ])
                                            .collect::<std::vec::Vec<_>>(),
                                        expected
                                            .iter()
                                            .map(|a| [
                                                a.name_start,
                                                a.name_end,
                                                a.value_start,
                                                a.value_end
                                            ])
                                            .collect::<std::vec::Vec<_>>()
                                    );
                                }
                                Planned::Incomplete => assert_eq!(
                                    reference,
                                    Ok(None),
                                    "{input:?} split={split} limit={limit}"
                                ),
                                Planned::Fallback => {}
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn accepted_name_characters_are_valid_xml_characters() {
        for rules in [NameRules::FourthEdition, NameRules::FifthEdition] {
            for code in 0..=0x10ffff {
                if let Some(c) = char::from_u32(code)
                    && rules.is_name_char(c)
                {
                    assert!(crate::is_xml_char(c), "{rules:?} {code:x}");
                }
            }
        }
    }

    #[test]
    fn fallback_stays_disabled_until_the_source_advances() {
        let input = "<r a='x' b='y'>";
        let mut records = Vec::new_in(Allocator::System);
        let mut scanner = TagScanner::default();
        // The first completed attribute exhausts the initially empty cache.
        assert!(matches!(
            scanner.scan(&input[..8], 0, 100, 10, NameRules::default(), &mut records),
            Planned::Fallback
        ));
        records.try_reserve_exact(2).unwrap();
        let pointer = records.as_ptr();
        for end in 8..=input.len() {
            assert!(matches!(
                scanner.scan(
                    &input[..end],
                    0,
                    100,
                    10,
                    NameRules::default(),
                    &mut records
                ),
                Planned::Fallback
            ));
            assert!(records.is_empty());
            assert_eq!(records.as_ptr(), pointer);
        }
        // A distinct physical token can use the available capacity again.
        assert!(matches!(
            scanner.scan(input, 1, 100, 10, NameRules::default(), &mut records),
            Planned::Complete { end, name_end: 2 } if end == input.len()
        ));
        assert_eq!(records.len(), 2);
        assert_eq!(records.as_ptr(), pointer);
    }

    fn legacy_parse_raw_attributes(
        text: &str,
        allow_refs: bool,
        result: &mut Vec<RawAttribute>,
        limit: usize,
        name_rules: NameRules,
    ) -> Result<(), Error> {
        let original_len = text.len();
        let mut text = text;
        result.clear();
        let error_at = |kind, message, remaining: &str| Error {
            kind,
            message,
            position: Position {
                byte_index: original_len - remaining.len(),
                line: 1,
                column: 0,
                byte_count: 0,
            },
        };
        while !text.is_empty() {
            let trimmed = text.trim_start_matches(whitespace);
            if trimmed.len() == text.len() {
                return Err(error_at(
                    ErrorKind::InvalidToken,
                    "attributes must be separated by whitespace",
                    text,
                ));
            }
            text = trimmed;
            if text.is_empty() {
                break;
            }
            if result.len() >= limit {
                return Err(error_at(
                    ErrorKind::LimitExceeded,
                    "attribute count limit exceeded",
                    text,
                ));
            }
            let name_offset = original_len - text.len();
            let (name, rest) = take_name(text, name_rules).ok_or(error_at(
                ErrorKind::InvalidToken,
                "invalid attribute name",
                text,
            ))?;
            let rest = rest.trim_start_matches(whitespace);
            let rest = rest
                .strip_prefix('=')
                .ok_or(error_at(
                    ErrorKind::InvalidToken,
                    "attribute is missing equals sign",
                    rest,
                ))?
                .trim_start_matches(whitespace);
            let quote = rest
                .chars()
                .next()
                .filter(|c| matches!(c, '\'' | '"'))
                .ok_or(error_at(
                    ErrorKind::InvalidToken,
                    "attribute value must be quoted",
                    rest,
                ))?;
            let rest = &rest[1..];
            let end = rest.find(quote).ok_or(error_at(
                ErrorKind::UnclosedToken,
                "unclosed attribute value",
                rest,
            ))?;
            let value = &rest[..end];
            let value_offset = original_len - rest.len();
            if let Some(offset) = value
                .find('<')
                .or_else(|| (!allow_refs).then(|| value.find('&')).flatten())
            {
                return Err(error_at(
                    ErrorKind::InvalidToken,
                    "invalid character in attribute value",
                    &rest[offset..],
                ));
            }
            try_push(
                result,
                RawAttribute {
                    name_start: name_offset,
                    name_end: name_offset + name.len(),
                    value_start: value_offset,
                    value_end: value_offset + value.len(),
                },
            )?;
            text = &rest[end + 1..];
        }
        Ok(())
    }
}
