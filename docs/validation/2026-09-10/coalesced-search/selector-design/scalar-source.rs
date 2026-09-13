        let coalesce = !self.text_line_boundaries && (!self.stack.is_empty() || self.fragment);
        let mut boundary = 0;
        let mut end = text
            .bytes()
            .enumerate()
            .find_map(|(index, byte)| {
                if matches!(byte, b'<' | b'&') {
                    return Some(index);
                }
                if byte == b'\n' || (!internal && byte == b'\r') {
                    if !coalesce || index >= 65_536 {
                        return Some(if boundary > 0 { boundary } else { index });
                    }
                    boundary = index
                        + if byte == b'\r' && text.as_bytes().get(index + 1) == Some(&b'\n') {
                            2
                        } else {
                            1
                        };
                }
                // Bound merging across lines, preserving the existing span of
                // an individual long line and its malformed-input prefix.
                (index >= 65_536 && boundary > 0).then_some(boundary)
            })
            .map_or(text.len(), |index| {
                if index == 0 && matches!(text.as_bytes()[0], b'\r' | b'\n') {
                    if text.starts_with("\r\n") { 2 } else { 1 }
                } else {
                    index
                }
            });
        if coalesce && end == text.len() && limit < self.source().remaining().len() {
            // Keep a converted buffer boundary at the last complete line when
            // possible. Otherwise merging earlier lines could shift the next
            // conversion window into a malformed token and emit extra data.
            let complete = text.strip_suffix('\r').unwrap_or(text);
            if let Some(newline) = complete
                .bytes()
                .rposition(|byte| byte == b'\n' || (!internal && byte == b'\r'))
            {
                end = newline + 1;
            }
        }
        if end == text.len() && !final_text {
            if text.ends_with('\r') {
                end -= 1;
            }
            // A forbidden CDATA terminator may straddle input chunks.
            while end > 0 && end + 2 >= text.len() && text.as_bytes()[end - 1] == b']' {
                end -= 1;
            }
        }
