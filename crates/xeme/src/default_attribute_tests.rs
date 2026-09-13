use std::fmt::Write;

use crate::{Config, EventKind, Parser};

#[test]
fn omitted_attribute_work_only_visits_declarations_with_values() {
    for count in [64, 128, 256, 512] {
        for with_defaults in [false, true] {
            let mut xml = std::string::String::from("<!DOCTYPE doc [<!ATTLIST r");
            for index in 0..count {
                let mode = if index % 2 == 0 {
                    "#IMPLIED"
                } else {
                    "#REQUIRED"
                };
                write!(xml, " a{index} CDATA {mode}").unwrap();
                if with_defaults && (index == 0 || index + 1 == count) {
                    write!(xml, " d{index} CDATA 'value'").unwrap();
                }
            }
            xml.push_str(">]><doc>");
            for _ in 0..count {
                xml.push_str("<r/>");
            }
            xml.push_str("</doc>");

            for width in [7, xml.len()] {
                let mut parser = Parser::new(Config::default());
                let mut elements = 0;
                for (index, chunk) in xml.as_bytes().chunks(width).enumerate() {
                    parser
                        .feed(chunk, (index + 1) * width >= xml.len())
                        .unwrap();
                    while let Some(event) = parser.next_event().unwrap() {
                        if let EventKind::StartElement { name, attributes } = event.kind
                            && name == "r"
                        {
                            elements += 1;
                            assert_eq!(attributes.len(), if with_defaults { 2 } else { 0 });
                            assert!(attributes.iter().all(|attr| !attr.specified));
                        }
                    }
                }
                assert_eq!(elements, count);
                // Count real loop visits: inspecting every declaration would
                // grow quadratically despite unchanged output and byte budgets.
                assert!(
                    parser.default_candidates_visited <= 2 * count,
                    "{count} declarations/elements, {with_defaults}, chunk {width}: {} visits",
                    parser.default_candidates_visited
                );
            }
        }
    }
}
