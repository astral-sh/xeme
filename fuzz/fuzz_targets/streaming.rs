#![no_main]

use libfuzzer_sys::fuzz_target;
use oriole::{Config, EventKind, Limits, Parser};

fn parse(data: &[u8], chunk: usize, namespaces: bool) -> Result<Vec<EventKind>, ()> {
    let config = Config {
        namespace_separator: namespaces.then_some('|'),
        limits: Limits {
            max_total_bytes: 65_536,
            max_token_bytes: 65_536,
            max_entity_expansion_bytes: 65_536,
            max_depth: 64,
            ..Limits::default()
        },
        ..Config::default()
    };
    let mut parser = Parser::new(config);
    let mut events = Vec::new();
    for piece in data.chunks(chunk) {
        parser.feed(piece, false).map_err(|_| ())?;
        drain(&mut parser, &mut events)?;
    }
    parser.feed(&[], true).map_err(|_| ())?;
    drain(&mut parser, &mut events)?;
    Ok(events)
}

fn drain(parser: &mut Parser, events: &mut Vec<EventKind>) -> Result<(), ()> {
    while let Some(event) = parser.next_event().map_err(|_| ())? {
        if let EventKind::Text(text) = event.kind {
            if let Some(EventKind::Text(previous)) = events.last_mut() {
                previous.try_push_str(&text).map_err(|_| ())?;
            } else {
                events.push(EventKind::Text(text));
            }
        } else {
            events.push(event.kind);
        }
    }
    Ok(())
}

fuzz_target!(|data: &[u8]| {
    let Some((&control, xml)) = data.split_first() else {
        return;
    };
    if xml.len() > 65_536 {
        return;
    }
    let namespaces = control & 128 != 0;
    let contiguous = parse(xml, xml.len().max(1), namespaces);
    let incremental = parse(xml, usize::from(control & 127) + 1, namespaces);
    assert_eq!(
        contiguous, incremental,
        "chunking changed acceptance or normalized events"
    );
});
