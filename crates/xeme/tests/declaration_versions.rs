use xeme::{Config, ErrorKind, Parser};

#[test]
fn version_grammar_is_shared_by_documents_text_declarations_and_bootstrap() {
    for context in 0..3 {
        for version in [
            "1.0", "1.2", "1.01", "banana", "2.0", "1", "1.", "", "a b", "1.١",
        ] {
            for encoding in ["UTF-8", "custom"] {
                let suffix = match context {
                    0 => "<r/>",
                    1 => "text",
                    _ => "<!ELEMENT r EMPTY>",
                };
                let input = format!("<?xml version='{version}' encoding='{encoding}'?>{suffix}");
                for width in [1, 7, input.len()] {
                    let config = Config::default();
                    let parent = Parser::new(config.clone());
                    let mut parser = match context {
                        0 => Parser::new(config),
                        1 => parent.external_child(Some(""), None).unwrap(),
                        _ => parent.external_child(None, None).unwrap(),
                    };
                    let result = (|| {
                        for (index, part) in input.as_bytes().chunks(width).enumerate() {
                            parser.feed(part, (index + 1) * width >= input.len())?;
                            while parser.next_event()?.is_some() {}
                        }
                        Ok::<_, xeme::Error>(())
                    })()
                    .map_err(|error| error.kind);
                    let valid = matches!(version, "1.0" | "1.2" | "1.01");
                    let expected = if encoding == "custom" && !version.is_ascii() {
                        // Bootstrap cannot interpret custom-encoded non-ASCII bytes.
                        Err(ErrorKind::UnknownEncoding)
                    } else if !valid {
                        Err(if context == 0 {
                            ErrorKind::XmlDeclaration
                        } else {
                            ErrorKind::TextDeclaration
                        })
                    } else if encoding == "custom" {
                        Err(ErrorKind::UnknownEncoding)
                    } else {
                        Ok(())
                    };
                    assert_eq!(
                        result, expected,
                        "context={context}, {input}, width={width}"
                    );
                }
            }
        }
    }
}
