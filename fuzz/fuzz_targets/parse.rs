#![no_main]

use libfuzzer_sys::fuzz_target;
use oriole::{Config, Limits, Parser};

fuzz_target!(|data: &[u8]| {
    let mut parser = Parser::new(Config {
        limits: Limits {
            max_total_bytes: 65_536,
            max_token_bytes: 65_536,
            max_entity_expansion_bytes: 65_536,
            max_depth: 64,
            ..Limits::default()
        },
        ..Config::default()
    });
    if parser.feed(data, true).is_ok() {
        while let Ok(Some(_)) = parser.next_event() {}
    }
});
