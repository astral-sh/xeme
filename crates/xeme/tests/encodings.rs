use xeme::{Config, ErrorKind, EventKind, Parser};

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

#[test]
fn an_unknown_declared_encoding_reports_its_value_location() {
    for (input, line, column, byte) in [
        (
            &b"<?xml version='1.0' encoding='unknown'?><r/>"[..],
            1,
            30,
            30,
        ),
        (
            &b"<?xml version='1.0'\r\n encoding='unknown'?><r/>"[..],
            2,
            11,
            32,
        ),
    ] {
        let mut parser = Parser::new(Config::default());
        parser.feed(input, true).unwrap();
        let error = parser.next_event().unwrap_err();
        assert_eq!(error.kind, ErrorKind::UnknownEncoding);
        assert_eq!(
            (
                error.position.line,
                error.position.column,
                error.position.byte_index
            ),
            (line, column, byte)
        );
    }
}

#[test]
fn ascii_declarations_of_utf16_report_an_encoding_mismatch() {
    for name in ["UTF-16", "utf-16", "UtF-16", "UTF-16LE", "UTF-16BE"] {
        for bom in ["", "\u{feff}"] {
            let input = format!("{bom}<?xml version='1.0' encoding='{name}'?><r/>");
            for width in [1, 2, 3, input.len()] {
                for context in [None, Some(None), Some(Some(""))] {
                    let mut parser = Parser::new(Config::default());
                    if let Some(context) = context {
                        parser = parser.external_child(context, None).unwrap();
                    }
                    assert_eq!(
                        encoded_content(&mut parser, input.as_bytes(), width),
                        Err(ErrorKind::IncorrectEncoding),
                        "{name}/{bom:?}/{width}/{context:?}",
                    );
                    assert!(parser.unknown_encoding().is_none());
                    assert_eq!(
                        parser.next_event().unwrap_err().position.byte_index,
                        input.find(name).unwrap(),
                    );
                }
            }
        }
    }
}

fn encoded_content(parser: &mut Parser, input: &[u8], width: usize) -> Result<String, ErrorKind> {
    let mut text = String::new();
    for (index, piece) in input.chunks(width).enumerate() {
        parser
            .feed(piece, (index + 1) * width >= input.len())
            .map_err(|error| error.kind)?;
        while let Some(event) = parser.next_event().map_err(|error| error.kind)? {
            if let EventKind::Text(value) = event.kind {
                text.push_str(&value);
            }
        }
    }
    assert!(parser.is_finished());
    Ok(text)
}

#[test]
fn external_latin1_content_preserves_bom_shaped_bytes() {
    for (input, expected) in [
        (&b"\xff\xfeL "[..], "ÿþL "),
        (&b"\xfe\xff L"[..], "þÿ L"),
        (&b"\xef\xbb\xbfX"[..], "ï»¿X"),
    ] {
        for setter in [false, true] {
            for width in 1..=input.len() {
                let parent = Parser::new(Config::default());
                let mut child = parent
                    .external_child_with_encoding(Some(""), (!setter).then_some("ISO-8859-1"))
                    .unwrap();
                drop(parent);
                if setter {
                    child.set_encoding(Some("ISO-8859-1")).unwrap();
                }
                assert_eq!(encoded_content(&mut child, input, width).unwrap(), expected);
            }
        }
    }
}

#[test]
fn unlabelled_external_content_does_not_infer_utf16_from_a_later_nul() {
    for input in [&b"a\0b\0c\0"[..], &b" \0X\0"[..], &b"X\0"[..]] {
        for width in 1..=input.len() {
            let parent = Parser::new(Config::default());
            let mut child = parent.external_child(None, None).unwrap();
            // An external DTD has separate prolog rules; construct a content
            // child from it to ensure the mode belongs to the new child.
            child = child.external_child(Some(""), None).unwrap();
            assert_eq!(
                encoded_content(&mut child, input, width),
                Err(ErrorKind::InvalidToken)
            );
        }
    }
}

#[test]
fn external_content_retains_explicit_encodings_and_utf16_signatures() {
    for (input, protocol, expected) in [
        (&b"\0a\0b\0c"[..], None, "abc"),
        (&b"<\0r\0/\0>\0"[..], None, ""),
        (&b"\0<\0r\0/\0>"[..], None, ""),
        (&b"\xff\xfea\0b\0c\0"[..], None, "abc"),
        (&b"a\0b\0c\0"[..], Some("UTF-16LE"), "abc"),
        (&b"a\0b\0c\0"[..], Some("UTF-16"), "abc"),
    ] {
        for width in 1..=input.len() {
            let parent = Parser::new(Config::default());
            let mut child = parent
                .external_child_with_encoding(Some(""), protocol)
                .unwrap();
            assert_eq!(encoded_content(&mut child, input, width).unwrap(), expected);
        }
    }
}

#[test]
fn dtd_and_value_children_keep_prolog_encoding_detection() {
    let declaration: Vec<_> = " <!ELEMENT r ANY>"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    for width in 1..=declaration.len() {
        let parent = Parser::new(Config::default());
        let mut dtd = parent.external_child(None, None).unwrap();
        encoded_content(&mut dtd, &declaration, width).unwrap();
    }
    let parent = Parser::new(Config::default());
    let mut dtd = parent.external_child(None, None).unwrap();
    dtd.set_param_entity_parsing(2);
    dtd.feed(b"<!ENTITY % p SYSTEM 'p'><!ENTITY e 'L%p;R'>", true)
        .unwrap();
    let mut value = None;
    while let Some(event) = dtd.next_event().unwrap() {
        match event.kind {
            EventKind::ExternalEntityReference(_) => {
                let mut child = dtd.external_child(None, None).unwrap();
                // The whole-buffer prolog signature selects UTF-16LE for an
                // entity value, whereas the same content child is invalid.
                encoded_content(&mut child, b"X\0", 2).unwrap();
                dtd.merge_external_subset(&child).unwrap();
            }
            EventKind::EntityDeclaration(declaration) if (declaration.name == "e") => {
                let xeme::EntityDeclaration { value: text, .. } =
                    xeme_storage::Box::into_inner(declaration);

                value = text.map(|text| text.to_string());
            }
            _ => {}
        }
    }
    assert_eq!(value.as_deref(), Some("LXR"));
}

#[test]
fn small_input_and_token_limits_still_allow_complete_utf8_and_utf16_tags() {
    let utf16: Vec<_> = std::iter::once(0xfeff)
        .chain("<r/>".encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    for input in [b"<r/>".as_slice(), utf16.as_slice()] {
        for chunk in [1, input.len()] {
            for max_token_bytes in [4, xeme::Limits::default().max_token_bytes] {
                let mut parser = Parser::new(Config {
                    limits: xeme::Limits {
                        max_total_bytes: input.len(),
                        max_token_bytes,
                        ..xeme::Limits::default()
                    },
                    ..Config::default()
                });
                parser.feed(&[], false).unwrap();
                assert!(parser.next_event().unwrap().is_none());
                let mut events = 0;
                for (index, piece) in input.chunks(chunk).enumerate() {
                    parser
                        .feed(piece, (index + 1) * chunk >= input.len())
                        .unwrap();
                    while parser.next_event().unwrap().is_some() {
                        events += 1;
                    }
                }
                assert_eq!(events, 2);
                assert!(parser.is_finished());
            }
        }
    }
}

#[test]
fn declaration_limits_precede_encoding_detection_across_chunk_boundaries() {
    for (declaration, within_limit) in [
        (
            "<?xml version='1.0' encoding='custom'?>",
            Err(ErrorKind::UnknownEncoding),
        ),
        (
            "<?xml version='1.0' encoding='UTF-16'?>",
            Err(ErrorKind::IncorrectEncoding),
        ),
        ("<?xml version='1.0' encoding='UTF-8'?>", Ok(String::new())),
        ("<?xml version='1.0'?>", Ok(String::new())),
        (
            "<?xml encoding='custom' version='1.0'?>",
            Err(ErrorKind::XmlDeclaration),
        ),
    ] {
        for bom in ["", "\u{feff}"] {
            let input = format!("{bom}{declaration}<r/>{}", " ".repeat(100));
            for width in [1, 7, input.len()] {
                for limit in [32, declaration.len() - 1, declaration.len()] {
                    let mut config = Config::default();
                    config.limits.max_token_bytes = limit;
                    let mut parser = Parser::new(config);
                    let result = encoded_content(&mut parser, input.as_bytes(), width);
                    assert_eq!(
                        result,
                        if limit < declaration.len() {
                            Err(ErrorKind::LimitExceeded)
                        } else {
                            within_limit.clone()
                        },
                        "{declaration}, bom={bom:?}, width={width}, limit={limit}",
                    );
                    if limit < declaration.len() {
                        assert!(parser.unknown_encoding().is_none());
                        assert_eq!(
                            parser.next_event().unwrap_err().kind,
                            ErrorKind::LimitExceeded
                        );
                    }
                }
            }
        }
    }
}
