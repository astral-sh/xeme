use oriole::{Config, ErrorKind, EventKind, Limits, Parser};

fn subset(bytes: &[u8], chunk: usize, config: Config) -> Result<(Parser, String), ErrorKind> {
    let mut parent = Parser::new(config);
    assert!(parent.set_param_entity_parsing(2));
    parent.feed(b"<!DOCTYPE r SYSTEM 'd'>", false).unwrap();
    while parent.next_event().unwrap().is_some() {}
    let mut child = parent.external_child(None, None).unwrap();
    child.set_default_events(true);
    let mut defaults = String::new();
    let length = bytes.len();
    for (index, bytes) in bytes.chunks(chunk).enumerate() {
        child
            .feed(bytes, (index + 1) * chunk >= length)
            .map_err(|error| error.kind)?;
        while let Some(event) = child.next_event().map_err(|error| error.kind)? {
            if matches!(event.kind, EventKind::Default) {
                defaults.push_str(child.current_raw().unwrap());
            }
        }
    }
    parent
        .merge_external_subset(&child)
        .map_err(|error| error.kind)?;
    Ok((parent, defaults))
}

#[test]
fn included_declarations_and_ignored_markup_across_chunks_and_encodings() {
    let dtd = "<![ INCLUDE [<!ENTITY e 'ok'><![IGNORE[<bad &undefined; %missing; <![ arbitrary nested ]]>]]><!ATTLIST r a CDATA 'yes'>]]>";
    let mut encodings = vec![dtd.as_bytes().to_vec()];
    for little in [true, false] {
        let mut bytes = if little {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for unit in dtd.encode_utf16() {
            bytes.extend_from_slice(&if little {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        encodings.push(bytes);
    }
    for bytes in encodings {
        for chunk in 1..=bytes.len() {
            let (mut parser, defaults) = subset(&bytes, chunk, Config::default()).unwrap();
            assert!(
                defaults
                    .contains("<![IGNORE[<bad &undefined; %missing; <![ arbitrary nested ]]>]]>")
            );
            parser.feed(b"<r>&e;</r>", true).unwrap();
            let mut text = String::new();
            while let Some(event) = parser.next_event().unwrap() {
                match event.kind {
                    EventKind::Text(value) => text.push_str(&value),
                    EventKind::StartElement { attributes, .. } => {
                        assert_eq!(attributes.len(), 1);
                        assert_eq!(attributes[0].value, "yes");
                    }
                    _ => {}
                }
            }
            assert_eq!(text, "ok");
        }
    }
}

#[test]
fn ignored_text_does_not_parse_references_but_checks_xml_characters() {
    for bytes in [b"<![IGNORE[\x01]]>".as_slice(), b"<![IGNORE[\0]]>"] {
        for chunk in 1..=bytes.len() {
            assert_eq!(
                subset(bytes, chunk, Config::default()).unwrap_err(),
                ErrorKind::InvalidToken
            );
        }
    }
    for (bytes, expected) in [
        (
            b"<![IGNORE[\xe2\x82".as_slice(),
            ErrorKind::PartialCharacter,
        ),
        (b"<![IGNORE[unfinished", ErrorKind::Syntax),
        (b"<![INCLUDE[", ErrorKind::IncompleteParameterEntity),
        (b"]]>", ErrorKind::Syntax),
        (b"<![OTHER[]]>", ErrorKind::Syntax),
    ] {
        for chunk in 1..=bytes.len() {
            assert_eq!(
                subset(bytes, chunk, Config::default()).unwrap_err(),
                expected,
                "{bytes:?}, chunk {chunk}"
            );
        }
    }
}

#[test]
fn incomplete_parameter_sections_and_stray_closings_are_rejected() {
    for (dtd, error) in [
        ("<!ENTITY % p ']]>'><![INCLUDE[%p;]]>", ErrorKind::Syntax),
        (
            "<!ENTITY % p '<![INCLUDE['>%p;]]>",
            ErrorKind::IncompleteParameterEntity,
        ),
        ("<!ENTITY % p '<![IGNORE['>%p;]]>", ErrorKind::Syntax),
    ] {
        for chunk in 1..=dtd.len() {
            assert_eq!(
                subset(dtd.as_bytes(), chunk, Config::default()).unwrap_err(),
                error
            );
        }
    }
    let dtd = "<!ENTITY % p '<![INCLUDE[<!ENTITY e \"ok\">]]>'>%p;";
    for chunk in 1..=dtd.len() {
        assert!(subset(dtd.as_bytes(), chunk, Config::default()).is_ok());
    }
}

#[test]
fn internal_subset_rejects_conditional_sections() {
    for keyword in ["INCLUDE", "IGNORE"] {
        let document = format!("<!DOCTYPE r [<![{keyword}[]]>]><r/>");
        let mut parser = Parser::new(Config::default());
        parser.feed(document.as_bytes(), true).unwrap();
        let error = loop {
            match parser.next_event() {
                Ok(Some(_)) => {}
                Ok(None) => panic!("internal conditional section accepted"),
                Err(error) => break error,
            }
        };
        assert_eq!(error.kind, ErrorKind::Syntax);
    }
}

#[test]
fn internal_parameter_entities_select_conditional_keywords_with_shared_limits() {
    let dtd =
        "<!ENTITY % mode 'INCLUDE'><!ENTITY % nested '&#37;mode;'><![%nested;[<!ENTITY e 'ok'>]]>";
    for chunk in 1..=dtd.len() {
        let (mut parent, raw) = subset(dtd.as_bytes(), chunk, Config::default()).unwrap();
        assert!(raw.contains("<![INCLUDE["));
        assert!(!raw.contains("<![%nested;["));
        parent.feed(b"<r>&e;</r>", true).unwrap();
        let mut text = String::new();
        while let Some(event) = parent.next_event().unwrap() {
            if let EventKind::Text(value) = event.kind {
                text.push_str(&value);
            }
        }
        assert_eq!(text, "ok");
    }
    let dtd = "<!ENTITY % loop '&#37;loop;'><![%loop;[]]>";
    assert_eq!(
        subset(dtd.as_bytes(), 1, Config::default()).unwrap_err(),
        ErrorKind::RecursiveEntityReference
    );
    for dtd in [
        "<!ENTITY % k 'IN'><![%k;CLUDE[]]>",
        "<!ENTITY % a 'IN'><!ENTITY % b 'CLUDE'><![%a;%b;[]]>",
    ] {
        for chunk in 1..=dtd.len() {
            assert_eq!(
                subset(dtd.as_bytes(), chunk, Config::default()).unwrap_err(),
                ErrorKind::Syntax
            );
        }
    }
    let config = Config {
        limits: Limits {
            max_entity_depth: 2,
            ..Limits::default()
        },
        ..Config::default()
    };
    let dtd = "<!ENTITY % q 'INCLUDE'><!ENTITY % p '<![&#37;q;[]]>'>%p;";
    for chunk in 1..=dtd.len() {
        assert_eq!(
            subset(dtd.as_bytes(), chunk, config.clone()).unwrap_err(),
            ErrorKind::LimitExceeded
        );
    }
}

#[test]
fn section_nesting_and_ignored_bytes_are_bounded() {
    let config = Config {
        limits: Limits {
            max_depth: 2,
            ..Limits::default()
        },
        ..Config::default()
    };
    for dtd in [
        "<![INCLUDE[<![INCLUDE[<![INCLUDE[]]>]]>]]>",
        "<![IGNORE[<![x<![x]]>]]>]]>",
    ] {
        assert_eq!(
            subset(dtd.as_bytes(), 1, config.clone()).unwrap_err(),
            ErrorKind::LimitExceeded
        );
    }
    let config = Config {
        limits: Limits {
            max_token_bytes: 32,
            ..Limits::default()
        },
        ..Config::default()
    };
    let dtd = format!("<![IGNORE[{}]]>", "x".repeat(32));
    assert_eq!(
        subset(dtd.as_bytes(), 1, config.clone()).unwrap_err(),
        ErrorKind::LimitExceeded
    );
    let header = format!("<![{}INCLUDE[]]>", " ".repeat(32));
    assert_eq!(
        subset(header.as_bytes(), 1, config).unwrap_err(),
        ErrorKind::LimitExceeded
    );
}

#[test]
fn skipped_header_parameters_preserve_markup_and_suppress_declarations() {
    for standalone in [false, true] {
        for mode in 0..=2 {
            for declared in [false, true] {
                let prefix = if declared { "<!ENTITY % k ' '>" } else { "" };
                let dtd = format!(
                    "{prefix}<![INCLUDE%k;%missing;[<!ENTITY e 'ok'><!ATTLIST r a CDATA 'yes'>]]>"
                );
                for chunk in [1, 7, dtd.len()] {
                    let mut parent = Parser::new(Config::default());
                    assert!(parent.set_param_entity_parsing(mode));
                    let document = format!(
                        "<?xml version='1.0' standalone='{}'?><!DOCTYPE r SYSTEM 'd'>",
                        if standalone { "yes" } else { "no" }
                    );
                    parent.feed(document.as_bytes(), false).unwrap();
                    while parent.next_event().unwrap().is_some() {}
                    let mut child = parent.external_child(None, None).unwrap();
                    assert!(child.set_param_entity_parsing(mode));
                    child.set_default_events(true);
                    let mut raw = String::new();
                    let mut notifications = Vec::new();
                    let mut declarations = 0;
                    for (index, bytes) in dtd.as_bytes().chunks(chunk).enumerate() {
                        child.feed(bytes, (index + 1) * chunk >= dtd.len()).unwrap();
                        while let Some(event) = child.next_event().unwrap() {
                            match event.kind {
                                EventKind::Default => raw.push_str(child.current_raw().unwrap()),
                                EventKind::NotStandalone => {
                                    notifications.push(event.position.byte_index)
                                }
                                EventKind::EntityDeclaration(declaration)
                                    if (declaration.name == "e") =>
                                {
                                    declarations += 1
                                }
                                EventKind::AttlistDeclaration(_) => declarations += 1,
                                EventKind::SkippedEntity { .. } => {
                                    panic!("header references do not emit SkippedEntity")
                                }
                                _ => {}
                            }
                        }
                    }
                    assert_eq!(declarations, if standalone { 2 } else { 0 });
                    assert_eq!(
                        notifications,
                        if mode == 0 && !standalone {
                            vec![prefix.len() + 10, prefix.len() + 13]
                        } else {
                            vec![]
                        }
                    );
                    let header = if mode == 0 || !declared {
                        "<![INCLUDE%k;%missing;["
                    } else {
                        "<![INCLUDE %missing;["
                    };
                    assert!(raw.starts_with(header), "{raw:?}");
                    if !standalone {
                        assert!(raw.contains("<!ENTITY e 'ok'><!ATTLIST r a CDATA 'yes'>"));
                    }
                }
            }
        }
    }
}

#[test]
fn skipped_header_parameters_still_require_a_complete_keyword() {
    for mode in 0..=2 {
        for standalone in [false, true] {
            for declared in [false, true] {
                let mut parent = Parser::new(Config::default());
                assert!(parent.set_param_entity_parsing(mode));
                let declaration = format!(
                    "<?xml version='1.0' standalone='{}'?>",
                    if standalone { "yes" } else { "no" }
                );
                parent.feed(declaration.as_bytes(), false).unwrap();
                while parent.next_event().unwrap().is_some() {}
                let mut child = parent.external_child(None, None).unwrap();
                assert!(child.set_param_entity_parsing(mode));
                let prefix = if declared {
                    "<!ENTITY % k 'INCLUDE'>"
                } else {
                    ""
                };
                child
                    .feed(format!("{prefix}<![%k;[]]>").as_bytes(), true)
                    .unwrap();
                let result = loop {
                    match child.next_event() {
                        Ok(Some(_)) => {}
                        Ok(None) => break Ok(()),
                        Err(error) => break Err(error.kind),
                    }
                };
                assert_eq!(
                    result,
                    if mode != 0 && declared {
                        Ok(())
                    } else {
                        Err(ErrorKind::Syntax)
                    }
                );
            }
        }
    }
}

#[test]
fn disabled_header_parameters_charge_callback_storage_to_the_shared_budget() {
    let config = Config {
        limits: Limits {
            max_entity_expansion_bytes: 8 * 1024,
            ..Limits::default()
        },
        ..Config::default()
    };
    let dtd = format!("<![INCLUDE{}[]]>", "%p;".repeat(128));
    for chunk in [1, 7, dtd.len()] {
        let parent = Parser::new(config.clone());
        let mut child = parent.external_child(None, None).unwrap();
        child.set_default_events(true);
        let mut failure = None;
        for (index, bytes) in dtd.as_bytes().chunks(chunk).enumerate() {
            child.feed(bytes, (index + 1) * chunk >= dtd.len()).unwrap();
            loop {
                match child.next_event() {
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(error) => {
                        failure = Some(error.kind);
                        break;
                    }
                }
            }
            if failure.is_some() {
                break;
            }
        }
        assert_eq!(failure, Some(ErrorKind::LimitExceeded));
    }
}

#[test]
fn skipped_declarations_parse_structure_without_expanding_literal_values() {
    for declaration in [
        "<!ENTITY e '&#0;'>",
        "<!ENTITY e '%bad name;'>",
        "<!ATTLIST r a CDATA '&#xD800;'>",
        "<!ATTLIST r a NMTOKENS '<&;'>",
        "<!ATTLIST r a CDATA '&e;'>",
    ] {
        let dtd = format!("<!ENTITY e '&e;'><![INCLUDE%missing;[{declaration}]]>");
        for chunk in [1, 7, dtd.len()] {
            let (_, raw) = subset(dtd.as_bytes(), chunk, Config::default()).unwrap();
            assert!(raw.contains(declaration));
        }
    }
    for (declaration, expected) in [
        ("<!ENTITY e '\u{1}'>", ErrorKind::InvalidToken),
        ("<!ATTLIST r a FOO 'value'>", ErrorKind::Syntax),
        ("<!ATTLIST r a () 'value'>", ErrorKind::Syntax),
        ("<!ATTLIST r a CDATA #FIXED>", ErrorKind::Syntax),
        ("<!ATTLIST r a CDATA unquoted>", ErrorKind::Syntax),
    ] {
        let dtd = format!("<![INCLUDE%missing;[{declaration}]]>");
        for chunk in [1, 7, dtd.len()] {
            assert_eq!(
                subset(dtd.as_bytes(), chunk, Config::default()).unwrap_err(),
                expected
            );
        }
    }
}

#[test]
fn parameter_header_delimiters_preserve_the_replacement_suffix() {
    for dtd in [
        r#"<!ENTITY % p "INCLUDE["><![%p;<!ENTITY e 'ok'>]]>"#,
        r#"<!ENTITY % p "["><![INCLUDE%p;<!ENTITY e 'ok'>]]>"#,
        r#"<!ENTITY % p "INCLUDE[<!ENTITY e "><![%p;'ok'>]]>"#,
        r#"<!ENTITY % p "INCLUDE[<!ENTITY e 'ok'>]]>"><![%p;"#,
        r#"<!ENTITY % q "INCLUDE["><!ENTITY % p "&#37;q;"><![%p;<!ENTITY e 'ok'>]]>"#,
    ] {
        let mut encodings = vec![dtd.as_bytes().to_vec()];
        for little in [false, true] {
            let mut bytes = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in dtd.encode_utf16() {
                bytes.extend_from_slice(&if little {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            encodings.push(bytes);
        }
        for bytes in encodings {
            for chunk in 1..=bytes.len() {
                let (mut parser, _) = subset(&bytes, chunk, Config::default()).unwrap();
                parser.feed(b"<r>&e;</r>", true).unwrap();
                let mut text = String::new();
                while let Some(event) = parser.next_event().unwrap() {
                    if let EventKind::Text(value) = event.kind {
                        text.push_str(&value);
                    }
                }
                assert_eq!(text, "ok", "{dtd}; chunk {chunk}");
            }
        }
    }
}

#[test]
fn malformed_headers_preserve_default_prefixes_before_the_error() {
    for (input, expected) in [
        ("<![%missing;[", "<![%missing;"),
        ("<![IN%missing;[", "<!["),
        ("<![%missing;CLUDE[", "<![%missing;"),
    ] {
        for width in 1..=input.len() {
            let parent = Parser::new(Config::default());
            let mut child = parent.external_child(None, None).unwrap();
            child.set_default_events(true);
            let mut raw = String::new();
            let mut failure = None;
            for (index, bytes) in input.as_bytes().chunks(width).enumerate() {
                child
                    .feed(bytes, (index + 1) * width >= input.len())
                    .unwrap();
                loop {
                    match child.next_event() {
                        Ok(Some(event)) => {
                            if matches!(event.kind, EventKind::Default) {
                                raw.push_str(child.current_raw().unwrap());
                            }
                        }
                        Ok(None) => break,
                        Err(error) => {
                            failure = Some(error.kind);
                            break;
                        }
                    }
                }
                if failure.is_some() {
                    break;
                }
            }
            assert_eq!(failure, Some(ErrorKind::Syntax));
            assert_eq!(raw, expected, "{input}; chunk {width}");
        }
    }
}
