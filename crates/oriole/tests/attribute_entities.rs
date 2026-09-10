use std::fmt::Write;

use oriole::{Config, ErrorKind, EventKind, Limits, Parser};

fn attribute(document: &str, config: Config) -> Result<std::string::String, ErrorKind> {
    let mut parser = Parser::new(config);
    parser.feed(document.as_bytes(), true).unwrap();
    let mut result = None;
    loop {
        match parser.next_event() {
            Ok(Some(event)) => {
                if let EventKind::StartElement { attributes, .. } = event.kind {
                    result = Some(attributes[0].value.as_str().to_owned());
                }
            }
            Ok(None) => return Ok(result.unwrap()),
            Err(error) => {
                assert_eq!(parser.next_event().unwrap_err(), error);
                assert_eq!(parser.feed(b"ignored", true).unwrap_err(), error);
                return Err(error.kind);
            }
        }
    }
}

#[test]
fn sixty_thousand_attribute_entities_use_a_bounded_call_stack() {
    const DEPTH: usize = 60_000;
    let mut document = "<!DOCTYPE r [<!ENTITY e0 'leaf'>".to_owned();
    for index in 1..DEPTH {
        write!(document, "<!ENTITY e{index} 'x&e{};'>", index - 1).unwrap();
    }
    write!(document, "]><r a='&e{};|&e0;'/>", DEPTH - 1).unwrap();
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let config = Config {
                limits: Limits {
                    max_entities: DEPTH,
                    max_entity_depth: DEPTH,
                    ..Limits::default()
                },
                ..Config::default()
            };
            let output = attribute(&document, config).unwrap();
            assert_eq!(output.len(), DEPTH - 1 + "leaf|leaf".len());
            assert!(output[..DEPTH - 1].bytes().all(|byte| byte == b'x'));
            assert_eq!(&output[DEPTH - 1..], "leaf|leaf");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn attribute_entity_cycles_and_depth_preserve_error_precedence() {
    for (declarations, value, depth, expected) in [
        (
            "<!ENTITY a '&b;'><!ENTITY b '&a;'>",
            "&a;",
            2,
            ErrorKind::RecursiveEntityReference,
        ),
        (
            "<!ENTITY a '&missing;'>",
            "&a;",
            1,
            ErrorKind::LimitExceeded,
        ),
        ("", "&missing;", 0, ErrorKind::LimitExceeded),
        ("", "&bad name;", 0, ErrorKind::InvalidToken),
        (
            "<!ENTITY a SYSTEM 'external'>",
            "&a;",
            1,
            ErrorKind::ExternalEntityInAttribute,
        ),
        (
            "<!ENTITY a '&b;'><!ENTITY b 'ok'>",
            "&a;",
            1,
            ErrorKind::LimitExceeded,
        ),
    ] {
        let document = format!("<!DOCTYPE r [{declarations}]><r a='{value}'/>");
        let config = Config {
            limits: Limits {
                max_entity_depth: depth,
                ..Limits::default()
            },
            ..Config::default()
        };
        assert_eq!(attribute(&document, config), Err(expected), "{document}");
    }
}

#[test]
fn branching_entities_reenter_completed_siblings_and_normalize_at_reference_boundaries() {
    let document = "<!DOCTYPE r [<!ENTITY leaf 'A&#13;B&#9;C'><!ENTITY left '&leaf;&#13;'><!ENTITY right '&#10;&leaf;'><!ENTITY diamond '&left;&right;'><!ATTLIST r a CDATA '&diamond;|&diamond;'>]><r/>";
    assert_eq!(
        attribute(document, Config::default()).unwrap(),
        "A B C  A B C|A B C  A B C"
    );
    let document = "<!DOCTYPE r [<!ENTITY leaf 'x'><!ENTITY branch '&leaf;&leaf;&leaf;'>]><r a='&branch;&branch;&leaf;'/>";
    assert_eq!(attribute(document, Config::default()).unwrap(), "xxxxxxx");
}

#[test]
fn wide_diamond_expansion_stops_at_the_existing_work_limit() {
    let mut document = "<!DOCTYPE r [<!ENTITY e0 'x'>".to_owned();
    for index in 1..16 {
        write!(document, "<!ENTITY e{index} '&e{0};&e{0};'>", index - 1).unwrap();
    }
    document.push_str("]><r a='&e15;'/>");
    let config = Config {
        limits: Limits {
            max_entity_expansion_bytes: 4096,
            ..Limits::default()
        },
        ..Config::default()
    };
    assert_eq!(attribute(&document, config), Err(ErrorKind::LimitExceeded));
    assert_eq!(
        attribute(&document, Config::default()).unwrap(),
        "x".repeat(32_768)
    );
}

#[test]
fn external_context_entity_names_are_active_during_attribute_expansion() {
    let mut parent = Parser::new(Config::default());
    parent
        .feed(b"<!DOCTYPE r [<!ENTITY e 'value'>]><r/>", true)
        .unwrap();
    while parent.next_event().unwrap().is_some() {}
    let mut child = parent.external_child(Some("e"), None).unwrap();
    child.feed(b"<r a='&e;'/>", true).unwrap();
    assert_eq!(
        child.next_event().unwrap_err().kind,
        ErrorKind::RecursiveEntityReference
    );
}
