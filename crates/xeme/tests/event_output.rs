use xeme::{Config, Error, ErrorKind, Event, EventKind, Parser};

fn retained_event() -> Event {
    let mut parser = Parser::new(Config::default());
    parser
        .feed(b"<retained attribute='original'/>", true)
        .unwrap();
    parser.next_event().unwrap().unwrap()
}

fn trace(xml: &[u8], width: usize, slot_api: bool) -> (Vec<String>, Option<Error>) {
    let mut parser = Parser::new(Config {
        namespace_separator: Some('|'),
        ..Config::default()
    });
    let mut slot = None;
    let mut events = Vec::new();
    for (index, part) in xml.chunks(width).enumerate() {
        parser.feed(part, (index + 1) * width >= xml.len()).unwrap();
        loop {
            let result = if slot_api {
                parser
                    .next_event_for_recycling_into(&mut slot)
                    .map(|token| {
                        assert_eq!(token.is_some(), slot.is_some());
                        slot.take()
                    })
            } else {
                parser.next_event()
            };
            match result {
                Ok(Some(event)) => events.push(format!("{event:?} {:?}", parser.current_raw())),
                Ok(None) => break,
                Err(error) => return (events, Some(error)),
            }
        }
    }
    (events, None)
}

#[test]
fn output_slots_preserve_owned_event_traces_and_error_prefixes() {
    for xml in [
        "<!DOCTYPE r [<!ENTITY e 'é'><!ATTLIST r a CDATA 'default'>]><r xmlns:p='u'><p:n/>&e;</r>",
        "<r xmlns:p='u' q:a='unbound'/>",
        "<r><n/></wrong>",
    ] {
        for utf16 in [false, true] {
            let bytes = if utf16 {
                [0xff, 0xfe]
                    .into_iter()
                    .chain(xml.encode_utf16().flat_map(u16::to_le_bytes))
                    .collect::<Vec<_>>()
            } else {
                xml.as_bytes().to_vec()
            };
            for width in [1, 3, 7, bytes.len()] {
                assert_eq!(trace(&bytes, width, false), trace(&bytes, width, true));
            }
        }
    }
    let mut parser = Parser::new(Config {
        namespace_separator: Some('|'),
        ..Config::default()
    });
    parser
        .feed(b"<r xmlns:p='u' q:a='unbound'/>", true)
        .unwrap();
    let mut slot = None;
    assert!(
        parser
            .next_event_for_recycling_into(&mut slot)
            .unwrap()
            .is_some()
    );
    assert!(matches!(
        slot.as_ref().unwrap().kind,
        EventKind::StartNamespace { .. }
    ));
    assert!(
        parser
            .next_event_for_recycling_into(&mut slot)
            .unwrap()
            .is_some()
    );
    assert!(matches!(
        slot.as_ref().unwrap().kind,
        EventKind::EndNamespace { .. }
    ));
    assert_eq!(
        parser
            .next_event_for_recycling_into(&mut slot)
            .unwrap_err()
            .kind,
        ErrorKind::UndefinedPrefix
    );
    assert!(slot.is_none());
}

#[test]
fn caller_slots_clear_on_wait_error_and_unknown_encoding_retry() {
    let retained = retained_event();
    let mut parser = Parser::new(Config::default());
    let mut slot = Some(retained_event());
    assert!(
        parser
            .next_event_for_recycling_into(&mut slot)
            .unwrap()
            .is_none()
    );
    assert!(slot.is_none());
    parser.feed(b"<r></wrong>", true).unwrap();
    assert!(
        parser
            .next_event_for_recycling_into(&mut slot)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        parser
            .next_event_for_recycling_into(&mut slot)
            .unwrap_err()
            .kind,
        ErrorKind::TagMismatch
    );
    assert!(slot.is_none());
    slot = Some(retained_event());
    assert_eq!(
        parser
            .next_event_for_recycling_into(&mut slot)
            .unwrap_err()
            .kind,
        ErrorKind::TagMismatch
    );
    assert!(slot.is_none());
    drop(parser);

    let mut parser = Parser::new(Config::default());
    parser
        .feed(b"<?xml version='1.0' encoding='custom'?><r>\x80</r>", true)
        .unwrap();
    slot = Some(retained_event());
    assert_eq!(
        parser
            .next_event_for_recycling_into(&mut slot)
            .unwrap_err()
            .kind,
        ErrorKind::UnknownEncoding
    );
    assert!(slot.is_none());
    let mut map = std::array::from_fn(|byte| byte as i32);
    map[128] = 0x20ac;
    parser.set_encoding_map("custom", map).unwrap();
    let mut text = String::new();
    while parser
        .next_event_for_recycling_into(&mut slot)
        .unwrap()
        .is_some()
    {
        if let EventKind::Text(value) = &slot.as_ref().unwrap().kind {
            text.push_str(value);
        }
    }
    assert!(slot.is_none() && parser.is_finished());
    assert_eq!(text, "€");
    drop(parser);
    let EventKind::StartElement { name, attributes } = retained.kind else {
        panic!("retained start")
    };
    assert_eq!(name, "retained");
    assert_eq!(attributes[0].value, "original");
}
