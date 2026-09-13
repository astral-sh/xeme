use super::{Config, Error, ErrorKind, EventKind, Parser};

#[derive(Clone, Copy, Debug)]
enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    SingleByte,
    MultiByte,
}

const ENCODINGS: [Encoding; 5] = [
    Encoding::Utf8,
    Encoding::Utf16Le,
    Encoding::Utf16Be,
    Encoding::SingleByte,
    Encoding::MultiByte,
];

impl Encoding {
    fn name(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf16Le => "UTF-16LE",
            Self::Utf16Be => "UTF-16BE",
            Self::SingleByte | Self::MultiByte => "custom",
        }
    }

    fn bytes(self, text: &str) -> Vec<u8> {
        match self {
            Self::Utf16Le => text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            Self::Utf16Be => text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
            _ => text.as_bytes().to_vec(),
        }
    }
}

fn parse(
    text: &str,
    encoding: Encoding,
    width: usize,
    empty_final: bool,
) -> (Parser, Option<Error>, String) {
    let input = encoding.bytes(text);
    let mut parser = Parser::new(Config {
        encoding: Some(encoding.name().into()),
        ..Config::default()
    });
    parser.set_default_events(true);
    let mut defaults = String::new();
    let chunks = input.chunks(width);
    let count = chunks.len();
    let feeds = chunks
        .enumerate()
        .map(|(index, chunk)| (chunk, !empty_final && index + 1 == count))
        .chain(empty_final.then_some((&[][..], true)));
    for (chunk, final_input) in feeds {
        if let Err(error) = parser.feed(chunk, final_input) {
            return (parser, Some(error), defaults);
        }
        loop {
            match parser.next_event() {
                Ok(Some(event)) => {
                    if matches!(event.kind, EventKind::Default) {
                        defaults.push_str(parser.current_raw().unwrap());
                    }
                }
                Ok(None) => break,
                Err(error) if error.kind == ErrorKind::UnknownEncoding => {
                    let mut map = std::array::from_fn(|index| index as i32);
                    if matches!(encoding, Encoding::MultiByte) {
                        map[0x80] = -2;
                        parser.set_multibyte_encoding_map("custom", map).unwrap();
                    } else {
                        parser.set_encoding_map("custom", map).unwrap();
                    }
                }
                Err(error) => return (parser, Some(error), defaults),
            }
        }
    }
    (parser, None, defaults)
}

#[test]
fn prolog_newline_lookahead_inspections_are_linear() {
    for encoding in ENCODINGS {
        for newline in ["\n", "\r", "\r\n", " \t\n"] {
            for count in [1024, 4096, 8192] {
                let prefix = newline.repeat(count);
                let (parser, error, defaults) =
                    parse(&format!("{prefix}<r/>"), encoding, usize::MAX, false);
                assert!(error.is_none(), "{encoding:?}: {error:?}");
                assert!(parser.is_finished());
                assert_eq!(defaults, prefix);
                assert!(
                    parser.prolog_bytes_inspected <= 2 * prefix.len(),
                    "{encoding:?}, {newline:?}, {count}: {} inspections",
                    parser.prolog_bytes_inspected
                );
            }
        }
    }
}

#[test]
fn converted_prolog_space_lookahead_inspections_are_linear() {
    for encoding in ENCODINGS {
        let prefix = " \t".repeat(4096);
        let (parser, error, defaults) =
            parse(&format!("{prefix}<r/>"), encoding, usize::MAX, false);
        assert!(error.is_none(), "{encoding:?}: {error:?}");
        assert!(parser.is_finished());
        assert_eq!(defaults, prefix);
        assert!(
            parser.prolog_bytes_inspected <= 2 * prefix.len(),
            "{encoding:?}: {} inspections",
            parser.prolog_bytes_inspected
        );
    }
}

#[test]
fn prolog_literal_prefixes_preserve_encoded_boundaries() {
    let prefixes = [
        "\r".to_owned(),
        "\r\n".to_owned(),
        " \t".to_owned(),
        " \r\n\t".to_owned(),
        format!("{}\r", " ".repeat(1023)),
        " ".repeat(1024),
        format!("{}\r", " ".repeat(2048)),
    ];
    for encoding in ENCODINGS {
        for prefix in &prefixes {
            for (suffix, kind, offset) in [
                ("'", ErrorKind::UnclosedToken, 0),
                ("\"", ErrorKind::UnclosedToken, 0),
                ("''", ErrorKind::Syntax, 0),
                ("'x'x", ErrorKind::InvalidToken, 3),
                ("'x' ", ErrorKind::Syntax, 0),
            ] {
                let text = format!("{prefix}{suffix}");
                for width in [1, 1023, 1024, 1025, usize::MAX] {
                    for empty_final in [false, true] {
                        let (_, error, defaults) = parse(&text, encoding, width, empty_final);
                        let error = error.expect("invalid prolog must fail");
                        assert_eq!(error.kind, kind, "{encoding:?}, {width}, {empty_final}");
                        assert_eq!(
                            error.position.byte_index,
                            encoding.bytes(&text[..prefix.len() + offset]).len(),
                            "{encoding:?}, {width}, {empty_final}"
                        );
                        assert_eq!(defaults, *prefix);
                    }
                }
            }
        }
    }
}
