use std::fmt::Write;

use oriole::{Config, ErrorKind, EventKind, Limits, Parser};

fn attribute(document: &str, config: Config) -> Result<std::string::String, ErrorKind> {
    let mut parser = Parser::new(config);
    assert!(parser.set_param_entity_parsing(2));
    parser.feed(document.as_bytes(), true).unwrap();
    let mut result = None;
    loop {
        match parser.next_event() {
            Ok(Some(event)) => {
                if let EventKind::StartElement { attributes, .. } = event.kind
                    && let Some(attribute) = attributes.first()
                {
                    result = Some(attribute.value.as_str().to_owned());
                }
            }
            Ok(None) => return Ok(result.unwrap()),
            Err(error) => {
                assert_eq!(parser.next_event().unwrap_err(), error);
                assert_eq!(parser.feed(b"ignored", true).unwrap_err(), error);
                return Err(error.kind);
            }
        }
    }
}

#[test]
fn sixty_thousand_attribute_entities_use_a_bounded_call_stack() {
    const DEPTH: usize = 60_000;
    let mut document = "<!DOCTYPE r [<!ENTITY e0 'leaf'>".to_owned();
    for index in 1..DEPTH {
        write!(document, "<!ENTITY e{index} 'x&e{};'>", index - 1).unwrap();
    }
    write!(document, "]><r a='&e{};|&e0;'/>", DEPTH - 1).unwrap();
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let config = Config {
                limits: Limits {
                    max_entities: DEPTH,
                    max_entity_depth: DEPTH,
                    ..Limits::default()
                },
                ..Config::default()
            };
            let output = attribute(&document, config).unwrap();
            assert_eq!(output.len(), DEPTH - 1 + "leaf|leaf".len());
            assert!(output[..DEPTH - 1].bytes().all(|byte| byte == b'x'));
            assert_eq!(&output[DEPTH - 1..], "leaf|leaf");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn attribute_entity_cycles_and_depth_preserve_error_precedence() {
    for (declarations, value, depth, expected) in [
        (
            "<!ENTITY a '&b;'><!ENTITY b '&a;'>",
            "&a;",
            2,
            ErrorKind::RecursiveEntityReference,
        ),
        (
            "<!ENTITY a '&missing;'>",
            "&a;",
            1,
            ErrorKind::LimitExceeded,
        ),
        ("", "&missing;", 0, ErrorKind::LimitExceeded),
        ("", "&bad name;", 0, ErrorKind::InvalidToken),
        (
            "<!ENTITY a SYSTEM 'external'>",
            "&a;",
            1,
            ErrorKind::ExternalEntityInAttribute,
        ),
        (
            "<!ENTITY a '&b;'><!ENTITY b 'ok'>",
            "&a;",
            1,
            ErrorKind::LimitExceeded,
        ),
    ] {
        let document = format!("<!DOCTYPE r [{declarations}]><r a='{value}'/>");
        let config = Config {
            limits: Limits {
                max_entity_depth: depth,
                ..Limits::default()
            },
            ..Config::default()
        };
        assert_eq!(attribute(&document, config), Err(expected), "{document}");
    }
}

#[test]
fn branching_entities_reenter_completed_siblings_and_normalize_at_reference_boundaries() {
    let document = "<!DOCTYPE r [<!ENTITY leaf 'A&#13;B&#9;C'><!ENTITY left '&leaf;&#13;'><!ENTITY right '&#10;&leaf;'><!ENTITY diamond '&left;&right;'><!ATTLIST r a CDATA '&diamond;|&diamond;'>]><r/>";
    assert_eq!(
        attribute(document, Config::default()).unwrap(),
        "A B C  A B C|A B C  A B C"
    );
    let document = "<!DOCTYPE r [<!ENTITY leaf 'x'><!ENTITY branch '&leaf;&leaf;&leaf;'>]><r a='&branch;&branch;&leaf;'/>";
    assert_eq!(attribute(document, Config::default()).unwrap(), "xxxxxxx");
}

#[test]
fn wide_diamond_expansion_stops_at_the_existing_work_limit() {
    let mut document = "<!DOCTYPE r [<!ENTITY e0 'x'>".to_owned();
    for index in 1..16 {
        write!(document, "<!ENTITY e{index} '&e{0};&e{0};'>", index - 1).unwrap();
    }
    document.push_str("]><r a='&e15;'/>");
    let config = Config {
        limits: Limits {
            max_entity_expansion_bytes: 4096,
            ..Limits::default()
        },
        ..Config::default()
    };
    assert_eq!(attribute(&document, config), Err(ErrorKind::LimitExceeded));
    assert_eq!(
        attribute(&document, Config::default()).unwrap(),
        "x".repeat(32_768)
    );
}

#[test]
fn external_context_entity_names_are_active_during_attribute_expansion() {
    let mut parent = Parser::new(Config::default());
    parent
        .feed(b"<!DOCTYPE r [<!ENTITY e 'value'>]><r/>", true)
        .unwrap();
    while parent.next_event().unwrap().is_some() {}
    let mut child = parent.external_child(Some("e"), None).unwrap();
    child.feed(b"<r a='&e;'/>", true).unwrap();
    assert_eq!(
        child.next_event().unwrap_err().kind,
        ErrorKind::RecursiveEntityReference
    );
}

#[test]
fn attribute_entities_count_the_containing_entity_depth() {
    for declaration in [
        "<!ENTITY outer \"<r a='&inner;'/>\">",
        "<!ENTITY % outer \"<!ATTLIST r a CDATA '&inner;'>\">",
    ] {
        let parameter = declaration.contains("ENTITY %");
        let document = format!(
            "<!DOCTYPE r [<!ENTITY inner 'value'>{declaration}{}]>{}",
            if parameter { "%outer;" } else { "" },
            if parameter {
                "<r/>"
            } else {
                "<root>&outer;</root>"
            },
        );
        for depth in [1, 2] {
            let config = Config {
                limits: Limits {
                    max_entity_depth: depth,
                    ..Limits::default()
                },
                ..Config::default()
            };
            assert_eq!(
                attribute(&document, config),
                if depth == 1 {
                    Err(ErrorKind::LimitExceeded)
                } else {
                    Ok("value".to_owned())
                },
                "{document}, depth={depth}",
            );
        }
    }
    let mut document =
        "<!DOCTYPE r [<!ENTITY inner 'value'><!ENTITY e0 \"<r a='&inner;'/>\">".to_owned();
    for depth in 1..32 {
        write!(document, "<!ENTITY e{depth} '&e{};'>", depth - 1).unwrap();
    }
    document.push_str("]><r>&e31;</r>");
    assert_eq!(
        attribute(&document, Config::default()),
        Err(ErrorKind::LimitExceeded)
    );
    let mut config = Config::default();
    config.limits.max_entity_depth += 1;
    assert_eq!(attribute(&document, config), Ok("value".to_owned()));
}

#[test]
fn default_attribute_entities_count_inherited_parameter_depth() {
    for depth in [2, 3] {
        let mut parent = Parser::new(Config {
            limits: Limits {
                max_entity_depth: depth,
                ..Limits::default()
            },
            ..Config::default()
        });
        parent.set_param_entity_parsing(2);
        parent.feed(b"<!DOCTYPE r [<!ENTITY e 'value'><!ENTITY % external SYSTEM 'external'><!ENTITY % outer '&#37;external;'>%outer;]><r/>", true).unwrap();
        let mut requested = false;
        while let Some(event) = parent.next_event().unwrap() {
            if matches!(event.kind, EventKind::ExternalEntityReference(_)) {
                requested = true;
                let mut child = parent.external_child(None, None).unwrap();
                child.feed(b"<!ATTLIST r a CDATA '&e;'>", true).unwrap();
                let mut value = None;
                let result = loop {
                    match child.next_event() {
                        Ok(Some(event)) => {
                            if let EventKind::AttlistDeclaration(declaration) = event.kind {
                                value = declaration.default.as_deref().map(str::to_owned);
                            }
                        }
                        Ok(None) => break Ok(value),
                        Err(error) => {
                            assert_eq!(child.next_event().unwrap_err(), error);
                            break Err(error.kind);
                        }
                    }
                };
                assert_eq!(
                    result,
                    if depth == 2 {
                        Err(ErrorKind::LimitExceeded)
                    } else {
                        Ok(Some("value".to_owned()))
                    }
                );
                break;
            }
        }
        assert!(requested);
    }
}

#[test]
fn attribute_entities_count_external_parent_depth() {
    for depth in [1, 2] {
        let mut parent = Parser::new(Config {
            limits: Limits {
                max_entity_depth: depth,
                ..Limits::default()
            },
            ..Config::default()
        });
        parent
            .feed(b"<!DOCTYPE r [<!ENTITY e 'value'>]><r/>", true)
            .unwrap();
        while parent.next_event().unwrap().is_some() {}
        let mut child = parent.external_child(Some(""), None).unwrap();
        child.feed(b"<r a='&e;'/>", true).unwrap();
        if depth == 1 {
            let error = child.next_event().unwrap_err();
            assert_eq!(error.kind, ErrorKind::LimitExceeded);
            assert_eq!(child.next_event().unwrap_err(), error);
        } else {
            let event = child.next_event().unwrap().unwrap();
            let EventKind::StartElement { attributes, .. } = event.kind else {
                panic!("expected start element")
            };
            assert_eq!(attributes[0].value, "value");
            while child.next_event().unwrap().is_some() {}
        }
    }
}

#[test]
fn replacement_crlf_is_two_spaces_but_physical_crlf_is_one() {
    for document in [
        "<!DOCTYPE r [<!ENTITY e 'A&#13;&#10;B'>]><r a='&e;'/>",
        "<!DOCTYPE r [<!ENTITY % p \"<!ATTLIST r a CDATA 'A&#13;&#10;B'>\">%p;]><r/>",
        "<!DOCTYPE r [<!ENTITY e 'A&#13;&#10;B'><!ATTLIST r a CDATA '&e;'>]><r/>",
        "<!DOCTYPE r [<!ENTITY e \"<r a='A&#13;&#10;B'/>\">]><outer>&e;</outer>",
        "<!DOCTYPE r [<!ENTITY e 'A&#13;&#10;B'><!ENTITY body \"<r a='&e;'/>\">]><outer>&body;</outer>",
    ] {
        assert_eq!(attribute(document, Config::default()).unwrap(), "A  B");
    }
    for document in [
        "<r a='A\r\nB'/>",
        "<!DOCTYPE r [<!ENTITY % p \"<!ATTLIST r a CDATA 'A\r\nB'>\">%p;]><r/>",
        "<!DOCTYPE r [<!ENTITY e 'A\r\nB'>]><r a='&e;'/>",
        "<!DOCTYPE r [<!ENTITY e \"<r a='A\r\nB'/>\">]><outer>&e;</outer>",
    ] {
        assert_eq!(attribute(document, Config::default()).unwrap(), "A B");
    }
    // Numeric references in the attribute itself bypass whitespace replacement.
    assert_eq!(
        attribute("<r a='A&#13;&#10;B'/>", Config::default()).unwrap(),
        "A\r\nB"
    );
}

#[test]
fn unparsed_entities_report_the_binary_reference_error_in_attributes() {
    for standalone in ["", "<?xml version='1.0' standalone='yes'?>"] {
        for (default, body) in [("", "<r a='&e;'/>"), ("<!ATTLIST r a CDATA '&e;'>", "<r/>")] {
            let document = format!(
                "{standalone}<!DOCTYPE r [<!NOTATION n SYSTEM 'n'><!ENTITY e SYSTEM 'unused' NDATA n>{default}]>{body}"
            );
            assert_eq!(
                attribute(&document, Config::default()),
                Err(ErrorKind::BinaryEntityReference)
            );
        }
    }
}

#[test]
fn converted_line_endings_keep_their_literal_or_replacement_origin() {
    for (document, expected) in [
        (
            b"<!DOCTYPE r [<!ATTLIST r a CDATA 'A\x80R\r\n\x80NB'>]><r/>".as_slice(),
            "A\r \nB",
        ),
        (
            b"<!DOCTYPE r [<!ENTITY e 'A\x80R\r\n\x80NB'><!ATTLIST r a CDATA '&e;'>]><r/>",
            "A   B",
        ),
        (
            b"<!DOCTYPE r [<!ENTITY % p \"<!ATTLIST r a CDATA 'A\x80R\r\n\x80NB'>\">%p;]><r/>",
            "A   B",
        ),
        (
            b"<!DOCTYPE r [<!ENTITY e \"<r a='A\x80R\r\n\x80NB'/>\">]><outer>&e;</outer>",
            "A   B",
        ),
    ] {
        for width in [1, 2, document.len()] {
            for namespace_separator in [None, Some('|')] {
                let mut parser = Parser::new(Config {
                    encoding: Some("custom".into()),
                    namespace_separator,
                    ..Config::default()
                });
                assert!(parser.set_param_entity_parsing(2));
                let mut values = Vec::new();
                for (index, bytes) in document.chunks(width).enumerate() {
                    parser
                        .feed(bytes, (index + 1) * width >= document.len())
                        .unwrap();
                    loop {
                        match parser.next_event() {
                            Ok(Some(event)) => {
                                if let EventKind::StartElement { attributes, .. } = event.kind {
                                    values.extend(
                                        attributes
                                            .iter()
                                            .map(|attribute| attribute.value.to_string()),
                                    );
                                }
                            }
                            Ok(None) => {
                                let Some(request) = parser.encoding_conversion() else {
                                    break;
                                };
                                let character = match request.bytes[1] {
                                    b'R' => b'\r',
                                    b'N' => b'\n',
                                    _ => panic!("unexpected conversion"),
                                };
                                parser
                                    .resolve_encoding_conversion(i32::from(character))
                                    .unwrap();
                            }
                            Err(error) if error.kind == ErrorKind::UnknownEncoding => {
                                let mut map =
                                    std::array::from_fn(
                                        |byte| if byte < 128 { byte as i32 } else { -1 },
                                    );
                                map[128] = -2;
                                parser.set_multibyte_encoding_map("custom", map).unwrap();
                            }
                            Err(error) => panic!("{document:?}: {error}"),
                        }
                    }
                }
                assert!(parser.is_finished());
                assert_eq!(
                    values,
                    [expected],
                    "{document:?}/{width}/{namespace_separator:?}"
                );
            }
        }
    }
}

#[test]
fn deep_inherited_context_is_indexed_once_for_many_shallow_attributes() {
    const DEPTH: usize = 60_000;
    const ATTRIBUTES: usize = 10_000;
    let mut parent = Parser::new(Config {
        limits: Limits {
            max_entity_depth: DEPTH + 1,
            ..Limits::default()
        },
        ..Config::default()
    });
    parent
        .feed(b"<!DOCTYPE r [<!ENTITY v 'leaf'>]><r/>", true)
        .unwrap();
    while parent.next_event().unwrap().is_some() {}
    let mut context = String::new();
    for i in 0..DEPTH {
        write!(context, "s{i}\u{c}").unwrap();
    }
    let mut child = parent.external_child(Some(&context), None).unwrap();
    drop(parent);
    child.set_hash_salt([41; 16]).unwrap();
    let mut input = String::from("<r");
    for i in 0..ATTRIBUTES {
        write!(input, " a{i}='&v;&v;'").unwrap();
    }
    input.push_str("/><r a='&v;'/>");
    let mut elements = 0;
    for (index, bytes) in input.as_bytes().chunks(4096).enumerate() {
        child
            .feed(bytes, (index + 1) * 4096 >= input.len())
            .unwrap();
        while let Some(event) = child.next_event().unwrap() {
            if let EventKind::StartElement { attributes, .. } = event.kind {
                let (count, value) = if elements == 0 {
                    (ATTRIBUTES, "leafleaf")
                } else {
                    (1, "leaf")
                };
                assert_eq!(attributes.len(), count);
                assert!(attributes.iter().all(|attribute| attribute.value == value));
                elements += 1;
            }
        }
    }
    assert_eq!(elements, 2);
}

#[test]
fn inherited_attribute_names_preserve_exact_general_membership_and_error_order() {
    for (context, input, depth, expected) in [
        (
            "missing",
            "<r a='&missing;'/>",
            0,
            ErrorKind::RecursiveEntityReference,
        ),
        ("missing", "<r a='&bad name;'/>", 0, ErrorKind::InvalidToken),
        ("%p", "<r a='&p;'/>", 0, ErrorKind::LimitExceeded),
    ] {
        let parent = Parser::new(Config::default());
        let mut child = parent.external_child(Some(context), None).unwrap();
        child
            .set_limits(Limits {
                max_entity_depth: depth,
                ..Limits::default()
            })
            .unwrap();
        child.feed(input.as_bytes(), true).unwrap();
        assert_eq!(child.next_event().unwrap_err().kind, expected);
    }
    // A live content source contributes no inherited general name. Replaying its
    // replacement in an attribute reaches the literal '<' error first.
    assert_eq!(
        attribute(
            "<!DOCTYPE r [<!ENTITY e \"<r a='&e;'/>\">]><outer>&e;</outer>",
            Config::default()
        ),
        Err(ErrorKind::InvalidToken)
    );
    let mut parent = Parser::new(Config::default());
    parent
        .feed(b"<!DOCTYPE r [<!ENTITY p 'ok'>]><r/>", true)
        .unwrap();
    while parent.next_event().unwrap().is_some() {}
    let mut child = parent.external_child(Some("%p"), None).unwrap();
    child.feed(b"<r a='&p;&p;'/>", true).unwrap();
    let EventKind::StartElement { attributes, .. } = child.next_event().unwrap().unwrap().kind
    else {
        panic!()
    };
    assert_eq!(attributes[0].value, "okok");
}
