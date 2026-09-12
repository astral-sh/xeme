//! A transient proof for ordinary native ASCII character data.

use wide::{i8x16, u8x16};

/// Return stop and LF masks for a valid ASCII prefix, one bit per byte lane.
#[inline]
fn ascii_masks(chunk: &[u8; 16]) -> (u32, u32) {
    let lanes = u8x16::new(*chunk).cast_signed();
    let linefeeds = lanes.simd_eq(i8x16::splat(b'\n' as i8));
    let tabs = lanes.simd_eq(i8x16::splat(b'\t' as i8));
    // Non-ASCII lanes are negative. LF and TAB remain valid ASCII text;
    // every other control, markup delimiter, or ']' ends the proven prefix.
    let stops = (lanes.simd_lt(i8x16::splat(0x20)) & !(linefeeds | tabs))
        | lanes.simd_eq(i8x16::splat(b'<' as i8))
        | lanes.simd_eq(i8x16::splat(b'&' as i8))
        | lanes.simd_eq(i8x16::splat(b']' as i8));
    (stops.to_bitmask(), linefeeds.to_bitmask())
}

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
            if let Some(chunk) = bytes[end..limit].first_chunk::<16>() {
                let (stops, linefeeds) = ascii_masks(chunk);
                let span = if stops == 0 {
                    16
                } else {
                    stops.trailing_zeros() as usize
                };
                // Only the first stop matters. An uncertain byte before markup
                // abandons the whole proof; bytes after a delimiter are ignored.
                if stops != 0 && !matches!(chunk[span], b'<' | b'&') {
                    return None;
                }
                let prefix_linefeeds = linefeeds & ((1_u32 << span) - 1);
                if prefix_linefeeds != 0 {
                    newlines += prefix_linefeeds.count_ones() as usize;
                    last_line_end = end + (u32::BITS - prefix_linefeeds.leading_zeros()) as usize;
                }
                end += span;
                if stops != 0 {
                    break 'scan;
                }
                continue;
            } else if let Some(chunk) = bytes[end..limit].first_chunk::<8>() {
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
    use super::{TextPlan, ascii_masks};

    #[test]
    fn every_ascii_byte_and_vector_lane_preserve_the_proof_boundary() {
        for byte in 0..=0x7f {
            for offset in 0..32 {
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
    fn vector_mask_matches_all_byte_values_in_every_lane() {
        for byte in 0..=u8::MAX {
            for lane in 0..16 {
                let mut chunk = [b'x'; 16];
                chunk[lane] = byte;
                let ordinary = matches!(byte, b'\n' | b'\t' | 0x20..=0x7f)
                    && !matches!(byte, b'<' | b'&' | b']');
                let stop = if ordinary { 0 } else { 1 << lane };
                let linefeed = if byte == b'\n' { 1 << lane } else { 0 };
                assert_eq!(
                    ascii_masks(&chunk),
                    (stop, linefeed),
                    "byte {byte} at {lane}"
                );
            }
        }
    }

    #[test]
    fn first_stop_limits_linefeeds_and_preserves_uncertain_byte_order() {
        assert_eq!(ascii_masks(&[b'\n'; 16]), (0, 0xffff));
        assert_eq!(ascii_masks(&[b'\t'; 16]), (0, 0));
        for end in [0, 7, 8, 15, 16, 23, 24, 31, 32] {
            let mut prefix = vec![b'x'; end];
            for lane in [0, 6, 7, 8, 14, 15, 16] {
                if lane < end {
                    prefix[lane] = b'\n';
                }
            }
            let prefix = String::from_utf8(prefix).unwrap();
            let newlines = prefix.bytes().filter(|&byte| byte == b'\n').count();
            let columns = prefix.rsplit('\n').next().unwrap().len();
            for delimiter in ['<', '&'] {
                let text = format!("{prefix}{delimiter}\n\n\t\r]\u{1}éxxxxxxxxxxxxxxxx");
                let plan = TextPlan::scan(&text).unwrap();
                assert_eq!(
                    (
                        plan.end,
                        plan.newlines,
                        plan.trailing_columns,
                        plan.leading_lf,
                    ),
                    (end, newlines, columns, prefix.starts_with('\n'))
                );
                for prior_cr in [false, true] {
                    let (mut line, mut column, mut previous_cr) = (3, 7, prior_cr);
                    plan.advance_position(&mut line, &mut column, &mut previous_cr);
                    let expected = if end == 0 {
                        (3, 7, prior_cr)
                    } else {
                        (3 + newlines - usize::from(prior_cr), columns, false)
                    };
                    assert_eq!((line, column, previous_cr), expected);
                }
            }
            for uncertain in ['\r', ']', '\u{1}', 'é'] {
                assert!(
                    TextPlan::scan(&format!("{prefix}{uncertain}yyyyyyyyyyyyyyyy<\n")).is_none()
                );
            }
        }
    }

    #[test]
    fn unaligned_short_tails_and_newline_positions_remain_exact() {
        // Every starting alignment, vector boundary, and 0..15-byte tail.
        let backing = "x".repeat(63);
        for offset in 0..16 {
            for len in 0..48 {
                assert_eq!(
                    TextPlan::scan(&backing[offset..offset + len]).unwrap().end,
                    len
                );
            }
        }
        // Complete LF/TAB vectors followed by every mixed short-tail length.
        for block in ["\n".repeat(32), "\t".repeat(32)] {
            for tail_len in 0..16 {
                let tail: String = "\n\t".chars().cycle().take(tail_len).collect();
                let text = format!("{block}{tail}");
                let plan = TextPlan::scan(&text).unwrap();
                let newlines = text.bytes().filter(|&byte| byte == b'\n').count();
                let columns = text.rsplit('\n').next().unwrap().len();
                assert_eq!(
                    (plan.end, plan.newlines, plan.trailing_columns),
                    (text.len(), newlines, columns)
                );
                for prior_cr in [false, true] {
                    let (mut line, mut column, mut previous_cr) = (3, 7, prior_cr);
                    plan.advance_position(&mut line, &mut column, &mut previous_cr);
                    let expected_line =
                        3 + newlines - usize::from(prior_cr && text.starts_with('\n'));
                    let expected_column = if newlines == 0 {
                        7 + text.len()
                    } else {
                        columns
                    };
                    assert_eq!(
                        (line, column, previous_cr),
                        (expected_line, expected_column, false)
                    );
                }
            }
        }
        for offset in 0..32 {
            let prefix = "x".repeat(offset);
            let text = format!("{prefix}\nyyyyyyyyyyyyyyyyy<\r]é");
            let plan = TextPlan::scan(&text).unwrap();
            assert_eq!(plan.end, offset + 18);
            for prior_cr in [false, true] {
                let (mut line, mut column, mut previous_cr) = (3, 7, prior_cr);
                plan.advance_position(&mut line, &mut column, &mut previous_cr);
                assert_eq!(line, 4 - usize::from(offset == 0 && prior_cr));
                assert_eq!(column, 17);
                assert!(!previous_cr);
            }
            assert!(TextPlan::scan(&format!("{prefix}éyyyyyyyyyyyyyyyy<")).is_none());
            assert!(TextPlan::scan(&format!("{prefix}😀yyyyyyyyyyyyyyyy<")).is_none());
        }
    }

    #[test]
    fn mixed_specials_preserve_stop_and_position_across_half_blocks() {
        for (text, end, column) in [
            ("\nxxxxx\n<\n\r]é", 7, 0),
            ("\nxxxxx\nx<\n\r]é", 8, 1),
            ("xxxxxxx\n\nxxxxxx<\n\r]é", 15, 6),
            ("xxxxxxx\nxxxxxxx\n<\n\r]é", 16, 0),
            ("xxxxxxx\n\nxxxxxx<\ré", 15, 6),
            ("xxxxxxx\nxxxxxxx\n<é\r", 16, 0),
            ("xxxxxxxxxxxxxxx\n\nxxxxxxx&\ré", 24, 7),
            ("\nxxxxxx\nxxxxxxxx<é", 16, 8),
        ] {
            let plan = TextPlan::scan(text).unwrap();
            assert_eq!(plan.end, end);
            for prior_cr in [false, true] {
                let (mut line, mut actual_column, mut previous_cr) = (3, 7, prior_cr);
                plan.advance_position(&mut line, &mut actual_column, &mut previous_cr);
                let expected_line = 5 - usize::from(prior_cr && text.starts_with('\n'));
                assert_eq!(
                    (line, actual_column, previous_cr),
                    (expected_line, column, false)
                );
            }
        }
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
