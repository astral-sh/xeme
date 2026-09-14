use xeme::{AdapterFrame, Config, Error, ErrorKind, EventKind, Limits, Parser, Position};

#[derive(Clone, Copy, Debug)]
enum Mode {
    Adapter,
    Native,
}

#[derive(Debug, PartialEq)]
struct Delivery {
    text: Option<String>,
    raw: Option<String>,
    position: Position,
    between: Position,
    work_limit: usize,
}

fn parser() -> Parser {
    let mut parser = Parser::new(Config {
        namespace_separator: Some('|'),
        limits: Limits {
            max_work_amplification: Some(3),
            ..Limits::default()
        },
        ..Config::default()
    });
    parser.enable_input_context();
    parser.set_text_line_boundaries(true);
    parser
}

fn next(
    parser: &mut Parser,
    frame: &mut AdapterFrame,
    mode: Mode,
) -> Result<Option<Delivery>, Error> {
    let mut event = None;
    let result = match mode {
        Mode::Adapter => parser
            .next_event_for_adapter_into(&mut event, frame)
            .map(|token| token.is_some()),
        Mode::Native => parser
            .next_event_for_c_text_context_into(&mut event, frame)
            .map(|token| token.is_some()),
    };
    if !matches!(result, Ok(true)) {
        assert!(event.is_none());
        assert!(!frame.is_active());
        return result.map(|_| None);
    }
    let (text, position) = if let Some(event) = event {
        assert!(!frame.is_active());
        let text = match event.kind {
            EventKind::Text(text) => Some(text.to_string()),
            _ => None,
        };
        (text, event.position)
    } else {
        let text = if let Some((start, count)) = frame.native_text_range_for_c() {
            let (context, base) = parser.input_context();
            Some(std::str::from_utf8(&context[start - base..start - base + count]).unwrap())
        } else {
            frame
                .text_bytes()
                .map(|bytes| std::str::from_utf8(bytes).unwrap())
        };
        if let Some(text) = text {
            assert_eq!(frame.callback_bytes(), text.len());
        }
        (text.map(str::to_owned), frame.position())
    };
    assert_eq!(parser.position(), position);
    Ok(Some(Delivery {
        text,
        raw: parser.current_raw().map(str::to_owned),
        position,
        between: parser.position_between_callbacks(true),
        work_limit: parser.work_bytes_limit(0),
    }))
}

fn collect(input: &[u8], width: usize, mode: Mode, pause: bool) -> Vec<Delivery> {
    let mut parser = parser();
    let mut frame = parser.adapter_frame();
    let mut deliveries = Vec::new();
    for (index, chunk) in input.chunks(width).enumerate() {
        parser
            .feed(chunk, (index + 1) * width >= input.len())
            .unwrap();
        loop {
            match next(&mut parser, &mut frame, mode) {
                Ok(Some(delivery)) => {
                    deliveries.push(delivery);
                    if pause {
                        // A C suspension releases its detached frame. The next
                        // call recreates it without consuming the next token.
                        parser.finish_adapter_frame(frame);
                        frame = parser.adapter_frame();
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    assert_eq!(next(&mut parser, &mut frame, mode), Err(error));
                    assert_eq!(parser.feed(b"ignored", false), Err(error));
                    deliveries.push(Delivery {
                        text: Some(format!("error {error:?}")),
                        raw: parser.current_raw().map(str::to_owned),
                        position: parser.position(),
                        between: parser.position_between_callbacks(true),
                        work_limit: parser.work_bytes_limit(0),
                    });
                    parser.finish_adapter_frame(frame);
                    return deliveries;
                }
            }
        }
    }
    assert!(parser.is_finished());
    parser.finish_adapter_frame(frame);
    deliveries
}

#[test]
fn native_delivery_keeps_each_callback_and_resume_position() {
    let documents = [
        "<r>&amp;&lt;&gt;&apos;&quot;&#9;&#10;&#13;&#x80;&#x10FFFF;tail</r>",
        "<r>&#000000000000000000000065;&amp;&#xD800;</r>",
        "<r>&amp;&#x41;&#0;</r>",
        "<r>alpha\tbeta\n\nlast\r\nline\rtail</r>",
        "\u{feff}<r>one\n<n/>two&amp;three\n<![CDATA[four\nfive]]>six</r>",
        "<r>valid\ninvalid]]></r>",
        "<r>valid\ninvalid\u{1}</r>",
        "<r>valid\nnonasciié\nend</wrong>",
        "<!DOCTYPE r [<!ENTITY e 'one&#10;two'>]><r>&e;three\nfour</r>",
        "<r xmlns:p='urn'><p:child/>text\nnext</r>",
    ];
    for document in documents {
        for width in [1, 2, 7, 16, document.len()] {
            let expected = collect(document.as_bytes(), width, Mode::Adapter, false);
            for pause in [false, true] {
                assert_eq!(
                    collect(document.as_bytes(), width, Mode::Native, pause),
                    expected,
                    "width={width}, pause={pause}, document={document:?}"
                );
            }
        }
    }
    for document in [
        b"<r>valid\npartial\xc3".as_slice(),
        b"<r>valid\ninvalid\xff",
    ] {
        for width in [1, document.len()] {
            assert_eq!(
                collect(document, width, Mode::Native, true),
                collect(document, width, Mode::Adapter, false),
                "width={width}, document={document:?}"
            );
        }
    }
}

#[test]
fn arena_edges_do_not_create_extra_callbacks_or_hide_malformed_suffixes() {
    for count in [1, 23, 24, 4095, 4096, 4097, 65_536, 65_537] {
        for suffix in [
            "\nend</r>",
            "&amp;</r>",
            "é\nend</r>",
            "]]></r>",
            "\u{1}</r>",
        ] {
            let document = format!("<r>{}{suffix}", "x".repeat(count));
            for width in [4096, document.len()] {
                assert_eq!(
                    collect(document.as_bytes(), width, Mode::Native, true),
                    collect(document.as_bytes(), width, Mode::Adapter, false),
                    "count={count}, suffix={suffix:?}, width={width}"
                );
            }
        }
    }
}

#[test]
fn amplification_failure_publishes_one_prefix_then_keeps_the_error() {
    let mut traces = Vec::new();
    for mode in [Mode::Adapter, Mode::Native] {
        let mut parser = parser();
        parser.feed(b"<r>&amp;prefix\nremaining</r>", true).unwrap();
        let mut frame = parser.adapter_frame();
        next(&mut parser, &mut frame, mode).unwrap().unwrap();
        assert_eq!(
            next(&mut parser, &mut frame, mode)
                .unwrap()
                .unwrap()
                .text
                .as_deref(),
            Some("&")
        );
        assert!(parser.set_entity_maximum_amplification(1.0));
        assert!(parser.set_entity_activation_threshold(0));
        let prefix = next(&mut parser, &mut frame, mode).unwrap().unwrap();
        assert_eq!(prefix.text.as_deref(), Some("prefix"));
        assert_eq!(prefix.raw.as_deref(), Some("prefix"));
        let error = next(&mut parser, &mut frame, mode).unwrap_err();
        assert_eq!(error.kind, ErrorKind::LimitExceeded);
        assert_eq!(next(&mut parser, &mut frame, mode), Err(error));
        assert_eq!(parser.current_raw(), Some("prefix"));
        traces.push((prefix, error));
        parser.finish_adapter_frame(frame);
    }
    assert_eq!(traces[0], traces[1]);
}

#[test]
fn native_delivery_rechecks_configuration_and_errors_after_callbacks() {
    for invalid_feed in [false, true] {
        let mut parser = parser();
        parser.feed(b"<r>one\ntwo\nthree</r>", true).unwrap();
        let mut frame = parser.adapter_frame();
        next(&mut parser, &mut frame, Mode::Native)
            .unwrap()
            .unwrap();
        assert_eq!(
            next(&mut parser, &mut frame, Mode::Native)
                .unwrap()
                .unwrap()
                .text
                .as_deref(),
            Some("one")
        );
        assert!(frame.native_text_range_for_c().is_some());
        let cursor = parser.position_between_callbacks(true);
        let error = if invalid_feed {
            Some(parser.feed(b"ignored", false).unwrap_err())
        } else {
            parser.set_text_line_boundaries(false);
            None
        };
        assert_eq!(parser.position_between_callbacks(true), cursor);
        assert_eq!(parser.current_raw(), Some("one"));
        let result = next(&mut parser, &mut frame, Mode::Native);
        if let Some(error) = error {
            assert_eq!(result, Err(error));
            assert!(!frame.is_active());
            assert_eq!(parser.current_raw(), Some("one"));
        } else {
            assert_eq!(
                result.unwrap().unwrap().text.as_deref(),
                Some("\ntwo\nthree")
            );
        }
        parser.finish_adapter_frame(frame);
    }
}
