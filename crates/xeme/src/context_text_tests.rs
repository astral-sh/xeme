use super::*;

#[test]
fn context_text_native_prefix_precedes_late_accounting_error() {
    let mut records = std::vec::Vec::new();
    for native in [false, true] {
        let mut parser = Parser::new(Config::default());
        parser.feed(b"<r>abc</r>", true).unwrap();
        parser.next_event().unwrap().unwrap();
        // Model prior successful child input in this root's shared budget,
        // then lower its amplification allowance before consuming root Text.
        assert!(parser.expanded.account(100, true, false));
        assert!(parser.set_entity_maximum_amplification(1.0));
        parser.set_entity_activation_threshold(0);
        let mut frame = parser.adapter_frame();
        let mut event = None;
        assert!(
            parser
                .next_event_for_adapter_mode_into(&mut event, &mut frame, native)
                .unwrap()
                .is_some()
        );
        assert!(event.is_none() && frame.is_text());
        if native {
            assert_eq!(frame.native_text_range_for_c(), Some((3, 3)));
        } else {
            assert_eq!(frame.text_bytes(), Some(b"abc".as_slice()));
        }
        assert_eq!(parser.current_raw(), Some("abc"));
        assert_eq!(frame.position(), parser.position());
        let position = frame.position();
        let error = parser
            .next_event_for_adapter_mode_into(&mut event, &mut frame, native)
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::LimitExceeded);
        assert!(event.is_none() && !frame.is_active());
        records.push((position, error, parser.current_raw().unwrap().to_owned()));
        parser.finish_adapter_frame(frame);
    }
    assert_eq!(records[0], records[1]);
}

#[test]
fn context_text_bounds_and_ordinary_api_reuse() {
    for count in [1, 23, 24, 4096, 4097] {
        let value = "a".repeat(count);
        let input = format!("<r>{value}<n/>{value}</r>");
        let mut parser = Parser::new(Config::default());
        parser.feed(input.as_bytes(), true).unwrap();
        parser.next_event().unwrap().unwrap();
        let mut frame = parser.adapter_frame();
        let mut event = None;
        parser
            .next_event_for_c_text_context_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        if count <= arena::MAX_ARENA_BYTES {
            assert_eq!(frame.native_text_range_for_c(), Some((3, count)));
            for getter in [0, 1, 2] {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        match getter {
                            0 => {
                                let _ = frame.text_bytes();
                            }
                            1 => {
                                let _ = frame.name_bytes();
                            }
                            _ => {
                                let _ = frame.attributes().count();
                            }
                        }
                    }))
                    .is_err()
                );
            }
        } else {
            assert!(frame.native_text_range_for_c().is_none());
            assert!(
                matches!(event.as_ref().map(|e| &e.kind), Some(EventKind::Text(text)) if text.as_str() == value)
            );
        }
        assert_eq!(parser.current_raw(), Some(value.as_str()));
        // Ordinary calls are legal immediately, and clear the native descriptor.
        for _ in 0..3 {
            parser
                .next_event_for_adapter_into(&mut event, &mut frame)
                .unwrap()
                .unwrap();
            assert!(frame.native_text_range_for_c().is_none());
        }
        if count <= arena::MAX_ARENA_BYTES {
            assert_eq!(frame.text_bytes(), Some(value.as_bytes()));
        }
        parser.finish_adapter_frame(frame);
    }
}

#[test]
fn context_text_excluded_sources_keep_owned_text() {
    let documents = [
        "<r>a\rb</r>",
        "<r><![CDATA[abc]]></r>",
        "<r>&amp;</r>",
        "<!DOCTYPE r [<!ENTITY e 'abc'>]><r>&e;</r>",
    ];
    for input in documents {
        for utf16 in [false, true] {
            let bytes = if utf16 {
                [0xfeff]
                    .into_iter()
                    .chain(input.encode_utf16())
                    .flat_map(u16::to_le_bytes)
                    .collect::<std::vec::Vec<_>>()
            } else {
                input.as_bytes().to_vec()
            };
            let mut parser = Parser::new(Config::default());
            parser.feed(&bytes, true).unwrap();
            let mut frame = parser.adapter_frame();
            let mut event = None;
            let mut text = std::string::String::new();
            while parser
                .next_event_for_c_text_context_into(&mut event, &mut frame)
                .unwrap()
                .is_some()
            {
                assert!(
                    frame.native_text_range_for_c().is_none(),
                    "{input}, utf16={utf16}"
                );
                if let Some(bytes) = frame.text_bytes() {
                    text.push_str(std::str::from_utf8(bytes).unwrap());
                } else if let Some(Event {
                    kind: EventKind::Text(value),
                    ..
                }) = &event
                {
                    text.push_str(value.as_str());
                }
            }
            assert_eq!(
                text,
                if input.contains('\r') {
                    "a\nb"
                } else if input.contains("&amp;") {
                    "&"
                } else {
                    "abc"
                }
            );
            parser.finish_adapter_frame(frame);
        }
    }
    let parent = Parser::new(Config::default());
    let mut child = parent.external_child(Some(""), None).unwrap();
    child.feed(b"abc", true).unwrap();
    let mut frame = child.adapter_frame();
    let mut event = None;
    child
        .next_event_for_c_text_context_into(&mut event, &mut frame)
        .unwrap()
        .unwrap();
    assert!(frame.native_text_range_for_c().is_none());
    assert_eq!(frame.text_bytes(), Some(b"abc".as_slice()));
    child.finish_adapter_frame(frame);
}

#[test]
fn context_text_foreign_generation_uses_owned_events() {
    let mut origin = Parser::new(Config::default());
    let mut frame = origin.adapter_frame();
    let mut parser = Parser::new(Config::default());
    parser.feed(b"<r>abc</r>", true).unwrap();
    parser.next_event().unwrap().unwrap();
    let mut event = None;
    parser
        .next_event_for_c_text_context_into(&mut event, &mut frame)
        .unwrap()
        .unwrap();
    assert!(!frame.is_active());
    assert!(matches!(event.unwrap().kind, EventKind::Text(value) if value.as_str() == "abc"));
    origin.finish_adapter_frame(frame);
}

#[test]
fn native_text_retains_partial_source_precharges() {
    for precharged in [0, 1, 2, 3] {
        let mut results = std::vec::Vec::new();
        for native in [false, true] {
            let mut parser = Parser::new(Config {
                limits: Limits {
                    max_work_amplification: Some(1),
                    ..Limits::default()
                },
                ..Config::default()
            });
            parser.enable_input_context();
            parser.set_text_line_boundaries(true);
            parser.feed(b"<r>abc</r>", true).unwrap();
            parser.next_event().unwrap().unwrap();
            parser.account_source(precharged).unwrap();
            let mut frame = parser.adapter_frame();
            let mut event = None;
            parser
                .next_event_for_adapter_mode_into(&mut event, &mut frame, native)
                .unwrap()
                .unwrap();
            results.push((
                frame.position(),
                parser.work_bytes_limit(0),
                parser.current_raw().unwrap().to_owned(),
            ));
            parser.finish_adapter_frame(frame);
        }
        assert_eq!(results[0], results[1]);
        assert_eq!(results[1].1, 6);
    }
}

#[test]
fn native_scalar_references_preserve_unique_and_shared_amplification_errors() {
    for shared in [false, true] {
        for indirect in [0, 100] {
            let mut results = std::vec::Vec::new();
            for native in [false, true] {
                let mut parser = Parser::new(Config::default());
                parser.enable_input_context();
                parser.set_text_line_boundaries(true);
                parser.feed(b"<r>&amp;&#65;</r>", true).unwrap();
                parser.next_event().unwrap().unwrap();
                let _owner = shared.then(|| parser.expanded.clone());
                assert!(parser.expanded.account(indirect, true, false));
                assert!(parser.set_entity_maximum_amplification(1.0));
                parser.set_entity_activation_threshold(0);
                let mut frame = parser.adapter_frame();
                let mut event = None;
                let result = parser
                    .next_event_for_adapter_mode_into(&mut event, &mut frame, native)
                    .map(|token| token.is_some());
                let text = frame
                    .is_active()
                    .then(|| frame.text_bytes().unwrap().to_vec());
                results.push((
                    result,
                    text,
                    parser.position(),
                    parser.position_between_callbacks(true),
                    parser.current_raw().map(str::to_owned),
                    parser.work_bytes_limit(0),
                ));
                if let Err(error) = result {
                    assert_eq!(error.kind, ErrorKind::LimitExceeded);
                    assert!(event.is_none() && !frame.is_active());
                    assert_eq!(
                        parser
                            .next_event_for_adapter_mode_into(&mut event, &mut frame, native)
                            .unwrap_err(),
                        error,
                    );
                }
                parser.finish_adapter_frame(frame);
            }
            assert_eq!(
                results[0], results[1],
                "shared={shared}, indirect={indirect}"
            );
        }
    }
}
