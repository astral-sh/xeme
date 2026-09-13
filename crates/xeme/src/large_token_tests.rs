//! Count bytes inspected by resumable token scans, independently of elapsed time.

use std::cell::Cell;

use super::{Config, Parser};

thread_local! {
    static INSPECTED: Cell<usize> = const { Cell::new(0) };
}

pub(super) fn inspect(bytes: usize) {
    INSPECTED.with(|count| count.set(count.get() + bytes));
}

#[test]
fn large_tokens_take_linear_scanning_work_across_small_feeds() {
    for size in [4096, 16384] {
        let name = "n".repeat(size);
        let value = "é𐀀".repeat(size / 6);
        let documents = [
            format!("<{name}/>"),
            format!("<r {name}='value'/>"),
            format!("<r a='{value}'/>"),
            format!("<{name}></{name}>"),
            format!("<!--{value}--><r/>"),
            format!("<?p {value}?><r/>"),
            format!("<?xml version='1.0' {}?><r/>", " ".repeat(size)),
            format!("<!DOCTYPE r SYSTEM '{name}'><r/>"),
            format!("<!DOCTYPE r [<!ELEMENT {name} ANY>]><r/>"),
            format!("<!DOCTYPE r [<!ENTITY a '{value}'>]><r>&a;</r>"),
            format!("<!DOCTYPE r [<!ENTITY {name} 'v'>]><r>&{name};</r>"),
            format!("<!DOCTYPE r []{}><r/>", " ".repeat(size)),
        ];
        for (case, document) in documents.iter().enumerate() {
            for encoding in ["UTF-8", "UTF-16LE", "UTF-16BE"] {
                let bytes: Vec<u8> = match encoding {
                    "UTF-16LE" => document.encode_utf16().flat_map(u16::to_le_bytes).collect(),
                    "UTF-16BE" => document.encode_utf16().flat_map(u16::to_be_bytes).collect(),
                    _ => document.as_bytes().to_vec(),
                };
                // Deferral is enabled by default. Disabling callback deferral
                // must still retain lexical progress across incomplete tokens.
                for deferral in [true, false] {
                    for width in [1, 31, 1024] {
                        let mut parser = Parser::new(Config {
                            // Automatic detection must also resume over a large
                            // UTF-8 XML declaration before its encoding is known.
                            encoding: (encoding != "UTF-8").then(|| encoding.to_owned()),
                            ..Config::default()
                        });
                        assert!(parser.reparse_deferral_enabled());
                        parser.set_reparse_deferral_enabled(deferral);
                        INSPECTED.with(|count| count.set(0));
                        for chunk in bytes.chunks(width) {
                            parser.feed(chunk, false).unwrap();
                            while parser.next_event().unwrap().is_some() {}
                        }
                        // Flush a completed token still waiting on geometric
                        // growth, including an empty final UTF-16 feed.
                        parser.feed(&[], true).unwrap();
                        while parser.next_event().unwrap().is_some() {}
                        assert!(parser.is_finished());
                        let inspected = INSPECTED.with(Cell::get);
                        assert!(
                            inspected >= size / 2,
                            "large token must exercise a measured scan"
                        );
                        assert!(
                            inspected <= 12 * document.len(),
                            "case={case}, size={size}, encoding={encoding}, \
                             deferral={deferral}, width={width}: \
                             {inspected} token byte inspections for {} decoded bytes",
                            document.len(),
                        );
                    }
                }
            }
        }
    }
}
