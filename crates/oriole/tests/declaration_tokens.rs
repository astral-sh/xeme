use oriole::{Config, ErrorKind, EventKind, Limits, Parser};

#[derive(Debug, Default)]
struct Declarations {
    values: Vec<(String, bool, String)>,
    defaults: String,
    attributes: usize,
    models: Vec<String>,
    attribute_values: Vec<String>,
}

fn drain(parser: &mut Parser, declarations: &mut Declarations) -> Result<(), ErrorKind> {
    while let Some(event) = parser.next_event().map_err(|error| error.kind)? {
        match event.kind {
            EventKind::EntityDeclaration {
                name,
                parameter,
                value: Some(value),
                ..
            } => {
                declarations
                    .values
                    .push((name.to_string(), parameter, value.to_string()));
            }
            EventKind::Default => declarations
                .defaults
                .push_str(parser.current_raw().unwrap()),
            EventKind::AttlistDeclaration { default, .. } => {
                declarations.attributes += 1;
                if let Some(value) = default {
                    declarations.attribute_values.push(value.to_string());
                }
            }
            EventKind::ElementDeclaration { model, .. } => {
                declarations.models.push(model.to_string())
            }
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
fn declaration_grammar_continues_across_complete_lexical_tokens() {
    let dtd = br#"<!ENTITY % name 'r'><!ENTITY % model '(a|'><!ENTITY % attrs 'a CDATA "x" b (x|y) "x"'><!ENTITY % literal '"X"'><!ELEMENT %name; %model;b)><!ATTLIST %name; %attrs;><!ENTITY e %literal;>"#;
    for chunk in 1..=dtd.len() {
        for mode in [1, 2] {
            let (_, declarations) = external(dtd, chunk, mode, false, Config::default()).unwrap();
            assert_eq!(declarations.values.last().unwrap().2, "X");
            assert_eq!(declarations.attributes, 2);
            assert_eq!(declarations.models.len(), 1);
            assert_eq!(
                declarations.models[0]
                    .split_whitespace()
                    .collect::<String>(),
                "(a|b)"
            );
        }
    }
}

#[test]
fn replacement_tokens_cannot_join_names_or_quoted_literals() {
    for (replacement, body, error) in [
        ("ab", "<!ELEMENT r (%p;cd)>", ErrorKind::Syntax),
        (
            "\"x",
            "<!ATTLIST r a CDATA %p;y\">",
            ErrorKind::UnclosedToken,
        ),
        ("*", "<!ELEMENT r (a)%p;>", ErrorKind::InvalidToken),
        ("EMPTY>", "<!ELEMENT r %p;>", ErrorKind::Syntax),
    ] {
        let dtd = format!("<!ENTITY % p '{replacement}'>{body}");
        for chunk in 1..=dtd.len() {
            assert_eq!(
                external(dtd.as_bytes(), chunk, 2, false, Config::default()).err(),
                Some(error),
                "{dtd}"
            );
        }
    }
}

#[test]
fn quoted_replacement_literals_preserve_numeric_carriage_returns() {
    let dtd = br#"<!ENTITY % literal '"A&#13;B"'><!ENTITY e %literal;>"#;
    let (_, declarations) = external(dtd, 1, 2, false, Config::default()).unwrap();
    assert_eq!(declarations.values.last().unwrap().2, "A\rB");
}

#[test]
fn missing_parameters_stop_later_declarations_at_their_grammar_position() {
    let dtd = br#"<!ATTLIST r a CDATA "A" %missing; b CDATA "B"><!ENTITY e "E">"#;
    for standalone in [false, true] {
        let (_, declarations) = external(dtd, 1, 2, standalone, Config::default()).unwrap();
        assert_eq!(declarations.attributes, if standalone { 2 } else { 1 });
        assert_eq!(declarations.values.len(), usize::from(standalone));
    }
}

#[test]
fn direct_internal_subset_declaration_references_are_rejected() {
    let xml = br#"<!DOCTYPE r [<!ENTITY % p 'EMPTY'><!ELEMENT r %p;>]><r/>"#;
    let mut parser = Parser::new(Config::default());
    parser.set_param_entity_parsing(2);
    parser.feed(xml, true).unwrap();
    assert_eq!(
        drain(&mut parser, &mut Declarations::default()),
        Err(ErrorKind::ParameterEntityReference)
    );
}

#[test]
fn grammar_and_value_expansion_share_the_depth_limit() {
    let dtd = br#"<!ENTITY % value 'X'><!ENTITY % literal '"&#37;value;"'><!ENTITY e %literal;>"#;
    let limits = Limits {
        max_entity_depth: 2,
        ..Limits::default()
    };
    assert_eq!(
        external(
            dtd,
            1,
            2,
            false,
            Config {
                limits,
                ..Config::default()
            }
        )
        .err(),
        Some(ErrorKind::LimitExceeded)
    );
}

#[test]
fn empty_replacement_work_output_and_callback_storage_are_bounded() {
    let cases = [
        (
            Limits {
                max_token_bytes: 64,
                ..Limits::default()
            },
            format!("<!ENTITY % p '{}'><!ELEMENT r (%p;|%p;)>", "a".repeat(40)),
            2,
        ),
        (
            Limits {
                max_entity_expansion_bytes: 4096,
                ..Limits::default()
            },
            format!("<!ENTITY % p ''><!ATTLIST r {}>", "%p;".repeat(200)),
            2,
        ),
        (
            Limits {
                max_entity_expansion_bytes: 4096,
                ..Limits::default()
            },
            format!("<!ATTLIST r {}>", "%missing;".repeat(200)),
            0,
        ),
    ];
    for (limits, dtd, mode) in cases {
        assert_eq!(
            external(
                dtd.as_bytes(),
                1,
                mode,
                false,
                Config {
                    limits,
                    ..Config::default()
                }
            )
            .err(),
            Some(ErrorKind::LimitExceeded)
        );
    }
}

#[test]
fn skipped_parameter_defaults_preserve_the_unhandled_declaration_suffix() {
    let dtd = br#"<!ENTITY % p ' '><!ATTLIST r a CDATA "A" %p; b CDATA "B"><!ENTITY e "E">"#;
    for standalone in [false, true] {
        let (_, declarations) = external(dtd, 1, 0, standalone, Config::default()).unwrap();
        assert_eq!(
            declarations.defaults,
            if standalone {
                "%p;"
            } else {
                "%p; b CDATA \"B\"><!ENTITY e \"E\">"
            }
        );
        assert_eq!(declarations.attributes, if standalone { 2 } else { 1 });
    }
}

#[test]
fn replacement_attribute_line_endings_are_not_normalized_twice() {
    let dtd = br#"<!ENTITY % literal '"A&#13;&#10;B"'><!ATTLIST r a CDATA %literal;>"#;
    for chunk in 1..=dtd.len() {
        let (_, declarations) = external(dtd, chunk, 2, false, Config::default()).unwrap();
        assert_eq!(declarations.attribute_values, ["A  B"]);
    }
}

#[test]
fn parameter_delimiters_close_declarations_and_preserve_following_source_text() {
    for (replacement, declaration, expected) in [
        (">", "<!ELEMENT r (#PCDATA) %p;", None),
        ("(#PCDATA)>", "<!ELEMENT r %p;", None),
        ("a CDATA #IMPLIED>", "<!ATTLIST r %p;", None),
        ("><!ENTITY z ", "<!ELEMENT r EMPTY %p; 'Z'>", Some("Z")),
        ("><!ENTITY z 'Z'>", "<!ELEMENT r EMPTY %p;", Some("Z")),
    ] {
        let dtd = format!("<!ENTITY % p \"{replacement}\">{declaration}");
        for chunk in 1..=dtd.len() {
            let (_, declarations) =
                external(dtd.as_bytes(), chunk, 2, false, Config::default()).unwrap();
            if let Some(value) = expected {
                assert!(
                    declarations
                        .values
                        .iter()
                        .any(|(name, parameter, actual)| name == "z"
                            && !parameter
                            && actual == value)
                );
            }
        }
    }
}

#[test]
fn nested_delimiter_sources_keep_quotes_and_reference_recursion_separate() {
    let dtd = br#"<!ENTITY % q "><!ENTITY z "><!ENTITY % p "&#37;q;"><!ELEMENT r EMPTY %p; "Z">"#;
    for chunk in 1..=dtd.len() {
        let (_, declarations) = external(dtd, chunk, 2, false, Config::default()).unwrap();
        assert_eq!(declarations.values.last().unwrap().2, "Z");
    }
    for dtd in [
        br#"<!ENTITY % p "><!ENTITY z '"><!ELEMENT r EMPTY %p;Z'>"#.as_slice(),
        br#"<!ENTITY % p "&#37;p;"><!ELEMENT r EMPTY %p;"#,
    ] {
        for chunk in 1..=dtd.len() {
            assert!(external(dtd, chunk, 2, false, Config::default()).is_err());
        }
    }
}
