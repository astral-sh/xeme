use xeme::{Config, ErrorKind, Limits, Parser};

fn drain(parser: &mut Parser) -> Result<(), ErrorKind> {
    while parser.next_event().map_err(|error| error.kind)?.is_some() {}
    Ok(())
}

fn parse(input: &[u8], width: usize, factor: f32) -> Result<(), ErrorKind> {
    let mut parser = Parser::new(Config::default());
    assert!(parser.set_entity_maximum_amplification(factor));
    assert!(parser.set_entity_activation_threshold(0));
    let chunks = input.len().div_ceil(width);
    for (index, bytes) in input.chunks(width).enumerate() {
        parser.feed(bytes, index + 1 == chunks).unwrap();
        drain(&mut parser)?;
    }
    Ok(())
}

#[test]
fn unconsumed_trailing_input_cannot_dilute_entity_amplification() {
    let document = format!(
        "<!DOCTYPE r [<!ENTITY e '{}'>]><r>{}</r>",
        "a".repeat(100),
        "&e;".repeat(10)
    );
    for padding in [0, 64 * 1024] {
        let padded = document.clone() + &" ".repeat(padding);
        let utf16: Vec<_> = [0xfeff]
            .into_iter()
            .chain(padded.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        for input in [padded.as_bytes(), utf16.as_slice()] {
            for width in [1, 7, input.len()] {
                assert_eq!(parse(input, width, 1.2), Err(ErrorKind::LimitExceeded));
                assert_eq!(parse(input, width, 100.0), Ok(()));
            }
        }
    }
}

#[test]
fn attribute_and_default_expansion_use_the_shared_relative_limit() {
    for document in [
        format!(
            "<!DOCTYPE r [<!ENTITY e '{}'>]><r a='{}'/>",
            "a".repeat(100),
            "&e;".repeat(10)
        ),
        format!(
            "<!DOCTYPE r [<!ENTITY e '{}'><!ATTLIST r a CDATA '&e;'>]><r/>",
            "a".repeat(100)
        ),
        "<r>&amp;&lt;</r>".to_owned(),
    ] {
        for width in [1, 7, document.len()] {
            assert_eq!(
                parse(document.as_bytes(), width, 1.0),
                Err(ErrorKind::LimitExceeded)
            );
            assert_eq!(parse(document.as_bytes(), width, 100.0), Ok(()));
        }
    }
    // Numeric references do not add replacement bytes to Expat's relative count.
    assert_eq!(parse(b"<r>&#65;&#x41;</r>", 1, 1.0), Ok(()));
}

#[test]
fn child_counters_share_root_settings_and_outlive_the_parent() {
    let parent = Parser::new(Config::default());
    assert!(parent.set_entity_maximum_amplification(2.0));
    assert!(parent.set_entity_activation_threshold(0));
    let mut first = parent.external_child_with_encoding(Some(""), None).unwrap();
    let mut second = parent.external_child_with_encoding(Some(""), None).unwrap();
    assert!(!first.set_entity_maximum_amplification(100.0));
    assert!(!first.set_entity_activation_threshold(u64::MAX));
    drop(parent);
    // With no root bytes, Expat uses a 22-byte minimal external declaration.
    first.feed(&[b'a'; 22], true).unwrap();
    assert_eq!(drain(&mut first), Ok(()));
    second.feed(b"b", true).unwrap();
    assert_eq!(drain(&mut second), Err(ErrorKind::LimitExceeded));
}

#[test]
fn byte_order_marks_count_even_without_a_following_text_token() {
    for bytes in [b"\xef\xbb\xbf".as_slice(), b"\xff\xfe", b"\xfe\xff"] {
        for final_input in [false, true] {
            let parent = Parser::new(Config::default());
            assert!(parent.set_entity_maximum_amplification(1.0));
            assert!(parent.set_entity_activation_threshold(0));
            for (context, encoding) in [(Some(""), None), (None, None), (Some(""), Some("UTF-16"))]
            {
                if bytes == b"\xef\xbb\xbf" && encoding.is_some() {
                    continue;
                }
                let mut child = parent
                    .external_child_with_encoding(context, encoding)
                    .unwrap();
                let result = child
                    .feed(bytes, final_input)
                    .map_err(|error| error.kind)
                    .and_then(|()| drain(&mut child));
                assert_eq!(
                    result,
                    Err(ErrorKind::LimitExceeded),
                    "{bytes:?}, final={final_input}, context={context:?}"
                );
            }
        }
    }
}

#[test]
fn explicit_unknown_encoding_is_resolved_before_bom_accounting() {
    let parent = Parser::new(Config::default());
    assert!(parent.set_entity_maximum_amplification(1.0));
    assert!(parent.set_entity_activation_threshold(0));
    let mut child = parent
        .external_child_with_encoding(Some(""), Some("caller-defined"))
        .unwrap();
    child.feed(b"\xef\xbb\xbf<r/>", true).unwrap();
    assert_eq!(drain(&mut child), Err(ErrorKind::UnknownEncoding));
    assert_eq!(child.unknown_encoding(), Some("caller-defined"));
}

#[test]
fn relative_controls_do_not_disable_absolute_work_limits() {
    let mut parser = Parser::new(Config {
        limits: Limits {
            max_entity_expansion_bytes: 64,
            ..Limits::default()
        },
        ..Config::default()
    });
    assert!(parser.set_entity_maximum_amplification(f32::INFINITY));
    assert!(parser.set_entity_activation_threshold(u64::MAX));
    let document = format!("<!DOCTYPE r [<!ENTITY e '{}'>]><r>&e;</r>", "a".repeat(256));
    parser.feed(document.as_bytes(), true).unwrap();
    assert_eq!(drain(&mut parser), Err(ErrorKind::LimitExceeded));
}
