use oriole::{Config, ErrorKind, NameRules, Parser};

fn parse(
    xml: &str,
    rules: NameRules,
    namespaces: bool,
    chunk: usize,
    utf16: bool,
) -> Result<(), ErrorKind> {
    let bytes = if utf16 {
        [0xff, 0xfe]
            .into_iter()
            .chain(xml.encode_utf16().flat_map(u16::to_le_bytes))
            .collect::<Vec<_>>()
    } else {
        xml.as_bytes().to_vec()
    };
    let mut parser = Parser::new(Config {
        name_rules: rules,
        namespace_separator: namespaces.then_some('|'),
        ..Config::default()
    });
    parser.set_param_entity_parsing(2);
    for (index, input) in bytes.chunks(chunk).enumerate() {
        parser
            .feed(input, (index + 1) * chunk >= bytes.len())
            .map_err(|error| error.kind)?;
        while parser.next_event().map_err(|error| error.kind)?.is_some() {}
    }
    assert!(parser.is_finished());
    Ok(())
}

#[test]
fn edition_applies_to_every_name_context_and_encoding() {
    assert_eq!(Config::default().name_rules, NameRules::FifthEdition);
    for (name, fourth) in [
        ("À", true),
        ("a\u{901}", true),
        ("\u{901}", false),
        ("\u{10000}", false),
        ("a\u{10000}", false),
    ] {
        let documents = [
            format!("<{name}></{name}>"),
            format!("<r {name}='v'/>"),
            format!("<?{name} data?><r/>"),
            format!("<!DOCTYPE {name}><{name}/>"),
            format!("<!DOCTYPE r [<!ENTITY {name} 'v'>]><r>&{name};</r>"),
            format!("<!DOCTYPE r [<!ENTITY {name} 'v'>]><r a='&{name};'/>"),
            format!("<!DOCTYPE r [<!ENTITY % {name} '<!ELEMENT r EMPTY>'>%{name};]><r/>"),
            format!("<!DOCTYPE r [<!ATTLIST r {name} CDATA 'v'>]><r/>"),
            format!(
                "<!DOCTYPE r [<!NOTATION {name} SYSTEM 'x'><!ENTITY e SYSTEM 'e' NDATA {name}>]><r/>"
            ),
            format!("<!DOCTYPE r [<!ELEMENT r ({name})><!ELEMENT {name} EMPTY>]><r><{name}/></r>"),
            format!("<!DOCTYPE r [<!ENTITY e '<{name}/>'>]><r>&e;</r>"),
        ];
        for xml in &documents {
            for rules in [NameRules::FourthEdition, NameRules::FifthEdition] {
                for namespaces in [false, true] {
                    for chunk in [1, 7, usize::MAX] {
                        for utf16 in [false, true] {
                            assert_eq!(
                                parse(xml, rules, namespaces, chunk, utf16).is_ok(),
                                fourth || rules == NameRules::FifthEdition,
                                "{xml:?}, {rules:?}, namespaces={namespaces}, chunk={chunk}, utf16={utf16}"
                            );
                        }
                    }
                }
            }
        }
        for xml in [
            format!("<{name}:r xmlns:{name}='u'/>"),
            format!("<p:{name} xmlns:p='u'/>"),
            format!("<r xmlns:{name}='u' {name}:a='v'/>"),
            format!("<r xmlns:p='u' p:{name}='v'/>"),
        ] {
            for rules in [NameRules::FourthEdition, NameRules::FifthEdition] {
                for chunk in [1, usize::MAX] {
                    assert_eq!(
                        parse(&xml, rules, true, chunk, false).is_ok(),
                        fourth || rules == NameRules::FifthEdition,
                        "{xml:?}, {rules:?}"
                    );
                }
            }
        }
    }
    // Nmtoken enumeration values can begin with a NameChar that is not NameStart.
    for (name, fourth) in [("\u{901}", true), ("\u{10000}", false)] {
        let xml = format!("<!DOCTYPE r [<!ATTLIST r a ({name}) '{name}'>]><r/>");
        for rules in [NameRules::FourthEdition, NameRules::FifthEdition] {
            assert_eq!(
                parse(&xml, rules, true, 1, false).is_ok(),
                fourth || rules == NameRules::FifthEdition
            );
        }
    }
}

#[test]
fn invalid_names_precede_entity_lookup_and_namespace_resolution() {
    for xml in [
        "<r>&\u{10000};</r>",
        "<r a='&\u{10000};'/>",
        "<!DOCTYPE r [%\u{10000};]><r/>",
    ] {
        for chunk in [1, 7, usize::MAX] {
            assert_eq!(
                parse(xml, NameRules::FourthEdition, false, chunk, false),
                Err(ErrorKind::InvalidToken),
                "{xml:?}"
            );
            assert_eq!(
                parse(xml, NameRules::FifthEdition, false, chunk, false),
                Err(ErrorKind::UndefinedEntity),
                "{xml:?}"
            );
        }
    }
    for xml in ["<\u{10000}:r/>", "<r \u{10000}:a='v'/>"] {
        assert_eq!(
            parse(xml, NameRules::FourthEdition, true, 1, false),
            Err(ErrorKind::InvalidToken)
        );
        assert_eq!(
            parse(xml, NameRules::FifthEdition, true, 1, false),
            Err(ErrorKind::UndefinedPrefix)
        );
    }
}

#[test]
fn external_general_and_parameter_children_inherit_the_edition() {
    for rules in [NameRules::FourthEdition, NameRules::FifthEdition] {
        let parent = Parser::new(Config {
            name_rules: rules,
            ..Config::default()
        });
        for (context, xml) in [
            (Some(""), "<\u{10000}/>"),
            (None, "<!ENTITY \u{10000} 'v'>"),
        ] {
            let mut child = parent.external_child(context, None).unwrap();
            let result = child.feed(xml.as_bytes(), true).and_then(|()| {
                while child.next_event()?.is_some() {}
                Ok(())
            });
            assert_eq!(result.is_ok(), rules == NameRules::FifthEdition);
            if let Err(error) = result {
                assert_eq!(error.kind, ErrorKind::InvalidToken);
            }
        }
    }
}

#[test]
fn malformed_model_and_enumeration_names_are_lexical_errors() {
    for name in ["\u{10000}", "a\u{10000}", "\u{e000}", "a\u{e000}"] {
        for xml in [
            format!("<!DOCTYPE r [<!ELEMENT r ({name})>]><r/>"),
            format!("<!DOCTYPE r [<!ATTLIST r a ({name}) 'v'>]><r/>"),
            format!("<!DOCTYPE r [<!ATTLIST r a NOTATION ({name}) 'v'>]><r/>"),
        ] {
            for chunk in [1, 7, usize::MAX] {
                assert_eq!(
                    parse(&xml, NameRules::FourthEdition, true, chunk, false),
                    Err(ErrorKind::InvalidToken),
                    "{xml:?}"
                );
            }
        }
    }
}

#[test]
fn custom_public_identifiers_classify_original_bytes_with_the_selected_edition() {
    let xml = b"<!DOCTYPE r PUBLIC '@^' 's'><r/>";
    for rules in [NameRules::FourthEdition, NameRules::FifthEdition] {
        for width in [1, 7, xml.len()] {
            let mut parser = Parser::new(Config {
                name_rules: rules,
                encoding: Some("custom".into()),
                ..Config::default()
            });
            let mut map = std::array::from_fn(|index| index as i32);
            map[usize::from(b'@')] = -2;
            map[usize::from(b'^')] = '\u{200c}' as i32;
            let mut public_id = None;
            let result = (|| {
                for (index, bytes) in xml.chunks(width).enumerate() {
                    parser.feed(bytes, (index + 1) * width >= xml.len())?;
                    loop {
                        match parser.next_event() {
                            Ok(Some(event)) => {
                                if let oriole::EventKind::StartDoctype(value) = event.kind {
                                    public_id = value.public_id.as_deref().map(str::to_owned);
                                }
                            }
                            Ok(None) if parser.encoding_conversion().is_some() => {
                                let conversion = parser.encoding_conversion().unwrap();
                                assert_eq!(&conversion.bytes[..2], b"@^");
                                parser.resolve_encoding_conversion(i32::from(b'A'))?;
                            }
                            Ok(None) => break,
                            Err(error) if error.kind == ErrorKind::UnknownEncoding => {
                                parser.set_multibyte_encoding_map("custom", map)?;
                            }
                            Err(error) => return Err(error),
                        }
                    }
                }
                Ok(())
            })();
            if rules == NameRules::FourthEdition {
                assert_eq!(result.unwrap_err().kind, ErrorKind::PublicId);
                assert!(public_id.is_none());
            } else {
                result.unwrap();
                assert!(parser.is_finished());
                assert_eq!(public_id.as_deref(), Some("A"));
            }
        }
    }
}
