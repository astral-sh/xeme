//! A transient proof for ordinary native ASCII character data.

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextPlan {
    pub(crate) end: usize,
    newlines: usize,
    trailing_columns: usize,
    leading_lf: bool,
}

impl TextPlan {
    /// Scan the complete coalesced span, abandoning the proof on uncertain bytes.
    /// A markup delimiter at the 64 KiB boundary still precedes the line cutoff.
    pub(crate) fn scan(text: &str) -> Option<Self> {
        const LIMIT: usize = 65_536;
        const HIGH: u64 = 0x8080_8080_8080_8080;
        const LOW: u64 = 0x0101_0101_0101_0101;
        let bytes = text.as_bytes();
        let limit = bytes.len().min(LIMIT);
        let mut end = 0;
        let mut newlines = 0;
        let mut last_line_end = 0;
        'scan: while end < limit {
            let scalar_end = (end + 8).min(limit);
            if let Some(chunk) = bytes[end..limit].first_chunk::<8>() {
                let word = u64::from_le_bytes(*chunk);
                let lt = word ^ 0x3c3c_3c3c_3c3c_3c3c;
                let amp = word ^ 0x2626_2626_2626_2626;
                let bracket = word ^ 0x5d5d_5d5d_5d5d_5d5d;
                // Skip only printable ASCII without markup or ']'. A possible
                // special byte goes through the scalar path; borrow-related
                // false positives cannot change validity or newline counts.
                let special = word
                    | (word.wrapping_sub(0x2020_2020_2020_2020) & !word)
                    | (lt.wrapping_sub(LOW) & !lt)
                    | (amp.wrapping_sub(LOW) & !amp)
                    | (bracket.wrapping_sub(LOW) & !bracket);
                if special & HIGH == 0 {
                    end += 8;
                    continue;
                }
            }
            while end < scalar_end {
                match bytes[end] {
                    b'<' | b'&' => break 'scan,
                    b'\n' => {
                        newlines += 1;
                        last_line_end = end + 1;
                    }
                    b'\t' | 0x20..=0x7f if bytes[end] != b']' => {}
                    _ => return None,
                }
                end += 1;
            }
        }
        if end == LIMIT && bytes.len() > LIMIT && !matches!(bytes[end], b'<' | b'&') {
            return None;
        }
        Some(Self {
            end,
            newlines,
            trailing_columns: end - last_line_end,
            leading_lf: bytes.first() == Some(&b'\n'),
        })
    }

    /// Apply the proven span at commit time, including a preceding token's CR.
    pub(crate) fn advance_position(
        self,
        line: &mut usize,
        column: &mut usize,
        previous_cr: &mut bool,
    ) {
        if self.end == 0 {
            return;
        }
        if self.newlines == 0 {
            *column += self.end;
        } else {
            *line += self.newlines - usize::from(self.leading_lf && *previous_cr);
            *column = self.trailing_columns;
        }
        *previous_cr = false;
    }
}

#[cfg(test)]
mod tests {
    use super::TextPlan;

    #[test]
    fn every_ascii_byte_and_word_lane_preserve_the_proof_boundary() {
        for byte in 0..=0x7f {
            for offset in 0..16 {
                let mut text = "x".repeat(offset);
                text.push(char::from(byte));
                text.push_str("yyyyyyyyyyyyyyyy<\r]\u{1}é");
                let plan = TextPlan::scan(&text);
                if matches!(byte, b'<' | b'&') {
                    assert_eq!(plan.unwrap().end, offset);
                } else if matches!(byte, b'\t' | b'\n' | 0x20..=0x7f) && byte != b']' {
                    assert_eq!(plan.unwrap().end, offset + 17);
                } else {
                    assert!(plan.is_none(), "byte {byte} at {offset}");
                }
            }
        }
        assert!(TextPlan::scan("line\ninvalidé<").is_none());
    }

    #[test]
    fn markup_at_the_cutoff_precedes_a_complete_line() {
        for offset in [65_535, 65_536, 65_537] {
            let prefix = format!("a\n{}", "x".repeat(offset - 2));
            for delimiter in ['<', '&'] {
                let plan = TextPlan::scan(&format!("{prefix}{delimiter}]\u{1}"));
                assert_eq!(
                    plan.map(|plan| plan.end),
                    (offset <= 65_536).then_some(offset)
                );
            }
        }
        assert_eq!(TextPlan::scan(&"x".repeat(65_536)).unwrap().end, 65_536);
        assert!(TextPlan::scan(&"x".repeat(65_537)).is_none());
    }
}
