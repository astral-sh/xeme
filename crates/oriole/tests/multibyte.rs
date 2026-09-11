use oriole::{Config, Error, ErrorKind, Event, EventKind, Parser};

fn map() -> [i32; 256] {
    let mut map = std::array::from_fn(|index| index as i32);
    map[0x80] = -2;
    map[0x81] = -3;
    map[0x82] = -4;
    map
}

fn drain(parser: &mut Parser, events: &mut Vec<Event>) -> Result<(), Error> {
    loop {
        match parser.next_event() {
            Ok(Some(event)) => events.push(event),
            Ok(None) => {
                let Some(request) = parser.encoding_conversion() else {
                    return Ok(());
                };
                assert_eq!(request.position.byte_count, usize::from(request.length));
                let character = match request.bytes[0] {
                    0x80 => 'é',
                    0x81 => '中',
                    0x82 => '界',
                    _ => unreachable!(),
                };
                parser.resolve_encoding_conversion(character as i32)?;
            }
            Err(error) if error.kind == ErrorKind::UnknownEncoding => {
                parser.set_multibyte_encoding_map("custom", map())?;
            }
            Err(error) => return Err(error),
        }
    }
}

fn parser() -> Parser {
    Parser::new(Config {
        encoding: Some("custom".into()),
        ..Config::default()
    })
}

#[test]
fn custom_names_match_original_byte_sequences() {
    fn parse_names(
        document: &[u8],
        width: usize,
        namespace_separator: Option<char>,
    ) -> Result<(), ErrorKind> {
        let mut parser = Parser::new(Config {
            encoding: Some("custom".into()),
            namespace_separator,
            ..Config::default()
        });
        let expected_name = if namespace_separator.is_some() {
            "urn|é"
        } else {
            "é"
        };
        let (mut starts, mut ends) = (0, 0);
        let mut encoding = map();
        encoding[0x84] = 'é' as i32;
        for (index, bytes) in document.chunks(width).enumerate() {
            parser
                .feed(bytes, (index + 1) * width >= document.len())
                .unwrap();
            loop {
                match parser.next_event() {
                    Ok(Some(event)) => match event.kind {
                        EventKind::StartElement { name, .. } => {
                            assert_eq!(name.as_str(), expected_name);
                            starts += 1;
                        }
                        EventKind::EndElement { name } => {
                            assert_eq!(name.as_str(), expected_name);
                            ends += 1;
                        }
                        _ => {}
                    },
                    Ok(None) if parser.encoding_conversion().is_some() => {
                        parser.resolve_encoding_conversion('é' as i32).unwrap();
                    }
                    Ok(None) => break,
                    Err(error) if error.kind == ErrorKind::UnknownEncoding => {
                        parser
                            .set_multibyte_encoding_map("custom", encoding)
                            .unwrap();
                    }
                    Err(error) => {
                        assert_eq!((starts, ends), (1, 0));
                        return Err(error.kind);
                    }
                }
            }
        }
        assert_eq!((starts, ends), (1, 1));
        Ok(())
    }
    for name in [b"\x80\0".as_slice(), b"\x81\0\0", b"\x84", b"\xe9"] {
        for end in [b"\x80\0".as_slice(), b"\x81\0\0", b"\x84", b"\xe9"] {
            for (namespace_separator, start, middle) in [
                (None, b"<".as_slice(), b"></".as_slice()),
                (Some('|'), b"<p:", b" xmlns:p='urn'></p:"),
                (Some('|'), b"<", b" xmlns='urn'></"),
            ] {
                let mut document = start.to_vec();
                document.extend_from_slice(name);
                document.extend_from_slice(middle);
                document.extend_from_slice(end);
                document.push(b'>');
                for width in [1, document.len()] {
                    assert_eq!(
                        parse_names(&document, width, namespace_separator),
                        if name == end {
                            Ok(())
                        } else {
                            Err(ErrorKind::TagMismatch)
                        },
                        "{document:?}, width={width}, namespace_separator={namespace_separator:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn multibyte_names_attributes_and_byte_positions_at_every_chunk_width() {
    let document = b"<\x80\0 a='\x81\0\0'>\x82\0\0\0</\x80\0>";
    for width in 1..=document.len() {
        let mut parser = parser();
        let mut events = Vec::new();
        for chunk in document.chunks(width) {
            parser.feed(chunk, false).unwrap();
            drain(&mut parser, &mut events).unwrap();
        }
        parser.feed(&[], true).unwrap();
        drain(&mut parser, &mut events).unwrap();
        let start = &events[0];
        let EventKind::StartElement { name, attributes } = &start.kind else {
            panic!("missing start element");
        };
        assert_eq!(name.as_str(), "é");
        assert_eq!(attributes[0].value.as_str(), "中");
        assert_eq!(start.position.byte_index, 0);
        let opening = document.iter().position(|byte| *byte == b'>').unwrap() + 1;
        assert_eq!(start.position.byte_count, opening);
        assert!(matches!(&events[1].kind, EventKind::Text(text) if text.as_str() == "界"));
        assert_eq!(events[1].position.byte_index, opening);
        assert_eq!(events[1].position.byte_count, 4);
        assert_eq!(events[2].position.byte_index, opening + 4);
        assert_eq!(events[2].position.byte_count, 5);
        assert_eq!(parser.position().byte_index, document.len());
    }
}

#[test]
fn incomplete_custom_sequences_wait_for_input_and_fail_at_finalization() {
    for (lead, length) in [(0x80, 2), (0x81, 3), (0x82, 4)] {
        for received in 1..length {
            let mut parser = parser();
            let mut events = Vec::new();
            parser.feed(b"<r>", false).unwrap();
            drain(&mut parser, &mut events).unwrap();
            let mut sequence = vec![0; received];
            sequence[0] = lead;
            parser.feed(&sequence, false).unwrap();
            drain(&mut parser, &mut events).unwrap();
            assert!(parser.encoding_conversion().is_none());
            parser.feed(&[], true).unwrap();
            let error = drain(&mut parser, &mut events).unwrap_err();
            assert_eq!(error.kind, ErrorKind::PartialCharacter);
            assert_eq!(error.position.byte_index, 3);
        }
    }
}

fn parse_ascii_aliases(document: &[u8], width: usize) -> Result<Vec<Event>, Error> {
    parse_ascii_aliases_with_map(document, width, map(), None)
}

fn parse_ascii_aliases_with_map(
    document: &[u8],
    width: usize,
    encoding: [i32; 256],
    namespace_separator: Option<char>,
) -> Result<Vec<Event>, Error> {
    let mut parser = Parser::new(Config {
        encoding: Some("custom".into()),
        namespace_separator,
        ..Config::default()
    });
    let mut events = Vec::new();
    for (index, bytes) in document.chunks(width).enumerate() {
        parser.feed(bytes, (index + 1) * width >= document.len())?;
        loop {
            match parser.next_event() {
                Ok(Some(event)) => events.push(event),
                Ok(None) => {
                    let Some(request) = parser.encoding_conversion() else {
                        break;
                    };
                    parser.resolve_encoding_conversion(i32::from(request.bytes[1]))?;
                }
                Err(error) if error.kind == ErrorKind::UnknownEncoding => {
                    parser.set_multibyte_encoding_map("custom", encoding)?;
                }
                Err(error) => return Err(error),
            }
        }
    }
    Ok(events)
}

#[test]
fn ascii_aliases_are_literal_data_in_text_cdata_and_attributes() {
    for scalar in [9, 10, 13].into_iter().chain(32..127) {
        for (prefix, suffix, attribute) in [
            (b"<r>".as_slice(), b"</r>".as_slice(), false),
            (b"<r><![CDATA[", b"]]></r>", false),
            (b"<r a='", b"'/>", true),
        ] {
            let mut document = prefix.to_vec();
            document.extend_from_slice(&[0x80, scalar]);
            document.extend_from_slice(suffix);
            for width in [1, 3, document.len()] {
                let events = parse_ascii_aliases(&document, width).unwrap();
                let value: String = if attribute {
                    let EventKind::StartElement { attributes, .. } = &events[0].kind else {
                        panic!("start element")
                    };
                    attributes[0].value.to_string()
                } else {
                    events
                        .iter()
                        .filter_map(|event| match &event.kind {
                            EventKind::Text(value) => Some(value.as_str()),
                            _ => None,
                        })
                        .collect()
                };
                assert_eq!(
                    value,
                    char::from(scalar).to_string(),
                    "{document:?}, width {width}"
                );
            }
        }
    }
}

#[test]
fn ascii_alias_names_keep_raw_delimiters_keywords_and_references_distinct() {
    for document in [
        b"<\x80A></\x80A>".as_slice(),
        b"<?\x80xml?><r/>",
        b"<!DOCTYPE r [<!ENTITY \x80e 'value'>]><r>&\x80e;</r>",
        b"<!DOCTYPE r [<!ENTITY e '\x80<'>]><r/>",
        b"<?xml version='\x80A.0'?><r/>",
    ] {
        for width in [1, document.len()] {
            parse_ascii_aliases(document, width).unwrap();
        }
    }
    for (document, expected) in [
        (b"<\x80A></A>".as_slice(), ErrorKind::TagMismatch),
        (b"<A></\x80A>", ErrorKind::TagMismatch),
        (b"<r A='1' \x80A='2'/>", ErrorKind::DuplicateAttribute),
        (b"<r>&\x80amp;</r>", ErrorKind::UndefinedEntity),
        (b"<r>&#\x80x41;</r>", ErrorKind::InvalidToken),
        (b"<r>&#x\x8041;</r>", ErrorKind::InvalidToken),
        (
            b"<!DOCTYPE r [<!ENTITY e '\x80<'>]><r>&e;</r>",
            ErrorKind::UnclosedToken,
        ),
    ] {
        for width in [1, document.len()] {
            assert_eq!(
                parse_ascii_aliases(document, width).unwrap_err().kind,
                expected,
                "{document:?}, width {width}"
            );
        }
    }
    for document in [
        b"<!\x80DOCTYPE r><r/>".as_slice(),
        b"<!DOCTYPE r [<!\x80ENTITY e 'v'>]><r/>",
        b"<!DOCTYPE r [<!ELEMENT r \x80EMPTY>]><r/>",
    ] {
        for width in [1, document.len()] {
            assert!(parse_ascii_aliases(document, width).is_err());
        }
    }
}

#[test]
fn converted_newlines_preserve_their_lexical_role() {
    for width in [1, 5, 100] {
        let document = b"<r a='\x80\r\n\r\n'>\x80\r\n\r\n</r>";
        let events = parse_ascii_aliases(document, width).unwrap();
        let EventKind::StartElement { attributes, .. } = &events[0].kind else {
            panic!("start element")
        };
        assert_eq!(attributes[0].value.as_str(), "\r  ");
        let text: String = events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::Text(value) => Some(value.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "\r\n\n");
        assert_eq!(events.last().unwrap().position.line, 5);
        for document in [b"<!--\x80\r--><r/>".as_slice(), b"<?p \x80\r?><r/>"] {
            let events = parse_ascii_aliases(document, width).unwrap();
            match &events[0].kind {
                EventKind::Comment(value) => assert_eq!(value.as_str(), "\n"),
                EventKind::ProcessingInstruction { data, .. } => assert_eq!(data.as_str(), "\n"),
                _ => panic!("comment or PI"),
            }
        }
    }
}

#[test]
fn multibyte_source_compaction_preserves_raw_offsets() {
    let mut parser = parser();
    let mut events = Vec::new();
    parser.feed(b"<r>", false).unwrap();
    drain(&mut parser, &mut events).unwrap();
    for _ in 0..40000 {
        parser.feed(b"\x80\0", false).unwrap();
        drain(&mut parser, &mut events).unwrap();
    }
    parser.feed(b"</r>", true).unwrap();
    drain(&mut parser, &mut events).unwrap();
    assert_eq!(events.last().unwrap().position.byte_index, 80003);
    assert_eq!(parser.position().byte_index, 80007);
}

#[test]
fn conversion_requests_preserve_lines_and_columns_across_crlf() {
    let document = b"<r>\r\n\x80\0\r\n\x81\0\0</r>";
    for width in 1..=document.len() {
        let mut parser = parser();
        let mut positions = Vec::new();
        for (index, chunk) in document.chunks(width).enumerate() {
            parser
                .feed(chunk, (index + 1) * width >= document.len())
                .unwrap();
            loop {
                match parser.next_event() {
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        let Some(request) = parser.encoding_conversion() else {
                            break;
                        };
                        positions.push((
                            request.position.byte_index,
                            request.position.line,
                            request.position.column,
                        ));
                        parser.resolve_encoding_conversion('é' as i32).unwrap();
                    }
                    Err(error) if error.kind == ErrorKind::UnknownEncoding => {
                        parser.set_multibyte_encoding_map("custom", map()).unwrap();
                    }
                    Err(error) => panic!("unexpected parse failure: {error:?}"),
                }
            }
        }
        assert_eq!(positions, [(5, 2, 0), (9, 3, 0)]);
        assert_eq!(parser.position().byte_index, document.len());
        assert_eq!(parser.position().line, 3);
        assert_eq!(parser.position().column, 5);
    }
}

#[test]
fn raw_qnames_are_checked_before_decoded_namespace_expansion() {
    for (document, name) in [
        (b"<a\x80:r xmlns:a='u'/>".as_slice(), "u|r"),
        (b"<a\x80: xmlns:a='u'/>", "u|"),
        (b"<a\x80:b:c xmlns:a='u'/>", "u|b:c"),
        (b"<a\x80:b\x80:c xmlns:a='u'/>", "u|b:c"),
    ] {
        for width in [1, document.len()] {
            let events = parse_ascii_aliases_with_map(document, width, map(), Some('|')).unwrap();
            assert!(events.iter().any(|event| matches!(&event.kind, EventKind::StartElement { name: actual, .. } if actual == name)));
        }
    }
    for document in [b"<a: xmlns:a='u'/>".as_slice(), b"<a:b:c xmlns:a='u'/>"] {
        assert_eq!(
            parse_ascii_aliases_with_map(document, 1, map(), Some('|'))
                .unwrap_err()
                .kind,
            ErrorKind::InvalidToken
        );
    }
}

#[test]
fn converted_spaces_follow_tokenized_attribute_source_roles() {
    for (value, expected) in [
        (b"A\x80  B".as_slice(), "A  B"),
        (b" A\x80  B ", "A B"),
        (b" A \x80 B ", "A  B"),
        (b"\x80 \x80 ", "  "),
    ] {
        let mut document = b"<!DOCTYPE r [<!ATTLIST r a NMTOKENS #IMPLIED>]><r a='".to_vec();
        document.extend_from_slice(value);
        document.extend_from_slice(b"'/>");
        for width in [1, document.len()] {
            let events = parse_ascii_aliases(&document, width).unwrap();
            let value = events
                .iter()
                .find_map(|event| match &event.kind {
                    EventKind::StartElement { attributes, .. } => {
                        Some(attributes[0].value.as_str())
                    }
                    _ => None,
                })
                .expect("start element");
            assert_eq!(value, expected);
        }
    }
}

#[test]
fn public_identifier_checks_use_the_original_custom_map_byte_classes() {
    let mut encoding = map();
    encoding[b'^' as usize] = 'é' as i32;
    encoding[b'$' as usize] = -2;
    for width in [1, 100] {
        let events = parse_ascii_aliases_with_map(
            b"<!DOCTYPE r PUBLIC '^$^' 's'><r/>",
            width,
            encoding,
            None,
        )
        .unwrap();
        assert!(events.iter().any(|event| matches!(&event.kind, EventKind::StartDoctype(declaration) if declaration.public_id.as_deref() == Some("é^"))));
        assert!(
            parse_ascii_aliases_with_map(
                b"<!DOCTYPE r PUBLIC '\x80A' 's'><r/>",
                width,
                encoding,
                None
            )
            .is_err()
        );
    }
}

#[test]
fn converted_attribute_identity_survives_offset_and_hash_paths() {
    for count in [0, 9] {
        let padding: String = (0..count).map(|index| format!(" x{index}='x'")).collect();
        let document = format!(
            "<!DOCTYPE root [<!ATTLIST r A CDATA 'default'>]><root><r{padding} @='first'/><r{padding} A='second'/><r/></root>"
        );
        let document = document
            .as_bytes()
            .iter()
            .flat_map(|byte| {
                if *byte == b'@' {
                    vec![0x80, b'A']
                } else {
                    vec![*byte]
                }
            })
            .collect::<Vec<_>>();
        for width in [1, document.len()] {
            let events = parse_ascii_aliases(&document, width).unwrap();
            let values: Vec<_> = events
                .iter()
                .filter_map(|event| match &event.kind {
                    EventKind::StartElement { name, attributes } if name == "r" => {
                        let matching: Vec<_> = attributes
                            .iter()
                            .filter(|attribute| attribute.name == "A")
                            .collect();
                        assert_eq!(matching.len(), 1);
                        Some((matching[0].value.as_str(), matching[0].specified))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                values,
                [("first", true), ("second", true), ("default", false)]
            );
        }
        let document = format!("<r{padding} @='first' A='second'/>");
        let document = document
            .as_bytes()
            .iter()
            .flat_map(|byte| {
                if *byte == b'@' {
                    vec![0x80, b'A']
                } else {
                    vec![*byte]
                }
            })
            .collect::<Vec<_>>();
        for width in [1, document.len()] {
            assert_eq!(
                parse_ascii_aliases(&document, width).unwrap_err().kind,
                ErrorKind::DuplicateAttribute
            );
        }
    }
}
