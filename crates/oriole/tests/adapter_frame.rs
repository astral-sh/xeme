use oriole::{Attribute, Config, Event, EventKind, Parser};
use oriole_storage::{Allocator, String, Vec, try_push};

fn text(bytes: &[u8]) -> String {
    String::try_from_str_in(
        std::str::from_utf8(bytes.strip_suffix(&[0]).unwrap()).unwrap(),
        Allocator::System,
    )
    .unwrap()
}

fn collect(
    input: &[u8],
    chunk: usize,
    config: Config,
    adapter: bool,
) -> (std::vec::Vec<std::string::String>, usize) {
    let mut parser = Parser::new(config);
    let mut frame = parser.adapter_frame();
    let mut records = std::vec::Vec::new();
    let mut frames = 0;
    'input: for (index, data) in input.chunks(chunk).enumerate() {
        if let Err(error) = parser.feed(data, (index + 1) * chunk >= input.len()) {
            records.push(format!("error {error:?} raw {:?}", parser.current_raw()));
            break;
        }
        loop {
            let mut event = None;
            let result = if adapter {
                parser.next_event_for_adapter_into(&mut event, &mut frame)
            } else {
                parser.next_event_for_recycling_into(&mut event)
            };
            let token = match result {
                Ok(Some(token)) => token,
                Ok(None) => break,
                Err(error) => {
                    assert!(!frame.is_active());
                    assert!(event.is_none());
                    records.push(format!("error {error:?} raw {:?}", parser.current_raw()));
                    break 'input;
                }
            };
            if frame.is_active() {
                assert!(event.is_none());
                frames += 1;
                let name = text(frame.name_bytes());
                let mut attributes = Vec::new_in(Allocator::System);
                for (name, value) in frame.attributes() {
                    try_push(
                        &mut attributes,
                        Attribute {
                            name: text(name),
                            value: text(value),
                            specified: true,
                        },
                    )
                    .unwrap();
                }
                assert_eq!(
                    frame.callback_bytes(),
                    name.len()
                        + attributes
                            .iter()
                            .map(|a| a.name.len() + a.value.len())
                            .sum::<usize>()
                );
                event = Some(Event {
                    kind: EventKind::StartElement { name, attributes },
                    position: frame.position(),
                });
            }
            let event = event.unwrap();
            assert_eq!(parser.position(), event.position);
            records.push(format!("{event:?} raw {:?}", parser.current_raw()));
            // Own all callback data before returning storage, as the adapter does.
            if !frame.is_active() {
                match event.kind {
                    EventKind::StartElement { name, attributes } => {
                        parser.recycle_start_element(token, name, attributes)
                    }
                    EventKind::EndElement { name } => parser.recycle_end_element(token, name),
                    _ => {}
                }
            }
        }
    }
    parser.finish_adapter_frame(frame);
    (records, frames)
}

#[test]
fn owned_frames_preserve_events_positions_raw_and_error_order_at_every_chunk_size() {
    let inputs = [
        "<root><n a='first' b='value'/><n a='α😀' b='next'/>text<n>body</n></root>",
        "<r><n a='first'/><n a='v' a='duplicate'/></r>",
        "<r><n a='first'/><n a='v' b='&missing;'/></r>",
        "<r><n a='first'/><n a='v' b='later'\0/></r>",
        "<r><n a='first'/><n a='incomplete",
        "<r><n a='first'/><n a='v'bad='x'/></r>",
        "<!DOCTYPE r [<!ATTLIST n a CDATA 'default'>]><r><n/><n b='v'/></r>",
    ];
    for input in inputs {
        for namespace_separator in [None, Some('|')] {
            for chunk in 1..=input.len() {
                let config = Config {
                    namespace_separator,
                    ..Config::default()
                };
                let owned = collect(input.as_bytes(), chunk, config.clone(), false);
                let adapter = collect(input.as_bytes(), chunk, config, true);
                assert_eq!(
                    adapter.0, owned.0,
                    "{input:?} chunk={chunk} namespaces={namespace_separator:?}"
                );
                if input.starts_with("<!DOCTYPE") {
                    assert_eq!(adapter.1, 0);
                } else {
                    assert!(adapter.1 > 0);
                }
            }
        }
    }
}

#[test]
fn utf16_and_low_limits_keep_owned_fallbacks() {
    let xml = "<r><n a='first'/><n a='α'/></r>";
    let mut utf16 = vec![0xff, 0xfe];
    utf16.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    for chunk in [1, 2, 7, utf16.len()] {
        let owned = collect(&utf16, chunk, Config::default(), false);
        let adapter = collect(&utf16, chunk, Config::default(), true);
        assert_eq!(adapter.0, owned.0);
        assert_eq!(adapter.1, 0);
    }
    for limit in 0..=xml.len() {
        let mut config = Config::default();
        config.limits.max_token_bytes = limit;
        for chunk in [1, 7, xml.len()] {
            assert_eq!(
                collect(xml.as_bytes(), chunk, config.clone(), false).0,
                collect(xml.as_bytes(), chunk, config.clone(), true).0
            );
        }
    }
}

#[test]
fn foreign_and_reset_frames_cannot_be_filled_by_another_generation() {
    let mut first = Parser::new(Config::default());
    let mut frame = first.adapter_frame();
    first.feed(b"<r/>", true).unwrap();
    let mut event = None;
    first
        .next_event_for_adapter_into(&mut event, &mut frame)
        .unwrap()
        .unwrap();
    assert!(frame.is_active());
    let mut second = Parser::new(Config::default());
    second.feed(b"<other/>", true).unwrap();
    second
        .next_event_for_adapter_into(&mut event, &mut frame)
        .unwrap()
        .unwrap();
    assert!(!frame.is_active());
    assert!(matches!(
        event.as_ref().unwrap().kind,
        EventKind::StartElement { .. }
    ));
    second.finish_adapter_frame(frame);
    let mut frame = first.adapter_frame();
    first = Parser::new(Config::default());
    first.feed(b"<reset/>", true).unwrap();
    first
        .next_event_for_adapter_into(&mut event, &mut frame)
        .unwrap()
        .unwrap();
    assert!(!frame.is_active());
    first.finish_adapter_frame(frame);
}

#[test]
fn namespace_identity_frames_follow_default_binding_scope() {
    let input = "<r><warm a='v'/><plain b='v'/><n xmlns='urn:default'><plain a='v'/><n xmlns=''><plain a='v'/></n><plain a='v'/></n><plain a='v'/></r>";
    for separator in ['|', '\0', 'x'] {
        for triplets in [false, true] {
            for chunk in 1..=input.len() {
                let config = Config {
                    namespace_separator: Some(separator),
                    namespace_triplets: triplets,
                    ..Config::default()
                };
                let owned = collect(input.as_bytes(), chunk, config.clone(), false);
                let adapter = collect(input.as_bytes(), chunk, config, true);
                assert_eq!(adapter.0, owned.0, "{separator:?} {triplets} {chunk}");
                // Root, then plain tags before, within and after the cleared
                // default binding; declaration tags retain namespace callbacks.
                assert_eq!(adapter.1, 4, "{separator:?} {triplets} {chunk}");
            }
        }
    }
}

#[test]
fn namespace_plans_preserve_errors_and_owned_expansions() {
    for input in [
        "<r xmlns:p='urn:p'><p:n a='v'/><p:n p:a='v'/><plain a='v'/></r>",
        "<r><warm a='v'/><n xmlns:p='urn:p' p:a='v'/></r>",
        "<r><warm a='v'/><n xmlns:p='urn:p' xmlns:q='urn:p' p:a='v' q:a='x'/></r>",
        "<r><warm a='v'/><n xmlns:xml='wrong'/></r>",
        "<r><warm a='v'/><n xmlns:p=''/></r>",
        "<r><warm a='v'/><n p:a='v'/></r>",
        "<r><warm a='v'/><:n a='v'/></r>",
        "<r><warm a='v'/><n: a='v'/></r>",
        "<r><warm a='v'/><p::n a='v'/></r>",
        "<r><warm a='v'/><n :a='v'/></r>",
        "<r><warm a='v'/><n a:='v'/></r>",
        "<r><warm a='v'/><n a:b:c='v'/></r>",
        "<r><warm a='v'/><n a='v' a='duplicate'/></r>",
        "<r><warm a='v'/><p::n a='v' b='unterminated",
        "<r><warm a='v'/><n a:b:c='v' b='&missing;'/></r>",
    ] {
        for separator in ['|', '\0', 'x'] {
            for triplets in [false, true] {
                for chunk in 1..=input.len() {
                    let config = Config {
                        namespace_separator: Some(separator),
                        namespace_triplets: triplets,
                        ..Config::default()
                    };
                    assert_eq!(
                        collect(input.as_bytes(), chunk, config.clone(), true).0,
                        collect(input.as_bytes(), chunk, config, false).0,
                        "{input:?} {separator:?} {triplets} {chunk}"
                    );
                }
            }
        }
    }
}
