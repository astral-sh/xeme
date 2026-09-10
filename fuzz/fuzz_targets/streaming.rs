#![no_main]

use libfuzzer_sys::fuzz_target;
use oriole::{Config, EventKind, Limits, NameRules, Parser};

fn parse(
    data: &[u8],
    chunk: usize,
    namespaces: bool,
    name_rules: NameRules,
) -> Result<Vec<EventKind>, ()> {
    let config = Config {
        namespace_separator: namespaces.then_some('|'),
        name_rules,
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
    let mut text = None;
    for piece in data.chunks(chunk) {
        parser.feed(piece, false).map_err(|_| ())?;
        drain(&mut parser, &mut events, &mut text)?;
    }
    parser.feed(&[], true).map_err(|_| ())?;
    drain(&mut parser, &mut events, &mut text)?;
    flush_text(&parser, &mut events, &mut text)?;
    Ok(events)
}

// Accumulate character data across input chunks with amortized growth, then
// freeze one immutable Text payload when the next nontext event arrives.
fn flush_text(
    parser: &Parser,
    events: &mut Vec<EventKind>,
    pending: &mut Option<String>,
) -> Result<(), ()> {
    if let Some(text) = pending.take() {
        let text = oriole::Text::try_from_str_in(&text, parser.allocator()).map_err(|_| ())?;
        events.push(EventKind::Text(text));
    }
    Ok(())
}

fn drain(
    parser: &mut Parser,
    events: &mut Vec<EventKind>,
    pending: &mut Option<String>,
) -> Result<(), ()> {
    while let Some(event) = parser.next_event().map_err(|_| ())? {
        if let EventKind::Text(text) = event.kind {
            let previous = pending.get_or_insert_with(String::new);
            previous.try_reserve(text.len()).map_err(|_| ())?;
            previous.push_str(&text);
        } else {
            flush_text(parser, events, pending)?;
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
    let name_rules = if control & 64 != 0 {
        NameRules::FourthEdition
    } else {
        NameRules::FifthEdition
    };
    let contiguous = parse(xml, xml.len().max(1), namespaces, name_rules);
    let incremental = parse(xml, usize::from(control & 63) + 1, namespaces, name_rules);
    assert_eq!(
        contiguous, incremental,
        "chunking changed acceptance or normalized events"
    );
});
