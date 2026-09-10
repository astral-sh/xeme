use oriole::{Config, ErrorKind, EventKind, Limits, Parser};

fn text<T: From<oriole_storage::String>>(value: &str) -> T {
    oriole_storage::String::try_from_str_in(value, oriole_storage::Allocator::System)
        .unwrap()
        .into()
}

fn parse(xml: &[u8], chunk: usize, config: Config) -> Result<Vec<EventKind>, ErrorKind> {
    let mut parser = Parser::new(config);
    let mut events = Vec::new();
    let mut pending: Option<String> = None;
    let mut input = xml.chunks(chunk).peekable();
    while let Some(bytes) = input.next() {
        parser
            .feed(bytes, input.peek().is_none())
            .map_err(|error| error.kind)?;
        while let Some(event) = parser.next_event().map_err(|error| error.kind)? {
            if let EventKind::Text(text) = event.kind {
                let previous = pending.get_or_insert_with(String::new);
                previous
                    .try_reserve(text.len())
                    .map_err(|_| ErrorKind::NoMemory)?;
                previous.push_str(&text);
            } else {
                if let Some(text) = pending.take() {
                    events.push(EventKind::Text(
                        oriole::Text::try_from_str_in(&text, parser.allocator())
                            .map_err(|_| ErrorKind::NoMemory)?,
                    ));
                }
                events.push(event.kind);
            }
        }
    }
    if let Some(text) = pending {
        events.push(EventKind::Text(
            oriole::Text::try_from_str_in(&text, parser.allocator())
                .map_err(|_| ErrorKind::NoMemory)?,
        ));
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
fn attribute_duplicates_and_defaults_across_small_and_large_elements() {
    for count in [1, 8, 9, 32] {
        let attributes: String = (0..count).map(|i| format!(" a{i}='{i}'")).collect();
        let valid = format!(
            "<!DOCTYPE r [<!ATTLIST r a0 CDATA 'default' extra CDATA 'added'>]><r{attributes}/>"
        );
        for chunk in [1, 7, valid.len()] {
            let events = parse(valid.as_bytes(), chunk, Config::default()).unwrap();
            let Some(EventKind::StartElement { attributes, .. }) = events
                .iter()
                .find(|event| matches!(event, EventKind::StartElement { .. }))
            else {
                panic!("missing root element");
            };
            assert_eq!(attributes.len(), count + 1);
            assert_eq!(attributes[0].value, "0");
            assert!(attributes[0].specified);
            assert_eq!(attributes[count].name, "extra");
            assert!(!attributes[count].specified);
        }
        let duplicate = format!("<r{attributes} a0='duplicate'/>");
        for chunk in [1, 7, duplicate.len()] {
            assert_eq!(
                parse(duplicate.as_bytes(), chunk, Config::default()),
                Err(ErrorKind::DuplicateAttribute)
            );
        }
    }
}

#[test]
fn streaming_tokens_references_and_normalization() {
    let events = every_chunk(
        "<?xml version='1.0'?><!-- prolog --><r a=' x\r\ny &#x9; &amp; '><n/>hé😀\r\n&amp;&#x1F600;<![CDATA[x<>&\r\ny]]><?target data?></r><!--done-->",
    );
    assert!(events.contains(&EventKind::Text(text("hé😀\n&😀"))));
    assert!(events.contains(&EventKind::Text(text("x<>&\ny"))));
    assert!(events.iter().any(|event| matches!(event, EventKind::StartElement { attributes, .. } if attributes.first().is_some_and(|attr| attr.value == " x y \t & "))));
}

#[test]
fn plain_attributes_and_short_references_keep_normalization_and_errors() {
    let events = every_chunk(
        "<!DOCTYPE r [<!ENTITY e 'entity'>]><r empty='' plain='hé😀' space=' x\t y\r\nz ' escaped='&lt;&amp;&#13;'>&lt;&gt;&amp;&apos;&quot;&#65;&#x1F600;&e;</r>",
    );
    let attributes = events
        .iter()
        .find_map(|event| {
            if let EventKind::StartElement { attributes, .. } = event {
                Some(attributes)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        attributes
            .iter()
            .map(|attribute| attribute.value.as_str())
            .collect::<Vec<_>>(),
        ["", "hé😀", " x  y z ", "<&\r"]
    );
    assert!(events.contains(&EventKind::Text(text("<>&'\"A😀entity"))));
    for xml in ["<r>&#0;</r>", "<r>&#x110000;</r>"] {
        for chunk in 1..=xml.len() {
            assert_eq!(
                parse(xml.as_bytes(), chunk, Config::default()),
                Err(ErrorKind::BadCharacterReference)
            );
        }
    }
    assert_eq!(
        parse(
            b"<!DOCTYPE r [<!ENTITY e '<'>]><r a='&e;'/>",
            1,
            Config::default()
        ),
        Err(ErrorKind::InvalidToken)
    );
}

#[test]
fn default_events_preserve_whitespace_without_character_data_callbacks() {
    let xml = "<?test processing instruction?>\r\n<!DOCTYPE r [ \n<!-- comment -->\t<!ELEMENT r EMPTY>\r\n]>\n<r/> \r\n";
    let ordinary = parse(xml.as_bytes(), 1, Config::default()).unwrap();
    assert!(
        !ordinary
            .iter()
            .any(|event| matches!(event, EventKind::Default | EventKind::Text(_)))
    );
    for chunk in 1..=xml.len() {
        let mut parser = Parser::new(Config::default());
        parser.set_default_events(true);
        let mut raw = String::new();
        for bytes in xml.as_bytes().chunks(chunk) {
            parser.feed(bytes, false).unwrap();
            while let Some(event) = parser.next_event().unwrap() {
                assert!(!matches!(event.kind, EventKind::Text(_)));
                raw.push_str(parser.current_raw().unwrap_or_default());
            }
        }
        parser.feed(&[], true).unwrap();
        while let Some(event) = parser.next_event().unwrap() {
            assert!(!matches!(event.kind, EventKind::Text(_)));
            raw.push_str(parser.current_raw().unwrap_or_default());
        }
        assert_eq!(raw, xml, "chunk {chunk}");
    }
    let mut parser = Parser::new(Config::default());
    parser.set_default_events(true);
    parser.feed(b"\n<r/>\n", true).unwrap();
    assert_eq!(
        parser.next_event().unwrap().unwrap().kind,
        EventKind::Default
    );
    parser.set_default_events(false);
    while let Some(event) = parser.next_event().unwrap() {
        assert!(!matches!(event.kind, EventKind::Default));
    }

    parser.set_default_events(true);
    let mut child = parser.external_child(None, None).unwrap();
    child.feed(b" \r\n", true).unwrap();
    assert_eq!(
        child.next_event().unwrap().unwrap().kind,
        EventKind::Default
    );
    assert_eq!(child.current_raw(), Some(" \r\n"));
    assert!(child.next_event().unwrap().is_none());
}

#[test]
fn truncated_cdata_openers_and_utf16_units_have_contextual_errors() {
    for (xml, kind) in [
        ("<![", ErrorKind::Syntax),
        ("<![<a/>", ErrorKind::Syntax),
        ("<a/><![", ErrorKind::JunkAfterDocumentElement),
        ("<a/><![<a/>", ErrorKind::JunkAfterDocumentElement),
        ("<a><![<a/>", ErrorKind::UnclosedToken),
        ("<a><![C<a/>", ErrorKind::UnclosedToken),
        ("<a><![CD<a/>", ErrorKind::InvalidToken),
        ("<a><![CDATA[", ErrorKind::UnclosedCdataSection),
    ] {
        for chunk in 1..=xml.len() {
            assert_eq!(parse(xml.as_bytes(), chunk, Config::default()), Err(kind));
        }
    }
    let mut xml = vec![0xfe, 0xff];
    for unit in "<a><![CDATA[Z".encode_utf16() {
        xml.extend_from_slice(&unit.to_be_bytes());
    }
    for (tail, kind) in [
        (&[0][..], ErrorKind::UnclosedCdataSection),
        (&[0xd8][..], ErrorKind::UnclosedCdataSection),
        (&[0xd8, 0x34][..], ErrorKind::PartialCharacter),
        (&[0xd8, 0x34, 0xdd][..], ErrorKind::PartialCharacter),
    ] {
        let mut input = xml.clone();
        input.extend_from_slice(tail);
        for chunk in 1..=input.len() {
            assert_eq!(parse(&input, chunk, Config::default()), Err(kind));
        }
    }
}

#[test]
fn external_fragments_emit_trailing_text_before_reporting_unbalanced_markup() {
    let parent = Parser::new(Config::default());
    for xml in ["<tag>\r", "<tag>]", "<tag>]]"] {
        for chunk in 1..=xml.len() {
            let mut child = parent.external_child(Some(""), None).unwrap();
            let mut text = String::new();
            for bytes in xml.as_bytes().chunks(chunk) {
                child.feed(bytes, false).unwrap();
                while let Some(event) = child.next_event().unwrap() {
                    if let EventKind::Text(value) = event.kind {
                        text.push_str(&value);
                    }
                }
            }
            child.feed(&[], true).unwrap();
            let error = loop {
                match child.next_event() {
                    Ok(Some(event)) => {
                        if let EventKind::Text(value) = event.kind {
                            text.push_str(&value);
                        }
                    }
                    Err(error) => break error,
                    Ok(None) => panic!("unbalanced external fragment was accepted"),
                }
            };
            assert_eq!(error.kind, ErrorKind::AsynchronousEntity);
            assert_eq!(text, xml[5..].replace('\r', "\n"));
        }
    }
}

#[test]
fn numeric_reference_syntax_is_distinct_from_forbidden_characters() {
    for (reference, kind) in [
        ("&#;", ErrorKind::InvalidToken),
        ("&#x;", ErrorKind::InvalidToken),
        ("&#+1;", ErrorKind::InvalidToken),
        ("&#-1;", ErrorKind::InvalidToken),
        ("&#X41;", ErrorKind::InvalidToken),
        ("&#x4]]>1;", ErrorKind::InvalidToken),
        ("&#x41]]>;", ErrorKind::InvalidToken),
        ("&#0;", ErrorKind::BadCharacterReference),
        ("&#xD800;", ErrorKind::BadCharacterReference),
        ("&#x110000;", ErrorKind::BadCharacterReference),
        ("&#99999999999999999999;", ErrorKind::BadCharacterReference),
    ] {
        for xml in [
            format!("<r>{reference}</r>"),
            format!("<r a='{reference}'/>"),
            format!("<!DOCTYPE r [<!ENTITY e '{reference}'>]><r/>"),
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
    for (xml, byte_index) in [
        ("<r>&#;</r>", 5),
        ("<r>&#x;</r>", 6),
        ("<r>&#x4]]>1;</r>", 7),
    ] {
        let mut parser = Parser::new(Config::default());
        parser.feed(xml.as_bytes(), true).unwrap();
        loop {
            match parser.next_event() {
                Ok(Some(_)) => {}
                Err(error) => {
                    assert_eq!(error.position.byte_index, byte_index);
                    break;
                }
                Ok(None) => panic!("invalid reference was accepted"),
            }
        }
    }
}

#[test]
fn malformed_prolog_text_and_dtd_repetition_markers_report_their_context() {
    for (xml, kind) in [
        (&b"]]><r/>"[..], ErrorKind::Syntax),
        (&b" \n]]><r/>"[..], ErrorKind::Syntax),
        (&b"<r/>]]>"[..], ErrorKind::JunkAfterDocumentElement),
        (&b"Lo\xa7\x94"[..], ErrorKind::InvalidToken),
        (
            &b"<!DOCTYPE r [<!ELEMENT r (a |b * >]><r/>"[..],
            ErrorKind::InvalidToken,
        ),
        (
            &b"<!DOCTYPE r [<!ELEMENT r (a) *>]><r/>"[..],
            ErrorKind::InvalidToken,
        ),
    ] {
        for chunk in 1..=xml.len() {
            assert_eq!(
                parse(xml, chunk, Config::default()),
                Err(kind),
                "{xml:?}, chunk {chunk}"
            );
        }
    }
    assert!(
        parse(
            b"<!DOCTYPE r [<!ELEMENT r (a )*>]><r/>",
            1,
            Config::default()
        )
        .is_ok()
    );
}

#[test]
fn unfinished_prolog_names_respect_the_token_limit() {
    for chunk in [1, 17, 64] {
        assert_eq!(
            parse(
                &[b'a'; 64],
                chunk,
                Config {
                    limits: Limits {
                        max_token_bytes: 32,
                        ..Limits::default()
                    },
                    ..Config::default()
                },
            ),
            Err(ErrorKind::LimitExceeded)
        );
    }
}

#[test]
fn repeated_external_entity_identifiers_consume_the_shared_expansion_budget() {
    for (xml, parameter, limit) in [
        (
            "<!DOCTYPE r [<!ENTITY e PUBLIC 'public' 'system'>]><r>&e;&e;&e;</r>",
            false,
            26,
        ),
        (
            "<!DOCTYPE r [<!ENTITY % e PUBLIC 'public' 'system'>%e;%e;%e;]><r/>",
            true,
            // Two sets of identifiers plus the shared parameter-read marker.
            24 + size_of::<bool>(),
        ),
    ] {
        for chunk in 1..=xml.len() {
            let mut parser = Parser::new(Config {
                limits: Limits {
                    max_entity_expansion_bytes: limit,
                    ..Limits::default()
                },
                ..Config::default()
            });
            if parameter {
                assert!(parser.set_param_entity_parsing(2));
            }
            let mut references = 0;
            let mut failure = None;
            'input: for bytes in xml.as_bytes().chunks(chunk) {
                parser.feed(bytes, false).unwrap();
                loop {
                    match parser.next_event() {
                        Ok(Some(event)) => {
                            if matches!(event.kind, EventKind::ExternalEntityReference(_)) {
                                references += 1;
                            }
                        }
                        Ok(None) => break,
                        Err(error) => {
                            failure = Some(error.kind);
                            break 'input;
                        }
                    }
                }
            }
            assert_eq!(
                failure,
                Some(ErrorKind::LimitExceeded),
                "chunk {chunk}, {xml}"
            );
            assert_eq!(references, 2);
        }
    }
}

#[test]
fn external_entity_namespace_context_consumes_the_expansion_budget() {
    let mut parser = Parser::new(Config {
        namespace_separator: Some('|'),
        limits: Limits {
            max_entity_expansion_bytes: 180,
            ..Limits::default()
        },
        ..Config::default()
    });
    let xml = format!(
        "<!DOCTYPE r [<!ENTITY e SYSTEM 'sys'>]><r xmlns:p='urn:{}'>&e;&e;</r>",
        "x".repeat(64)
    );
    parser.feed(xml.as_bytes(), true).unwrap();
    let context = loop {
        let event = parser.next_event().unwrap().unwrap();
        if let EventKind::ExternalEntityReference(declaration) = event.kind {
            let oriole::ExternalEntityReference { context, .. } =
                oriole_storage::Box::into_inner(declaration);

            break context.unwrap();
        }
    };
    assert!(context.contains("p=urn:"));
    assert_eq!(
        parser.next_event().unwrap_err().kind,
        ErrorKind::LimitExceeded
    );
}

#[test]
fn attribute_declaration_types_omit_grammar_whitespace() {
    let events = every_chunk(
        "<!DOCTYPE r [<!ATTLIST r a ( one | two | three ) #REQUIRED b NOTATION \t( foo | bar ) #IMPLIED c NOTATION (foo) 'bar' d CDATA 'é'>]><r a='two'/>",
    );
    let declarations: Vec<_> = events
        .iter()
        .filter_map(|event| {
            if let EventKind::AttlistDeclaration(declaration) = event {
                let oriole::AttributeDeclaration {
                    attribute_type,
                    default,
                    required,
                    ..
                } = declaration.as_ref();

                Some((attribute_type.as_str(), default.as_deref(), *required))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        declarations,
        [
            ("(one|two|three)", None, true),
            ("NOTATION(foo|bar)", None, false),
            ("NOTATION(foo)", Some("bar"), false),
            ("CDATA", Some("é"), false),
        ]
    );
}

#[test]
fn namespaces_apply_to_dtd_names_and_entity_references() {
    let namespaces = Config {
        namespace_separator: Some('|'),
        ..Config::default()
    };
    // These are XML Names in a parser without namespaces. Namespace-aware
    // declarations instead require QNames, or NCNames for entities/notations.
    for xml in [
        "<!DOCTYPE a:b:c><r/>",
        "<!DOCTYPE :r><r/>",
        "<!DOCTYPE r:><r/>",
        "<!DOCTYPE p:1r><r/>",
        "<!DOCTYPE r [<!ENTITY a:b 'x'>]><r/>",
        "<!DOCTYPE r [<!ENTITY % a:b 'x'>]><r/>",
        "<!DOCTYPE r [<!NOTATION a:b SYSTEM 'x'>]><r/>",
        "<!DOCTYPE r [<!ENTITY e SYSTEM 'x' NDATA a:b>]><r/>",
        "<!DOCTYPE r [<!ATTLIST r a NOTATION (a:b) #IMPLIED>]><r/>",
        "<!DOCTYPE r [<!ELEMENT a:b:c EMPTY>]><r/>",
        "<!DOCTYPE r [<!ELEMENT r (a:b:c)>]><r/>",
        "<!DOCTYPE r [<!ELEMENT r (#PCDATA|a:b:c)*>]><r/>",
        "<!DOCTYPE r [<!ATTLIST a:b:c a CDATA #IMPLIED>]><r/>",
        "<!DOCTYPE r [<!ATTLIST r a:b:c CDATA #IMPLIED>]><r/>",
    ] {
        assert!(parse(xml.as_bytes(), 1, Config::default()).is_ok(), "{xml}");
        for chunk in 1..=xml.len() {
            assert_eq!(
                parse(xml.as_bytes(), chunk, namespaces.clone()),
                Err(ErrorKind::Syntax),
                "{xml}, chunk {chunk}"
            );
        }
    }
    // Validate before skipping an unresolved entity in an unread external DTD.
    for xml in [
        "<!DOCTYPE r SYSTEM 'x'><r>&a:b;</r>",
        "<!DOCTYPE r SYSTEM 'x'><r a='&a:b;'/>",
        "<!DOCTYPE r [%a:b;]><r/>",
        "<!DOCTYPE r [<!ENTITY e '&a:b;'>]><r/>",
        "<!DOCTYPE r SYSTEM 'x' [<!ATTLIST r a CDATA '&a:b;'>]><r/>",
    ] {
        assert!(parse(xml.as_bytes(), 1, Config::default()).is_ok(), "{xml}");
        for chunk in 1..=xml.len() {
            assert_eq!(
                parse(xml.as_bytes(), chunk, namespaces.clone()),
                Err(ErrorKind::InvalidToken),
                "{xml}, chunk {chunk}"
            );
        }
    }
    // DTD prefixes need no binding until use. Ordinary enumerations are
    // NMTOKENs and can contain colons; NOTATION enumerations are NCNames.
    let xml = "<!DOCTYPE p:r [<!ELEMENT p:r (p:child)><!ATTLIST p:r p:a (a:b|:c:) 'a:b'><!NOTATION n SYSTEM 'x'><!ATTLIST p:r kind NOTATION (n) #IMPLIED>]><p:r xmlns:p='urn:p'/>";
    let expected = parse(xml.as_bytes(), xml.len(), namespaces.clone()).unwrap();
    for chunk in 1..=xml.len() {
        assert_eq!(
            parse(xml.as_bytes(), chunk, namespaces.clone()).unwrap(),
            expected
        );
    }
    assert!(expected.iter().any(|event| matches!(event,
        EventKind::StartElement { name, attributes }
        if name == "urn:p|r" && attributes.iter().any(|attribute|
            attribute.name == "urn:p|a" && attribute.value == "a:b"))));
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
    assert!(events.contains(&EventKind::Text(text("hello"))));
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
fn character_reference_carriage_returns_survive_entity_replacement() {
    let events = every_chunk(
        "<!DOCTYPE r [<!ENTITY e 'a&#13;b<![CDATA[c&#13;d]]><!--e&#13;f--><?pi g&#13;h?>'><!ENTITY nl 'a\r\nb'>]><r>&e;&nl;</r>",
    );
    assert!(events.contains(&EventKind::Text(text("a\rb"))));
    assert!(events.contains(&EventKind::Text(text("c\rd"))));
    assert!(events.contains(&EventKind::Text(text("a\nb"))));
    assert!(events.contains(&EventKind::Comment(text("e\rf"))));
    assert!(events.contains(&EventKind::ProcessingInstruction {
        target: text("pi"),
        data: text("g\rh")
    }));
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
                    .contains(&EventKind::Text(text("hé😀")))
            );
        }
    }
    let latin = b"<?xml version='1.0' encoding='ISO-8859-1'?><r>\xe9</r>";
    for chunk in 1..=latin.len() {
        assert!(
            parse(latin, chunk, Config::default())
                .unwrap()
                .contains(&EventKind::Text(text("é")))
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
fn reused_default_attributes_consume_the_expansion_budget() {
    let documents = [
        "<!DOCTYPE r [<!ATTLIST item a CDATA '12345678'>]><r><item/><item/></r>",
        "<!DOCTYPE r [<!ENTITY e '12345678'><!ATTLIST item a CDATA '&e;'>]><r><item/><item/></r>",
        "<!DOCTYPE r [<!ATTLIST item abcdefghij CDATA ''>]><r><item/><item/></r>",
    ];
    for xml in documents {
        for chunk in [1, 7, xml.len()] {
            let config = Config {
                limits: Limits {
                    max_entity_expansion_bytes: 16,
                    ..Limits::default()
                },
                ..Config::default()
            };
            assert_eq!(
                parse(xml.as_bytes(), chunk, config),
                Err(ErrorKind::LimitExceeded),
                "{xml}, chunk {chunk}"
            );
        }
    }
    // Explicit attributes do not reuse the declared default and must not be charged.
    let config = Config {
        limits: Limits {
            max_entity_expansion_bytes: 0,
            ..Limits::default()
        },
        ..Config::default()
    };
    assert!(parse(b"<!DOCTYPE r [<!ATTLIST item a CDATA 'default'>]><r><item a='own'/><item a='own'/></r>", 1, config).is_ok());
}

#[test]
fn reused_namespace_uris_and_declaration_names_consume_the_expansion_budget() {
    for xml in [
        "<r xmlns:p='0123456789'><p:a/><p:a/></r>",
        "<r xmlns:p='0123456789'><a p:x='1' p:y='2'/></r>",
        "<!DOCTYPE r [<!ATTLIST abcdefghij a CDATA #IMPLIED b CDATA #IMPLIED c CDATA #IMPLIED>]><r/>",
    ] {
        for chunk in [1, 7, xml.len()] {
            let config = Config {
                namespace_separator: Some('|'),
                limits: Limits {
                    max_entity_expansion_bytes: 16,
                    ..Limits::default()
                },
                ..Config::default()
            };
            assert_eq!(
                parse(xml.as_bytes(), chunk, config),
                Err(ErrorKind::LimitExceeded),
                "{xml}, chunk {chunk}"
            );
        }
    }
}

#[test]
fn namespace_separators_preserve_legacy_uri_characters() {
    for (separator, document, expected) in [
        (':', "<p:r xmlns:p='urn:a:b'/>", "urn:a:b:r"),
        ('!', "<p:r xmlns:p='urn:a!b'/>", "urn:a!b!r"),
        ('|', "<p:r xmlns:p='urn:a:b'/>", "urn:a:b|r"),
    ] {
        for width in [1, 7, document.len()] {
            let events = parse(
                document.as_bytes(),
                width,
                Config {
                    namespace_separator: Some(separator),
                    ..Config::default()
                },
            )
            .unwrap();
            assert!(events.iter().any(|event| matches!(
                event, EventKind::StartElement { name, .. } if name == expected
            )));
        }
    }
    for (separator, document) in [
        ('|', "<r xmlns='urn:a|b'/>"),
        ('\n', "<r xmlns='urn:a&#10;b'/>"),
        ('}', "<r xmlns='urn:a&#125;b'/>"),
    ] {
        assert_eq!(
            parse(
                document.as_bytes(),
                1,
                Config {
                    namespace_separator: Some(separator),
                    ..Config::default()
                }
            ),
            Err(ErrorKind::Syntax)
        );
    }
}

#[test]
fn namespace_constraints() {
    for (xml, kind) in [
        ("<p:r/>", ErrorKind::UndefinedPrefix),
        ("<r xmlns:='urn:r'/>", ErrorKind::InvalidToken),
        ("<r xmlns:=''/>", ErrorKind::InvalidToken),
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
fn unprefixed_attributes_can_match_serialized_namespace_names() {
    let accepted = "<r xmlns:p='a' p:b='1' axb='2'/>";
    let rejected = "<r xmlns:p='ax' xmlns:q='a' p:b='1' q:xb='2'/>";
    for triplets in [false, true] {
        let config = Config {
            namespace_separator: Some('x'),
            namespace_triplets: triplets,
            ..Config::default()
        };
        for chunk in 1..=accepted.len() {
            let events = parse(accepted.as_bytes(), chunk, config.clone()).unwrap();
            let attributes = events
                .iter()
                .find_map(|event| match event {
                    EventKind::StartElement { attributes, .. } => Some(attributes),
                    _ => None,
                })
                .unwrap();
            assert_eq!(attributes.len(), 2);
            assert_eq!(attributes[0].name, if triplets { "axbxp" } else { "axb" });
            assert_eq!(attributes[1].name, "axb");
        }
        // Expat compares serialized expanded names among prefixed attributes,
        // even when the separator also appears in their URIs or local names.
        for chunk in 1..=rejected.len() {
            assert_eq!(
                parse(rejected.as_bytes(), chunk, config.clone()),
                Err(ErrorKind::DuplicateAttribute),
            );
        }
    }
}

#[test]
fn newer_minor_versions_keep_xml_10_character_rules() {
    for version in ["1.0", "1.1", "1.7", "1.01", "1.000"] {
        let xml = format!("<?xml version='{version}'?><r/>");
        for width in [1, 7, xml.len()] {
            let events = parse(xml.as_bytes(), width, Config::default()).unwrap();
            assert!(matches!(
                &events[0],
                EventKind::XmlDeclaration { version: actual, .. } if actual == version
            ));
        }
        let invalid = format!("<?xml version='{version}'?><r>&#x1;</r>");
        assert_eq!(
            parse(invalid.as_bytes(), 1, Config::default()),
            Err(ErrorKind::BadCharacterReference)
        );
    }
    for version in ["", "1.", "1.x", "2.0", "01.0", "1.\u{0661}"] {
        let xml = format!("<?xml version='{version}'?><r/>");
        assert_eq!(
            parse(xml.as_bytes(), 1, Config::default()),
            Err(ErrorKind::XmlDeclaration)
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
fn external_entities_require_application_supplied_content() {
    let events = parse(
        b"<!DOCTYPE r [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><r>&x;</r>",
        1,
        Config::default(),
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(event, EventKind::ExternalEntityReference(declaration) if matches!(declaration.as_ref(), oriole::ExternalEntityReference { system_id, .. } if system_id.as_deref() == Some("file:///etc/passwd")))));
    assert!(
        parse(
            b"<!DOCTYPE r [<!ENTITY % x SYSTEM 'https://example.com'>%x;]><r/>",
            1,
            Config::default()
        )
        .is_ok()
    );
}

#[test]
fn external_entity_children_inherit_namespaces_and_declarations() {
    let mut parent = Parser::new(Config {
        namespace_separator: Some('|'),
        ..Config::default()
    });
    parent.feed(b"<!DOCTYPE r [<!ENTITY internal 'hello'><!ENTITY external SYSTEM 'child.xml'>]><r xmlns:p='urn:p'>&external;</r>", true).unwrap();
    while let Some(event) = parent.next_event().unwrap() {
        if let EventKind::ExternalEntityReference(declaration) = event.kind {
            let oriole::ExternalEntityReference { context, .. } =
                oriole_storage::Box::into_inner(declaration);

            let mut child = parent.external_child(context.as_deref(), None).unwrap();
            child
                .feed(b"<?xml encoding='UTF-8'?>text<p:a/>&internal;<p:b/>", true)
                .unwrap();
            let mut events = Vec::new();
            while let Some(event) = child.next_event().unwrap() {
                events.push(event.kind);
            }
            assert!(child.is_finished());
            assert!(events.contains(&EventKind::TextDeclaration {
                version: None,
                encoding: text("UTF-8")
            }));
            assert!(events.contains(&EventKind::Text(text("hello"))));
            assert!(events.iter().any(
                |event| matches!(event, EventKind::StartElement {name, ..} if name == "urn:p|a")
            ));
        }
    }
}

#[test]
fn external_entity_cycles_and_expansion_budgets_are_shared() {
    let mut parent = Parser::new(Config {
        limits: Limits {
            // Inherited declarations, callback metadata, and externally supplied
            // replacement text consume the same budget across all children.
            max_entity_expansion_bytes: 16 * 1024,
            ..Limits::default()
        },
        ..Config::default()
    });
    parent
        .feed(
            b"<!DOCTYPE r [<!ENTITY x SYSTEM 'child.xml'>]><r>&x;</r>",
            true,
        )
        .unwrap();
    while let Some(event) = parent.next_event().unwrap() {
        if let EventKind::ExternalEntityReference(declaration) = event.kind {
            let oriole::ExternalEntityReference { context, .. } =
                oriole_storage::Box::into_inner(declaration);

            let mut child = parent.external_child(context.as_deref(), None).unwrap();
            child.feed(b"&x;", true).unwrap();
            assert_eq!(
                child.next_event().unwrap_err().kind,
                ErrorKind::RecursiveEntityReference
            );
            let mut sibling = parent.external_child(Some(""), None).unwrap();
            sibling.feed(&[b'x'; 8 * 1024], true).unwrap();
            while sibling.next_event().unwrap().is_some() {}
            let mut sibling = parent.external_child(Some(""), None).unwrap();
            assert_eq!(
                sibling.feed(&[b'x'; 8 * 1024], true).unwrap_err().kind,
                ErrorKind::LimitExceeded
            );
        }
    }
}

#[test]
fn custom_single_byte_encoding_retries_buffered_input_once() {
    let mut parser = Parser::new(Config::default());
    parser
        .feed(b"<?xml version='1.0' encoding='custom'?><r>\x80</r>", true)
        .unwrap();
    assert_eq!(
        parser.next_event().unwrap_err().kind,
        ErrorKind::UnknownEncoding
    );
    assert_eq!(parser.unknown_encoding(), Some("custom"));
    let mut map = std::array::from_fn(|index| index as i32);
    map[128] = 0x20ac;
    parser.set_encoding_map("custom", map).unwrap();
    let mut text = String::new();
    while let Some(event) = parser.next_event().unwrap() {
        if let EventKind::Text(value) = event.kind {
            text.push_str(&value);
        }
    }
    assert_eq!(text, "€");
    assert!(parser.is_finished());
}

#[test]
fn queued_dtd_events_have_individual_raw_tokens() {
    let mut parser = Parser::new(Config::default());
    parser
        .feed(
            b"<!DOCTYPE r [<!ENTITY e 'value'><!ELEMENT r EMPTY>]><r/>",
            true,
        )
        .unwrap();
    let mut raw = Vec::new();
    while let Some(event) = parser.next_event().unwrap() {
        raw.push((event.kind, parser.current_raw().unwrap_or("").to_owned()));
    }
    assert_eq!(raw[0].1, "<!DOCTYPE r [");
    assert_eq!(raw[1].1, "<!ENTITY e 'value'>");
    assert_eq!(raw[2].1, "<!ELEMENT r EMPTY>");
    assert_eq!(raw[3].1, "]>");
    assert_eq!(raw[4].1, "<r/>");
    assert_eq!(raw[5].1, "");
}

#[test]
fn raw_tokens_and_owned_events_survive_reusing_the_raw_buffer() {
    let mut parser = Parser::new(Config::default());
    parser
        .feed(b"<r><child/>a&amp;b<![CDATA[c]]></r>", true)
        .unwrap();
    let mut retained = Vec::new();
    let mut raw = Vec::new();
    while let Some(event) = parser.next_event().unwrap() {
        raw.push(parser.current_raw().map(str::to_owned));
        retained.push(event.kind);
    }
    assert_eq!(
        raw,
        [
            Some("<r>"),
            Some("<child/>"),
            None,
            Some("a"),
            Some("&amp;"),
            Some("b"),
            Some("<![CDATA["),
            Some("c"),
            Some("]]>"),
            Some("</r>")
        ]
        .map(|raw| raw.map(str::to_owned))
    );
    assert!(matches!(&retained[1], EventKind::StartElement {name, ..} if name == "child"));
    assert_eq!(retained[3], EventKind::Text(text("a")));
    assert_eq!(retained[4], EventKind::Text(text("&")));
}

#[test]
fn reparse_deferral_is_optional_and_final_input_always_flushes() {
    let mut parser = Parser::new(Config::default());
    parser.feed(b"<r attribute='long", false).unwrap();
    assert!(parser.next_event().unwrap().is_none());
    parser.feed(b"'/>", false).unwrap();
    assert!(parser.next_event().unwrap().is_none());
    parser.set_reparse_deferral_enabled(false);
    assert!(matches!(
        parser.next_event().unwrap().unwrap().kind,
        EventKind::StartElement { .. }
    ));
    while parser.next_event().unwrap().is_some() {}
    parser.feed(b"", true).unwrap();
    assert!(parser.next_event().unwrap().is_none());
    assert!(parser.is_finished());
}

#[test]
fn unread_external_subsets_allow_skipped_entities_without_loading_them() {
    let events = every_chunk("<!DOCTYPE r SYSTEM 'absent'><r a='1&unknown;2'>&unknown;</r>");
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EventKind::SkippedEntity {name, ..} if name == "unknown"))
    );
    assert!(events.iter().any(|event| matches!(event, EventKind::StartElement {attributes, ..} if attributes[0].value == "12")));
}

#[test]
fn external_dtd_declarations_merge_before_document_content() {
    let mut parser = Parser::new(Config::default());
    assert!(parser.set_param_entity_parsing(2));
    parser
        .feed(b"<!DOCTYPE r SYSTEM 'test.dtd'><r>&external;</r>", true)
        .unwrap();
    let mut seen = false;
    while let Some(event) = parser.next_event().unwrap() {
        match event.kind {
            EventKind::ExternalEntityReference(declaration) if declaration.context.is_none() => {
                let mut child = parser.external_child_with_encoding(None, None).unwrap();
                child.feed(b"<?xml encoding='UTF-8'?><!ENTITY external 'loaded'><!ATTLIST r default CDATA 'yes'>", true).unwrap();
                while child.next_event().unwrap().is_some() {}
                parser.merge_external_subset(&child).unwrap();
            }
            EventKind::StartElement { attributes, .. } => assert_eq!(attributes[0].value, "yes"),
            EventKind::Text(value) => {
                assert_eq!(value, "loaded");
                seen = true;
            }
            _ => {}
        }
    }
    assert!(seen);
}

#[test]
fn indexed_declarations_preserve_order_types_ids_and_external_precedence() {
    let mut parser = Parser::new(Config::default());
    assert!(parser.set_param_entity_parsing(2));
    parser.feed(b"<!DOCTYPE r SYSTEM 'test.dtd' [<!ATTLIST r second CDATA 'b' first NMTOKENS 'a' id ID #IMPLIED><!ATTLIST r first CDATA 'ignored' third CDATA 'c'>]><r id='identifier' first=' x  y '/>", true).unwrap();
    let mut seen = false;
    while let Some(event) = parser.next_event().unwrap() {
        match event.kind {
            EventKind::ExternalEntityReference(declaration) if declaration.context.is_none() => {
                let mut child = parser.external_child_with_encoding(None, None).unwrap();
                child
                    .feed(
                        b"<!ATTLIST r second CDATA 'ignored' fourth CDATA 'd'>",
                        true,
                    )
                    .unwrap();
                while child.next_event().unwrap().is_some() {}
                parser.merge_external_subset(&child).unwrap();
            }
            EventKind::StartElement { attributes, .. } => {
                assert_eq!(parser.id_attribute_index(), Some(0));
                let actual: Vec<_> = attributes
                    .iter()
                    .map(|attribute| {
                        (
                            attribute.name.as_str(),
                            attribute.value.as_str(),
                            attribute.specified,
                        )
                    })
                    .collect();
                assert_eq!(
                    actual,
                    [
                        ("id", "identifier", true),
                        ("first", "x y", true),
                        ("second", "b", false),
                        ("third", "c", false),
                        ("fourth", "d", false)
                    ]
                );
                seen = true;
            }
            _ => {}
        }
    }
    assert!(seen);
}

#[test]
fn many_declared_attributes_can_be_repeated_without_indirect_expansion() {
    use std::fmt::Write;
    let mut xml = std::string::String::from("<!DOCTYPE r [<!ATTLIST item");
    for index in 0..2_000 {
        write!(xml, " a{index} NMTOKENS #IMPLIED").unwrap();
    }
    xml.push_str(">]><r>");
    for _ in 0..4 {
        xml.push_str("<item");
        for index in (0..2_000).rev() {
            write!(xml, " a{index}=' x  y '").unwrap();
        }
        xml.push_str("/>");
    }
    xml.push_str("</r>");
    // Only repeated ATTLIST callback names consume the indirect-byte budget;
    // all document attributes are supplied directly and normalized by indexed type.
    let config = Config {
        limits: Limits {
            max_entity_expansion_bytes: 8_000,
            ..Limits::default()
        },
        ..Config::default()
    };
    let events = parse(xml.as_bytes(), 4_096, config).unwrap();
    let elements: Vec<_> = events
        .iter()
        .filter_map(|event| {
            if let EventKind::StartElement { name, attributes } = event {
                (name == "item").then_some(attributes)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(elements.len(), 4);
    for attributes in elements {
        assert_eq!(attributes.len(), 2_000);
        assert_eq!(attributes.first().unwrap().name, "a1999");
        assert_eq!(attributes.last().unwrap().name, "a0");
        assert!(
            attributes
                .iter()
                .all(|attribute| attribute.value == "x y" && attribute.specified)
        );
    }
}

#[test]
fn parameter_entities_expand_declarations_with_explicit_processing() {
    let mut parser = Parser::new(Config::default());
    assert!(parser.set_param_entity_parsing(2));
    let xml = b"<!DOCTYPE r [<!ENTITY % defs '<!ENTITY text \"expanded\">'>%defs;]><r>&text;</r>";
    for (index, byte) in xml.iter().enumerate() {
        parser.feed(&[*byte], index + 1 == xml.len()).unwrap();
        while let Some(event) = parser.next_event().unwrap() {
            if let EventKind::Text(value) = event.kind {
                assert_eq!(value, "expanded");
            }
        }
    }
    assert!(parser.is_finished());
}

#[test]
fn standalone_documents_reject_entities_declared_in_parameter_entities() {
    for external in [false, true] {
        for standalone in [false, true] {
            for attribute in [false, true] {
                let mut parser = Parser::new(Config::default());
                assert!(parser.set_param_entity_parsing(2));
                let mut xml = if standalone {
                    "<?xml version='1.0' standalone='yes'?>"
                } else {
                    ""
                }
                .to_owned();
                xml.push_str(if external {
                    "<!DOCTYPE r SYSTEM 'test.dtd'>"
                } else {
                    "<!DOCTYPE r [<!ENTITY % p \"<!ENTITY e 'value'>\">%p;]>"
                });
                xml.push_str(if attribute {
                    "<r a='&e;'/>"
                } else {
                    "<r>&e;</r>"
                });
                parser.feed(xml.as_bytes(), true).unwrap();
                let result = loop {
                    match parser.next_event() {
                        Ok(Some(event)) => {
                            if matches!(
                                event.kind,
                                EventKind::ExternalEntityReference(declaration) if matches!(declaration.as_ref(), oriole::ExternalEntityReference { context: None, .. }
                            )) {
                                let mut child =
                                    parser.external_child_with_encoding(None, None).unwrap();
                                child.feed(b"<!ENTITY e 'value'>", true).unwrap();
                                while child.next_event().unwrap().is_some() {}
                                parser.merge_external_subset(&child).unwrap();
                            }
                        }
                        Ok(None) => break Ok(()),
                        Err(error) => break Err(error.kind),
                    }
                };
                assert_eq!(
                    result,
                    if standalone {
                        Err(ErrorKind::EntityDeclaredInParameterEntity)
                    } else {
                        Ok(())
                    },
                    "external={external}, standalone={standalone}, attribute={attribute}"
                );
            }
        }
    }
}

#[test]
fn foreign_dtd_requests_have_null_identifiers() {
    let mut parser = Parser::new(Config::default());
    assert!(parser.set_param_entity_parsing(2));
    assert!(parser.set_use_foreign_dtd(true));
    parser.feed(b"<r/>", true).unwrap();
    assert!(matches!(
        parser.next_event().unwrap().unwrap().kind,
        EventKind::ExternalEntityReference(declaration) if matches!(declaration.as_ref(), oriole::ExternalEntityReference {
            context: None,
            system_id: None,
            public_id: None
        }
    )));
    while parser.next_event().unwrap().is_some() {}
}

#[test]
fn partial_token_after_progress_is_retried_without_deferral() {
    let mut parser = Parser::new(Config::default());
    parser.feed(b"<root><element>text</element", false).unwrap();
    while parser.next_event().unwrap().is_some() {}
    parser.feed(b">", false).unwrap();
    assert!(
        matches!(parser.next_event().unwrap().unwrap().kind, EventKind::EndElement {name} if name == "element")
    );
    parser.feed(b"</root>", true).unwrap();
    while parser.next_event().unwrap().is_some() {}
}

#[test]
fn line_break_callbacks_are_separate_in_text_and_cdata() {
    let mut parser = Parser::new(Config::default());
    parser
        .feed(b"<r>a\nb\r\nc<![CDATA[d\ne]]></r>", true)
        .unwrap();
    let mut data = Vec::new();
    while let Some(event) = parser.next_event().unwrap() {
        if let EventKind::Text(value) = event.kind {
            data.push(value);
        }
    }
    assert_eq!(
        data,
        ["a", "\n", "b", "\n", "c", "d", "\n", "e"].map(text::<oriole::Text>)
    );
}

#[test]
fn non_xml_whitespace_in_a_declaration_has_a_declaration_error() {
    let mut parser = Parser::new(Config::default());
    parser
        .feed(b"<?xml version\xc2\x85='1.0'?>\r\n", true)
        .unwrap();
    assert_eq!(
        parser.next_event().unwrap_err().kind,
        ErrorKind::XmlDeclaration
    );
}

#[test]
fn external_subset_callback_occurs_before_a_root_is_available() {
    let mut parser = Parser::new(Config::default());
    assert!(parser.set_param_entity_parsing(1));
    parser
        .feed(
            b"<!DOCTYPE external SYSTEM 'unsupported://non-existing'>\n",
            false,
        )
        .unwrap();
    let mut requested = false;
    while let Some(event) = parser.next_event().unwrap() {
        if let EventKind::ExternalEntityReference(declaration) = event.kind
            && declaration.context.is_none()
        {
            let oriole::ExternalEntityReference { system_id, .. } =
                oriole_storage::Box::into_inner(declaration);

            assert_eq!(system_id.as_deref(), Some("unsupported://non-existing"));
            requested = true;
        }
    }
    assert!(requested);
}

#[test]
fn changing_the_encoding_preserves_entity_and_deferral_options() {
    let mut parser = Parser::new(Config::default());
    assert!(parser.set_param_entity_parsing(2));
    assert!(parser.set_use_foreign_dtd(true));
    parser.set_expand_internal_entities(false);
    parser.set_reparse_deferral_enabled(false);
    parser.set_encoding(Some("UTF-8")).unwrap();
    assert!(!parser.reparse_deferral_enabled());
    parser
        .feed(b"<!DOCTYPE r [<!ENTITY e 'value'>]><r>&e;</r>", true)
        .unwrap();
    let mut foreign = false;
    let mut skipped = false;
    while let Some(event) = parser.next_event().unwrap() {
        match event.kind {
            EventKind::ExternalEntityReference(declaration)
                if declaration.context.is_none() && declaration.system_id.is_none() =>
            {
                foreign = true
            }
            EventKind::SkippedEntity {
                name,
                parameter: false,
            } if name == "e" => skipped = true,
            _ => {}
        }
    }
    assert!(foreign && skipped);
}

#[test]
fn a_failed_encoding_setter_preserves_autodetection() {
    use oriole_storage::{AllocationTracker, Allocator, with_tracking};
    let allocator = Allocator::System.trackable();
    let tracker = AllocationTracker::try_new_in(allocator).unwrap();
    let mut parser = with_tracking(&tracker, || {
        Parser::try_new_in(Config::default(), allocator)
    })
    .unwrap();
    assert!(tracker.set_maximum_amplification(10_000.0));
    tracker.set_activation_threshold(0);
    let error = with_tracking(&tracker, || parser.set_encoding(Some("UTF-8"))).unwrap_err();
    assert_eq!(error.kind, ErrorKind::NoMemory);
    let xml = b"<r/>";
    assert!(tracker.add_direct_bytes(xml.len() as u64));
    with_tracking(&tracker, || {
        parser.feed(xml, true).unwrap();
        while parser.next_event().unwrap().is_some() {}
    });
    assert!(parser.is_finished());
}

#[test]
fn many_newlines_and_a_fragmented_dtd_closer_are_streamed() {
    for cdata in [false, true] {
        let mut xml = std::string::String::from("<r>");
        if cdata {
            xml.push_str("<![CDATA[");
        }
        xml.push_str(&"x\n".repeat(32_768));
        if cdata {
            xml.push_str("]]>");
        }
        xml.push_str("</r>");
        let mut parser = Parser::new(Config::default());
        parser.feed(xml.as_bytes(), true).unwrap();
        let mut callbacks = 0;
        while let Some(event) = parser.next_event().unwrap() {
            if matches!(event.kind, EventKind::Text(_)) {
                callbacks += 1;
            }
        }
        assert_eq!(callbacks, 65_536);
    }
    let mut parser = Parser::new(Config::default());
    parser.feed(b"<!DOCTYPE r []", false).unwrap();
    while parser.next_event().unwrap().is_some() {}
    for _ in 0..32_768 {
        parser.feed(b" ", false).unwrap();
        assert!(parser.next_event().unwrap().is_none());
    }
    parser.feed(b"><r/>", true).unwrap();
    while parser.next_event().unwrap().is_some() {}
    assert!(parser.is_finished());
}

#[test]
fn malformed_tokens_report_the_offending_byte() {
    for (xml, kind, byte) in [
        ("<\n", ErrorKind::InvalidToken, 1),
        ("<r><<", ErrorKind::InvalidToken, 4),
        ("<r a='x\u{1}'/>", ErrorKind::InvalidToken, 7),
        ("<r><!-- a---></r>", ErrorKind::InvalidToken, 11),
        ("<!DOCTYPEdoc><doc/>", ErrorKind::InvalidToken, 12),
        ("<!DOCTYPE r [<!]>", ErrorKind::InvalidToken, 15),
        ("<!DOCTYPE r [<![", ErrorKind::Syntax, 13),
        ("<!DOCTYPE r [<!ELEMENT r ANY>", ErrorKind::NoElements, 29),
    ] {
        let mut parser = Parser::new(Config::default());
        parser.feed(xml.as_bytes(), true).unwrap();
        let error = loop {
            match parser.next_event() {
                Err(error) => break error,
                Ok(Some(_)) => {}
                Ok(None) => panic!("accepted {xml:?}"),
            }
        };
        assert_eq!(
            (error.kind, error.position.byte_index),
            (kind, byte),
            "{xml:?}"
        );
    }
}

#[test]
fn overlapping_processing_instruction_delimiters_never_panic() {
    for xml in [
        "<!DOCTYPE r[<?>?>",
        "<!DOCTYPE r [<?>x<?>",
        "<!DOCTYPE r [<?>LEM8888 (a,b*)>\n<!DOCTYPE r [<?>LEM8888 (a,b*)>",
        "<?>x<?>",
    ] {
        for chunk in 1..=xml.len() {
            assert!(
                parse(xml.as_bytes(), chunk, Config::default()).is_err(),
                "{xml}, chunk {chunk}"
            );
        }
    }
}

#[test]
fn conversion_buffer_boundaries_preserve_utf8_and_cdata_delimiters() {
    let mut xml = std::string::String::from("<r><![CDATA[");
    xml.push_str(&"x".repeat(1023));
    xml.push_str("]]>");
    xml.push_str(&"é".repeat(1025));
    xml.push_str("</r>");
    let mut utf16 = vec![0xff, 0xfe];
    utf16.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    let mut parser = Parser::new(Config::default());
    parser.feed(&utf16, true).unwrap();
    let mut lengths = Vec::new();
    let mut content = std::string::String::new();
    while let Some(event) = parser.next_event().unwrap() {
        if let EventKind::Text(value) = event.kind {
            lengths.push(value.len());
            content.push_str(&value);
        }
    }
    assert_eq!(lengths, [1023, 1024, 1024, 2]);
    assert_eq!(content, "x".repeat(1023) + &"é".repeat(1025));
}

#[test]
fn attribute_offsets_preserve_owned_events_across_reuse_and_large_tags() {
    let mut xml = String::from(
        "<?xml version='1.0' standalone='yes'?><!DOCTYPE r [<!ATTLIST p:é default CDATA 'fallback'>]><r xmlns:p='urn:p'><p:é naïve='left&amp;é' a='first'/><b",
    );
    for index in 0..129 {
        use std::fmt::Write;
        write!(xml, " a{index}='{index}'").unwrap();
    }
    xml.push_str("/><p:z p:last='tail' plain='final'/></r>");
    let mut utf16 = vec![0xff, 0xfe];
    utf16.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    for bytes in [xml.as_bytes(), &utf16] {
        for chunk in [1, 7, 4096, bytes.len()] {
            let events = parse(
                bytes,
                chunk,
                Config {
                    namespace_separator: Some('|'),
                    namespace_triplets: true,
                    ..Config::default()
                },
            )
            .unwrap();
            let starts: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    EventKind::StartElement { name, attributes } => Some((name, attributes)),
                    _ => None,
                })
                .collect();
            assert_eq!(starts.len(), 4);
            assert_eq!(starts[1].0, "urn:p|é|p");
            let attributes = starts[1].1;
            assert_eq!(attributes.len(), 3);
            assert_eq!(attributes[0].name, "naïve");
            assert_eq!(attributes[0].value, "left&é");
            assert_eq!(attributes[1].value, "first");
            assert_eq!(attributes[2].name, "default");
            assert_eq!(attributes[2].value, "fallback");
            assert!(!attributes[2].specified);
            assert_eq!(starts[2].1.len(), 129);
            assert_eq!(starts[2].1[128].name, "a128");
            assert_eq!(starts[2].1[128].value, "128");
            assert_eq!(starts[3].0, "urn:p|z|p");
            assert_eq!(starts[3].1[0].name, "urn:p|last|p");
            assert_eq!(starts[3].1[0].value, "tail");
            assert_eq!(starts[3].1[1].value, "final");
        }
    }
}

#[test]
fn raw_attribute_validation_precedes_value_and_namespace_processing() {
    for attributes in ["a='&missing;'", "a='one' a='two'", "xmlns:xml='wrong'"] {
        let xml = format!("<r><child {attributes} broken/></r>");
        for chunk in [1, 7, xml.len()] {
            let mut parser = Parser::new(Config {
                namespace_separator: Some('|'),
                ..Config::default()
            });
            let mut starts = Vec::new();
            let mut result = Ok(());
            let mut chunks = xml.as_bytes().chunks(chunk).peekable();
            'input: while let Some(bytes) = chunks.next() {
                parser.feed(bytes, chunks.peek().is_none()).unwrap();
                loop {
                    match parser.next_event() {
                        Ok(Some(event)) => {
                            if let EventKind::StartElement { name, .. } = event.kind {
                                starts.push(name);
                            }
                        }
                        Ok(None) => break,
                        Err(error) => {
                            result = Err(error);
                            break 'input;
                        }
                    }
                }
            }
            let error = result.unwrap_err();
            assert_eq!(error.kind, ErrorKind::InvalidToken);
            assert_eq!(error.message, "attribute is missing equals sign");
            assert_eq!(starts.len(), 1);
            assert_eq!(starts[0], "r");
        }
    }
}
