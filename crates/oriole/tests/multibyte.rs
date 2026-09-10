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

#[test]
fn custom_conversion_cannot_synthesize_xml_syntax_in_any_context() {
    for (prefix, suffix) in [
        (b"<r>".as_slice(), b"</r>".as_slice()),
        (b"<r a='", b"'/>"),
        (b"<r><![CDATA[", b"]]></r>"),
        (b"<", b"/>"),
        (b"<!", b"OCTYPE r><r/>"),
        (b"<?", b"ml version='1.0'?><r/>"),
        (b"<r>&#", b"41;</r>"),
        (b"<r>&#x", b";</r>"),
        (b"<r>&", b"mp;</r>"),
        (b"<!DOCTYPE r [<!ENTITY e '", b"'>]><r/>"),
    ] {
        for character in [
            '<', '>', '&', '\'', '"', ' ', '\t', '\r', '\n', ']', '[', ';', '#', ':', '-', '.',
            '0', 'x', 'A', 'D', 'a', '_',
        ] {
            let mut parser = parser();
            let mut document = prefix.to_vec();
            document.extend_from_slice(b"\x80\0");
            document.extend_from_slice(suffix);
            parser.feed(&document, true).unwrap();
            assert_eq!(
                parser.next_event().unwrap_err().kind,
                ErrorKind::UnknownEncoding
            );
            parser.set_multibyte_encoding_map("custom", map()).unwrap();
            while parser.next_event().unwrap().is_some() {}
            let request = parser.encoding_conversion().unwrap();
            assert_eq!(request.position.byte_index, prefix.len());
            let error = parser
                .resolve_encoding_conversion(character as i32)
                .unwrap_err();
            assert_eq!(error.kind, ErrorKind::InvalidToken);
            assert_eq!(error.position.byte_index, prefix.len());
            assert_eq!(parser.next_event().unwrap_err(), error);
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
