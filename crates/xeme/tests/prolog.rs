use xeme::{Config, Error, ErrorKind, EventKind, Parser};

fn parse(xml: &[u8], width: usize, deferral: bool) -> (Error, String) {
    parse_finalization(xml, width, deferral, false)
}

fn parse_finalization(
    xml: &[u8],
    width: usize,
    deferral: bool,
    empty_final: bool,
) -> (Error, String) {
    let mut parser = Parser::new(Config::default());
    parser.set_reparse_deferral_enabled(deferral);
    let mut text = String::new();
    let chunks = xml.chunks(width);
    let count = chunks.len();
    let feeds = chunks
        .enumerate()
        .map(|(index, chunk)| (chunk, !empty_final && index + 1 == count))
        .chain(empty_final.then_some((&[][..], true)));
    for (chunk, final_input) in feeds {
        if let Err(error) = parser.feed(chunk, final_input) {
            return (error, text);
        }
        loop {
            match parser.next_event() {
                Ok(Some(event)) => {
                    if let EventKind::Text(value) = event.kind {
                        text.push_str(&value);
                    }
                }
                Ok(None) => break,
                Err(error) => return (error, text),
            }
        }
    }
    panic!("invalid XML unexpectedly parsed");
}

#[test]
fn prolog_carriage_return_before_quote_does_not_hide_final_errors() {
    for (xml, kind, offset) in [
        ("\r'", ErrorKind::UnclosedToken, 1),
        ("\r\"", ErrorKind::UnclosedToken, 1),
        ("\u{feff}\r'", ErrorKind::UnclosedToken, 4),
        (" \t\r'", ErrorKind::UnclosedToken, 3),
        ("\r\r'", ErrorKind::UnclosedToken, 2),
        ("\r\n'", ErrorKind::UnclosedToken, 2),
        ("\n'", ErrorKind::UnclosedToken, 1),
        ("\r'<r ", ErrorKind::UnclosedToken, 1),
        ("\r'</r", ErrorKind::UnclosedToken, 1),
        ("\r'\u{1}</r", ErrorKind::InvalidToken, 2),
        ("\r''", ErrorKind::Syntax, 1),
        ("\r'' ", ErrorKind::Syntax, 1),
        ("\r'é'x", ErrorKind::InvalidToken, 5),
        ("\r<r ", ErrorKind::UnclosedToken, 1),
        ("\r</r", ErrorKind::UnclosedToken, 1),
        ("\r</1", ErrorKind::InvalidToken, 3),
    ] {
        for deferral in [false, true] {
            for width in 1..=xml.len() {
                for empty_final in [false, true] {
                    let (error, text) =
                        parse_finalization(xml.as_bytes(), width, deferral, empty_final);
                    assert_eq!(error.kind, kind, "{xml:?}, {width}, {empty_final}");
                    assert_eq!(error.position.byte_index, offset, "{xml:?}");
                    assert!(text.is_empty());
                }
            }
        }
    }
}

#[test]
fn prolog_quotes_are_scanned_before_the_grammar_rejects_them() {
    for (xml, kind, offset) in [
        ("\"", ErrorKind::UnclosedToken, 0),
        ("\"x", ErrorKind::UnclosedToken, 0),
        ("'x", ErrorKind::UnclosedToken, 0),
        ("\"x\"", ErrorKind::Syntax, 0),
        ("\"x\" ", ErrorKind::Syntax, 0),
        ("\"x\">", ErrorKind::Syntax, 0),
        ("\"x\"%", ErrorKind::Syntax, 0),
        ("\"x\"[", ErrorKind::Syntax, 0),
        ("\"x\"y", ErrorKind::InvalidToken, 3),
        ("'x'x", ErrorKind::InvalidToken, 3),
        ("\"<\"x", ErrorKind::InvalidToken, 3),
        ("\"x\"<r/>", ErrorKind::InvalidToken, 3),
        ("\"x\u{1}", ErrorKind::InvalidToken, 2),
        (" \"x\"y", ErrorKind::InvalidToken, 4),
        ("\r\n\"x\"y", ErrorKind::InvalidToken, 5),
        ("\"é\"x", ErrorKind::InvalidToken, 4),
    ] {
        for deferral in [false, true] {
            for width in 1..=xml.len() {
                let (error, _) = parse(xml.as_bytes(), width, deferral);
                assert_eq!(error.kind, kind, "{xml:?}, {width}, {deferral}");
                assert_eq!(
                    error.position.byte_index, offset,
                    "{xml:?}, {width}, {deferral}"
                );
            }
        }
    }
}

#[test]
fn prolog_literal_decoding_errors_keep_the_tokenizer_precedence() {
    for (xml, kind, offset) in [
        (&b"\"x\xf0"[..], ErrorKind::PartialCharacter, 0),
        (&b"\"x\xff"[..], ErrorKind::InvalidToken, 2),
        (&b"\"x\"\xf0"[..], ErrorKind::InvalidToken, 3),
        (&b"\"x\"\xff"[..], ErrorKind::InvalidToken, 3),
        (&b"\"\x01\xf0"[..], ErrorKind::InvalidToken, 1),
    ] {
        for width in 1..=xml.len() {
            let (error, _) = parse(xml, width, true);
            assert_eq!(error.kind, kind, "{xml:?}, {width}");
            assert_eq!(error.position.byte_index, offset, "{xml:?}, {width}");
        }
    }
}

#[test]
fn prolog_literal_positions_follow_the_wire_encoding() {
    let xml = "\r\n\"é\"x";
    for little_endian in [false, true] {
        let mut bytes = if little_endian {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for unit in xml.encode_utf16() {
            bytes.extend(if little_endian {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        for width in 1..=bytes.len() {
            let (error, _) = parse(&bytes, width, true);
            assert_eq!(error.kind, ErrorKind::InvalidToken);
            assert_eq!(error.position.byte_index, 12);
            assert_eq!(error.position.line, 2);
            assert_eq!(error.position.column, 3);
        }
    }
}

#[test]
fn incomplete_prolog_literals_are_bounded_and_resume_linearly() {
    let xml = format!("\"{}\"x", "é".repeat(8192));
    let (error, _) = parse(xml.as_bytes(), 1, false);
    assert_eq!(error.kind, ErrorKind::InvalidToken);
    assert_eq!(error.position.byte_index, xml.len() - 1);

    let mut config = Config::default();
    config.limits.max_token_bytes = 16;
    let mut parser = Parser::new(config);
    parser.feed(b"\"", false).unwrap();
    assert!(parser.next_event().unwrap().is_none());
    for _ in 0..15 {
        parser.feed(b"x", false).unwrap();
        assert!(parser.next_event().unwrap().is_none());
    }
    parser.feed(b"x", false).unwrap();
    assert_eq!(
        parser.next_event().unwrap_err().kind,
        ErrorKind::LimitExceeded
    );
}

#[test]
fn deferred_references_preserve_text_delivered_before_a_later_invalid_token() {
    // Expat defers the reference and the following start tag independently.
    // At one-byte feeds it delivers `e` before receiving the forbidden terminator.
    let xml = b"<r>&#x41;<n a=\"x\">e]]>";
    for deferral in [false, true] {
        let (error, text) = parse(xml, 1, deferral);
        assert_eq!(error.kind, ErrorKind::InvalidToken);
        assert_eq!(error.position.byte_index, 21);
        assert_eq!(text, "Ae");
    }
    // In a complete input buffer the invalid text token is never delivered.
    let (error, text) = parse(xml, xml.len(), true);
    assert_eq!(error.kind, ErrorKind::InvalidToken);
    assert_eq!(text, "A");
}
