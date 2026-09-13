use oriole::{Config, ErrorKind, EventKind, Parser};

#[test]
fn foreign_dtd_read_acknowledgement_controls_notifications_and_unknown_entities() {
    for doctype in ["", "<!DOCTYPE r>", "<!DOCTYPE r []>"] {
        for mode in 0..=2 {
            for standalone in [false, true] {
                // No handler, handler skip, created only, and initialized empty child.
                for action in 0..4 {
                    let xml = format!(
                        "<?xml version='1.0' standalone='{}'?>{doctype}<r>&missing;</r>",
                        if standalone { "yes" } else { "no" }
                    );
                    for width in [1, xml.len()] {
                        let mut parser = Parser::new(Config::default());
                        assert!(parser.set_use_foreign_dtd(true));
                        assert!(parser.set_param_entity_parsing(mode));
                        let mut notifications = 0;
                        let mut external = 0;
                        let mut error = None;
                        for (index, chunk) in xml.as_bytes().chunks(width).enumerate() {
                            parser
                                .feed(chunk, (index + 1) * width >= xml.len())
                                .unwrap();
                            loop {
                                let event = match parser.next_event() {
                                    Ok(Some(event)) => event,
                                    Ok(None) => break,
                                    Err(failure) => {
                                        error = Some(failure.kind);
                                        break;
                                    }
                                };
                                match event.kind {
                                    EventKind::ExternalEntityReference(_) => {
                                        external += 1;
                                        match action {
                                            0 => parser.external_entity_handler_absent(),
                                            1 => {}
                                            2 => {
                                                let _ = parser.external_child(None, None).unwrap();
                                            }
                                            3 => {
                                                let mut child =
                                                    parser.external_child(None, None).unwrap();
                                                child.feed(b"", false).unwrap();
                                                assert!(child.next_event().unwrap().is_none());
                                            }
                                            _ => unreachable!(),
                                        }
                                    }
                                    EventKind::NotStandalone => notifications += 1,
                                    _ => {}
                                }
                            }
                            if error.is_some() {
                                break;
                            }
                        }
                        let enabled = mode == 2 || (mode == 1 && !standalone);
                        assert_eq!(external, usize::from(enabled));
                        assert_eq!(
                            notifications,
                            usize::from(enabled && action == 3 && !standalone)
                        );
                        let undefined = standalone || (enabled && matches!(action, 1 | 2));
                        assert_eq!(
                            error,
                            undefined.then_some(ErrorKind::UndefinedEntity),
                            "{doctype} mode{mode} standalone{standalone} action{action}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn unread_foreign_dtd_preserves_prior_parameter_references() {
    let mut parser = Parser::new(Config::default());
    assert!(parser.set_use_foreign_dtd(true));
    assert!(parser.set_param_entity_parsing(2));
    parser
        .feed(b"<!DOCTYPE r [<!ENTITY % p ''>%p;]><r>&missing;</r>", true)
        .unwrap();
    let mut foreign = 0;
    let mut skipped = 0;
    while let Some(event) = parser.next_event().unwrap() {
        match event.kind {
            EventKind::ExternalEntityReference(_) => foreign += 1,
            EventKind::SkippedEntity { .. } => skipped += 1,
            _ => {}
        }
    }
    assert_eq!((foreign, skipped), (1, 1));
}
