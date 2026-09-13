use xeme::{Config, ErrorKind, EventKind, Limits, Parser};

#[derive(Clone, Copy)]
enum Load {
    Parse(&'static [u8]),
    Skip,
    EmptyNonfinal,
    IgnoreError(&'static [u8]),
}

#[test]
fn value_continuations_preserve_grammar_and_default_callback_state() {
    let bodies = [
        "<!ENTITY e %missing;'L%p;R'>",
        "<!ENTITY % quoted \"'L&#37;p;R'\"><!ENTITY e %quoted;>",
        "<!ENTITY % quoted \"'L&#37;p;R'\"><!ENTITY e %quoted; %missing;>",
    ];
    for (case, body) in bodies.iter().enumerate() {
        let text =
            format!("<!ENTITY % p SYSTEM 'p'>{body}<!ENTITY after 'A'><!ATTLIST r a CDATA 'yes'>");
        for standalone in [false, true] {
            // A missing reference before the value suppresses the whole
            // declaration in a non-standalone document, so no child is loaded.
            for defaults in [false, true] {
                for change in [false, true] {
                    for width in [1, 7, text.len()] {
                        let mut parent = Parser::new(Config::default());
                        if standalone {
                            parent
                                .feed(b"<?xml version='1.0' standalone='yes'?>", false)
                                .unwrap();
                            while parent.next_event().unwrap().is_some() {}
                        }
                        let mut parser = parent.external_child(None, None).unwrap();
                        parser.set_param_entity_parsing(2);
                        parser.set_default_events(defaults);
                        let mut values = Vec::new();
                        let mut raw = String::new();
                        let mut attributes = 0;
                        for (index, chunk) in text.as_bytes().chunks(width).enumerate() {
                            parser
                                .feed(chunk, (index + 1) * width >= text.len())
                                .unwrap();
                            while let Some(event) = parser.next_event().unwrap() {
                                if let Some(fragment) = parser.current_raw() {
                                    raw.push_str(fragment);
                                }
                                match event.kind {
                                    EventKind::ExternalEntityReference(_) => {
                                        if change {
                                            parser.set_default_events(!defaults);
                                        }
                                        let mut child = parser.external_child(None, None).unwrap();
                                        child.feed(b"X", true).unwrap();
                                        while child.next_event().unwrap().is_some() {}
                                        parser.merge_external_subset(&child).unwrap();
                                    }
                                    EventKind::EntityDeclaration(declaration)
                                        if declaration.value.is_some() =>
                                    {
                                        let xeme::EntityDeclaration {
                                            name,
                                            value: stored_value,
                                            ..
                                        } = xeme_storage::Box::into_inner(declaration);
                                        let value = stored_value.expect("matched optional field");

                                        values.push((name.to_string(), value.to_string()));
                                    }
                                    EventKind::AttlistDeclaration(_) => attributes += 1,
                                    _ => {}
                                }
                            }
                        }
                        let before_skipped = case == 0 && !standalone;
                        let after_skipped = case != 1 && !standalone;
                        assert_eq!(
                            values
                                .iter()
                                .any(|(name, value)| name == "e" && value == "LXR"),
                            !before_skipped
                        );
                        assert_eq!(
                            values.iter().any(|(name, _)| name == "after"),
                            !after_skipped
                        );
                        assert_eq!(attributes, usize::from(!after_skipped));
                        if case == 0 && standalone && defaults {
                            assert_eq!(raw.matches("<!ENTITY e %missing;").count(), 1);
                        }
                        if !before_skipped && change && !defaults {
                            assert!(
                                raw.contains("'L%p;R'"),
                                "missing newly enabled suffix: {raw}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn unread_values_default_the_gap_before_trailing_grammar() {
    let parent = Parser::new(Config::default());
    let mut parser = parent.external_child(None, None).unwrap();
    parser.set_param_entity_parsing(2);
    parser.set_default_events(true);
    parser
        .feed(
            b"<!ENTITY % p SYSTEM 'p'><!ENTITY % quoted \"'L&#37;p;R'\"><!ENTITY e %quoted; %missing;><!ENTITY after 'A'>",
            true,
        )
        .unwrap();
    let mut after_value = false;
    let mut defaults = String::new();
    while let Some(event) = parser.next_event().unwrap() {
        match event.kind {
            EventKind::ExternalEntityReference(_) => {
                // Return without creating a child; the parent must retain its
                // current declaration but stop processing later declarations.
            }
            EventKind::EntityDeclaration(declaration) if (declaration.name == "e") => {
                let xeme::EntityDeclaration { value, .. } =
                    xeme_storage::Box::into_inner(declaration);

                assert_eq!(value.as_deref(), Some("LR"));
                assert_eq!(parser.current_raw(), Some("'L%p;R'"));
                after_value = true;
            }
            EventKind::Default if after_value => defaults.push_str(parser.current_raw().unwrap()),
            EventKind::EntityDeclaration(declaration) => {
                let xeme::EntityDeclaration { name, .. } =
                    xeme_storage::Box::into_inner(declaration);
                assert_ne!(name, "after")
            }
            _ => {}
        }
    }
    assert!(after_value);
    assert_eq!(defaults, " %missing;><!ENTITY after 'A'>");
}

fn drain(
    parser: &mut Parser,
    loads: &[(&str, Load)],
    chunk: usize,
    declarations: &mut Vec<(String, String)>,
    depth: usize,
) -> Result<(), ErrorKind> {
    assert!(
        depth < 8,
        "recursive references must fail before loading again"
    );
    while let Some(event) = parser.next_event().map_err(|error| error.kind)? {
        match event.kind {
            EventKind::ExternalEntityReference(declaration) if declaration.system_id.is_some() => {
                let xeme::ExternalEntityReference {
                    context,
                    system_id: stored_system_id,
                    ..
                } = xeme_storage::Box::into_inner(declaration);
                let system = stored_system_id.expect("matched optional field");

                assert!(context.is_none());
                let load = loads
                    .iter()
                    .find(|(name, _)| *name == system.as_str())
                    .unwrap()
                    .1;
                if matches!(load, Load::Skip) {
                    continue;
                }
                let mut child = parser
                    .external_child(None, None)
                    .map_err(|error| error.kind)?;
                let bytes = match load {
                    Load::Parse(bytes) | Load::IgnoreError(bytes) => bytes,
                    Load::EmptyNonfinal => b"",
                    Load::Skip => unreachable!(),
                };
                let mut result = Ok(());
                if bytes.is_empty() {
                    child
                        .feed(b"", !matches!(load, Load::EmptyNonfinal))
                        .map_err(|error| error.kind)?;
                    result = drain(&mut child, loads, chunk, declarations, depth + 1);
                }
                for (index, part) in bytes.chunks(chunk).enumerate() {
                    child
                        .feed(part, (index + 1) * chunk >= bytes.len())
                        .map_err(|error| error.kind)?;
                    result = drain(&mut child, loads, chunk, declarations, depth + 1);
                    if result.is_err() {
                        break;
                    }
                }
                if !matches!(load, Load::IgnoreError(_)) {
                    result?;
                }
                if child.is_finished() {
                    parser
                        .merge_external_subset(&child)
                        .map_err(|error| error.kind)?;
                }
            }
            EventKind::EntityDeclaration(declaration) if declaration.value.is_some() => {
                let xeme::EntityDeclaration {
                    name,
                    value: stored_value,
                    ..
                } = xeme_storage::Box::into_inner(declaration);
                let value = stored_value.expect("matched optional field");

                declarations.push((name.to_string(), value.to_string()));
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse(
    bytes: &[u8],
    chunk: usize,
    mode: u8,
    standalone: bool,
    loads: &[(&str, Load)],
    config: Config,
) -> Result<Vec<(String, String)>, ErrorKind> {
    let mut root = Parser::new(config);
    if standalone {
        root.feed(b"<?xml version='1.0' standalone='yes'?>", false)
            .unwrap();
        while root.next_event().unwrap().is_some() {}
    }
    let mut parser = root
        .external_child(None, None)
        .map_err(|error| error.kind)?;
    parser.set_default_events(true);
    assert!(parser.set_param_entity_parsing(mode));
    let mut declarations = Vec::new();
    for (index, part) in bytes.chunks(chunk).enumerate() {
        parser
            .feed(part, (index + 1) * chunk >= bytes.len())
            .map_err(|error| error.kind)?;
        drain(&mut parser, loads, chunk, &mut declarations, 0)?;
    }
    Ok(declarations)
}

#[test]
fn newer_versions_in_external_values_preserve_replacement_text() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ENTITY e 'L%p;R'>";
    for body in [
        b"<?xml version='1.1' encoding='UTF-8'?>X".as_slice(),
        b"<?xml version='1.7' encoding='UTF-8'?>X".as_slice(),
    ] {
        for chunk in 1..=body.len() {
            assert_eq!(
                parse(
                    dtd,
                    chunk,
                    2,
                    false,
                    &[("p", Load::Parse(body))],
                    Config::default()
                )
                .unwrap(),
                [("e".into(), "LXR".into())]
            );
        }
    }
}

#[test]
fn external_value_bytes_remain_data_at_every_chunk_width() {
    let dtd = "<!ENTITY % p SYSTEM 'p'><!ENTITY e 'L%p;R'><!ENTITY after 'A'>";
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
                let declarations = parse(
                    &bytes,
                    chunk,
                    mode,
                    false,
                    &[("p", Load::Parse(b"\"&#x1F600;&#13;&#10;&amp;\""))],
                    Config::default(),
                )
                .unwrap();
                assert_eq!(
                    declarations,
                    [
                        ("e".into(), "L\"😀\r\n&amp;\"R".into()),
                        ("after".into(), "A".into())
                    ]
                );
            }
        }
    }
}

#[test]
fn initialized_unread_and_ignored_error_values_preserve_distinct_results() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ENTITY e 'L%p;R'><!ENTITY after 'A'>";
    for (load, expected, skip) in [
        (Load::Parse(b"X"), "LXR", false),
        (Load::Skip, "LR", true),
        (Load::EmptyNonfinal, "LR", false),
        (Load::IgnoreError(b"&amp;"), "LR", false),
        (Load::IgnoreError(b"\"a&#0;b\""), "L\"aR", false),
    ] {
        for standalone in [false, true] {
            for chunk in [1, 3, dtd.len()] {
                let values =
                    parse(dtd, chunk, 0, standalone, &[("p", load)], Config::default()).unwrap();
                assert_eq!(values[0], ("e".into(), expected.into()));
                assert_eq!(values.len(), if skip && !standalone { 1 } else { 2 });
            }
        }
    }
}

#[test]
fn nested_value_output_and_child_truncation_follow_source_boundaries() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY % i 'I'><!ENTITY e 'L%p;R'><!ENTITY after 'A'>";
    for (p, q, expected, skip) in [
        (b"X%q;Y".as_slice(), Load::Parse(b"Q"), "LXQYR", false),
        (
            b"X%q;Y".as_slice(),
            Load::IgnoreError(b"\"Q&#0;Z\""),
            "LX\"QYR",
            false,
        ),
        (b"X%i;Y".as_slice(), Load::Parse(b""), "LXR", false),
        (b"X%missing;Y".as_slice(), Load::Parse(b""), "LXR", true),
    ] {
        let values = parse(
            dtd,
            1,
            2,
            false,
            &[("p", Load::Parse(p)), ("q", q)],
            Config::default(),
        )
        .unwrap();
        assert_eq!(values[1], ("e".into(), expected.into()));
        assert_eq!(values.len(), if skip { 2 } else { 3 });
    }
}

#[test]
fn value_children_inherit_active_parameter_names_and_depth() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY % e 'L%p;R'>";
    for loads in [
        vec![("p", Load::Parse(b"%p;"))],
        vec![("p", Load::Parse(b"%e;"))],
        vec![("p", Load::Parse(b"%q;")), ("q", Load::Parse(b"%p;"))],
    ] {
        assert_eq!(
            parse(dtd, 1, 2, false, &loads, Config::default()),
            Err(ErrorKind::RecursiveEntityReference)
        );
    }
    let config = Config {
        limits: Limits {
            max_entity_depth: 2,
            ..Limits::default()
        },
        ..Config::default()
    };
    assert_eq!(
        parse(
            dtd,
            1,
            2,
            false,
            &[("p", Load::Parse(b"%q;")), ("q", Load::Parse(b"Q"))],
            config
        ),
        Err(ErrorKind::LimitExceeded)
    );
}

#[test]
fn value_channel_survives_custom_encoding_recovery_and_parent_drop() {
    fn assert_send<T: Send>() {}
    assert_send::<Parser>();
    let root = Parser::new(Config::default());
    let mut dtd = root.external_child(None, None).unwrap();
    dtd.feed(b"<!ENTITY % p SYSTEM 'p'><!ENTITY e '%p;'>", true)
        .unwrap();
    while !matches!(
        dtd.next_event().unwrap().unwrap().kind,
        EventKind::ExternalEntityReference(_)
    ) {}
    let mut child = dtd
        .external_child_with_encoding(None, Some("custom"))
        .unwrap();
    child.feed(b"\"\x80&#13;\"", true).unwrap();
    assert_eq!(
        child.next_event().unwrap_err().kind,
        ErrorKind::UnknownEncoding
    );
    let mut map = std::array::from_fn(|index| index as i32);
    map[128] = 0x20ac;
    child.set_encoding_map("custom", map).unwrap();
    while child.next_event().unwrap().is_some() {}
    dtd.merge_external_subset(&child).unwrap();
    let value = dtd.next_event().unwrap().unwrap();
    assert!(
        matches!(value.kind, EventKind::EntityDeclaration(declaration) if matches!(declaration.as_ref(), xeme::EntityDeclaration { value: Some(value), .. } if value == "\"€\r\""))
    );
    drop(root);
    drop(dtd);
    assert!(child.is_finished());
    drop(child);
}

#[test]
fn external_value_output_and_family_work_have_independent_limits() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ENTITY e '%p;%p;%p;'>";
    let config = Config {
        limits: Limits {
            max_token_bytes: 40,
            ..Limits::default()
        },
        ..Config::default()
    };
    assert_eq!(
        parse(
            dtd,
            1,
            2,
            false,
            &[("p", Load::Parse(b"abcdefghijklmnopqrst"))],
            config
        ),
        Err(ErrorKind::LimitExceeded)
    );
    let config = Config {
        limits: Limits {
            max_entity_expansion_bytes: 512,
            ..Limits::default()
        },
        ..Config::default()
    };
    assert_eq!(
        parse(dtd, 1, 2, false, &[("p", Load::Parse(b"X"))], config),
        Err(ErrorKind::LimitExceeded)
    );
}

#[test]
fn child_xml_declaration_changes_standalone_without_disabling_ancestor_mode() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY e 'L%p;R'>%q;<!ENTITY after 'A'>";
    let values = parse(
        dtd,
        1,
        1,
        false,
        &[
            (
                "p",
                Load::Parse(b"ignored <?xml version='1.0' standalone='yes'?>X%missing;Y"),
            ),
            ("q", Load::Parse(b"<!ENTITY loaded 'yes'>")),
        ],
        Config::default(),
    )
    .unwrap();
    assert_eq!(
        values,
        [
            ("e".into(), "LXR".into()),
            ("loaded".into(), "yes".into()),
            ("after".into(), "A".into())
        ]
    );
}

#[test]
fn external_value_internal_references_remain_open_in_the_shared_dtd() {
    let failures: &[&[u8]] = &[
        b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b 'B'><!ENTITY n '%a;%a;'>",
        b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b 'B'><!ENTITY n '%a;%b;'>",
        b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b 'B'><!ENTITY n '%a;'><!ENTITY m '%b;'>",
        b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b 'B'><!ENTITY n '%a;'><!ENTITY m '%a;'>",
        b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b ''><!ENTITY n '%a;'>%b;",
        b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b 'EMPTY'><!ENTITY n '%a;'><!ELEMENT r %b;>",
        b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b 'INCLUDE'><!ENTITY n '%a;'><![%b;[<!ELEMENT r EMPTY>]]>",
        b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b 'B'><!ENTITY % c SYSTEM 'c'><!ENTITY n '%c;'><!ENTITY m '%b;'>",
    ];
    for bytes in failures {
        for chunk in [1, 7, bytes.len()] {
            assert_eq!(
                parse(
                    bytes,
                    chunk,
                    2,
                    false,
                    &[("a", Load::Parse(b"%b;")), ("c", Load::Parse(b"%a;"))],
                    Config::default(),
                ),
                Err(ErrorKind::RecursiveEntityReference),
                "{} / chunk {chunk}",
                std::str::from_utf8(bytes).unwrap()
            );
        }
    }
    // Only the first internal reference is opened: the external value processor
    // discards its tail. Missing parameters do not open later references either.
    let bytes = b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b 'B'><!ENTITY % c 'C'><!ENTITY n '%a;%c;'>";
    for (value, expected) in [(b"L%b;%c;".as_slice(), "LC"), (b"%missing;%b;", "C")] {
        let declarations = parse(
            bytes,
            1,
            2,
            false,
            &[("a", Load::Parse(value))],
            Config::default(),
        )
        .unwrap();
        assert!(
            declarations
                .iter()
                .any(|(name, value)| name == "n" && value == expected)
        );
    }
}

#[test]
fn open_value_state_is_shared_by_siblings_but_not_general_entity_copies() {
    let root = Parser::new(Config::default());
    let mut dtd = root.external_child(None, None).unwrap();
    dtd.feed(
        b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b 'B'><!ENTITY n '%a;'>",
        true,
    )
    .unwrap();
    while !matches!(
        dtd.next_event().unwrap().unwrap().kind,
        EventKind::ExternalEntityReference(_)
    ) {}
    let mut first = dtd.external_child(None, None).unwrap();
    let mut sibling = dtd.external_child(None, None).unwrap();
    first.feed(b"L%b;R", true).unwrap();
    while first.next_event().unwrap().is_some() {}
    // General-content parsers copy the DTD and clear the open state, including
    // copies made after the original entity was left open.
    let general = dtd.external_child(Some(""), None).unwrap();
    let mut copied_dtd = general.external_child(None, None).unwrap();
    drop(first);
    drop(dtd);
    drop(root);
    drop(general);
    sibling.feed(b"%b;", true).unwrap();
    assert_eq!(
        sibling.next_event().unwrap_err().kind,
        ErrorKind::RecursiveEntityReference
    );
    copied_dtd.feed(b"<!ENTITY m '%b;'>", true).unwrap();
    let declaration = copied_dtd.next_event().unwrap().unwrap();
    assert!(matches!(
        declaration.kind,
        EventKind::EntityDeclaration(ref declaration)
            if declaration.name == "m" && declaration.value.as_deref() == Some("B")
    ));
    assert!(copied_dtd.next_event().unwrap().is_none());
}
