use xeme::{Config, ErrorKind, EventKind, Parser};

#[test]
fn parameter_references_supply_space_after_the_declaration_marker() {
    for prefix in [
        "<!ENTITY % sp ''>",
        "<!ENTITY % sp ' \n\t'>",
        "<!ENTITY % empty ''><!ENTITY % sp '&#37;empty;'>",
    ] {
        for (definition, value, system, public) in [
            ("'value'", Some("value"), None, None),
            ("SYSTEM 'p'", None, Some("p"), None),
            ("PUBLIC 'public' 'p'", None, Some("p"), Some("public")),
        ] {
            let dtd = format!("{prefix}<!ENTITY %%sp;p {definition}>");
            let encodings = [
                dtd.as_bytes().to_vec(),
                [
                    vec![0xff, 0xfe],
                    dtd.encode_utf16().flat_map(u16::to_le_bytes).collect(),
                ]
                .concat(),
            ];
            for input in encodings {
                for width in 1..=input.len() {
                    for mode in 0..=2 {
                        let parent = Parser::new(Config::default());
                        let mut parser = parent.external_child(None, None).unwrap();
                        parser.set_param_entity_parsing(mode);
                        let mut found = 0;
                        for (index, chunk) in input.chunks(width).enumerate() {
                            parser
                                .feed(chunk, (index + 1) * width >= input.len())
                                .unwrap();
                            while let Some(event) = parser.next_event().unwrap() {
                                if let EventKind::EntityDeclaration(declaration) = event.kind
                                    && declaration.name == "p"
                                {
                                    found += 1;
                                    assert!(declaration.parameter);
                                    assert_eq!(declaration.value.as_deref(), value);
                                    assert_eq!(declaration.system_id.as_deref(), system);
                                    assert_eq!(declaration.public_id.as_deref(), public);
                                }
                            }
                        }
                        assert!(parser.is_finished());
                        assert_eq!(found, usize::from(mode != 0), "{dtd}, {width}, {mode}");
                    }
                }
            }
        }
    }
}

#[test]
fn marker_lookahead_preserves_grammar_and_internal_subset_restrictions() {
    for (dtd, error) in [
        ("<!ENTITY %%p 'value'>", ErrorKind::InvalidToken),
        ("<!ENTITY %% 'value'>", ErrorKind::Syntax),
        ("<!ENTITY %%%p; p 'value'>", ErrorKind::Syntax),
        ("<!ELEMENT r (%%p;p;)*>", ErrorKind::InvalidToken),
        ("<!NOTATION %%p; SYSTEM 'n'>", ErrorKind::InvalidToken),
        ("<!ENTI%% p 'value'>", ErrorKind::InvalidToken),
        (
            "<!ENTITY % sp ''><!ENTITY e %%sp; 'value'>",
            ErrorKind::Syntax,
        ),
    ] {
        for internal in [false, true] {
            let input = if internal {
                format!("<!DOCTYPE r [{dtd}]><r/>")
            } else {
                dtd.into()
            };
            for width in 1..=input.len() {
                let parent = Parser::new(Config::default());
                let mut parser = if internal {
                    Parser::new(Config::default())
                } else {
                    parent.external_child(None, None).unwrap()
                };
                parser.set_param_entity_parsing(2);
                let result = (|| {
                    for (index, chunk) in input.as_bytes().chunks(width).enumerate() {
                        parser.feed(chunk, (index + 1) * width >= input.len())?;
                        while parser.next_event()?.is_some() {}
                    }
                    Ok::<_, xeme::Error>(())
                })();
                assert_eq!(result.unwrap_err().kind, error, "{input}, {width}");
            }
        }
    }
    let input = b"<!DOCTYPE r [<!ENTITY % sp ''><!ENTITY %%sp;p 'value'>]><r/>";
    for width in 1..=input.len() {
        let mut parser = Parser::new(Config::default());
        let result = (|| {
            for (index, chunk) in input.chunks(width).enumerate() {
                parser.feed(chunk, (index + 1) * width >= input.len())?;
                while parser.next_event()?.is_some() {}
            }
            Ok::<_, xeme::Error>(())
        })();
        assert_eq!(
            result.unwrap_err().kind,
            ErrorKind::ParameterEntityReference
        );
    }
}
