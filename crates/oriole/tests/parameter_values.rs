use oriole::{Config, ErrorKind, EventKind, Limits, Parser};

#[derive(Debug, Default)]
struct Declarations {
    values: Vec<(String, bool, String)>,
    defaults: String,
    attributes: usize,
}

fn drain(parser: &mut Parser, declarations: &mut Declarations) -> Result<(), ErrorKind> {
    while let Some(event) = parser.next_event().map_err(|error| error.kind)? {
        match event.kind {
            EventKind::EntityDeclaration(declaration) if declaration.value.is_some() => {
                let oriole::EntityDeclaration {
                    name,
                    parameter,
                    value: stored_value,
                    ..
                } = oriole_storage::Box::into_inner(declaration);
                let value = stored_value.expect("matched optional field");

                declarations
                    .values
                    .push((name.to_string(), parameter, value.to_string()));
            }
            EventKind::Default => declarations
                .defaults
                .push_str(parser.current_raw().unwrap()),
            EventKind::AttlistDeclaration(_) => declarations.attributes += 1,
            _ => {}
        }
    }
    Ok(())
}

fn external(
    bytes: &[u8],
    chunk: usize,
    mode: u8,
    standalone: bool,
    config: Config,
) -> Result<(Parser, Declarations), ErrorKind> {
    let mut parent = Parser::new(config);
    parent.set_param_entity_parsing(2);
    parent
        .feed(
            if standalone {
                b"<?xml version='1.0' standalone='yes'?><!DOCTYPE r SYSTEM 'd'>"
            } else {
                b"<!DOCTYPE r SYSTEM 'd'>"
            },
            false,
        )
        .unwrap();
    drain(&mut parent, &mut Declarations::default())?;
    let mut child = parent
        .external_child(None, None)
        .map_err(|error| error.kind)?;
    child.set_param_entity_parsing(mode);
    child.set_default_events(true);
    let mut declarations = Declarations::default();
    for (index, bytes_chunk) in bytes.chunks(chunk).enumerate() {
        child
            .feed(bytes_chunk, (index + 1) * chunk >= bytes.len())
            .map_err(|error| error.kind)?;
        drain(&mut child, &mut declarations)?;
    }
    parent
        .merge_external_subset(&child)
        .map_err(|error| error.kind)?;
    Ok((parent, declarations))
}

#[test]
fn replacement_boundaries_preserve_quotes_references_and_line_endings() {
    let dtd = "<!ENTITY % p '&#34;&#39;<> &amp;'><!ENTITY % cr '&#13;'><!ENTITY % numeric '&#38;#65;'><!ENTITY % nested '&#37;numeric;'><!ENTITY e 'L%p;:%nested;:%cr;\nR'><!ENTITY literal '&#37;missing;'>";
    let expected = "L\"'<> &amp;:A:\r\nR";
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
            for mode in 0..=2 {
                let (_, declarations) =
                    external(&bytes, chunk, mode, false, Config::default()).unwrap();
                assert_eq!(declarations.values[4], ("e".into(), false, expected.into()));
                assert_eq!(declarations.values[5].2, "%missing;");
            }
        }
    }
    let dtd = b"<!ENTITY % p 'A\r\nB\rC'><!ENTITY e '%p;'>";
    for chunk in 1..=dtd.len() {
        let (_, declarations) = external(dtd, chunk, 2, false, Config::default()).unwrap();
        assert_eq!(declarations.values[1].2, "A\nB\nC");
    }
}

#[test]
fn missing_parameters_finish_current_value_and_control_later_declarations() {
    let dtd = b"<!ENTITY % p 'L&#37;missing;R'><!ENTITY e 'A%p;B'><!ENTITY after 'later'><!ATTLIST r a CDATA 'yes'>";
    for standalone in [false, true] {
        for mode in 0..=2 {
            for chunk in 1..=dtd.len() {
                let (mut parent, declarations) =
                    external(dtd, chunk, mode, standalone, Config::default()).unwrap();
                assert_eq!(declarations.values[1], ("e".into(), false, "ALRB".into()));
                assert_eq!(declarations.values.len(), if standalone { 3 } else { 2 });
                assert_eq!(declarations.attributes, usize::from(standalone));
                assert_eq!(
                    declarations.defaults,
                    if standalone {
                        ""
                    } else {
                        "><!ENTITY after 'later'><!ATTLIST r a CDATA 'yes'>"
                    }
                );
                if !standalone {
                    parent.feed(b"<r>&e;</r>", true).unwrap();
                    let mut text = String::new();
                    while let Some(event) = parent.next_event().unwrap() {
                        if let EventKind::Text(value) = event.kind {
                            text.push_str(&value);
                        }
                    }
                    assert_eq!(text, "ALRB");
                }
            }
        }
    }
}

#[test]
fn references_must_be_complete_within_each_replacement() {
    for (dtd, expected) in [
        (
            "<!ENTITY % p '&#38;'><!ENTITY e '%p;amp;'>",
            ErrorKind::InvalidToken,
        ),
        (
            "<!ENTITY % p '&#37;'><!ENTITY e '%p;p;'>",
            ErrorKind::InvalidToken,
        ),
        (
            "<!ENTITY e '%missing;&#0;'>",
            ErrorKind::BadCharacterReference,
        ),
        ("<!ENTITY % p '%p;'>", ErrorKind::RecursiveEntityReference),
        (
            "<!ENTITY % p '&#37;q;'><!ENTITY % q '&#37;p;'><!ENTITY e '%p;'>",
            ErrorKind::RecursiveEntityReference,
        ),
        (
            "<!ENTITY % p \"<!ENTITY e '&#37;p;'>\">%p;",
            ErrorKind::RecursiveEntityReference,
        ),
    ] {
        for chunk in 1..=dtd.len() {
            assert_eq!(
                external(dtd.as_bytes(), chunk, 2, false, Config::default()).unwrap_err(),
                expected,
                "{dtd}, chunk {chunk}"
            );
        }
    }
}

#[test]
fn legal_context_and_first_declaration_take_precedence() {
    for (raw, expected) in [
        ("%p;", ErrorKind::ParameterEntityReference),
        ("%missing;", ErrorKind::ParameterEntityReference),
        ("%missing", ErrorKind::InvalidToken),
        ("%bad name;", ErrorKind::InvalidToken),
    ] {
        let doc = format!("<!DOCTYPE r [<!ENTITY % p 'P'><!ENTITY e '{raw}'>]><r/>");
        for chunk in 1..=doc.len() {
            let mut parser = Parser::new(Config::default());
            parser.set_param_entity_parsing(2);
            let mut result = Ok(());
            for (index, bytes) in doc.as_bytes().chunks(chunk).enumerate() {
                parser
                    .feed(bytes, (index + 1) * chunk >= doc.len())
                    .unwrap();
                result = drain(&mut parser, &mut Declarations::default());
                if result.is_err() {
                    break;
                }
            }
            assert_eq!(result.unwrap_err(), expected);
        }
    }
    let dtd = b"<!ENTITY % p 'old'><!ENTITY % p '%p;'><!ENTITY p '%p;'><!ENTITY e '%p;'>";
    let (_, declarations) = external(dtd, 1, 0, false, Config::default()).unwrap();
    assert_eq!(
        declarations.values,
        [
            ("p".into(), true, "old".into()),
            ("p".into(), false, "old".into()),
            ("e".into(), false, "old".into())
        ]
    );
    let doc =
        b"<!DOCTYPE r [<!ENTITY % p 'P'><!ENTITY % d \"<!ENTITY e '&#37;p;'>\">%d;]><r>&e;</r>";
    for chunk in 1..=doc.len() {
        let mut parser = Parser::new(Config::default());
        parser.set_param_entity_parsing(2);
        let mut declarations = Declarations::default();
        for (index, bytes) in doc.chunks(chunk).enumerate() {
            parser
                .feed(bytes, (index + 1) * chunk >= doc.len())
                .unwrap();
            drain(&mut parser, &mut declarations).unwrap();
        }
        assert_eq!(declarations.values[2], ("e".into(), false, "P".into()));
    }
}

#[test]
fn replacement_depth_output_and_empty_reference_work_are_bounded() {
    let cases = [
        (
            Limits {
                max_entity_depth: 2,
                ..Limits::default()
            },
            "<!ENTITY % p 'x'><!ENTITY % q '&#37;p;'><!ENTITY e '%q;'>".to_string(),
        ),
        (
            Limits {
                max_token_bytes: 64,
                ..Limits::default()
            },
            format!("<!ENTITY % p '{}'><!ENTITY e '%p;%p;'>", "x".repeat(40)),
        ),
        (
            Limits {
                max_entity_expansion_bytes: 4096,
                ..Limits::default()
            },
            format!("<!ENTITY % p ''><!ENTITY e '{}'>", "%p;".repeat(200)),
        ),
    ];
    for (limits, dtd) in cases {
        let config = Config {
            limits,
            ..Config::default()
        };
        assert_eq!(
            external(dtd.as_bytes(), 1, 2, false, config).unwrap_err(),
            ErrorKind::LimitExceeded
        );
    }
}

#[test]
fn standalone_mode_one_skips_root_parameters_but_processes_external_dtds() {
    let dtd = b"<!ENTITY % p 'P'><!ENTITY % d \"<!ENTITY e '&#37;p;'>\">%d;";
    for mode in 0..=2 {
        for chunk in 1..=dtd.len() {
            let (_, child) = external(dtd, chunk, mode, true, Config::default()).unwrap();
            assert_eq!(
                child.values.iter().any(|(name, _, _)| name == "e"),
                mode != 0
            );
            let mut root = Parser::new(Config::default());
            root.set_param_entity_parsing(mode);
            let document = format!(
                "<?xml version='1.0' standalone='yes'?><!DOCTYPE r [{}]><r/>",
                std::str::from_utf8(dtd).unwrap()
            );
            let mut declarations = Declarations::default();
            for (index, bytes) in document.as_bytes().chunks(chunk).enumerate() {
                root.feed(bytes, (index + 1) * chunk >= document.len())
                    .unwrap();
                drain(&mut root, &mut declarations).unwrap();
            }
            assert_eq!(
                declarations.values.iter().any(|(name, _, _)| name == "e"),
                mode == 2
            );
        }
    }
    let recursive = b"<!ENTITY % p \"<!ENTITY e '&#37;p;'>\">%p;";
    assert_eq!(
        external(recursive, 1, 1, true, Config::default()).unwrap_err(),
        ErrorKind::RecursiveEntityReference
    );
}

#[test]
fn external_children_inherit_the_effective_standalone_parameter_mode() {
    let dtd = b"<!ENTITY % d \"<!ENTITY e 'value'>\">%d;";
    for root_mode in 0..=2 {
        for override_mode in [None, Some(1)] {
            let mut root = Parser::new(Config::default());
            root.set_param_entity_parsing(root_mode);
            root.feed(b"<?xml version='1.0' standalone='yes'?>", false)
                .unwrap();
            drain(&mut root, &mut Declarations::default()).unwrap();
            let mut child = root.external_child(None, None).unwrap();
            if let Some(mode) = override_mode {
                child.set_param_entity_parsing(mode);
            }
            child.feed(dtd, true).unwrap();
            let mut declarations = Declarations::default();
            drain(&mut child, &mut declarations).unwrap();
            assert_eq!(
                declarations.values.iter().any(|(name, _, _)| name == "e"),
                root_mode == 2 || override_mode.is_some()
            );
        }
    }
}
