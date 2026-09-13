use xeme::{Config, ErrorKind, Limits, Parser};

#[test]
fn cumulative_input_limit_stops_streamed_text_without_entities() {
    // Model decompressor output without allocating the entire expanded document.
    // Draining each chunk must not reset the cumulative input allowance.
    const LIMIT: usize = 4096;
    for width in [1, 7, LIMIT] {
        let mut parser = Parser::new(Config {
            limits: Limits {
                max_total_bytes: LIMIT,
                ..Limits::default()
            },
            ..Config::default()
        });
        parser.feed(b"<r>", false).unwrap();
        while parser.next_event().unwrap().is_some() {}

        let chunk = vec![b'x'; width];
        let mut remaining = LIMIT - 3;
        while remaining != 0 {
            let count = remaining.min(width);
            parser.feed(&chunk[..count], false).unwrap();
            while parser.next_event().unwrap().is_some() {}
            remaining -= count;
        }

        let error = parser.feed(b"x", false).unwrap_err();
        assert_eq!(error.kind, ErrorKind::LimitExceeded);
        assert_eq!(error.message, "input byte limit exceeded");
        assert_eq!(parser.next_event().unwrap_err(), error);
        assert_eq!(parser.feed(b"</r>", true).unwrap_err(), error);
    }
}
