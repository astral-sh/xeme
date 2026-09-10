use oriole::{Config, ErrorKind, EventKind, Limits, Parser};

fn text(value: &str) -> oriole_storage::String {
    oriole_storage::String::try_from_str_in(value, oriole_storage::Allocator::System).unwrap()
}

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
                    previous.push_str(&text).unwrap();
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
    assert!(events.contains(&EventKind::Text(text("hé😀\n&😀"))));
    assert!(events.contains(&EventKind::Text(text("x<>&\ny"))));
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
fn external_entities_require_application_supplied_content() {
    let events = parse(
        b"<!DOCTYPE r [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><r>&x;</r>",
        1,
        Config::default(),
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(event, EventKind::ExternalEntityReference { system_id, .. } if system_id.as_deref() == Some("file:///etc/passwd"))));
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
        if let EventKind::ExternalEntityReference { context, .. } = event.kind {
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
            max_entity_expansion_bytes: 16,
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
        if let EventKind::ExternalEntityReference { context, .. } = event.kind {
            let mut child = parent.external_child(context.as_deref(), None).unwrap();
            child.feed(b"&x;", true).unwrap();
            assert_eq!(
                child.next_event().unwrap_err().kind,
                ErrorKind::RecursiveEntityReference
            );
            let mut sibling = parent.external_child(Some(""), None).unwrap();
            sibling.feed(b"1234567890", true).unwrap();
            while sibling.next_event().unwrap().is_some() {}
            let mut sibling = parent.external_child(Some(""), None).unwrap();
            assert_eq!(
                sibling.feed(b"1234", true).unwrap_err().kind,
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
            EventKind::ExternalEntityReference { context: None, .. } => {
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
fn foreign_dtd_requests_have_null_identifiers() {
    let mut parser = Parser::new(Config::default());
    assert!(parser.set_param_entity_parsing(2));
    assert!(parser.set_use_foreign_dtd(true));
    parser.feed(b"<r/>", true).unwrap();
    assert!(matches!(
        parser.next_event().unwrap().unwrap().kind,
        EventKind::ExternalEntityReference {
            context: None,
            system_id: None,
            public_id: None
        }
    ));
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
    assert_eq!(data, ["a", "\n", "b", "\n", "c", "d", "\n", "e"].map(text));
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
        if let EventKind::ExternalEntityReference {
            context: None,
            system_id,
            ..
        } = event.kind
        {
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
            EventKind::ExternalEntityReference {
                context: None,
                system_id: None,
                ..
            } => foreign = true,
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
