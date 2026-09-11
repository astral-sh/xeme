use super::*;

/// Reach a native Text delivery crossing the existing 64 KiB discard boundary.
fn pending_discard(suffix: &str, final_input: bool) -> (Parser, AdapterFrame, NativeLocation) {
    let input = format!("<r>{}\r\n<n>éé{suffix}", "x".repeat(65_525));
    let mut parser = Parser::new(Config::default());
    parser.feed(input.as_bytes(), final_input).unwrap();
    let mut frame = parser.adapter_frame();
    let mut event = None;
    for _ in 0..4 {
        parser
            .next_event_for_c_coordinates_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
    }
    assert!(event.is_none());
    assert_eq!(frame.native_text_range_for_c(), Some((65_533, 4)));
    let AdapterLocation::Native(native) = frame.location_for_c() else {
        panic!("bounded native Text stays unresolved");
    };
    assert_eq!(
        parser.position(),
        Position {
            byte_index: 65_533,
            byte_count: 4,
            line: 2,
            column: 3,
        }
    );
    assert!(parser.sources[0].compaction_due());
    assert_eq!(parser.sources[0].text.len(), input.len());
    (parser, frame, native)
}

#[test]
fn next_delivery_discards_after_saving_the_committed_event_even_when_it_fails() {
    let (mut parser, mut frame, native) = pending_discard("</m></r>", true);
    let expected = parser.position();
    let mut event = None;
    let error = parser
        .next_event_for_c_coordinates_into(&mut event, &mut frame)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::TagMismatch);
    assert_eq!(error.position.byte_index, 65_539);
    assert!(!frame.is_active());
    assert!(event.is_none());
    assert!(!parser.sources[0].compaction_due());
    assert_eq!(parser.sources[0].text.as_str(), "</m></r>");
    assert_eq!(parser.position(), expected);
    assert_eq!(parser.resolve_native_location_for_c(native), Some(expected));
    assert_eq!(parser.resolve_native_location_for_c(native), Some(expected));
    parser.finish_adapter_frame(frame);
}

#[test]
fn feed_discards_before_appending_and_keeps_the_old_c_publication_resolvable() {
    let (mut parser, mut frame, native) = pending_discard("", false);
    let expected = parser.position();
    let allocation = parser.sources[0].text.as_ptr();
    parser.feed(b"</n></r>", true).unwrap();
    assert!(!parser.sources[0].compaction_due());
    assert_eq!(parser.sources[0].text.as_str(), "</n></r>");
    // Discard retains capacity, so this short append needs no larger backing block.
    assert_eq!(parser.sources[0].text.as_ptr(), allocation);
    assert_eq!(parser.resolve_native_location_for_c(native), Some(expected));
    let mut event = None;
    parser
        .next_event_for_c_coordinates_into(&mut event, &mut frame)
        .unwrap()
        .unwrap();
    assert_eq!(parser.resolve_native_location_for_c(native), None);
    while parser
        .next_event_for_c_coordinates_into(&mut event, &mut frame)
        .unwrap()
        .is_some()
    {}
    parser.finish_adapter_frame(frame);
}
