use oriole::{Config, ErrorKind, EventKind, Limits, Parser};

#[derive(Default, Debug)]
struct Seen {
    entities: Vec<String>,
    skipped: Vec<(String, bool)>,
    defaults: String,
    text: String,
    not_standalone: usize,
    attributes: usize,
}

fn drain(parser: &mut Parser, seen: &mut Seen) -> Result<(), ErrorKind> {
    while let Some(event) = parser.next_event().map_err(|error| error.kind)? {
        match event.kind {
            EventKind::EntityDeclaration(value) => seen.entities.push(value.name.to_string()),
            EventKind::SkippedEntity { name, parameter } => {
                seen.skipped.push((name.to_string(), parameter))
            }
            EventKind::Default => seen.defaults.push_str(parser.current_raw().unwrap()),
            EventKind::Text(text) => seen.text.push_str(&text),
            EventKind::NotStandalone => seen.not_standalone += 1,
            EventKind::StartElement { attributes, .. } => seen.attributes += attributes.len(),
            _ => {}
        }
    }
    Ok(())
}

fn parse(xml: &str, mode: u8, chunk: usize, config: Config) -> Result<Seen, ErrorKind> {
    let mut parser = Parser::new(config);
    parser.set_param_entity_parsing(mode);
    parser.set_default_events(true);
    let mut seen = Seen::default();
    for (index, bytes) in xml.as_bytes().chunks(chunk).enumerate() {
        parser
            .feed(bytes, (index + 1) * chunk >= xml.len())
            .map_err(|error| error.kind)?;
        drain(&mut parser, &mut seen)?;
    }
    Ok(seen)
}

#[test]
fn missing_parameter_stops_later_declarations_and_preserves_existing_entities() {
    for standalone in ["", "<?xml version='1.0' standalone='no'?>"] {
        let xml = format!(
            "{standalone}<!DOCTYPE r [<!ENTITY before 'B'>%missing;<!ENTITY after 'A'><!ATTLIST r a CDATA 'v'>]><r>&before;&after;</r>"
        );
        for mode in 0..=2 {
            for chunk in [1, 7, xml.len()] {
                let seen = parse(&xml, mode, chunk, Config::default()).unwrap();
                assert_eq!(seen.entities, ["before"]);
                assert_eq!(seen.text, "B");
                assert_eq!(seen.attributes, 0);
                assert_eq!(
                    seen.skipped
                        .iter()
                        .any(|(name, parameter)| name == "missing" && *parameter),
                    mode != 0
                );
                assert_eq!(seen.defaults.contains("%missing;"), mode == 0);
                assert_eq!(seen.not_standalone, usize::from(mode == 0));
            }
        }
    }
}

#[test]
fn standalone_requires_a_declaration_only_outside_internal_replacements() {
    let direct = "<?xml version='1.0' standalone='yes'?><!DOCTYPE r [%missing;<!ENTITY after 'A'>]><r>&after;</r>";
    let nested = "<?xml version='1.0' standalone='yes'?><!DOCTYPE r [<!ENTITY % p '&#37;missing;'>%p;<!ENTITY after 'A'>]><r>&after;</r>";
    for chunk in [1, 7, direct.len()] {
        assert_eq!(
            parse(direct, 2, chunk, Config::default()).unwrap_err(),
            ErrorKind::UndefinedEntity
        );
        for mode in [0, 1] {
            let seen = parse(direct, mode, chunk, Config::default()).unwrap();
            assert_eq!(seen.text, "A");
            assert!(seen.defaults.contains("%missing;"));
            assert!(seen.skipped.is_empty());
        }
        let seen = parse(nested, 2, chunk, Config::default()).unwrap();
        assert_eq!(seen.text, "A");
        assert_eq!(seen.not_standalone, 0);
        assert_eq!(seen.skipped, [("missing".into(), true)]);
    }
}

#[test]
fn child_and_later_sibling_keep_the_skipped_declaration_state() {
    let mut parent = Parser::new(Config::default());
    parent.set_param_entity_parsing(2);
    let mut child = parent.external_child(None, None).unwrap();
    child.feed(b"%missing;<!ENTITY after 'A'>", true).unwrap();
    let mut seen = Seen::default();
    drain(&mut child, &mut seen).unwrap();
    assert_eq!(seen.skipped, [("missing".into(), true)]);
    assert!(seen.entities.is_empty());
    parent.merge_external_subset(&child).unwrap();
    let mut sibling = parent.external_child(None, None).unwrap();
    drop(parent);
    drop(child);
    sibling.feed(b"<!ENTITY later 'L'>%again;", true).unwrap();
    let mut seen = Seen::default();
    drain(&mut sibling, &mut seen).unwrap();
    assert!(seen.entities.is_empty());
    assert_eq!(seen.skipped, [("again".into(), true)]);
}

#[test]
fn skipped_declarations_still_validate_names_and_enforce_limits() {
    for mode in [0, 2] {
        for xml in [
            "<!DOCTYPE r [%bad@name;]><r/>",
            "<!DOCTYPE r [%missing;<!ELEMENT r (bad@name)>]><r/>",
        ] {
            assert_eq!(
                parse(xml, mode, 1, Config::default()).unwrap_err(),
                ErrorKind::InvalidToken
            );
        }
        let config = Config {
            limits: Limits {
                max_token_bytes: 16,
                ..Limits::default()
            },
            ..Config::default()
        };
        assert_eq!(
            parse(
                "<!DOCTYPE r [%this_reference_is_too_long;]><r/>",
                mode,
                1,
                config
            )
            .unwrap_err(),
            ErrorKind::LimitExceeded
        );
    }
    let config = Config {
        limits: Limits {
            max_entities: 0,
            ..Limits::default()
        },
        ..Config::default()
    };
    // Skipping an absent declaration consumes no entity-table slot.
    assert!(
        parse(
            "<!DOCTYPE r [%missing;<!ENTITY ignored 'v'>]><r/>",
            2,
            1,
            config.clone()
        )
        .is_ok()
    );
    assert_eq!(
        parse("<!DOCTYPE r [<!ENTITY e 'v'>%missing;]><r/>", 2, 1, config).unwrap_err(),
        ErrorKind::LimitExceeded
    );
    let config = Config {
        limits: Limits {
            max_entity_expansion_bytes: 0,
            ..Limits::default()
        },
        ..Config::default()
    };
    assert_eq!(
        parse(
            "<!DOCTYPE r [<!ENTITY % p '&#37;missing;'>%p;]><r/>",
            2,
            1,
            config
        )
        .unwrap_err(),
        ErrorKind::LimitExceeded
    );
}

#[test]
fn disabled_parameter_defaults_restore_custom_name_aliases() {
    let xml = b"<!DOCTYPE r [%\x80\0;]><r/>";
    for width in [1, 7, xml.len()] {
        let mut parser = Parser::new(Config {
            encoding: Some("custom".into()),
            ..Config::default()
        });
        parser.set_default_events(true);
        parser.set_param_entity_parsing(0);
        let mut defaults = String::new();
        for (index, bytes) in xml.chunks(width).enumerate() {
            parser
                .feed(bytes, (index + 1) * width >= xml.len())
                .unwrap();
            loop {
                match parser.next_event() {
                    Ok(Some(event)) if matches!(event.kind, EventKind::Default) => {
                        defaults.push_str(parser.current_raw().unwrap());
                    }
                    Ok(Some(_)) => {}
                    Ok(None) if parser.encoding_conversion().is_some() => {
                        parser.resolve_encoding_conversion(i32::from(b'A')).unwrap();
                    }
                    Ok(None) => break,
                    Err(error) if error.kind == ErrorKind::UnknownEncoding => {
                        let mut map = std::array::from_fn(|index| index as i32);
                        map[128] = -2;
                        parser.set_multibyte_encoding_map("custom", map).unwrap();
                    }
                    Err(error) => panic!("unexpected error: {error}"),
                }
            }
        }
        assert!(parser.is_finished());
        assert!(defaults.contains("%A;"), "{defaults:?}");
        assert!(!defaults.contains('À'), "{defaults:?}");
    }
}
