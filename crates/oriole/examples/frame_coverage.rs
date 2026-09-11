use oriole::{AdapterFrame, Attribute, Config, Event, EventKind, NameRules, Parser};

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let input = std::fs::read(&args[1]).unwrap();
    let mut parser = Parser::new(Config {
        name_rules: NameRules::FourthEdition,
        namespace_separator: (args.get(2).is_some_and(|value| value == "1")).then_some('|'),
        ..Config::default()
    });
    let mut frame = parser.adapter_frame();
    let mut starts = 0;
    let mut framed = 0;
    let mut attributes = 0;
    let mut events = 0;
    for (index, bytes) in input.chunks(4096).enumerate() {
        parser
            .feed(bytes, (index + 1) * 4096 >= input.len())
            .unwrap();
        loop {
            let mut event = None;
            let Some(token) = parser
                .next_event_for_adapter_into(&mut event, &mut frame)
                .unwrap()
            else {
                break;
            };
            events += 1;
            if let Some(name) = frame.take_end_name() {
                parser.recycle_end_element(token, name);
                continue;
            }
            if frame.text_bytes().is_some() {
                continue;
            }
            if frame.is_active() {
                starts += 1;
                framed += 1;
                attributes += frame.attributes().len();
            } else {
                match event.unwrap().kind {
                    EventKind::StartElement {
                        name,
                        attributes: attrs,
                    } => {
                        starts += 1;
                        parser.recycle_start_element(token, name, attrs);
                    }
                    EventKind::EndElement { name } => parser.recycle_end_element(token, name),
                    _ => {}
                }
            }
        }
    }
    parser.finish_adapter_frame(frame);
    println!(
        "{{\"starts\":{starts},\"frames\":{framed},\"framed_attributes\":{attributes},\"events\":{events},\"event_size\":{},\"event_kind_size\":{},\"frame_size\":{},\"attribute_size\":{},\"parser_size\":{}}}",
        size_of::<Event>(),
        size_of::<EventKind>(),
        size_of::<AdapterFrame>(),
        size_of::<Attribute>(),
        size_of::<Parser>()
    );
}
