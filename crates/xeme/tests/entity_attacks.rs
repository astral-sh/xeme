//! Attack-shaped inputs with bounded theoretical output, so a regression cannot
//! turn the test suite itself into a billion-byte allocation.

use std::fmt::Write;

use xeme::{Config, Error, ErrorKind, EventKind, Parser};

const SMALL_BUDGET: usize = 64 * 1024;

#[derive(Clone, Copy, Debug)]
enum Protection {
    Default,
    Absolute,
    Relative,
}

impl Protection {
    fn parser(self) -> Parser {
        let mut config = Config::default();
        match self {
            Self::Default => {}
            Self::Absolute => config.limits.max_entity_expansion_bytes = SMALL_BUDGET,
            Self::Relative => config.limits.max_entity_expansion_bytes = usize::MAX,
        }
        let parser = Parser::new(config);
        match self {
            Self::Default => {}
            Self::Absolute => {
                assert!(parser.set_entity_maximum_amplification(f32::INFINITY));
                assert!(parser.set_entity_activation_threshold(u64::MAX));
            }
            Self::Relative => {
                assert!(parser.set_entity_maximum_amplification(2.0));
                assert!(parser.set_entity_activation_threshold(SMALL_BUDGET as u64));
            }
        }
        parser
    }

    fn output_limit(self) -> usize {
        match self {
            Self::Default => Config::default().limits.max_entity_expansion_bytes,
            Self::Absolute | Self::Relative => SMALL_BUDGET,
        }
    }
}

fn drain(parser: &mut Parser, output: &mut usize, limit: usize) -> Result<(), Error> {
    while let Some(event) = parser.next_event()? {
        match event.kind {
            EventKind::Text(value) => *output += value.len(),
            EventKind::Comment(value) => *output += value.len(),
            EventKind::StartElement { attributes, .. } => {
                *output += attributes
                    .iter()
                    .map(|attribute| attribute.value.len())
                    .sum::<usize>();
            }
            EventKind::ExternalEntityReference(reference) => {
                assert_eq!(reference.system_id.as_deref(), Some("empty"));
                let mut child = parser.external_child(None, None)?;
                child.feed(b"", true)?;
                drain(&mut child, output, limit)?;
                parser.merge_external_subset(&child)?;
            }
            _ => {}
        }
        assert!(*output <= limit, "expansion escaped the output allowance");
    }
    Ok(())
}

fn assert_rejected(document: &str, protection: Protection, width: usize, utf16: bool) {
    let input = if utf16 {
        [0xfeff]
            .into_iter()
            .chain(document.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    } else {
        document.as_bytes().to_vec()
    };
    let mut parser = protection.parser();
    assert!(parser.set_param_entity_parsing(2));
    let mut output = 0;
    let result = (|| {
        for chunk in input.chunks(width) {
            parser.feed(chunk, false)?;
            drain(&mut parser, &mut output, protection.output_limit())?;
        }
        parser.feed(b"", true)?;
        drain(&mut parser, &mut output, protection.output_limit())
    })();
    let error = result.expect_err("entity bomb must be rejected");
    assert_eq!(error.kind, ErrorKind::LimitExceeded);
    match protection {
        Protection::Default => assert!(matches!(
            error.message,
            "entity amplification limit exceeded" | "entity expansion byte limit exceeded"
        )),
        Protection::Absolute => assert_eq!(error.message, "entity expansion byte limit exceeded"),
        Protection::Relative => assert_eq!(error.message, "entity amplification limit exceeded"),
    }
    assert_eq!(parser.next_event().unwrap_err(), error);
}

fn general_entities(exponential: bool, large: bool) -> (String, String) {
    if exponential {
        let mut declarations = format!("<!ENTITY e0 '{}'>", "x".repeat(512));
        let depth = if large { 5 } else { 4 };
        for index in 1..=depth {
            let references = format!("&e{};", index - 1).repeat(8);
            write!(declarations, "<!ENTITY e{index} '{references}'>").unwrap();
        }
        // At most 16 MiB even if all protections regress.
        (declarations, format!("&e{depth};"))
    } else {
        (
            format!("<!ENTITY e '{}'>", "x".repeat(4096)),
            "&e;".repeat(if large { 4096 } else { 128 }),
        )
    }
}

fn document(declarations: &str, references: &str, context: usize) -> String {
    match context {
        0 => format!("<!DOCTYPE r [{declarations}]><r>{references}</r>"),
        1 => format!("<!DOCTYPE r [{declarations}]><r a='{references}'/>"),
        2 => format!("<!DOCTYPE r [{declarations}<!ATTLIST r a CDATA '{references}'>]><r/>"),
        _ => unreachable!(),
    }
}

#[test]
fn defaults_reject_exponential_and_quadratic_general_entity_bombs() {
    for exponential in [false, true] {
        let (declarations, references) = general_entities(exponential, true);
        for context in 0..3 {
            assert_rejected(
                &document(&declarations, &references, context),
                Protection::Default,
                usize::MAX,
                false,
            );
        }
    }
}

#[test]
fn independent_budgets_stop_bombs_in_content_attributes_and_defaults() {
    for exponential in [false, true] {
        let (declarations, references) = general_entities(exponential, false);
        for context in 0..3 {
            let document = document(&declarations, &references, context);
            for protection in [Protection::Absolute, Protection::Relative] {
                for width in [1, 7, usize::MAX] {
                    for utf16 in [false, true] {
                        assert_rejected(&document, protection, width, utf16);
                    }
                }
            }
        }
    }
}

#[test]
fn parameter_bombs_are_bounded_in_dtd_sources_and_value_continuations() {
    for context in 0..3 {
        let leaf = if context == 0 {
            format!("<!--{}-->", "x".repeat(512))
        } else {
            "x".repeat(512)
        };
        let mut document = format!("<!DOCTYPE r [<!ENTITY % e0 '{leaf}'>");
        for index in 1..=4 {
            // Keep the nested references delayed until the selected context
            // processes them, instead of expanding them in the root subset.
            let references = format!("&#37;e{};", index - 1).repeat(8);
            write!(document, "<!ENTITY % e{index} '{references}'>").unwrap();
        }
        match context {
            0 => document.push_str("%e4;]><r/>"),
            1 => document.push_str(
                "<!ENTITY % define \"<!ENTITY g '&#37;e4;'>\">%define;]><r>&g;</r>",
            ),
            2 => document.push_str(
                "<!ENTITY % ext SYSTEM 'empty'><!ENTITY % define \"<!ENTITY g '&#37;ext;&#37;e4;'>\">%define;]><r>&g;</r>",
            ),
            _ => unreachable!(),
        }
        for protection in [Protection::Absolute, Protection::Relative] {
            for width in [1, 7, usize::MAX] {
                for utf16 in [false, true] {
                    assert_rejected(&document, protection, width, utf16);
                }
            }
        }
    }
}
