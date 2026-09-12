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
                if let Some(name) = frame.take_end_name() {
                    event = Some(Event {
                        kind: EventKind::EndElement { name },
                        position: frame.position(),
                    });
                } else if let Some(bytes) = frame.text_bytes() {
                    assert_eq!(frame.callback_bytes(), bytes.len());
                    event = Some(Event {
                        kind: EventKind::Text(
                            Text::try_from_str_in(
                                std::str::from_utf8(bytes).unwrap(),
                                Allocator::System,
                            )
                            .unwrap(),
                        ),
                        position: frame.position(),
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
                        position: frame.position(),
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
                // Identity names and both inherited-default names use frames;
                // declaration tags retain their namespace callbacks.
                assert_eq!(adapter.1, 6, "{separator:?} {triplets} {chunk}");
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
        "<r><p:n a='v' a='duplicate'/></r>",
        "<r><xmlns:n a='v' a='duplicate'/></r>",
        "<r xmlns:p='u'><p:n></p:m></r>",
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
fn expanded_element_frames_preserve_packed_names_and_literal_attributes() {
    for (input, expected_frames) in [
        ("<r xmlns='urn:m'><n a='α😀'><n b='v'/></n></r>", 2),
        ("<p:r xmlns:p='urn:π'><p:é a='v'><p:é/></p:é></p:r>", 2),
        // NUL and colon separators can make expanded and raw spellings equal.
        ("<r xmlns:p='p:'><p:n a='v'><p:n/></p:n></r>", 2),
        ("<r xmlns:p='p'><p:n a='v'><p:n/></p:n></r>", 2),
        // Live packed names exhaust both cached owners before a longer name.
        (
            "<r xmlns:p='urn:example'><p:node><p:node><p:node><p:élong a='v'/></p:node></p:node></p:node><p:node/></r>",
            5,
        ),
    ] {
        for separator in ['|', '\0', ':', 'λ'] {
            for triplets in [false, true] {
                for name_rules in [
                    oriole::NameRules::FourthEdition,
                    oriole::NameRules::FifthEdition,
                ] {
                    for chunk in 1..=input.len() {
                        let config = Config {
                            namespace_separator: Some(separator),
                            namespace_triplets: triplets,
                            name_rules,
                            ..Config::default()
                        };
                        let adapter = collect(input.as_bytes(), chunk, config.clone(), true);
                        let owned = collect(input.as_bytes(), chunk, config, false);
                        assert_eq!(
                            adapter.0, owned.0,
                            "{input:?} {separator:?} {triplets} {chunk}"
                        );
                        assert_eq!(
                            adapter.1, expected_frames,
                            "{input:?} {separator:?} {triplets} {chunk}"
                        );
                    }
                }
            }
        }
    }
    for count in [8, 9, 128, 129] {
        let attrs = (0..count)
            .map(|index| format!(" a{index}='{index}'"))
            .collect::<std::string::String>();
        // The planner records only into existing capacity; the first wide tag
        // warms it through the owned path before the valid/duplicate pair.
        let input = format!("<r xmlns='u'><warm{attrs}/><n{attrs}/><n{attrs} a0='duplicate'/></r>");
        for chunk in [1, 4096, input.len()] {
            let config = Config {
                namespace_separator: Some('|'),
                ..Config::default()
            };
            let adapter = collect(input.as_bytes(), chunk, config.clone(), true);
            let owned = collect(input.as_bytes(), chunk, config, false);
            assert_eq!(adapter.0, owned.0, "{count} {chunk}");
            assert_eq!(adapter.1, usize::from(count <= 128), "{count} {chunk}");
            assert!(adapter.0.last().unwrap().contains("DuplicateAttribute"));
        }
    }
}

#[test]
fn expanded_element_frames_keep_uri_work_and_arena_limits() {
    let tag = "<p:n a='v'/>";
    for separator in ['|', '\0', 'λ'] {
        let separator_bytes = if separator == '\0' {
            0
        } else {
            2 * separator.len_utf8()
        };
        for triplets in [false, true] {
            for extra in [0, 1, 2] {
                let uri = "u".repeat(4096 - tag.len() - separator_bytes - 1 + extra);
                let input = format!("<r xmlns:p='{uri}'>{tag}{tag}</r>");
                for chunk in [1, 4096, input.len()] {
                    let config = Config {
                        namespace_separator: Some(separator),
                        namespace_triplets: triplets,
                        ..Config::default()
                    };
                    let adapter = collect(input.as_bytes(), chunk, config.clone(), true);
                    let owned = collect(input.as_bytes(), chunk, config, false);
                    assert_eq!(
                        adapter.0, owned.0,
                        "{separator:?} {triplets} {extra} {chunk}"
                    );
                    assert_eq!(adapter.1, if extra == 2 { 0 } else { 2 });
                }
            }
        }
    }
    let input = "<r xmlns:p='urn:long'><p:n a='v'/><p:n/><p:n/></r>";
    for limit in 0..=3 * "urn:long".len() {
        let mut config = Config {
            namespace_separator: Some('|'),
            ..Config::default()
        };
        config.limits.max_entity_expansion_bytes = limit;
        for chunk in [1, 7, input.len()] {
            assert_eq!(
                collect(input.as_bytes(), chunk, config.clone(), true).0,
                collect(input.as_bytes(), chunk, config.clone(), false).0,
                "URI work limit {limit}, chunk {chunk}"
            );
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
fn reference_frames_preserve_boundaries_positions_and_error_prefixes() {
    for xml in [
        "<r>a&amp;&lt;&gt;&apos;&quot;&#13;&#10;&#xE9;&#x1F600;z</r>",
        "<r>a&amp;bad]]></r>",
        "<r>a&amp;&#0;</r>",
        "<r>a&amp;&#x110000;</r>",
        "<r>a&amp;&missing;</r>",
        "<r>a&amp;&#x1F",
        "<!DOCTYPE r [<!ENTITY e 'a&amp;&#x1F600;b'>]><r>&e;</r>",
    ] {
        let utf16 = [0xfeff]
            .into_iter()
            .chain(xml.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect::<std::vec::Vec<_>>();
        for input in [xml.as_bytes(), utf16.as_slice()] {
            for chunk in 1..=input.len() {
                for limit in [1, 4, 16, usize::MAX] {
                    let mut config = Config::default();
                    config.limits.max_token_bytes = limit;
                    assert_eq!(
                        collect(input, chunk, config.clone(), false).0,
                        collect(input, chunk, config, true).0,
                        "{xml:?} chunk={chunk} limit={limit}"
                    );
                }
            }
        }
    }
}

#[test]
fn decoded_reference_frame_owns_bytes_and_waits_for_accounting() {
    let mut parser = Parser::new(Config::default());
    parser.feed(b"<r>&#x1F600;</r>", true).unwrap();
    parser.next_event().unwrap().unwrap();
    let mut frame = parser.adapter_frame();
    let mut event = None;
    parser
        .next_event_for_c_text_context_into(&mut event, &mut frame)
        .unwrap()
        .unwrap();
    assert!(event.is_none());
    assert_eq!(frame.text_bytes(), Some("😀".as_bytes()));
    assert_eq!(frame.native_text_range_for_c(), None);
    assert_eq!(frame.callback_bytes(), 4);
    assert_eq!(parser.current_raw(), Some("&#x1F600;"));
    assert_eq!(parser.position(), frame.position());
    drop(parser);
    assert_eq!(frame.text_bytes(), Some("😀".as_bytes()));

    let parent = Parser::new(Config::default());
    parent.set_entity_maximum_amplification(f32::INFINITY);
    parent.set_entity_activation_threshold(0);
    let mut child = parent.external_child_with_encoding(Some(""), None).unwrap();
    child.feed(b"&amp;", true).unwrap();
    parent.set_entity_maximum_amplification(1.0);
    let mut frame = child.adapter_frame();
    let error = child
        .next_event_for_adapter_into(&mut event, &mut frame)
        .unwrap_err();
    assert_eq!(error.kind, oriole::ErrorKind::LimitExceeded);
    assert!(!frame.is_active());
    assert!(event.is_none());
    child.finish_adapter_frame(frame);
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
