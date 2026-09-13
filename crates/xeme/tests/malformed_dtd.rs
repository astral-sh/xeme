use xeme::{Config, ErrorKind, Parser};

fn parse(bytes: &[u8], width: usize, external: bool) -> Result<(), ErrorKind> {
    let parent = Parser::new(Config::default());
    let mut parser = if external {
        parent.external_child_with_encoding(None, None).unwrap()
    } else {
        parent
    };
    assert!(parser.set_param_entity_parsing(2));
    let chunks = bytes.len().div_ceil(width);
    for (index, bytes) in bytes.chunks(width).enumerate() {
        parser
            .feed(bytes, index + 1 == chunks)
            .map_err(|error| error.kind)?;
        while parser.next_event().map_err(|error| error.kind)?.is_some() {}
    }
    Ok(())
}

#[test]
fn malformed_dtd_tokens_have_lexical_errors_in_every_encoding_boundary() {
    for (xml, external, kind) in [
        ("<!DOCTYPE 1+ []><r/>", false, ErrorKind::InvalidToken),
        ("<!DOCTYPE r:!bad []><r/>", false, ErrorKind::InvalidToken),
        ("<!DOCTYPE 1 []><r/>", false, ErrorKind::Syntax),
        ("<!DOCTYPE r+ []><r/>", false, ErrorKind::Syntax),
        ("<!DOCTYPE <foo []><r/>", false, ErrorKind::Syntax),
        ("<!ATTLIST <r a CDATA 'x'>", true, ErrorKind::Syntax),
        ("<!ATTLIST $r a CDATA 'x'>", true, ErrorKind::InvalidToken),
        (
            "<!ATTLIST r a CDATA #!IMPLIED>",
            true,
            ErrorKind::InvalidToken,
        ),
        ("<!ATTLIST r a CDATA #1>", true, ErrorKind::InvalidToken),
        ("<!ATTLIST r a CDATA #OTHER>", true, ErrorKind::Syntax),
        ("$", true, ErrorKind::InvalidToken),
        ("word", true, ErrorKind::Syntax),
        ("'unterminated", true, ErrorKind::UnclosedToken),
        ("'complete'", true, ErrorKind::Syntax),
    ] {
        let utf16: Vec<_> = [0xfeff]
            .into_iter()
            .chain(xml.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        for bytes in [xml.as_bytes(), utf16.as_slice()] {
            for width in 1..=bytes.len() {
                assert_eq!(parse(bytes, width, external), Err(kind), "{xml:?}, {width}");
            }
        }
    }
}

#[test]
fn invalid_public_ids_and_forbidden_document_closures_have_specific_errors() {
    for (xml, kind) in [
        (
            "<!DOCTYPE r PUBLIC '{bad}' 'system'><r/>",
            ErrorKind::PublicId,
        ),
        (
            "<!DOCTYPE r [<!ENTITY e PUBLIC '{bad}' 'system'>]><r/>",
            ErrorKind::PublicId,
        ),
        (
            "<!DOCTYPE r [<!NOTATION n PUBLIC '{bad}'>]><r/>",
            ErrorKind::PublicId,
        ),
        ("<!DOCTYPE r></r>", ErrorKind::InvalidToken),
        ("<r/></r>", ErrorKind::InvalidToken),
        (
            "<!DOCTYPE r [<!ENTITY % e ']><r/>'>%e;",
            ErrorKind::InvalidToken,
        ),
    ] {
        for width in 1..=xml.len() {
            assert_eq!(
                parse(xml.as_bytes(), width, false),
                Err(kind),
                "{xml:?}, {width}"
            );
        }
    }
}
