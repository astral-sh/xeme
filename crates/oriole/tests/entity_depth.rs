use std::fmt::Write;

use oriole::{Config, ErrorKind, EventKind, Limits, Parser};

fn config(depth: usize) -> Config {
    Config {
        limits: Limits {
            max_entity_depth: depth + 4,
            max_entities: depth + 4,
            ..Limits::default()
        },
        ..Config::default()
    }
}

fn drain(
    parser: &mut Parser,
    text: &mut String,
    values: &mut Vec<String>,
) -> Result<(), ErrorKind> {
    while let Some(event) = parser.next_event().map_err(|error| error.kind)? {
        match event.kind {
            EventKind::Text(value) => text.push_str(&value),
            EventKind::EntityDeclaration(declaration) if declaration.name == "g" => {
                values.push(declaration.value.as_deref().unwrap_or("").to_owned());
            }
            EventKind::ExternalEntityReference(reference) => {
                assert_eq!(reference.system_id.as_deref(), Some("empty"));
                // The safe API permits rekeying while a value continuation owns
                // frames; every retained index must preserve its membership.
                parser.set_hash_salt([31; 16]).unwrap();
                let mut child = parser.external_child(None, None).unwrap();
                child.feed(b"", true).unwrap();
                while child.next_event().unwrap().is_some() {}
                parser.merge_external_subset(&child).unwrap();
            }
            _ => {}
        }
    }
    Ok(())
}

#[test]
fn sixty_thousand_content_sources_use_a_small_host_stack_and_release_membership() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            const DEPTH: usize = 60_000;
            let mut xml = String::from("<!DOCTYPE r [<!ENTITY s0 'deep'>");
            for i in 1..DEPTH {
                write!(xml, "<!ENTITY s{i} '&s{};'>", i - 1).unwrap();
            }
            write!(xml, "]><r>&s{};&s0;&s{};</r>", DEPTH - 1, DEPTH - 1).unwrap();
            let mut parser = Parser::new(config(DEPTH));
            let mut text = String::new();
            for chunk in xml.as_bytes().chunks(4096) {
                parser.feed(chunk, false).unwrap();
                drain(&mut parser, &mut text, &mut Vec::new()).unwrap();
            }
            parser.feed(b"", true).unwrap();
            drain(&mut parser, &mut text, &mut Vec::new()).unwrap();
            assert_eq!(text, "deepdeepdeep");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn seventy_thousand_delayed_parameter_values_preserve_output_and_pop_names() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            const DEPTH: usize = 70_000;
            let mut xml = String::from("<!DOCTYPE r [<!ENTITY % s0 'deep'>");
            for i in 1..DEPTH {
                write!(xml, "<!ENTITY % s{i} '&#37;s{};'>", i - 1).unwrap();
            }
            write!(
                xml,
                "<!ENTITY % define_g \"<!ENTITY g '&#37;s{};&#37;s0;'>\">%define_g;]><r>&g;</r>",
                DEPTH - 1
            )
            .unwrap();
            let mut parser = Parser::new(config(DEPTH));
            parser.set_param_entity_parsing(2);
            parser.feed(xml.as_bytes(), true).unwrap();
            let mut text = String::new();
            let mut values = Vec::new();
            drain(&mut parser, &mut text, &mut values).unwrap();
            assert_eq!(text, "deepdeep");
            assert_eq!(values, ["deepdeep"]);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn parameter_sources_and_owned_value_continuations_release_completed_names() {
    const DEPTH: usize = 4096;
    for owned_value in [false, true] {
        let mut xml = String::from("<!DOCTYPE r [<!ENTITY % s0 'deep'>");
        for i in 1..DEPTH {
            write!(xml, "<!ENTITY % s{i} '&#37;s{};'>", i - 1).unwrap();
        }
        if owned_value {
            xml.push_str("<!ENTITY % external SYSTEM 'empty'>");
            write!(xml, "<!ENTITY % define_g \"<!ENTITY g '&#37;external;&#37;s{};&#37;s0;'>\">%define_g;]><r>&g;</r>", DEPTH - 1).unwrap();
        } else {
            // A separate between-declaration chain expands complete comments.
            xml.push_str("<!ENTITY % c0 '<!--leaf-->'>");
            for i in 1..DEPTH {
                write!(xml, "<!ENTITY % c{i} '&#37;c{};'>", i - 1).unwrap();
            }
            write!(xml, "%c{};%c0;]><r/>", DEPTH - 1).unwrap();
        }
        let mut cfg = config(DEPTH * 2);
        cfg.limits.max_entity_depth = DEPTH + 4;
        let mut parser = Parser::new(cfg);
        parser.set_param_entity_parsing(2);
        let mut text = String::new();
        let mut values = Vec::new();
        for chunk in xml.as_bytes().chunks(7) {
            parser.feed(chunk, false).unwrap();
            drain(&mut parser, &mut text, &mut values).unwrap();
        }
        parser.feed(b"", true).unwrap();
        drain(&mut parser, &mut text, &mut values).unwrap();
        if owned_value {
            assert_eq!(text, "deepdeep");
            assert_eq!(values, ["deepdeep"]);
        }
    }
}

#[test]
fn deep_cycles_and_unbalanced_replacements_remain_terminal() {
    for (definitions, body, error) in [
        (
            "<!ENTITY a '&b;'><!ENTITY b '&a;'>",
            "<r>&a;</r>",
            ErrorKind::RecursiveEntityReference,
        ),
        (
            "<!ENTITY a '<x>'>",
            "<r>&a;</r>",
            ErrorKind::AsynchronousEntity,
        ),
        (
            "<!ENTITY % a '&#37;b;'><!ENTITY % b '&#37;a;'>%a;",
            "<r/>",
            ErrorKind::RecursiveEntityReference,
        ),
        (
            "<!ENTITY % a '&#37;b;'><!ENTITY % b '&#37;a;'><!ENTITY % d \"<!ENTITY g '&#37;a;'>\">%d;",
            "<r/>",
            ErrorKind::RecursiveEntityReference,
        ),
    ] {
        let xml = format!("<!DOCTYPE r [{definitions}]>{body}");
        for width in [1, 7, xml.len()] {
            let mut parser = Parser::new(Config::default());
            parser.set_param_entity_parsing(2);
            let mut result = Ok(());
            for bytes in xml.as_bytes().chunks(width) {
                parser.feed(bytes, false).unwrap();
                result = drain(&mut parser, &mut String::new(), &mut Vec::new());
                if result.is_err() {
                    break;
                }
            }
            if result.is_ok() {
                // A complete declaration may still await a deferred reparse.
                parser.feed(b"", true).unwrap();
                result = drain(&mut parser, &mut String::new(), &mut Vec::new());
            }
            assert_eq!(result, Err(error));
            assert_eq!(parser.next_event().unwrap_err().kind, error);
        }
    }
}

#[test]
fn changing_hash_salt_preserves_live_source_and_inherited_membership() {
    let mut parser = Parser::new(Config::default());
    parser
        .feed(
            b"<!DOCTYPE r [<!ENTITY a '<!--pause-->&b;'><!ENTITY b '&a;'>]><r>&a;</r>",
            true,
        )
        .unwrap();
    loop {
        if matches!(
            parser.next_event().unwrap().unwrap().kind,
            EventKind::Comment(_)
        ) {
            break;
        }
    }
    parser.set_hash_salt([17; 16]).unwrap();
    assert_eq!(
        drain(&mut parser, &mut String::new(), &mut Vec::new()),
        Err(ErrorKind::RecursiveEntityReference)
    );

    let mut parent = Parser::new(Config::default());
    parent
        .feed(b"<!DOCTYPE r [<!ENTITY a 'unused'>]><r/>", true)
        .unwrap();
    drain(&mut parent, &mut String::new(), &mut Vec::new()).unwrap();
    let mut child = parent.external_child(Some("a\u{c}a"), None).unwrap();
    child.set_hash_salt([29; 16]).unwrap();
    child.feed(b"&a;", true).unwrap();
    assert_eq!(
        child.next_event().unwrap_err().kind,
        ErrorKind::RecursiveEntityReference
    );
}

#[test]
fn resolving_unknown_encoding_keeps_inherited_recursion_membership() {
    let mut parent = Parser::new(Config::default());
    parent
        .feed(
            b"<!DOCTYPE r [<!ENTITY a 'incorrectly-expanded'>]><r/>",
            true,
        )
        .unwrap();
    drain(&mut parent, &mut String::new(), &mut Vec::new()).unwrap();
    let mut child = parent
        .external_child_with_encoding(Some("a"), Some("custom"))
        .unwrap();
    child.feed(b"&a;", true).unwrap();
    assert_eq!(
        child.next_event().unwrap_err().kind,
        ErrorKind::UnknownEncoding
    );
    child
        .set_encoding_map("custom", std::array::from_fn(|index| index as i32))
        .unwrap();
    assert_eq!(
        child.next_event().unwrap_err().kind,
        ErrorKind::RecursiveEntityReference
    );
}

#[test]
fn external_children_preserve_active_general_entity_depth() {
    fn feed(
        parser: &mut Parser,
        input: &[u8],
        width: usize,
        nested: bool,
        text: &mut String,
    ) -> Result<(), ErrorKind> {
        for chunk in input.chunks(width).chain(std::iter::once(&[][..])) {
            parser.feed(chunk, chunk.is_empty()).unwrap();
            loop {
                let event = match parser.next_event() {
                    Ok(Some(event)) => event,
                    Ok(None) => break,
                    Err(error) => {
                        assert_eq!(parser.next_event().unwrap_err(), error);
                        return Err(error.kind);
                    }
                };
                match event.kind {
                    EventKind::Text(value) => text.push_str(&value),
                    EventKind::ExternalEntityReference(reference) => {
                        let input = match reference.system_id.as_deref() {
                            Some("first") if nested => b"&middle;".as_slice(),
                            Some("first" | "second") => b"&inner;",
                            other => panic!("unexpected external entity {other:?}"),
                        };
                        let mut child = parser
                            .external_child(reference.context.as_deref(), None)
                            .map_err(|error| error.kind)?;
                        feed(&mut child, input, width, nested, text)?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    let document = b"<!DOCTYPE r [<!ENTITY inner 'v'><!ENTITY outer '&first;'><!ENTITY first SYSTEM 'first'><!ENTITY middle '&second;'><!ENTITY second SYSTEM 'second'>]><r>&outer;&outer;</r>";
    for (nested, required_depth) in [(false, 3), (true, 5)] {
        for depth in [required_depth - 1, required_depth] {
            for width in [1, 7, document.len()] {
                let mut parser = Parser::new(Config {
                    limits: Limits {
                        max_entity_depth: depth,
                        ..Limits::default()
                    },
                    ..Config::default()
                });
                let mut text = String::new();
                let result = feed(&mut parser, document, width, nested, &mut text);
                if depth < required_depth {
                    assert_eq!(result, Err(ErrorKind::LimitExceeded));
                    assert!(text.is_empty());
                } else {
                    assert_eq!(result, Ok(()));
                    assert_eq!(text, "vv");
                }
            }
        }
    }
}
