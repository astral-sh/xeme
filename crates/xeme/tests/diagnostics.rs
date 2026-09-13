use xeme::{Config, Error, ErrorKind, Parser};

fn parse_error(xml: &[u8], chunk: usize) -> Error {
    parse_error_with_deferral(xml, chunk, true)
}

fn parse_error_with_deferral(xml: &[u8], chunk: usize, deferral: bool) -> Error {
    let mut parser = Parser::new(Config::default());
    parser.set_reparse_deferral_enabled(deferral);
    let chunks = xml.chunks(chunk);
    let count = chunks.len();
    for (index, bytes) in chunks.enumerate() {
        if let Err(error) = parser.feed(bytes, index + 1 == count) {
            return error;
        }
        loop {
            match parser.next_event() {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(error) => return error,
            }
        }
    }
    panic!("invalid XML unexpectedly parsed");
}

#[test]
fn invalid_reference_and_end_tag_prefixes_take_precedence_over_eof() {
    for (xml, offset) in [
        ("<r>&/r>", 4),
        ("<r>&/", 4),
        ("<r>&name/", 8),
        ("<r>&9", 4),
        ("<r>&\u{301}", 4),
        ("<r>&é/", 6),
        ("<r>&;", 4),
        ("<r>&#X", 5),
        ("<r>&#;", 5),
        ("<r>&#x;", 6),
        ("<r>&#xg", 6),
        ("<r>&#9z", 6),
        ("<r>&#x110000z", 12),
        ("<r></r\"", 6),
        ("<r></r\">", 6),
        ("<r></r/", 6),
        ("<r></r x", 7),
        ("<r></r\r\n\"", 8),
        ("<r></r\u{1}", 6),
        ("<é></é\"", 8),
    ] {
        for deferral in [false, true] {
            for chunk in 1..=xml.len() {
                let error = parse_error_with_deferral(xml.as_bytes(), chunk, deferral);
                assert_eq!(
                    error.kind,
                    ErrorKind::InvalidToken,
                    "{xml}, {chunk}, {deferral}"
                );
                assert_eq!(
                    error.position.byte_index, offset,
                    "{xml}, {chunk}, {deferral}"
                );
            }
        }
    }
}

#[test]
fn valid_unfinished_reference_and_end_tag_prefixes_remain_unclosed() {
    for xml in [
        "<r>&",
        "<r>&name",
        "<r>&é",
        "<r>&#",
        "<r>&#x",
        "<r>&#x0",
        "<r>&#x110000",
        "<r>&#9999999999999999999999999999999999",
        "<r></",
        "<r></r",
        "<r></r\u{301}",
        "<r></r \t\r\n",
    ] {
        for deferral in [false, true] {
            for chunk in 1..=xml.len() {
                let error = parse_error_with_deferral(xml.as_bytes(), chunk, deferral);
                assert_eq!(
                    error.kind,
                    ErrorKind::UnclosedToken,
                    "{xml}, {chunk}, {deferral}"
                );
                assert_eq!(error.position.byte_index, 3, "{xml}, {chunk}, {deferral}");
            }
        }
    }
    for xml in ["<r>&#x0;", "<r>&#x110000;", "<r>&#999999999999999999999;"] {
        assert_eq!(
            parse_error(xml.as_bytes(), 1).kind,
            ErrorKind::BadCharacterReference
        );
    }
}

#[test]
fn malformed_prefixes_precede_a_later_decoding_failure() {
    for (xml, offset) in [
        (&b"<r>&/\xf0"[..], 4),
        (&b"<r></r\"\xf0"[..], 6),
        (&b"<r>&#xg\xff"[..], 6),
    ] {
        for chunk in 1..=xml.len() {
            let error = parse_error(xml, chunk);
            assert_eq!(error.kind, ErrorKind::InvalidToken);
            assert_eq!(error.position.byte_index, offset);
        }
    }
    for xml in [&b"<r>&name\xf0"[..], &b"<r></r\xf0"[..]] {
        assert_eq!(parse_error(xml, 1).kind, ErrorKind::PartialCharacter);
    }
}

#[test]
fn long_unicode_names_and_end_tag_whitespace_resume_across_small_feeds() {
    let name = "é".repeat(8192);
    let xml = format!("<!DOCTYPE r [<!ENTITY {name} 'value'>]><r>&{name};</r \r\n>");
    for width in [1, 3, 4096] {
        let mut parser = Parser::new(Config::default());
        parser.set_reparse_deferral_enabled(false);
        let chunks = xml.as_bytes().chunks(width);
        let count = chunks.len();
        for (index, bytes) in chunks.enumerate() {
            parser.feed(bytes, index + 1 == count).unwrap();
            while parser.next_event().unwrap().is_some() {}
        }
    }
    let xml = format!("<{name}></{name} \r\n>");
    let mut parser = Parser::new(Config::default());
    parser.set_reparse_deferral_enabled(false);
    for (index, byte) in xml.as_bytes().iter().enumerate() {
        parser
            .feed(std::slice::from_ref(byte), index + 1 == xml.len())
            .unwrap();
        while parser.next_event().unwrap().is_some() {}
    }
}

#[test]
fn illegal_less_than_in_attributes_is_not_an_unclosed_token() {
    for (xml, offset) in [
        ("<r a='<'>", 6),
        ("<r a=<0\" b=\"x\">", 5),
        ("<r a='x<", 7),
        ("<r><n a='x<", 10),
    ] {
        for chunk in 1..=xml.len() {
            let error = parse_error(xml.as_bytes(), chunk);
            assert_eq!(error.kind, ErrorKind::InvalidToken, "{xml}, {chunk}");
            assert_eq!(error.position.byte_index, offset, "{xml}, {chunk}");
        }
    }
}

#[test]
fn dtd_entity_literals_and_escaped_attributes_can_contain_less_than() {
    let xml = b"<!DOCTYPE r [<!ENTITY unused '<'>]><r a='&lt;'/>";
    for chunk in 1..=xml.len() {
        let mut parser = Parser::new(Config::default());
        let chunks = xml.chunks(chunk);
        let count = chunks.len();
        for (index, bytes) in chunks.enumerate() {
            parser.feed(bytes, index + 1 == count).unwrap();
            while parser.next_event().unwrap().is_some() {}
        }
    }
}
