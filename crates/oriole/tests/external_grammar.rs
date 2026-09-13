use oriole::{Config, ErrorKind, EventKind, Parser};

#[test]
fn declaration_errors_distinguish_documents_from_external_text() {
    let cases = [
        (
            "<?xml?>",
            Some(ErrorKind::XmlDeclaration),
            Some(ErrorKind::TextDeclaration),
        ),
        (
            "<?xml version='1.0'?>",
            None,
            Some(ErrorKind::TextDeclaration),
        ),
        (
            "<?xml version='1.0' encoding='UTF-8' standalone='yes'?>",
            None,
            Some(ErrorKind::TextDeclaration),
        ),
        (
            "<?xml version='1.0' encoding 'UTF-8'?>",
            Some(ErrorKind::XmlDeclaration),
            Some(ErrorKind::TextDeclaration),
        ),
        (
            "<?xml encoding='UTF-8'?>",
            Some(ErrorKind::XmlDeclaration),
            None,
        ),
        (
            "<?xml encoding='UTF8' version='1.0'?>",
            Some(ErrorKind::XmlDeclaration),
            Some(ErrorKind::TextDeclaration),
        ),
        (
            "<?xml version='1.0' encoding='UTF8' extra='x'?>",
            Some(ErrorKind::XmlDeclaration),
            Some(ErrorKind::TextDeclaration),
        ),
        (
            "<?xml version='1.0' encoding='UTF8'?>",
            Some(ErrorKind::UnknownEncoding),
            Some(ErrorKind::UnknownEncoding),
        ),
        (
            "<?XML encoding='UTF8'?>",
            Some(ErrorKind::InvalidToken),
            Some(ErrorKind::InvalidToken),
        ),
        (
            "<?XmL?>",
            Some(ErrorKind::InvalidToken),
            Some(ErrorKind::InvalidToken),
        ),
        ("<?xml-stylesheet?>", None, None),
    ];
    for (token, document_error, text_error) in cases {
        // None is a document; Some holds the general or parameter/subset context.
        for context in [None, Some(Some("")), Some(None)] {
            let expected = if context.is_none() {
                document_error
            } else {
                text_error
            };
            let input = if context.is_none() {
                format!("{token}<r/>")
            } else {
                token.to_owned()
            };
            let mut first_error = None;
            for width in 1..=input.len() {
                let parent = Parser::new(Config::default());
                let mut parser = match context {
                    None => Parser::new(Config::default()),
                    Some(context) => parent.external_child(context, None).unwrap(),
                };
                let mut actual = None;
                'feed: for (index, bytes) in input.as_bytes().chunks(width).enumerate() {
                    parser
                        .feed(bytes, (index + 1) * width >= input.len())
                        .unwrap();
                    loop {
                        match parser.next_event() {
                            Ok(Some(_)) => {}
                            Ok(None) => break,
                            Err(error) => {
                                assert_eq!(parser.next_event().unwrap_err(), error);
                                assert_eq!(parser.feed(&[], true).unwrap_err(), error);
                                actual = Some(error);
                                break 'feed;
                            }
                        }
                    }
                }
                assert_eq!(
                    actual.map(|error| error.kind),
                    expected,
                    "{token}, {context:?}, {width}"
                );
                if let Some(error) = actual {
                    // Classification does not make coordinates depend on feed boundaries.
                    assert_eq!(*first_error.get_or_insert(error), error);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Read {
    Skip,
    Create,
    Empty,
    Nonfinal,
}

fn parse(dtd: &[u8], width: usize, read: Read, standalone: bool) -> Result<Vec<String>, ErrorKind> {
    let mut root = Parser::new(Config::default());
    if standalone {
        root.feed(b"<?xml version='1.0' standalone='yes'?>", false)
            .unwrap();
        while root.next_event().unwrap().is_some() {}
    }
    let mut parser = root.external_child(None, None).unwrap();
    parser.set_param_entity_parsing(2);
    let mut events = Vec::new();
    for (index, bytes) in dtd.chunks(width).enumerate() {
        parser
            .feed(bytes, (index + 1) * width >= dtd.len())
            .map_err(|e| e.kind)?;
        while let Some(event) = parser.next_event().map_err(|e| e.kind)? {
            match event.kind {
                EventKind::AttlistDeclaration(declaration) => {
                    let oriole::AttributeDeclaration { name, default, .. } =
                        oriole_storage::Box::into_inner(declaration);

                    events.push(format!("attr:{name}={}", default.as_deref().unwrap_or("")))
                }
                EventKind::EntityDeclaration(declaration) if declaration.value.is_some() => {
                    let oriole::EntityDeclaration {
                        name,
                        value: stored_value,
                        ..
                    } = oriole_storage::Box::into_inner(declaration);
                    let value = stored_value.expect("matched optional field");
                    events.push(format!("entity:{name}={value}"))
                }
                EventKind::ElementDeclaration { name, .. } => {
                    events.push(format!("element:{name}"))
                }
                EventKind::NotationDeclaration(declaration) => {
                    let oriole::NotationDeclaration { name, .. } =
                        oriole_storage::Box::into_inner(declaration);

                    events.push(format!("notation:{name}"))
                }
                EventKind::ExternalEntityReference(declaration) => {
                    let oriole::ExternalEntityReference { system_id, .. } =
                        oriole_storage::Box::into_inner(declaration);

                    events.push(format!(
                        "external:{}",
                        system_id.as_deref().unwrap_or("NULL")
                    ));
                    if !matches!(read, Read::Skip) {
                        let mut child = parser.external_child(None, None).map_err(|e| e.kind)?;
                        if !matches!(read, Read::Create) {
                            let data = if system_id.as_deref() == Some("value") {
                                b"X".as_slice()
                            } else {
                                b""
                            };
                            child
                                .feed(data, !matches!(read, Read::Nonfinal))
                                .map_err(|e| e.kind)?;
                            while child.next_event().map_err(|e| e.kind)?.is_some() {}
                            if matches!(read, Read::Empty) {
                                parser.merge_external_subset(&child).map_err(|e| e.kind)?;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(events)
}

#[test]
fn external_grammar_commits_attributes_before_read_or_skip() {
    let dtd =
        b"<!ENTITY % p SYSTEM 'p'><!ATTLIST r a CDATA 'A' %p; b CDATA 'B'><!ENTITY after 'Z'>";
    for width in 1..=dtd.len() {
        for standalone in [false, true] {
            for read in [Read::Skip, Read::Create, Read::Empty, Read::Nonfinal] {
                let events = parse(dtd, width, read, standalone).unwrap();
                let mut expected = vec!["attr:a=A", "external:p"];
                if standalone || matches!(read, Read::Empty | Read::Nonfinal) {
                    expected.extend(["attr:b=B", "entity:after=Z"]);
                }
                assert_eq!(events, expected, "width={width}, standalone={standalone}");
            }
        }
    }
}

#[test]
fn declaration_roles_commit_at_their_last_required_token() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ELEMENT r (a) %p;><!NOTATION n SYSTEM 'n' %p;><!ENTITY e 'E' %p;><!ENTITY s SYSTEM 's' %p;><!NOTATION pub PUBLIC 'public' %p;>";
    for width in 1..=dtd.len() {
        assert_eq!(
            parse(dtd, width, Read::Empty, false).unwrap(),
            [
                "element:r",
                "external:p",
                "notation:n",
                "external:p",
                "entity:e=E",
                "external:p",
                "external:p",
                "external:p",
                "notation:pub"
            ]
        );
    }
}

#[test]
fn value_child_completes_before_a_later_grammar_child() {
    let dtd = b"<!ENTITY % p SYSTEM 'p'><!ENTITY % v SYSTEM 'value'><!ENTITY e %p; 'L%v;R' %p;><!ENTITY after 'A'>";
    for width in 1..=dtd.len() {
        assert_eq!(
            parse(dtd, width, Read::Empty, false).unwrap(),
            [
                "external:p",
                "external:value",
                "entity:e=LXR",
                "external:p",
                "entity:after=A"
            ]
        );
    }
}

#[test]
fn grammar_tokens_do_not_splice_across_external_entities() {
    for dtd in [
        b"<!ENTITY % p SYSTEM 'p'><!ELEMENT r EMP%p;TY>".as_slice(),
        b"<!ENTITY % p SYSTEM 'p'><!ATTLIST r a CDA%p;TA 'A'>",
    ] {
        for width in 1..=dtd.len() {
            assert_eq!(
                parse(dtd, width, Read::Empty, false).unwrap_err(),
                ErrorKind::Syntax
            );
        }
    }
}

#[test]
fn completed_attributes_are_visible_before_the_final_declaration_delimiter() {
    let parent = Parser::new(Config::default());
    let mut parser = parent.external_child(None, None).unwrap();
    parser.set_param_entity_parsing(2);
    parser
        .feed(b"<!ENTITY % p SYSTEM 'p'><!ATTLIST r a CDATA %p;", false)
        .unwrap();
    while let Some(event) = parser.next_event().unwrap() {
        if matches!(event.kind, EventKind::ExternalEntityReference(_)) {
            let mut child = parser.external_child(None, None).unwrap();
            child.feed(b"", true).unwrap();
            while child.next_event().unwrap().is_some() {}
            parser.merge_external_subset(&child).unwrap();
        }
    }
    for (bytes, expected) in [(b"'A' ".as_slice(), "a"), (b"b CDATA 'B' ".as_slice(), "b")] {
        parser.feed(bytes, false).unwrap();
        let mut names = Vec::new();
        while let Some(event) = parser.next_event().unwrap() {
            if let EventKind::AttlistDeclaration(declaration) = event.kind {
                let oriole::AttributeDeclaration { name, .. } =
                    oriole_storage::Box::into_inner(declaration);

                names.push(name.to_string());
            }
        }
        assert_eq!(names, [expected]);
    }
    parser.feed(b">", true).unwrap();
    while parser.next_event().unwrap().is_some() {}
}

#[test]
fn empty_grammar_children_share_a_finite_work_budget() {
    let mut config = Config::default();
    config.limits.max_entity_expansion_bytes = 4096;
    config.limits.max_depth = 8;
    let parent = Parser::new(config);
    let mut parser = parent.external_child(None, None).unwrap();
    parser.set_param_entity_parsing(2);
    let text = format!(
        "<!ENTITY % p SYSTEM 'p'><!ATTLIST r {}a CDATA 'A'>",
        "%p; ".repeat(100)
    );
    parser.feed(text.as_bytes(), true).unwrap();
    let mut requests = 0;
    let error = loop {
        match parser.next_event() {
            Err(error) => break error,
            Ok(Some(event)) if matches!(event.kind, EventKind::ExternalEntityReference(_)) => {
                requests += 1;
                let mut child = match parser.external_child(None, None) {
                    Ok(child) => child,
                    Err(error) => break error,
                };
                child.feed(b"", true).unwrap();
                while child.next_event().unwrap().is_some() {}
                if let Err(error) = parser.merge_external_subset(&child) {
                    break error;
                }
            }
            Ok(Some(_)) => {}
            Ok(None) => panic!("one hundred child snapshots must exhaust the shared work budget"),
        }
    };
    assert!(requests > 0 && requests < 100);
    assert_eq!(error.kind, ErrorKind::LimitExceeded);
}

#[test]
fn undefined_default_entities_follow_document_or_external_context() {
    for standalone in [false, true] {
        let mut parent = Parser::new(Config::default());
        if standalone {
            parent
                .feed(b"<?xml version='1.0' standalone='yes'?>", false)
                .unwrap();
            while parent.next_event().unwrap().is_some() {}
        }
        let mut external = parent.external_child(None, None).unwrap();
        external
            .feed(b"<!ATTLIST r a CDATA 'L&missing;R'>", true)
            .unwrap();
        let mut found = false;
        while let Some(event) = external.next_event().unwrap() {
            if let EventKind::AttlistDeclaration(declaration) = event.kind {
                let oriole::AttributeDeclaration { default, .. } =
                    oriole_storage::Box::into_inner(declaration);

                found = true;
                assert_eq!(default.as_deref(), Some("LR"));
            }
        }
        assert!(found);
        parent
            .feed(
                b"<!DOCTYPE r [<!ATTLIST r a CDATA 'L&missing;R'>]><r/>",
                true,
            )
            .unwrap();
        loop {
            match parent.next_event() {
                Err(error) => {
                    assert_eq!(error.kind, ErrorKind::UndefinedEntity);
                    break;
                }
                Ok(Some(_)) => {}
                Ok(None) => panic!("document subset must reject an undefined default entity"),
            }
        }
    }
}

#[test]
fn partially_declared_ids_are_inherited_with_context_specific_slot_semantics() {
    for (declaration, parameter, system, public) in [
        ("<!ENTITY % e SYSTEM 's' %p;>", true, Some("s"), None),
        (
            "<!ENTITY % e PUBLIC '  public   id  ' %p; 's'>",
            true,
            None,
            Some("public id"),
        ),
        (
            "<!ENTITY % e PUBLIC ' public  id ' 's' %p;>",
            true,
            Some("s"),
            Some("public id"),
        ),
        ("<!ENTITY e SYSTEM 's' %p;>", false, Some("s"), None),
        ("<!ENTITY e PUBLIC 'public' %p; 's'>", false, None, None),
    ] {
        let parent = Parser::new(Config::default());
        let mut parser = parent.external_child(None, None).unwrap();
        parser.set_param_entity_parsing(2);
        let dtd = format!("<!ENTITY % p SYSTEM 'p'>{declaration}");
        parser.feed(dtd.as_bytes(), true).unwrap();
        while let Some(event) = parser.next_event().unwrap() {
            if matches!(event.kind, EventKind::ExternalEntityReference(_)) {
                let mut child = parser
                    .external_child(if parameter { None } else { Some("") }, None)
                    .unwrap();
                child
                    .feed(if parameter { b"%e;" } else { b"&e;" }, true)
                    .unwrap();
                let mut requests = Vec::new();
                while let Some(event) = child.next_event().unwrap() {
                    if let EventKind::ExternalEntityReference(declaration) = event.kind {
                        let oriole::ExternalEntityReference {
                            system_id,
                            public_id,
                            ..
                        } = oriole_storage::Box::into_inner(declaration);

                        requests.push((
                            system_id.map(|value| value.to_string()),
                            public_id.map(|value| value.to_string()),
                        ));
                    }
                }
                if parameter || system.is_some() {
                    assert_eq!(
                        requests,
                        [(system.map(str::to_string), public.map(str::to_string))]
                    );
                } else {
                    assert!(requests.is_empty());
                }
            }
        }
    }
}

#[test]
fn enumeration_callback_capture_does_not_change_semantic_defaults() {
    for (kind, expected) in [
        ("(x|%p;y)", Some("(y)")),
        ("(%p;x|y)", Some("(x|y)")),
        ("(x|y) %p;", None),
        ("NOTATION (x|%p;y)", Some("NOTATION(y)")),
    ] {
        let mut parent = Parser::new(Config::default());
        let mut parser = parent.external_child(None, None).unwrap();
        parser.set_param_entity_parsing(2);
        parser.set_attlist_handler_enabled(false);
        let dtd = format!("<!ENTITY % p SYSTEM 'p'><!ATTLIST r a {kind} '  x  '>");
        parser.feed(dtd.as_bytes(), true).unwrap();
        let mut types = Vec::new();
        while let Some(event) = parser.next_event().unwrap() {
            match event.kind {
                EventKind::ExternalEntityReference(_) => {
                    parser.set_attlist_handler_enabled(true);
                    let mut child = parser.external_child(None, None).unwrap();
                    child.feed(b"", true).unwrap();
                    while child.next_event().unwrap().is_some() {}
                    parser.merge_external_subset(&child).unwrap();
                }
                EventKind::AttlistDeclaration(declaration) => {
                    let oriole::AttributeDeclaration { attribute_type, .. } =
                        oriole_storage::Box::into_inner(declaration);

                    types.push(attribute_type.to_string())
                }
                _ => {}
            }
        }
        assert_eq!(types, expected.into_iter().collect::<Vec<_>>());
        parent.merge_external_subset(&parser).unwrap();
        parent.feed(b"<r/>", true).unwrap();
        let mut found = false;
        while let Some(event) = parent.next_event().unwrap() {
            if let EventKind::StartElement { attributes, .. } = event.kind {
                found = true;
                assert_eq!(attributes.len(), 1);
                assert_eq!(attributes[0].value.as_str(), "x");
            }
        }
        assert!(found);
    }
}
