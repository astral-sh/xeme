use oriole::{Config, EventKind, Parser, Position};

#[test]
fn line_and_character_columns_survive_encoding_and_input_boundaries() {
    let prefix = "é".repeat(48);
    let input = format!("<r>é😀a\r\nb\rc\nd\t<e a='{prefix}é\r\nx'/></r>");
    let input = input.as_str();
    let start = input.find("<e ").unwrap();
    let end = input.find("</r>").unwrap();
    for encoding in ["utf8", "utf16le", "utf16be"] {
        let bytes = encode(input, encoding);
        for width in 1..=bytes.len() {
            let mut parser = Parser::new(Config::default());
            let mut positions = Vec::new();
            for (index, chunk) in bytes.chunks(width).enumerate() {
                parser
                    .feed(chunk, (index + 1) * width >= bytes.len())
                    .unwrap();
                while let Some(event) = parser.next_event().unwrap() {
                    match event.kind {
                        EventKind::StartElement { name, .. } | EventKind::EndElement { name }
                            if name == "e" =>
                        {
                            positions.push(event.position)
                        }
                        _ => {}
                    }
                }
            }
            assert_eq!(
                positions,
                [
                    Position {
                        byte_index: encoded_offset(input, start, encoding),
                        byte_count: encoded_offset(input, end, encoding)
                            - encoded_offset(input, start, encoding),
                        line: 4,
                        column: 2,
                    },
                    Position {
                        byte_index: encoded_offset(input, end, encoding),
                        byte_count: 0,
                        line: 5,
                        column: 4,
                    },
                ],
                "{encoding}, width {width}",
            );
        }
    }
}

#[test]
fn line_tracking_survives_source_compaction_and_nonadjacent_crlf() {
    let input = format!("<r>{}\r\né😀<e/>\r   \n<e/></r>", "x".repeat(65_536));
    let offsets: Vec<_> = input
        .match_indices("<e/>")
        .map(|(offset, _)| offset)
        .collect();
    for encoding in ["utf8", "utf16le", "utf16be"] {
        let bytes = encode(&input, encoding);
        for width in [1, 3, 4096, 65_536, bytes.len()] {
            let mut parser = Parser::new(Config::default());
            let mut positions = Vec::new();
            for (index, chunk) in bytes.chunks(width).enumerate() {
                parser
                    .feed(chunk, (index + 1) * width >= bytes.len())
                    .unwrap();
                while let Some(event) = parser.next_event().unwrap() {
                    if let EventKind::StartElement { name, .. } = event.kind
                        && name == "e"
                    {
                        positions.push((
                            event.position.byte_index,
                            event.position.line,
                            event.position.column,
                        ));
                    }
                }
            }
            assert_eq!(
                positions,
                [
                    (encoded_offset(&input, offsets[0], encoding), 2, 2),
                    (encoded_offset(&input, offsets[1], encoding), 4, 0),
                ],
                "{encoding}, width {width}",
            );
        }
    }
}

#[test]
fn unbalanced_entity_errors_stay_at_the_reference_after_the_source_is_popped() {
    for replacement in ["<n>", "<n>&empty;"] {
        let input = format!(
            "<!DOCTYPE r [<!ENTITY empty ''><!ENTITY open '{replacement}'>]>\r\n<r>é&open;</n></r>"
        );
        let reference = input.rfind("&open;").unwrap();
        for encoding in ["utf8", "utf16le", "utf16be"] {
            let bytes = encode(&input, encoding);
            for width in 1..=bytes.len() {
                let mut parser = Parser::new(Config::default());
                let error = 'input: {
                    for (index, chunk) in bytes.chunks(width).enumerate() {
                        parser
                            .feed(chunk, (index + 1) * width >= bytes.len())
                            .unwrap();
                        loop {
                            match parser.next_event() {
                                Ok(Some(_)) => {}
                                Ok(None) => break,
                                Err(error) => break 'input error,
                            }
                        }
                    }
                    panic!("unbalanced entity was accepted");
                };
                assert_eq!(error.kind, oriole::ErrorKind::AsynchronousEntity);
                assert_eq!(
                    error.position,
                    Position {
                        byte_index: encoded_offset(&input, reference, encoding),
                        byte_count: 0,
                        line: 2,
                        column: 4,
                    },
                    "{encoding}, width {width}, replacement {replacement:?}",
                );
            }
        }
    }
}

fn encode(input: &str, encoding: &str) -> Vec<u8> {
    if encoding == "utf8" {
        return input.as_bytes().to_vec();
    }
    let big_endian = encoding == "utf16be";
    let mut bytes = if big_endian {
        vec![0xfe, 0xff]
    } else {
        vec![0xff, 0xfe]
    };
    bytes.extend(input.encode_utf16().flat_map(|unit| {
        if big_endian {
            unit.to_be_bytes()
        } else {
            unit.to_le_bytes()
        }
    }));
    bytes
}

fn encoded_offset(input: &str, offset: usize, encoding: &str) -> usize {
    if encoding == "utf8" {
        offset
    } else {
        2 + input[..offset].encode_utf16().count() * 2
    }
}
