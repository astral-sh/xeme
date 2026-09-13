use xeme::{Config, ErrorKind, Parser};

#[test]
fn token_limits_report_the_containing_character_in_original_input() {
    for (prefix, suffix, dtd) in [
        ("<r a='", "'/>", false),
        ("<!--", "--><r/>", false),
        ("<?p ", "?><r/>", false),
        ("<!DOCTYPE r SYSTEM '", "'><r/>", false),
        ("<!ELEMENT ", " ANY>", true),
    ] {
        for scalar in ['é', '€', '𐀀'] {
            for mode in 0..5 {
                if (mode >= 2 && u32::from(scalar) > 0xffff) || (mode == 4 && scalar != 'é') {
                    continue;
                }
                let xml = format!("\r\n{prefix}{scalar}{suffix}");
                let input = match mode {
                    0 => xml.as_bytes().to_vec(),
                    1 => std::iter::once(0xfeff)
                        .chain(xml.encode_utf16())
                        .flat_map(u16::to_le_bytes)
                        .collect(),
                    4 => xml.chars().map(|c| c as u8).collect(),
                    _ => xml
                        .chars()
                        .flat_map(|c| {
                            if c == scalar {
                                if mode == 2 { vec![128] } else { vec![128, 129] }
                            } else {
                                vec![c as u8]
                            }
                        })
                        .collect(),
                };
                for inside in 1..scalar.len_utf8() {
                    for width in [1, 7, input.len()] {
                        for deferral in [false, true] {
                            let mut config = Config::default();
                            config.limits.max_token_bytes = prefix.len() + inside;
                            let encoding = match mode {
                                2 | 3 => Some("custom"),
                                4 => Some("ISO-8859-1"),
                                _ => None,
                            };
                            config.encoding = encoding.map(str::to_owned);
                            let parent = Parser::new(config);
                            let mut parser = if dtd {
                                parent.external_child_with_encoding(None, encoding).unwrap()
                            } else {
                                parent
                            };
                            parser.set_reparse_deferral_enabled(deferral);
                            let mut failure = None;
                            'input: for (index, bytes) in input.chunks(width).enumerate() {
                                parser
                                    .feed(bytes, (index + 1) * width >= input.len())
                                    .unwrap();
                                loop {
                                    match parser.next_event() {
                                        Ok(Some(_)) => continue,
                                        Ok(None) => {
                                            if parser.encoding_conversion().is_some() {
                                                parser
                                                    .resolve_encoding_conversion(scalar as i32)
                                                    .unwrap();
                                                continue;
                                            }
                                            break;
                                        }
                                        Err(error)
                                            if error.kind == ErrorKind::UnknownEncoding
                                                && matches!(mode, 2 | 3) =>
                                        {
                                            let mut map = std::array::from_fn(|byte| byte as i32);
                                            map[128] = if mode == 2 { scalar as i32 } else { -2 };
                                            if mode == 2 {
                                                parser.set_encoding_map("custom", map)
                                            } else {
                                                parser.set_multibyte_encoding_map("custom", map)
                                            }
                                            .unwrap();
                                        }
                                        Err(error) => {
                                            failure = Some(error);
                                            break 'input;
                                        }
                                    }
                                }
                            }
                            let error = failure.expect("oversized token must fail");
                            assert_eq!(error.kind, ErrorKind::LimitExceeded);
                            let index = if mode == 1 {
                                2 + 2 * (2 + prefix.len())
                            } else {
                                2 + prefix.len()
                            };
                            assert_eq!(
                                (
                                    error.position.byte_index,
                                    error.position.line,
                                    error.position.column
                                ),
                                (index, 2, prefix.len()),
                                "{prefix:?}, {scalar}, mode={mode}, inside={inside}, width={width}, deferral={deferral}"
                            );
                            assert_eq!(parser.next_event().unwrap_err(), error);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn character_safe_end_tags_and_streamed_cdata_keep_their_limit_behavior() {
    for width in [1, 7, 64] {
        for (xml, limit, expected) in [("<r></é>", 3, Some(5)), ("<r><![CDATA[é€𐀀]]></r>", 4, None)]
        {
            let mut config = Config::default();
            config.limits.max_token_bytes = limit;
            let mut parser = Parser::new(config);
            let mut failure = None;
            'input: for (index, bytes) in xml.as_bytes().chunks(width).enumerate() {
                parser
                    .feed(bytes, (index + 1) * width >= xml.len())
                    .unwrap();
                loop {
                    match parser.next_event() {
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(error) => {
                            assert_eq!(error.kind, ErrorKind::LimitExceeded);
                            failure = Some(error.position.byte_index);
                            break 'input;
                        }
                    }
                }
            }
            assert_eq!(failure, expected);
            if failure.is_none() {
                assert!(parser.is_finished());
            }
        }
    }
}
