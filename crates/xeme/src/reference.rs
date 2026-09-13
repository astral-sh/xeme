//! Bounded recognition of complete native scalar references.

use crate::names::is_xml_char;

pub(crate) struct ScalarReference {
    pub(crate) bytes: usize,
    pub(crate) character: char,
    pub(crate) predefined: bool,
}

impl ScalarReference {
    /// Validate and decode common spellings once. Incomplete, invalid, general,
    /// and unusually long references retain the resumable scanner's diagnostics.
    pub(crate) fn scan(text: &str, limit: usize) -> Option<Self> {
        let bytes = text.as_bytes();
        let (spelling, character) = match bytes.get(1)? {
            b'a' if text.starts_with("&amp;") => (5, '&'),
            b'a' if text.starts_with("&apos;") => (6, '\''),
            b'l' if text.starts_with("&lt;") => (4, '<'),
            b'g' if text.starts_with("&gt;") => (4, '>'),
            b'q' if text.starts_with("&quot;") => (6, '"'),
            b'#' => {
                let hexadecimal = bytes.get(2) == Some(&b'x');
                let start = if hexadecimal { 3 } else { 2 };
                let radix = if hexadecimal { 16_u32 } else { 10_u32 };
                let mut value = 0_u32;
                for (index, &byte) in bytes.iter().enumerate().take(limit.min(16)).skip(start) {
                    if byte == b';' {
                        return (index > start)
                            .then(|| char::from_u32(value))
                            .flatten()
                            .filter(|&character| is_xml_char(character))
                            .map(|character| Self {
                                bytes: index + 1,
                                character,
                                predefined: false,
                            });
                    }
                    let digit = match byte {
                        b'0'..=b'9' => u32::from(byte - b'0'),
                        b'a'..=b'f' if hexadecimal => u32::from(byte - b'a') + 10,
                        b'A'..=b'F' if hexadecimal => u32::from(byte - b'A') + 10,
                        _ => return None,
                    };
                    value = value.checked_mul(radix)?.checked_add(digit)?;
                }
                return None;
            }
            _ => return None,
        };
        (spelling <= limit).then_some(Self {
            bytes: spelling,
            character,
            predefined: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ScalarReference;

    #[test]
    fn native_scalar_plans_match_reference_decoding_and_token_limits() {
        for spelling in [
            "&amp;",
            "&apos;",
            "&lt;",
            "&gt;",
            "&quot;",
            "&#9;",
            "&#10;",
            "&#13;",
            "&#32;",
            "&#65;",
            "&#x7F;",
            "&#x80;",
            "&#xD7FF;",
            "&#xE000;",
            "&#xFFFD;",
            "&#65536;",
            "&#x10FFFF;",
            "&#00000065;",
            "&#x00000041;",
        ] {
            for limit in 0..=spelling.len() + 1 {
                let plan = ScalarReference::scan(spelling, limit);
                assert_eq!(
                    plan.is_some(),
                    limit >= spelling.len(),
                    "{spelling}/{limit}"
                );
                if let Some(plan) = plan {
                    assert_eq!(plan.bytes, spelling.len());
                    assert_eq!(
                        Some(plan.character),
                        crate::character_reference(&spelling[1..spelling.len() - 1]).unwrap()
                    );
                    assert_eq!(plan.predefined, !spelling.starts_with("&#"));
                }
            }
            for end in 0..spelling.len() {
                assert!(ScalarReference::scan(&spelling[..end], usize::MAX).is_none());
            }
        }
    }

    #[test]
    fn uncertain_or_invalid_references_keep_the_general_scanner() {
        for spelling in [
            "&name;",
            "&amper;",
            "&Amp;",
            "&;",
            "&#;",
            "&#x;",
            "&#X41;",
            "&#0;",
            "&#1;",
            "&#xD800;",
            "&#xFFFE;",
            "&#xFFFF;",
            "&#x110000;",
            "&#4294967361;",
            "&#-65;",
            "&#+65;",
            "&# 65;",
            "&#65x;",
            "&#xＧ;",
            "&#000000000000000000000065;",
        ] {
            assert!(
                ScalarReference::scan(spelling, usize::MAX).is_none(),
                "{spelling}"
            );
        }
    }
}
