use oriole::{Config, ErrorKind, EventKind, Limits, Parser};

fn parse(xml: &[u8], chunk: usize, config: Config) -> Result<Vec<EventKind>, ErrorKind> {
    let mut parser = Parser::new(config);
    let mut events = Vec::new();
    let mut input = xml.chunks(chunk).peekable();
    while let Some(bytes) = input.next() {
        parser
            .feed(bytes, input.peek().is_none())
            .map_err(|error| error.kind)?;
        while let Some(event) = parser.next_event().map_err(|error| error.kind)? {
            if let EventKind::Text(text) = event.kind {
                if let Some(EventKind::Text(previous)) = events.last_mut() {
                    previous.push_str(&text);
                } else {
                    events.push(EventKind::Text(text));
                }
            } else {
                events.push(event.kind);
            }
        }
    }
    assert!(parser.is_finished());
    Ok(events)
}

fn every_chunk(xml: &str) -> Vec<EventKind> {
    let expected = parse(xml.as_bytes(), xml.len(), Config::default()).unwrap();
    for chunk in 1..=xml.len() {
        assert_eq!(
            parse(xml.as_bytes(), chunk, Config::default()).unwrap(),
            expected,
            "chunk {chunk}"
        );
    }
    expected
}

#[test]
fn streaming_tokens_references_and_normalization() {
    let events = every_chunk(
        "<?xml version='1.0'?><!-- prolog --><r a=' x\r\ny &#x9; &amp; '><n/>hé😀\r\n&amp;&#x1F600;<![CDATA[x<>&\r\ny]]><?target data?></r><!--done-->",
    );
    assert!(events.contains(&EventKind::Text("hé😀\n&😀".into())));
    assert!(events.contains(&EventKind::Text("x<>&\ny".into())));
    assert!(events.iter().any(|event| matches!(event, EventKind::StartElement { attributes, .. } if attributes.first().is_some_and(|attr| attr.value == " x y \t & "))));
}

#[test]
fn namespaces_and_triplets() {
    let xml = b"<r xmlns='urn:r' xmlns:a='urn:a' a:x='1' x='2'><a:c xmlns=''/></r>";
    let config = Config {
        namespace_separator: Some('|'),
        namespace_triplets: true,
        ..Config::default()
    };
    let events = parse(xml, 1, config).unwrap();
    assert!(events.iter().any(|event| matches!(event, EventKind::StartElement { name, attributes } if name == "urn:r|r" && attributes[0].name == "urn:a|x|a" && attributes[1].name == "x")));
    assert!(
        events.iter().any(
            |event| matches!(event, EventKind::StartElement { name, .. } if name == "urn:a|c|a")
        )
    );
}

#[test]
fn dtd_entities_defaults_and_content_models() {
    let events = every_chunk(
        "<!DOCTYPE r [<!ELEMENT r (#PCDATA|b)*><!ELEMENT b EMPTY><!ENTITY word 'hello'><!ENTITY markup '<b/>&word;'><!ATTLIST r a CDATA 'default' b NMTOKENS '  a   b  '>]><r>&markup;</r>",
    );
    assert!(events.iter().any(|event| matches!(event, EventKind::StartElement {name, attributes} if name == "r" && attributes[0].value == "default" && !attributes[0].specified && attributes[1].value == "a b")));
    assert!(events.contains(&EventKind::Text("hello".into())));
}

#[test]
fn nested_entity_markup_is_balanced() {
    assert_eq!(
        parse(
            b"<!DOCTYPE r [<!ENTITY e '<a>'>]><r>&e;</a></r>",
            1,
            Config::default()
        ),
        Err(ErrorKind::AsynchronousEntity)
    );
    assert_eq!(
        parse(
            b"<!DOCTYPE r [<!ENTITY e '</r>'>]><r>&e;",
            1,
            Config::default()
        ),
        Err(ErrorKind::AsynchronousEntity)
    );
    assert_eq!(
        parse(
            b"<!DOCTYPE r [<!ENTITY a '&b;'><!ENTITY b '&a;'>]><r>&a;</r>",
            1,
            Config::default()
        ),
        Err(ErrorKind::RecursiveEntityReference)
    );
}

#[test]
fn encodings_and_split_surrogates() {
    let xml = "<?xml version='1.0' encoding='UTF-16'?><r>hé😀</r>";
    for little in [true, false] {
        let mut bytes = if little {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        bytes.extend(xml.encode_utf16().flat_map(|unit| {
            if little {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            }
        }));
        for chunk in 1..=bytes.len() {
            assert!(
                parse(&bytes, chunk, Config::default())
                    .unwrap()
                    .contains(&EventKind::Text("hé😀".into()))
            );
        }
    }
    let latin = b"<?xml version='1.0' encoding='ISO-8859-1'?><r>\xe9</r>";
    for chunk in 1..=latin.len() {
        assert!(
            parse(latin, chunk, Config::default())
                .unwrap()
                .contains(&EventKind::Text("é".into()))
        );
    }
}

#[test]
fn reject_malformed_xml() {
    for (xml, kind) in [
        ("<a></b>", ErrorKind::TagMismatch),
        ("<a/><b/>", ErrorKind::JunkAfterDocumentElement),
        ("<a x='1' x='2'/>", ErrorKind::DuplicateAttribute),
        ("<a>&missing;</a>", ErrorKind::UndefinedEntity),
        ("<a>&#0;</a>", ErrorKind::BadCharacterReference),
        ("<a>&#xD800;</a>", ErrorKind::BadCharacterReference),
        ("<a>]]></a>", ErrorKind::InvalidToken),
        ("<a><!--a--b--></a>", ErrorKind::InvalidToken),
        ("<a x='<x>'/>", ErrorKind::InvalidToken),
        ("<1/>", ErrorKind::InvalidToken),
        (
            " <?xml version='1.0'?><r/>",
            ErrorKind::MisplacedXmlDeclaration,
        ),
        ("<r><![CDATA[x</r>", ErrorKind::UnclosedCdataSection),
    ] {
        for chunk in 1..=xml.len() {
            assert_eq!(
                parse(xml.as_bytes(), chunk, Config::default()),
                Err(kind),
                "{xml}, chunk {chunk}"
            );
        }
    }
}

#[test]
fn resource_limits_cannot_be_bypassed_by_chunking() {
    let config = Config {
        limits: Limits {
            max_depth: 2,
            ..Limits::default()
        },
        ..Config::default()
    };
    assert_eq!(
        parse(b"<a><b><c/></b></a>", 1, config),
        Err(ErrorKind::LimitExceeded)
    );
    let config = Config {
        limits: Limits {
            max_token_bytes: 12,
            ..Limits::default()
        },
        ..Config::default()
    };
    assert_eq!(
        parse(b"<a long='xxxxxxxxxxxx'/>", 1, config),
        Err(ErrorKind::LimitExceeded)
    );
    let config = Config {
        limits: Limits {
            max_entity_expansion_bytes: 16,
            ..Limits::default()
        },
        ..Config::default()
    };
    assert_eq!(
        parse(
            b"<!DOCTYPE r [<!ENTITY a '123456789'>]><r>&a;&a;</r>",
            1,
            config
        ),
        Err(ErrorKind::LimitExceeded)
    );
}

#[test]
fn namespace_constraints() {
    for (xml, kind) in [
        ("<p:r/>", ErrorKind::UndefinedPrefix),
        ("<r xmlns:xml='wrong'/>", ErrorKind::ReservedPrefixXml),
        ("<r xmlns:xmlns='wrong'/>", ErrorKind::ReservedPrefixXmlns),
        (
            "<r xmlns:a='same' xmlns:b='same' a:x='1' b:x='2'/>",
            ErrorKind::DuplicateAttribute,
        ),
        ("<r xmlns:p=''/>", ErrorKind::UndeclaringPrefix),
    ] {
        assert_eq!(
            parse(
                xml.as_bytes(),
                1,
                Config {
                    namespace_separator: Some('|'),
                    ..Config::default()
                }
            ),
            Err(kind)
        );
    }
}

#[test]
fn byte_line_and_column_positions() {
    let mut parser = Parser::new(Config::default());
    parser.feed("<r>é\r\n<n/></r>".as_bytes(), true).unwrap();
    let mut positions = Vec::new();
    while let Some(event) = parser.next_event().unwrap() {
        positions.push((event.kind, event.position));
    }
    let position = positions
        .iter()
        .find(|(event, _)| matches!(event, EventKind::StartElement {name, ..} if name == "n"))
        .unwrap()
        .1;
    assert_eq!(
        (
            position.byte_index,
            position.line,
            position.column,
            position.byte_count
        ),
        (7, 2, 0, 4)
    );
}

#[test]
fn dtd_comments_do_not_confuse_subset_scanning() {
    every_chunk("<!DOCTYPE r [<!-- ] > [ --><?pi ] > ?><!ENTITY x ']>'>]><r>&x;</r>");
}

#[test]
fn external_entities_are_explicitly_rejected_without_io() {
    assert_eq!(
        parse(
            b"<!DOCTYPE r [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><r>&x;</r>",
            1,
            Config::default()
        ),
        Err(ErrorKind::ExternalEntityHandling)
    );
    assert_eq!(
        parse(
            b"<!DOCTYPE r [<!ENTITY % x SYSTEM 'https://example.com'>%x;]><r/>",
            1,
            Config::default()
        ),
        Err(ErrorKind::ExternalEntityHandling)
    );
}
