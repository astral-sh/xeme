use xeme::{Config, ErrorKind, Event, EventKind, Parser};

fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

#[test]
fn text_raw_spans_and_owned_values_survive_every_feed_boundary() {
    let xml = "<r>a\r\nb\rc\ndé😀&amp;e\r\nf</r>";
    for utf16 in [false, true] {
        let input = if utf16 {
            [
                vec![0xff, 0xfe],
                xml.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            ]
            .concat()
        } else {
            xml.as_bytes().to_vec()
        };
        for chunk in 1..=input.len() {
            let mut parser = Parser::new(Config::default());
            let mut retained: Vec<Event> = Vec::new();
            for (index, bytes) in input.chunks(chunk).enumerate() {
                parser
                    .feed(bytes, (index + 1) * chunk >= input.len())
                    .unwrap();
                while let Some(event) = parser.next_event().unwrap() {
                    if let EventKind::Text(value) = &event.kind {
                        let raw = parser.current_raw().unwrap();
                        if raw == "&amp;" {
                            assert_eq!(value, "&");
                        } else {
                            assert_eq!(value.as_ref(), normalized(raw));
                            assert_eq!(
                                event.position.byte_count,
                                if utf16 {
                                    raw.encode_utf16().count() * 2
                                } else {
                                    raw.len()
                                }
                            );
                        }
                        retained.push(event);
                    }
                }
            }
            assert!(parser.is_finished());
            drop(parser);
            let joined: String = retained
                .iter()
                .filter_map(|event| {
                    if let EventKind::Text(value) = &event.kind {
                        Some(value.as_ref())
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(joined, "a\nb\nc\ndé😀&e\nf");
        }
    }
}

#[test]
fn merging_keeps_the_complete_prefix_before_malformed_lines() {
    for (data, expected) in [
        ("a\nbad]]>".to_string(), "a\n"),
        ("a\r\nbad]]>\u{1}".to_string(), "a\n"),
        ("a\rb\nbad]]>".to_string(), "a\nb\n"),
        (format!("a\n{}]]>", "x".repeat(70_000)), "a\n"),
        (format!("{}]]>", "x".repeat(70_000)), ""),
        ("a\nbad\u{1}]]>".to_string(), "a\nbad"),
    ] {
        let mut parser = Parser::new(Config::default());
        parser
            .feed(format!("<r>{data}</r>").as_bytes(), true)
            .unwrap();
        let mut joined = String::new();
        let error = loop {
            match parser.next_event() {
                Ok(Some(event)) => {
                    if let EventKind::Text(value) = event.kind {
                        joined.push_str(&value);
                    }
                }
                Ok(None) => panic!("malformed input accepted"),
                Err(error) => break error,
            }
        };
        assert_eq!(error.kind, ErrorKind::InvalidToken);
        assert_eq!(joined, expected);
    }
}

#[test]
fn merging_is_bounded_and_never_splits_a_carriage_return_pair() {
    for offset in [65_534, 65_535, 65_536, 65_537] {
        let data = format!("a\n{}\r\né{}", "x".repeat(offset - 2), "y\n".repeat(40_000));
        let mut parser = Parser::new(Config::default());
        parser
            .feed(format!("<r>{data}</r>").as_bytes(), true)
            .unwrap();
        let mut joined = String::new();
        while let Some(event) = parser.next_event().unwrap() {
            if let EventKind::Text(value) = event.kind {
                let raw = parser.current_raw().unwrap();
                assert!(!raw.ends_with('\r'));
                // A single original line keeps its previous unbounded span;
                // newly merged short lines remain within 64 KiB plus CRLF.
                if raw.contains('\n') {
                    assert!(raw.len() <= 65_537);
                }
                joined.push_str(&value);
            }
        }
        assert_eq!(joined, normalized(&data));
    }
}

#[test]
fn converted_windows_do_not_emit_part_of_a_later_malformed_line() {
    for newline in ["\n", "\r", "\r\n"] {
        let data = format!("a\n{}{newline}bad]]>\u{1}", "x".repeat(65_533));
        let xml = format!("<r>{data}</r>");
        let input: Vec<u8> = [
            vec![0xff, 0xfe],
            xml.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        ]
        .concat();
        let mut parser = Parser::new(Config::default());
        parser.feed(&input, true).unwrap();
        let mut joined = String::new();
        loop {
            match parser.next_event() {
                Ok(Some(event)) => {
                    if let EventKind::Text(value) = event.kind {
                        joined.push_str(&value);
                    }
                }
                Ok(None) => panic!("malformed input accepted"),
                Err(error) => {
                    assert_eq!(error.kind, ErrorKind::InvalidToken);
                    break;
                }
            }
        }
        assert_eq!(joined, format!("a\n{}\n", "x".repeat(65_533)));
    }
}

#[test]
fn external_children_preserve_default_coalescing() {
    let parent = Parser::new(Config::default());
    for mut parser in [parent.external_child(Some(""), None).unwrap(), parent] {
        let xml = "<r>a\nb\r\nc</r>";
        parser.feed(xml.as_bytes(), true).unwrap();
        let mut values = Vec::new();
        while let Some(event) = parser.next_event().unwrap() {
            if let EventKind::Text(text) = event.kind {
                values.push(text.to_string());
            }
        }
        assert_eq!(values, ["a\nb\nc"]);
    }
}

#[test]
fn ascii_text_keeps_raw_spans_and_eager_positions_across_feeds() {
    let short = "<r>\nalpha\tbeta\u{7f}\n\ngamma<n/>tail\n</r>".to_string();
    let long = format!("<r>a\n{}<n/>\nend</r>", "x".repeat(65_534));
    for xml in [short, long] {
        let widths: Vec<_> = if xml.len() < 100 {
            (1..=xml.len()).collect()
        } else {
            vec![4096, 65_536, xml.len()]
        };
        for width in widths {
            let mut parser = Parser::new(Config::default());
            let mut joined = String::new();
            for (index, chunk) in xml.as_bytes().chunks(width).enumerate() {
                parser
                    .feed(chunk, (index + 1) * width >= xml.len())
                    .unwrap();
                while let Some(event) = parser.next_event().unwrap() {
                    let position = event.position;
                    let prefix = &xml[..position.byte_index];
                    assert_eq!(
                        position.line,
                        prefix.bytes().filter(|b| *b == b'\n').count() + 1
                    );
                    assert_eq!(position.column, prefix.rsplit('\n').next().unwrap().len());
                    assert_eq!(parser.position(), position);
                    if let EventKind::Text(text) = event.kind {
                        let raw = parser.current_raw().unwrap();
                        assert_eq!(raw, text.as_ref());
                        assert_eq!(position.byte_count, raw.len());
                        assert_eq!(&xml[position.byte_index..][..raw.len()], raw);
                        joined.push_str(&text);
                    }
                }
            }
            assert!(parser.is_finished());
            assert_eq!(
                joined,
                xml.replace("<r>", "")
                    .replace("<n/>", "")
                    .replace("</r>", "")
            );
        }
    }
}

#[test]
fn native_text_plan_matches_ascii_fallback_events_and_errors() {
    fn collect(input: &[u8], width: usize, encoding: Option<String>) -> Vec<String> {
        let mut parser = Parser::new(Config {
            encoding,
            ..Config::default()
        });
        let mut records = Vec::new();
        for (index, chunk) in input.chunks(width).enumerate() {
            parser
                .feed(chunk, (index + 1) * width >= input.len())
                .unwrap();
            loop {
                match parser.next_event() {
                    Ok(Some(event)) => {
                        records.push(format!("{event:?} raw {:?}", parser.current_raw()));
                    }
                    Ok(None) => break,
                    Err(error) => {
                        records.push(format!("{error:?} raw {:?}", parser.current_raw()));
                        return records;
                    }
                }
            }
        }
        records
    }
    for data in [
        "alpha\tbeta\n\ngamma\u{7f}&amp;tail",
        "\nxxxxx\n<child/>\nxxxxxx\nxxxxxxxx&amp;\tend",
        "xxxxxxx\nxxxxxxx\n<child/>\nxxxxxx\nxxxxxxxx&amp;\n\nend",
        "\nxxxxxx\nxxxxxxxx\u{1}<child/>",
        "\nxxxxxx\nxxxxxxxx<child/>\n\nbad]]>",
        "a\r\nb\rc\nd",
        "good\nbad]]>",
        "good\nbad\u{1}]]>",
        "good\nbad]]>\u{1}",
    ] {
        let xml = format!("<r>{data}</r>");
        for width in 1..=xml.len() {
            assert_eq!(
                collect(xml.as_bytes(), width, None),
                collect(xml.as_bytes(), width, Some("US-ASCII".to_string())),
                "{data:?}, width {width}"
            );
        }
    }
}
