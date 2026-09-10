use oriole::{Config, ErrorKind, EventKind, Parser};

fn mapped_text(input: &[u8], map: [i32; 256]) -> Result<String, ErrorKind> {
    let mut parser = Parser::new(Config::default());
    parser.feed(input, true).map_err(|error| error.kind)?;
    assert_eq!(
        parser.next_event().unwrap_err().kind,
        ErrorKind::UnknownEncoding
    );
    parser
        .set_encoding_map("custom", map)
        .map_err(|error| error.kind)?;
    let mut output = String::new();
    while let Some(event) = parser.next_event().map_err(|error| error.kind)? {
        if let EventKind::Text(text) = event.kind {
            output.push_str(&text);
        }
    }
    Ok(output)
}

#[test]
fn custom_encodings_accept_duplicate_non_markup_characters() {
    // CPython's cp1006 codec has two byte values for one presentation-form letter.
    // Expat permits these aliases while protecting bytes used by XML syntax.
    let mut map = std::array::from_fn(|byte| byte as i32);
    map[128] = 0x20ac;
    map[129] = 0x20ac;
    assert_eq!(
        mapped_text(
            b"<?xml version='1.0' encoding='custom'?><r>\x80\x81</r>",
            map
        ),
        Ok("€€".into()),
    );
}

#[test]
fn custom_encodings_can_leave_ordinary_ascii_punctuation_undefined() {
    let mut map = std::array::from_fn(|byte| byte as i32);
    // HZ treats '~' as an encoding escape and leaves it undefined in its map.
    map[usize::from(b'~')] = -1;
    assert_eq!(
        mapped_text(b"<?xml version='1.0' encoding='custom'?><r>plain</r>", map),
        Ok("plain".into()),
    );
}

#[test]
fn a_declared_custom_encoding_can_follow_a_utf8_bom() {
    let map = std::array::from_fn(|byte| byte as i32);
    assert_eq!(
        mapped_text(
            b"\xef\xbb\xbf<?xml version='1.0' encoding='custom'?><r>plain</r>",
            map
        ),
        Ok("plain".into()),
    );
}

#[test]
fn custom_encodings_cannot_alias_markup_or_encode_supplementary_characters() {
    for value in [i32::from(b'<'), i32::from(b'a'), 0x10000] {
        let mut map = std::array::from_fn(|byte| byte as i32);
        map[128] = value;
        assert_eq!(
            mapped_text(b"<?xml version='1.0' encoding='custom'?><r>\x80</r>", map),
            Err(ErrorKind::UnknownEncoding),
        );
    }
}

#[test]
fn utf16_detection_allows_leading_whitespace_without_a_bom() {
    for big_endian in [true, false] {
        let bytes: Vec<_> = " \r\n<r/>"
            .encode_utf16()
            .flat_map(|unit| {
                if big_endian {
                    unit.to_be_bytes()
                } else {
                    unit.to_le_bytes()
                }
            })
            .collect();
        for chunk in [1, 2, 3, bytes.len()] {
            let mut parser = Parser::new(Config::default());
            for piece in bytes.chunks(chunk) {
                parser.feed(piece, false).unwrap();
                while parser.next_event().unwrap().is_some() {}
            }
            parser.feed(&[], true).unwrap();
            while parser.next_event().unwrap().is_some() {}
            assert!(parser.is_finished());
        }
    }
}

#[test]
fn incomplete_utf16_unit_and_surrogate_have_distinct_errors() {
    for (input, expected, line, column) in [
        (&b"\0\r\n"[..], ErrorKind::UnclosedToken, 2, 0),
        (&b"<\0r\0>\0\0"[..], ErrorKind::UnclosedToken, 1, 3),
        (&b"<\0r\0>\0\0\xd8"[..], ErrorKind::PartialCharacter, 1, 3),
    ] {
        let mut parser = Parser::new(Config::default());
        parser.feed(input, true).unwrap();
        loop {
            match parser.next_event() {
                Ok(Some(_)) => {}
                Ok(None) => panic!("truncated UTF-16 was accepted"),
                Err(error) => {
                    assert_eq!(error.kind, expected);
                    assert_eq!((error.position.line, error.position.column), (line, column));
                    break;
                }
            }
        }
    }
}
