use xeme::{Config, ErrorKind, EventKind, Limits, Parser};

#[derive(Clone, Copy)]
enum Load {
    Parse(&'static [u8]),
    Skip,
    CreateOnly,
    EmptyNonfinal,
    BadProtocol,
    IgnoreError,
    GeneralChild,
}

#[derive(Debug, Default)]
struct Events {
    declarations: Vec<String>,
    trace: Vec<String>,
    notifications: Vec<String>,
}

fn drain(
    parser: &mut Parser,
    actor: &str,
    loads: &[(&str, Load)],
    chunk: usize,
    events: &mut Events,
    depth: usize,
) -> Result<(), ErrorKind> {
    assert!(
        depth < 8,
        "recursive references must fail before recursively loading"
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
                events.trace.push(format!("{actor}:external:{system}"));
                let load = loads
                    .iter()
                    .find(|(name, _)| *name == system.as_str())
                    .unwrap()
                    .1;
                if matches!(load, Load::Skip) {
                    continue;
                }
                let mut child = parser
                    .external_child(
                        matches!(load, Load::GeneralChild).then_some(""),
                        matches!(load, Load::BadProtocol).then(|| "BOGUS".into()),
                    )
                    .map_err(|error| error.kind)?;
                match load {
                    Load::CreateOnly => continue,
                    Load::EmptyNonfinal => {
                        child.feed(b"", false).map_err(|error| error.kind)?;
                        drain(&mut child, &system, loads, chunk, events, depth + 1)?;
                    }
                    Load::BadProtocol | Load::IgnoreError => {
                        child
                            .feed(
                                if matches!(load, Load::IgnoreError) {
                                    b"?"
                                } else {
                                    b""
                                },
                                true,
                            )
                            .map_err(|error| error.kind)?;
                        let error = drain(&mut child, &system, loads, chunk, events, depth + 1)
                            .unwrap_err();
                        assert!(matches!(
                            error,
                            ErrorKind::UnknownEncoding
                                | ErrorKind::Syntax
                                | ErrorKind::InvalidToken
                        ));
                    }
                    Load::GeneralChild => {
                        child.feed(b"text", true).map_err(|error| error.kind)?;
                        drain(&mut child, &system, loads, chunk, events, depth + 1)?;
                    }
                    Load::Parse(bytes) => {
                        if bytes.is_empty() {
                            child.feed(b"", true).map_err(|error| error.kind)?;
                        }
                        for (index, part) in bytes.chunks(chunk).enumerate() {
                            child
                                .feed(part, (index + 1) * chunk >= bytes.len())
                                .map_err(|error| error.kind)?;
                            drain(&mut child, &system, loads, chunk, events, depth + 1)?;
                        }
                        drain(&mut child, &system, loads, chunk, events, depth + 1)?;
                    }
                    Load::Skip => unreachable!(),
                }
                if child.is_finished() && child.is_external_subset() {
                    parser
                        .merge_external_subset(&child)
                        .map_err(|error| error.kind)?;
                }
            }
            EventKind::EntityDeclaration(declaration) => {
                let xeme::EntityDeclaration { name, .. } =
                    xeme_storage::Box::into_inner(declaration);

                events.trace.push(format!("{actor}:entity:{name}"));
                events.declarations.push(name.to_string());
            }
            EventKind::Default => events
                .trace
                .push(format!("{actor}:default:{}", parser.current_raw().unwrap())),
            EventKind::NotStandalone => {
                events.trace.push(format!("{actor}:not-standalone"));
                events.notifications.push(actor.to_string());
            }
            EventKind::SkippedEntity { name, parameter } => {
                assert!(parameter);
                events.trace.push(format!("{actor}:skipped:{name}"));
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
) -> Result<Events, ErrorKind> {
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
    parser.set_param_entity_parsing(mode);
    let mut events = Events::default();
    for (index, part) in bytes.chunks(chunk).enumerate() {
        parser
            .feed(part, (index + 1) * chunk >= bytes.len())
            .map_err(|error| error.kind)?;
        drain(&mut parser, "d", loads, chunk, &mut events, 0)?;
    }
    Ok(events)
}

#[test]
fn child_declarations_are_imported_before_the_header_suffix_at_every_chunk_width() {
    let dtd = "<!ENTITY % p SYSTEM 'p'><![%p;%k;[<!ENTITY e 'ok'>]]>";
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
            let events = parse(
                &bytes,
                chunk,
                2,
                false,
                &[("p", Load::Parse(b"<!ENTITY % k 'INCLUDE'>"))],
                Config::default(),
            )
            .unwrap();
            assert_eq!(events.declarations, ["p", "k", "e"]);
            assert_eq!(
                events.trace,
                [
                    "d:entity:p",
                    "d:default:<![",
                    "d:external:p",
                    "p:entity:k",
                    "d:not-standalone",
                    "d:default:INCLUDE[",
                    "d:entity:e",
                    "d:default:]]>"
                ]
            );
        }
    }
}

#[test]
fn parsed_empty_and_unread_children_have_distinct_processing_and_notifications() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><![INCLUDE%p;[<!ENTITY e 'ok'>]]><!ENTITY after 'later'>";
    for (load, read) in [
        (Load::Parse(b""), true),
        (Load::EmptyNonfinal, true),
        (Load::IgnoreError, true),
        (Load::Skip, false),
        (Load::CreateOnly, false),
        (Load::BadProtocol, false),
        (Load::GeneralChild, false),
    ] {
        for standalone in [false, true] {
            for mode in 0..=2 {
                for chunk in [1, 7, dtd.len()] {
                    let events = parse(
                        dtd,
                        chunk,
                        mode,
                        standalone,
                        &[("p", load)],
                        Config::default(),
                    )
                    .unwrap();
                    assert_eq!(
                        events.declarations.len(),
                        if standalone || (mode != 0 && read) {
                            3
                        } else {
                            1
                        }
                    );
                    assert_eq!(
                        events.notifications.len(),
                        usize::from(!standalone && (mode == 0 || read))
                    );
                    assert_eq!(
                        events
                            .trace
                            .iter()
                            .filter(|event| *event == "d:external:p")
                            .count(),
                        usize::from(mode != 0)
                    );
                }
            }
        }
    }
}

#[test]
fn nested_external_read_state_and_missing_child_entities_propagate() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><![INCLUDE%p;[<!ENTITY e 'ok'>]]><!ENTITY after 'later'>";
    for (q, expected) in [(Load::Parse(b""), vec!["p", "d"]), (Load::Skip, vec![])] {
        let events = parse(
            dtd,
            1,
            2,
            false,
            &[("p", Load::Parse(b"%q; ")), ("q", q)],
            Config::default(),
        )
        .unwrap();
        assert_eq!(events.notifications, expected);
        assert_eq!(
            events.declarations.len(),
            if matches!(q, Load::Skip) { 2 } else { 4 }
        );
    }
    let events = parse(
        dtd,
        1,
        2,
        false,
        &[("p", Load::Parse(b"%missing;"))],
        Config::default(),
    )
    .unwrap();
    assert_eq!(events.declarations, ["p", "q"]);
    assert_eq!(events.notifications, ["d"]);
    assert!(events.trace.contains(&"p:skipped:missing".to_string()));
}

#[test]
fn external_parameter_recursion_inherits_internal_and_external_ancestors() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><![INCLUDE%p;[]]>";
    for loads in [
        vec![("p", Load::Parse(b"%p;"))],
        vec![("p", Load::Parse(b"%q;")), ("q", Load::Parse(b"%q;"))],
        vec![("p", Load::Parse(b"<!ENTITY e '%p;'>"))],
    ] {
        assert_eq!(
            parse(dtd, 1, 2, false, &loads, Config::default()).unwrap_err(),
            ErrorKind::RecursiveEntityReference
        );
    }
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ENTITY % inside '&#37;p;'><![INCLUDE%inside;[]]>";
    assert_eq!(
        parse(
            dtd,
            1,
            2,
            false,
            &[("p", Load::Parse(b"%inside;"))],
            Config::default()
        )
        .unwrap_err(),
        ErrorKind::RecursiveEntityReference
    );
}

#[test]
fn header_continuations_share_expansion_and_nesting_limits() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ENTITY % inner '&#37;p;'><!ENTITY % outer '&#37;inner;'><![INCLUDE%outer;[]]>";
    let config = Config {
        limits: Limits {
            max_entity_depth: 3,
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
            &[("p", Load::Parse(b"<!ENTITY % x ''>%x;"))],
            config
        )
        .unwrap_err(),
        ErrorKind::LimitExceeded
    );
    let dtd = format!(
        "<!ENTITY % p SYSTEM 'p'><![INCLUDE{}[]]>",
        "%p;".repeat(1000)
    );
    let config = Config {
        limits: Limits {
            max_entity_expansion_bytes: 8192,
            ..Limits::default()
        },
        ..Config::default()
    };
    assert_eq!(
        parse(
            dtd.as_bytes(),
            1,
            2,
            false,
            &[("p", Load::Parse(b""))],
            config
        )
        .unwrap_err(),
        ErrorKind::LimitExceeded
    );
}

#[test]
fn header_children_are_acknowledged_only_during_the_request_and_outlive_parents() {
    for load in [false, true] {
        let root = Parser::new(Config::default());
        let mut parser = root.external_child(None, None).unwrap();
        parser.set_default_events(true);
        parser.set_param_entity_parsing(2);
        parser
            .feed(
                b"<!ENTITY % p SYSTEM 'p'><![INCLUDE%p;[<!ENTITY e 'ok'>]]>",
                true,
            )
            .unwrap();
        let mut declarations = Vec::new();
        let mut notifications = 0;
        let mut retained = None;
        while let Some(event) = parser.next_event().unwrap() {
            match event.kind {
                EventKind::Default if parser.current_raw() == Some("<![INCLUDE") => {
                    // A child created by the preceding Default callback has no
                    // relationship to the external request that follows it.
                    let mut unrelated = parser.external_child(None, None).unwrap();
                    unrelated.feed(b"", true).unwrap();
                    assert!(unrelated.next_event().unwrap().is_none());
                }
                EventKind::ExternalEntityReference(_) if load => {
                    let mut child = parser.external_child(None, None).unwrap();
                    child.feed(b"", false).unwrap();
                    assert!(child.next_event().unwrap().is_none());
                    retained = Some(child);
                }
                EventKind::EntityDeclaration(declaration) => {
                    let xeme::EntityDeclaration { name, .. } =
                        xeme_storage::Box::into_inner(declaration);

                    declarations.push(name.to_string());
                }
                EventKind::NotStandalone => notifications += 1,
                _ => {}
            }
        }
        assert_eq!(declarations.len(), if load { 2 } else { 1 });
        assert_eq!(notifications, usize::from(load));
        drop(parser);
        drop(root);
        if let Some(mut child) = retained {
            child.feed(b"<!ENTITY retained 'valid'>", true).unwrap();
            assert!(matches!(
                child.next_event().unwrap().unwrap().kind,
                EventKind::EntityDeclaration(_)
            ));
            assert!(child.next_event().unwrap().is_none());
            assert!(child.is_finished());
        }
    }
}

#[test]
fn a_missing_header_parameter_suppresses_declarations_in_later_children() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><![INCLUDE%missing;%p;[<!ENTITY e 'E'>]]>";
    for standalone in [false, true] {
        for mode in [1, 2] {
            for chunk in [1, 7, dtd.len()] {
                let events = parse(
                    dtd,
                    chunk,
                    mode,
                    standalone,
                    &[("p", Load::Parse(b"<!ENTITY imported 'yes'>"))],
                    Config::default(),
                )
                .unwrap();
                assert_eq!(
                    events.declarations,
                    if standalone {
                        vec!["p", "imported", "e"]
                    } else {
                        vec!["p"]
                    }
                );
                assert!(events.trace.iter().any(|event| event == "d:external:p"));
            }
        }
    }
}
