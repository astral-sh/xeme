use oriole::{Attribute, Config, Event, EventKind, Parser};
use oriole_storage::{Allocator, String, Text, Vec, try_push};

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
    collect_mode(input, chunk, config, u8::from(adapter))
}

fn collect_mode(
    input: &[u8],
    chunk: usize,
    config: Config,
    mode: u8,
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
            let result = match mode {
                0 => parser.next_event_for_recycling_into(&mut event),
                1 => parser.next_event_for_adapter_into(&mut event, &mut frame),
                2 => parser.next_event_for_c_coordinates_into(&mut event, &mut frame),
                _ => unreachable!(),
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
                let position = match frame.location_for_c() {
                    oriole::AdapterLocation::Position(position) => {
                        assert_eq!(frame.position(), position);
                        position
                    }
                    oriole::AdapterLocation::Native(native) => {
                        let position = parser.position(); // Pure ordinary API stays exact.
                        assert_eq!(position.byte_index, native.byte_index());
                        assert_eq!(position.byte_count, native.byte_count());
                        if records.len() % 7 == 0 {
                            assert_eq!(
                                parser.resolve_native_location_for_c(native),
                                Some(position)
                            );
                        }
                        position
                    }
                };
                if let Some(name) = frame.take_end_name() {
                    event = Some(Event {
                        kind: EventKind::EndElement { name },
                        position,
                    });
                } else if let Some(bytes) = frame.native_text_range_for_c().map_or_else(
                    || frame.text_bytes(),
                    |(start, count)| Some(&input[start..start + count]),
                ) {
                    assert_eq!(frame.callback_bytes(), bytes.len());
                    event = Some(Event {
                        kind: EventKind::Text(
                            Text::try_from_str_in(
                                std::str::from_utf8(bytes).unwrap(),
                                Allocator::System,
                            )
                            .unwrap(),
                        ),
                        position,
                    });
                } else {
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
                        position,
                    });
                }
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
fn literal_frames_keep_duplicate_checks_and_selected_name_rules() {
    for count in [8, 9] {
        let attributes = (0..count)
            .map(|index| format!(" a{index}='{index}'"))
            .collect::<std::string::String>();
        for (name, duplicate) in [
            ("plain", false),
            ("plain", true),
            ("p::n", false),
            ("n\u{200c}", false),
        ] {
            let input = format!(
                "<r><warm{attributes}/><{name}{attributes}{}/></r>",
                if duplicate { " a0='duplicate'" } else { "" }
            );
            for name_rules in [
                oriole::NameRules::FourthEdition,
                oriole::NameRules::FifthEdition,
            ] {
                for namespace_separator in [None, Some('|')] {
                    for chunk in [1, 7, input.len()] {
                        let config = Config {
                            namespace_separator,
                            name_rules,
                            ..Config::default()
                        };
                        let owned = collect(input.as_bytes(), chunk, config.clone(), false);
                        let adapter = collect(input.as_bytes(), chunk, config, true);
                        assert_eq!(
                            adapter.0, owned.0,
                            "{input:?} {name_rules:?} {namespace_separator:?} {chunk}"
                        );
                    }
                }
            }
        }
    }
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
fn detached_end_frames_keep_native_and_namespace_undo_boundaries() {
    for (xml, expected) in [
        ("<r><n></n><e/></r>", vec!["n", "r"]),
        ("<r><n></n ></r >", vec![]),
        ("<r xmlns:p='u'><p:n></p:n></r>", vec!["u|n"]),
        ("<r xmlns='u'><n></n></r>", vec!["u|n"]),
        ("<!DOCTYPE r><r><n></n></r>", vec![]),
    ] {
        for utf16 in [false, true] {
            let input = if utf16 {
                [0xfeff]
                    .into_iter()
                    .chain(xml.encode_utf16())
                    .flat_map(u16::to_le_bytes)
                    .collect::<std::vec::Vec<_>>()
            } else {
                xml.as_bytes().to_vec()
            };
            for chunk in 1..=input.len() {
                let mut parser = Parser::new(Config {
                    namespace_separator: Some('|'),
                    ..Config::default()
                });
                parser.set_reparse_deferral_enabled(false);
                let mut frame = parser.adapter_frame();
                let mut names = std::vec::Vec::new();
                for (index, bytes) in input.chunks(chunk).enumerate() {
                    parser
                        .feed(bytes, (index + 1) * chunk >= input.len())
                        .unwrap();
                    loop {
                        let mut event = None;
                        let Some(token) = parser
                            .next_event_for_adapter_into(&mut event, &mut frame)
                            .unwrap()
                        else {
                            break;
                        };
                        if let Some(name) = frame.take_end_name() {
                            assert!(event.is_none());
                            assert_eq!(frame.callback_bytes(), name.len());
                            assert!(parser.current_raw().unwrap().starts_with("</"));
                            names.push(name.as_str().to_owned());
                            parser.recycle_end_element(token, name);
                        }
                    }
                }
                assert!(parser.is_finished());
                assert_eq!(names, if utf16 { vec![] } else { expected.clone() });
                parser.finish_adapter_frame(frame);
            }
        }
    }
}

#[test]
fn detached_end_frames_recheck_child_publication_and_generation() {
    let mut parser = Parser::new(Config::default());
    let mut frame = parser.adapter_frame();
    parser.feed(b"<r><n></n></r>", true).unwrap();
    let mut event = None;
    for _ in 0..2 {
        parser
            .next_event_for_adapter_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
    }
    let mut child = parser.external_child_with_encoding(None, None).unwrap();
    child.feed(b"<!ATTLIST r a CDATA 'v'>", true).unwrap();
    while child.next_event().unwrap().is_some() {}
    parser
        .next_event_for_adapter_into(&mut event, &mut frame)
        .unwrap()
        .unwrap();
    assert!(!frame.is_active());
    assert!(matches!(event.unwrap().kind, EventKind::EndElement { .. }));
    parser.finish_adapter_frame(frame);

    let mut first = Parser::new(Config::default());
    let mut frame = first.adapter_frame();
    first.feed(b"<old></old>", true).unwrap();
    let mut event = None;
    for _ in 0..2 {
        first
            .next_event_for_adapter_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
    }
    assert!(frame.is_active()); // Leave the original End owner in the old frame.
    let mut replacement = Parser::new(Config::default());
    replacement.feed(b"<new></new>", true).unwrap();
    replacement.next_event().unwrap().unwrap();
    replacement
        .next_event_for_adapter_into(&mut event, &mut frame)
        .unwrap()
        .unwrap();
    assert!(!frame.is_active());
    let EventKind::EndElement { name } = event.unwrap().kind else {
        panic!("owned fallback")
    };
    assert_eq!(name.as_str(), "new");
    replacement.finish_adapter_frame(frame);
}

#[test]
fn text_frames_match_owned_projection_at_inline_heap_and_fallback_boundaries() {
    for count in [1, 23, 24, 4095, 4096, 4097] {
        let text = "x".repeat(count);
        for content in [
            text.clone(),
            format!("<![CDATA[{text}]]>"),
            format!("{text}\r\nend"),
        ] {
            for namespace_separator in [None, Some('|')] {
                let xml = format!("<r>{content}<n a='warm'/>{content}</r>");
                let utf16 = [0xfeff]
                    .into_iter()
                    .chain(xml.encode_utf16())
                    .flat_map(u16::to_le_bytes)
                    .collect::<std::vec::Vec<_>>();
                for input in [xml.as_bytes(), utf16.as_slice()] {
                    for chunk in [1, 23, 4096, input.len()] {
                        let config = Config {
                            namespace_separator,
                            ..Config::default()
                        };
                        assert_eq!(
                            collect(input, chunk, config.clone(), false).0,
                            collect(input, chunk, config, true).0,
                            "count={count}, chunk={chunk}, namespace={namespace_separator:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn ordinary_text_precedes_accounting_failure_but_cdata_does_not() {
    for cdata in [false, true] {
        let parent = Parser::new(Config::default());
        parent.set_entity_maximum_amplification(f32::INFINITY);
        parent.set_entity_activation_threshold(0);
        let mut parser = parent.external_child_with_encoding(Some(""), None).unwrap();
        parser
            .feed(if cdata { b"<![CDATA[abc]]>" } else { b"abc" }, true)
            .unwrap();
        if cdata {
            assert!(matches!(
                parser.next_event().unwrap().unwrap().kind,
                EventKind::StartCdata
            ));
        }
        parent.set_entity_maximum_amplification(1.0);
        let mut frame = parser.adapter_frame();
        let mut event = None;
        let result = parser.next_event_for_adapter_into(&mut event, &mut frame);
        assert!(event.is_none());
        if cdata {
            assert_eq!(result.unwrap_err().kind, oriole::ErrorKind::LimitExceeded);
            assert!(!frame.is_active());
        } else {
            assert!(result.unwrap().is_some());
            assert_eq!(frame.text_bytes(), Some(b"abc".as_slice()));
            assert_eq!(parser.position(), frame.position());
            assert_eq!(parser.current_raw(), Some("abc"));
            assert_eq!(
                parser
                    .next_event_for_adapter_into(&mut event, &mut frame)
                    .unwrap_err()
                    .kind,
                oriole::ErrorKind::LimitExceeded
            );
            assert!(!frame.is_active());
            assert!(event.is_none());
        }
        parser.finish_adapter_frame(frame);
    }
}

#[test]
fn lazy_coordinates_match_eager_events_through_fallbacks_and_compaction() {
    let documents = [
        "\u{feff}<r>é\r\n<n a='x'>α😀</n><e/>tail</r>".to_owned(),
        "<r><n></n><bad a='x' a='y'></bad></r>".to_owned(),
        "<r><n></n><n xmlns='u'><n/></n><n><![CDATA[x\r\ny]]>&amp;</n></r>".to_owned(),
        "<!DOCTYPE r [<!ENTITY e 'body'>]><r><n>&e;</n></r>".to_owned(),
        format!("<r>{}</r>", "<n>é\n</n>\n".repeat(7_000)),
        "<r><n></n><bad a='incomplete".to_owned(),
    ];
    for xml in documents {
        let utf16 = [0xfeff]
            .into_iter()
            .chain(xml.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect::<std::vec::Vec<_>>();
        for bytes in [xml.as_bytes(), utf16.as_slice()] {
            for namespace_separator in [None, Some('|')] {
                let chunks = if bytes.len() < 256 {
                    (1..=bytes.len()).collect()
                } else {
                    vec![4_096, 65_536, bytes.len()]
                };
                for chunk in chunks {
                    let config = Config {
                        namespace_separator,
                        ..Config::default()
                    };
                    assert_eq!(
                        collect_mode(bytes, chunk, config.clone(), 2).0,
                        collect_mode(bytes, chunk, config, 1).0,
                        "chunk={chunk}, ns={namespace_separator:?}, bytes={}",
                        bytes.len()
                    );
                }
            }
        }
    }
}

#[test]
fn lazy_locations_reject_stale_and_foreign_descriptors_and_keep_eager_transitions() {
    let mut config = Config::default();
    config.limits.max_total_bytes = 20;
    let mut parser = Parser::new(config);
    parser.feed(b"<r><n></n><e/></r>", false).unwrap();
    let mut old_frame = parser.adapter_frame();
    let mut event = None;
    parser
        .next_event_for_c_coordinates_into(&mut event, &mut old_frame)
        .unwrap()
        .unwrap();
    let oriole::AdapterLocation::Native(old) = old_frame.location_for_c() else {
        panic!("native root")
    };
    let expected = parser.position();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| old_frame.position())).is_err()
    );
    // A rejected feed does not flush or replace the unresolved publication.
    assert!(parser.feed(b"more bytes", false).is_err());
    assert_eq!(parser.position(), expected);
    assert_eq!(parser.resolve_native_location_for_c(old), Some(expected));
    parser.finish_adapter_frame(old_frame);

    let mut parser = Parser::new(Config::default());
    parser.feed(b"<r><n></n><e/></r>", true).unwrap();
    let mut first = parser.adapter_frame();
    let mut second = parser.adapter_frame();
    parser
        .next_event_for_c_coordinates_into(&mut event, &mut first)
        .unwrap()
        .unwrap();
    let oriole::AdapterLocation::Native(old) = first.location_for_c() else {
        panic!("native root")
    };
    parser
        .next_event_for_c_coordinates_into(&mut event, &mut second)
        .unwrap()
        .unwrap();
    assert_eq!(parser.resolve_native_location_for_c(old), None);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| first.position())).is_err());
    let oriole::AdapterLocation::Native(current) = second.location_for_c() else {
        panic!("native child")
    };
    let mut foreign = Parser::new(Config::default());
    foreign.feed(b"<r><n></n></r>", true).unwrap();
    let mut foreign_frame = foreign.adapter_frame();
    foreign
        .next_event_for_c_coordinates_into(&mut event, &mut foreign_frame)
        .unwrap()
        .unwrap();
    assert_eq!(foreign.resolve_native_location_for_c(current), None);
    foreign
        .next_event_for_c_coordinates_into(&mut event, &mut first)
        .unwrap()
        .unwrap();
    assert!(!first.is_active() && event.is_some());
    // Ordinary delivery after the explicit C seam owns an eager Position.
    parser
        .next_event_for_adapter_into(&mut event, &mut second)
        .unwrap()
        .unwrap();
    assert_eq!(second.position(), parser.position());
    parser.finish_adapter_frame(first);
    parser.finish_adapter_frame(second);
    foreign.finish_adapter_frame(foreign_frame);
}
