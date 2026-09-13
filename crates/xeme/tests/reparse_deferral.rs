use xeme::{Config, ErrorKind, EventKind, Parser};

fn encoded(text: &str, encoding: usize, first: bool) -> Vec<u8> {
    match encoding {
        0 => text.as_bytes().to_vec(),
        1 | 2 => {
            let mut bytes = if first {
                if encoding == 1 {
                    vec![0xff, 0xfe]
                } else {
                    vec![0xfe, 0xff]
                }
            } else {
                Vec::new()
            };
            for word in text.encode_utf16() {
                bytes.extend(if encoding == 1 {
                    word.to_le_bytes()
                } else {
                    word.to_be_bytes()
                });
            }
            bytes
        }
        _ => unreachable!(),
    }
}

fn elements(parser: &mut Parser) -> usize {
    let mut count = 0;
    while let Some(event) = parser.next_event().unwrap() {
        count += usize::from(matches!(event.kind, EventKind::ElementDeclaration { .. }));
    }
    count
}

#[test]
fn dtd_deferral_is_inherited_and_flushes_on_growth_disable_or_final_input() {
    for encoding in 0..3 {
        for enabled in [false, true] {
            for flush in 0..3 {
                let mut parent = Parser::new(Config::default());
                parent.set_reparse_deferral_enabled(enabled);
                let mut child = parent.external_child(None, None).unwrap();
                assert_eq!(child.reparse_deferral_enabled(), enabled);
                child
                    .feed(&encoded("<!ELEMENT document ANY>\n", encoding, true), false)
                    .unwrap();
                assert_eq!(elements(&mut child), 1);
                child
                    .feed(&encoded("<!ELEMENT ", encoding, false), false)
                    .unwrap();
                assert_eq!(elements(&mut child), 0);
                for _ in 0..100 {
                    child
                        .feed(&encoded(&"e".repeat(100), encoding, false), false)
                        .unwrap();
                    assert_eq!(elements(&mut child), 0);
                }
                child
                    .feed(&encoded(" ANY>\n", encoding, false), flush == 2)
                    .unwrap();
                let mut count = elements(&mut child);
                assert_eq!(count, usize::from(!enabled || flush == 2));
                if flush == 2 {
                    continue;
                }
                child.feed(&[], false).unwrap();
                assert_eq!(elements(&mut child), 0);
                if flush == 1 {
                    child.set_reparse_deferral_enabled(false);
                    child.feed(&[], false).unwrap();
                    count += elements(&mut child);
                } else {
                    for _ in 0..101 {
                        child
                            .feed(&encoded(&" ".repeat(100), encoding, false), false)
                            .unwrap();
                        count += elements(&mut child);
                    }
                }
                assert_eq!(count, 1);
                child
                    .feed(&encoded("<!ELEMENT after ANY>", encoding, false), true)
                    .unwrap();
                assert_eq!(elements(&mut child), 1);
            }
        }
    }
}

#[test]
fn attlist_events_finish_before_the_following_incomplete_dtd_token() {
    let parent = Parser::new(Config::default());
    let mut child = parent.external_child(None, None).unwrap();
    child
        .feed(b"<!ATTLIST r a CDATA 'A' b CDATA 'B'><!ELEMENT ", false)
        .unwrap();
    let mut names = Vec::new();
    while let Some(event) = child.next_event().unwrap() {
        if let EventKind::AttlistDeclaration(attribute) = event.kind {
            names.push(attribute.name.to_string());
            child.set_reparse_deferral_enabled(false);
            child.set_reparse_deferral_enabled(true);
        }
    }
    assert_eq!(names, ["a", "b"]);
    child.feed(&[b'e'; 100], false).unwrap();
    assert_eq!(elements(&mut child), 0);
    child.feed(b" ANY>", false).unwrap();
    assert_eq!(elements(&mut child), 0);
    child.feed(&[], true).unwrap();
    assert_eq!(elements(&mut child), 1);
}

#[test]
fn final_input_and_disabling_deferral_preserve_malformed_dtd_errors() {
    for final_input in [false, true] {
        let parent = Parser::new(Config::default());
        let mut child = parent.external_child(None, None).unwrap();
        child.feed(b"<!ELEMENT ", false).unwrap();
        assert_eq!(elements(&mut child), 0);
        child.feed(&[b'e'; 100], false).unwrap();
        assert_eq!(elements(&mut child), 0);
        child.feed(b" WRONG>", final_input).unwrap();
        if !final_input {
            assert_eq!(elements(&mut child), 0);
            child.set_reparse_deferral_enabled(false);
            child.feed(&[], false).unwrap();
        }
        assert_eq!(child.next_event().unwrap_err().kind, ErrorKind::Syntax);
    }
}

#[test]
fn token_limit_and_decoder_errors_bypass_pending_dtd_deferral() {
    for decoder_error in [false, true] {
        let mut config = Config::default();
        config.limits.max_token_bytes = 128;
        let parent = Parser::new(config);
        let mut child = parent.external_child(None, None).unwrap();
        child.feed(b"<!ELEMENT ", false).unwrap();
        assert_eq!(elements(&mut child), 0);
        child.feed(&[b'e'; 100], false).unwrap();
        assert_eq!(elements(&mut child), 0);
        if decoder_error {
            // A decoding failure must flush even though the decoded token has
            // not grown enough to trigger an ordinary reparse.
            child.feed(b"\xff", false).unwrap();
        } else {
            // 129 bytes exceeds the token limit without doubling the 110-byte
            // unfinished declaration recorded by the last scan.
            child.feed(&[b'e'; 19], false).unwrap();
        }
        let expected = if decoder_error {
            ErrorKind::InvalidToken
        } else {
            ErrorKind::LimitExceeded
        };
        let error = child.next_event().unwrap_err();
        assert_eq!(error.kind, expected);
        assert_eq!(child.next_event().unwrap_err(), error);
    }
}
