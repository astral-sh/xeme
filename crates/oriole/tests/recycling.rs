//! Opt-in reuse must preserve ordinary owned-event and error behavior.

use oriole::{Config, Error, EventKind, Parser};

fn trace(xml: &[u8], width: usize, config: Config, recycle: bool) -> (Vec<String>, Option<Error>) {
    let mut parser = Parser::new(config);
    let mut events = Vec::new();
    for (index, input) in xml.chunks(width).enumerate() {
        if let Err(error) = parser.feed(input, (index + 1) * width >= xml.len()) {
            return (events, Some(error));
        }
        loop {
            let next = if recycle {
                parser
                    .next_event_for_recycling()
                    .map(|event| event.map(|(event, token)| (event, Some(token))))
            } else {
                parser
                    .next_event()
                    .map(|event| event.map(|event| (event, None)))
            };
            match next {
                Ok(Some((event, token))) => {
                    events.push(format!("{event:?}"));
                    if let (Some(token), EventKind::StartElement { mut attributes, .. }) =
                        (token, event.kind)
                    {
                        for attribute in &mut attributes {
                            attribute.name.try_push('\0').unwrap();
                            attribute.value.try_push('\0').unwrap();
                        }
                        parser.recycle_attributes(token, attributes);
                    }
                }
                Ok(None) => break,
                Err(error) => return (events, Some(error)),
            }
        }
    }
    assert!(parser.is_finished());
    (events, None)
}

#[test]
fn recycling_preserves_normalization_defaults_namespaces_and_error_order() {
    let prefix = "<!DOCTYPE r [<!ENTITY e 'entity'><!ATTLIST n a NMTOKENS 'default value' b CDATA 'fallback'>]><r xmlns='urn:default' xmlns:p='urn:é'><n a='  one  two  ' b='literal'/><empty/>";
    for tail in [
        "<n a='&e;' p:z='préfixe' b='physical\r\n\tspace'/><n a='' b=''/><n/><n b='changed' a='typed value'/></r>",
        "<n a='one' a='duplicate'/></r>",
        "<n a='&undefined;' broken></r>",
        "<n a='&undefined;'/></r>",
    ] {
        let xml = format!("{prefix}{tail}");
        let utf16: Vec<_> = [0xff, 0xfe]
            .into_iter()
            .chain(xml.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        for input in [xml.as_bytes(), utf16.as_slice()] {
            for width in [1, 7, input.len()] {
                for (separator, triplets) in [(None, false), (Some('|'), false), (Some('|'), true)]
                {
                    let config = Config {
                        namespace_separator: separator,
                        namespace_triplets: triplets,
                        ..Config::default()
                    };
                    assert_eq!(
                        trace(input, width, config.clone(), true),
                        trace(input, width, config, false),
                        "width={width}, separator={separator:?}, triplets={triplets}"
                    );
                }
            }
        }
    }
}
