use oriole::{Config, Error, ErrorKind, Parser};

fn parse_error(xml: &[u8], chunk: usize) -> Error {
    let mut parser = Parser::new(Config::default());
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
