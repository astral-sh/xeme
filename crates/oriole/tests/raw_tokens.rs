use oriole::{Config, ErrorKind, Parser};

#[test]
fn terminal_errors_retain_complete_raw_markup_in_every_feed_boundary() {
    let cases = [
        ("<r>", "<!--a--b-->", ErrorKind::InvalidToken),
        (
            "<r>",
            "<?xml version='1.0'?>",
            ErrorKind::MisplacedXmlDeclaration,
        ),
        ("", "<?XML?>", ErrorKind::InvalidToken),
        ("<r>", "<?XmL encoding='UTF8'?>", ErrorKind::InvalidToken),
        ("<r>", "<n a='1' a='2'/>", ErrorKind::DuplicateAttribute),
        ("<r>", "<missing:n/>", ErrorKind::UndefinedPrefix),
        ("<r>", "</wrong>", ErrorKind::TagMismatch),
        (
            "<!--previous-->",
            "<!DOCTYPE r PUBLIC '{}' 'x'>",
            ErrorKind::PublicId,
        ),
        (
            "<!--previous-->",
            "<!DOCTYPE broken! [",
            ErrorKind::InvalidToken,
        ),
    ];
    for (prefix, token, expected) in cases {
        let input = format!("{prefix}{token}");
        for utf16 in [false, true] {
            let bytes = if utf16 {
                [
                    b"\xff\xfe".as_slice(),
                    &input
                        .encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect::<Vec<_>>(),
                ]
                .concat()
            } else {
                input.as_bytes().to_vec()
            };
            for chunk in 1..=bytes.len() {
                let mut parser = Parser::new(Config {
                    namespace_separator: Some('|'),
                    ..Config::default()
                });
                let mut found = false;
                for (index, part) in bytes.chunks(chunk).enumerate() {
                    parser
                        .feed(part, (index + 1) * chunk >= bytes.len())
                        .unwrap();
                    loop {
                        match parser.next_event() {
                            Ok(Some(_)) => {}
                            Ok(None) => break,
                            Err(error) => {
                                assert_eq!(
                                    error.kind, expected,
                                    "{input:?}, UTF16={utf16}, chunk={chunk}"
                                );
                                assert_eq!(
                                    parser.current_raw(),
                                    Some(token),
                                    "{input:?}, UTF16={utf16}, chunk={chunk}"
                                );
                                assert_eq!(parser.next_event().unwrap_err(), error);
                                assert_eq!(parser.current_raw(), Some(token));
                                found = true;
                                break;
                            }
                        }
                    }
                    if found {
                        break;
                    }
                }
                assert!(found);
            }
        }
    }
}
