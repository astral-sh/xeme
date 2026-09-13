use super::*;

type Coordinates = [i64; 4];

struct State {
    parser: XML_Parser,
    trigger: &'static str,
    inside: Option<Coordinates>,
}

unsafe fn coordinates(parser: XML_Parser) -> Coordinates {
    // SAFETY: Each test owns the live parser through all getter calls.
    unsafe {
        [
            XML_GetCurrentByteIndex(parser) as i64,
            XML_GetCurrentByteCount(parser) as i64,
            XML_GetCurrentLineNumber(parser) as i64,
            XML_GetCurrentColumnNumber(parser) as i64,
        ]
    }
}

unsafe fn record(arg: *mut c_void, event: &str) {
    // SAFETY: State remains live; only raw field accesses span callback reentry.
    unsafe {
        let state = arg.cast::<State>();
        if (*state).trigger == event && (*state).inside.is_none() {
            let position = coordinates((*state).parser);
            (*state).inside = Some(position);
            assert_eq!(XML_StopParser((*state).parser, 1), OK);
            assert_eq!(coordinates((*state).parser), position);
        }
    }
}

unsafe extern "C" fn start(arg: *mut c_void, name: *const c_char, _: *const *const c_char) {
    // SAFETY: Names are callback-scoped C strings.
    unsafe {
        if CStr::from_ptr(name).to_bytes() == b"x" {
            record(arg, "start");
        }
    }
}
unsafe extern "C" fn end(arg: *mut c_void, name: *const c_char) {
    // SAFETY: Names are callback-scoped C strings.
    unsafe {
        if CStr::from_ptr(name).to_bytes() == b"x" {
            record(arg, "end");
        }
    }
}
unsafe extern "C" fn text(arg: *mut c_void, bytes: *const c_char, length: c_int) {
    // SAFETY: The callback provides length readable bytes.
    unsafe {
        if std::slice::from_raw_parts(bytes.cast::<u8>(), length as usize) == b"text" {
            record(arg, "text");
        }
    }
}
unsafe extern "C" fn pi(arg: *mut c_void, _: *const c_char, _: *const c_char) {
    // SAFETY: The test retains callback state throughout parsing.
    unsafe { record(arg, "pi") };
}
unsafe extern "C" fn comment(arg: *mut c_void, _: *const c_char) {
    // SAFETY: The test retains callback state throughout parsing.
    unsafe { record(arg, "comment") };
}
unsafe extern "C" fn end_cdata(arg: *mut c_void) {
    // SAFETY: The test retains callback state throughout parsing.
    unsafe { record(arg, "end-cdata") };
}
unsafe extern "C" fn attlist(
    arg: *mut c_void,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: c_int,
) {
    // SAFETY: The test retains callback state throughout parsing.
    unsafe { record(arg, "attlist") };
}

unsafe extern "C" fn normalized_text(arg: *mut c_void, _: *const c_char, _: c_int) {
    // SAFETY: The test retains callback state throughout parsing.
    unsafe { record(arg, "text") };
}

#[test]
fn suspended_content_reports_the_consumed_position_outside_callbacks() {
    // SAFETY: Parent, optional child, state and input remain owned until free.
    unsafe {
        for (kind, content, first, length, column) in [
            ("start", "<x></x>", 5, 3, 0),
            ("end", "<x></x>", 8, 4, 3),
            ("text", "text", 5, 4, 0),
            ("pi", "<?pi data?>", 5, 11, 0),
            ("comment", "<!--data-->", 5, 11, 0),
            ("end-cdata", "<![CDATA[text]]>", 18, 3, 13),
        ] {
            for utf16 in [false, true] {
                for buffer_api in [false, true] {
                    for (external, streamed) in
                        [(false, false), (true, false), (false, true), (true, true)]
                    {
                        let parent = XML_ParserCreate(ptr::null());
                        let parser = if external {
                            XML_ExternalEntityParserCreate(parent, c"".as_ptr(), ptr::null())
                        } else {
                            parent
                        };
                        assert!(!parser.is_null());
                        let mut state = State {
                            parser,
                            trigger: kind,
                            inside: None,
                        };
                        XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                        XML_SetElementHandler(parser, Some(start), Some(end));
                        XML_SetCharacterDataHandler(parser, Some(text));
                        XML_SetProcessingInstructionHandler(parser, Some(pi));
                        XML_SetCommentHandler(parser, Some(comment));
                        XML_SetEndCdataSectionHandler(parser, Some(end_cdata));
                        let document = format!("<r\n >{content}</r>");
                        let input = if utf16 {
                            [0xff, 0xfe]
                                .into_iter()
                                .chain(document.encode_utf16().flat_map(u16::to_le_bytes))
                                .collect::<std::vec::Vec<_>>()
                        } else {
                            document.into_bytes()
                        };
                        let width = if utf16 { 2 } else { 1 };
                        let bom = if utf16 { 2 } else { 0 };
                        let cutoff = if streamed {
                            (bom + (first + length) * width) as usize
                        } else {
                            input.len()
                        };
                        let prefix = &input[..cutoff];
                        let result = if buffer_api {
                            let buffer = XML_GetBuffer(parser, prefix.len() as c_int).cast::<u8>();
                            assert!(!buffer.is_null());
                            ptr::copy_nonoverlapping(prefix.as_ptr(), buffer, prefix.len());
                            XML_ParseBuffer(parser, prefix.len() as c_int, c_int::from(!streamed))
                        } else {
                            XML_Parse(
                                parser,
                                prefix.as_ptr().cast(),
                                prefix.len() as c_int,
                                c_int::from(!streamed),
                            )
                        };
                        assert_eq!(result, SUSPENDED);
                        assert_eq!(
                            state.inside,
                            Some([bom + first * width, length * width, 2, column + 2])
                        );
                        assert_eq!(
                            coordinates(parser),
                            [bom + (first + length) * width, 0, 2, column + length + 2]
                        );
                        assert_eq!(XML_ResumeParser(parser), OK);
                        assert_eq!(XML_GetCurrentByteIndex(parser) as usize, cutoff);
                        assert_eq!(XML_GetCurrentByteCount(parser), 0);
                        if streamed {
                            let tail = &input[cutoff..];
                            assert_eq!(
                                XML_Parse(parser, tail.as_ptr().cast(), tail.len() as c_int, 1),
                                OK
                            );
                        }
                        assert_eq!(XML_GetCurrentByteIndex(parser) as usize, input.len());
                        XML_ParserFree(parser);
                        if external {
                            XML_ParserFree(parent);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn suspended_entity_anchors_and_declaration_lookahead_stay_at_the_callback() {
    // SAFETY: Each test retains its parser, inputs and state through resume.
    unsafe {
        for (kind, document) in [
            ("start", "<!DOCTYPE r [<!ENTITY e '<x></x>'>]><r>&e;</r>"),
            (
                "attlist",
                "<!DOCTYPE r [<!ATTLIST r a CDATA 'v' b CDATA 'w'>]><r/>",
            ),
        ] {
            let parser = XML_ParserCreate(ptr::null());
            let mut state = State {
                parser,
                trigger: kind,
                inside: None,
            };
            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
            XML_SetStartElementHandler(parser, Some(start));
            XML_SetAttlistDeclHandler(parser, Some(attlist));
            assert_eq!(
                XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
                SUSPENDED
            );
            assert_eq!(&coordinates(parser)[..2], &state.inside.unwrap()[..2]);
            assert_eq!(XML_ResumeParser(parser), OK);
            XML_ParserFree(parser);
        }
    }
}

#[test]
fn normalized_text_suspension_reports_source_coordinates() {
    // SAFETY: Parser, input and callback state remain live through resume.
    unsafe {
        for (content, after) in [
            ("&#10;", [8, 0, 1, 8]),
            ("&amp;", [8, 0, 1, 8]),
            ("\r", [4, 0, 2, 0]),
            ("\r\n", [5, 0, 2, 0]),
            ("é", [5, 0, 1, 4]),
        ] {
            let parser = XML_ParserCreate(ptr::null());
            let mut state = State {
                parser,
                trigger: "text",
                inside: None,
            };
            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
            XML_SetCharacterDataHandler(parser, Some(normalized_text));
            let document = format!("<r>{content}</r>");
            assert_eq!(
                XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
                SUSPENDED
            );
            assert_eq!(state.inside, Some([3, content.len() as i64, 1, 3]));
            assert_eq!(coordinates(parser), after);
            assert_eq!(XML_ResumeParser(parser), OK);
            XML_ParserFree(parser);
        }
    }
}

#[test]
fn successful_nonfinal_calls_and_suspended_prolog_keep_distinct_byte_positions() {
    // SAFETY: Each parser and its callback state are owned until free.
    unsafe {
        for (prefix, suffix) in [
            ("<?pi data?>", "<r/>"),
            ("<!--data-->", "<r/>"),
            ("<r>", "</r>"),
            ("<r>text", "</r>"),
            ("<r/><!--data-->", ""),
        ] {
            let parser = XML_ParserCreate(ptr::null());
            assert_eq!(
                XML_Parse(parser, prefix.as_ptr().cast(), prefix.len() as c_int, 0),
                OK
            );
            assert_eq!(
                coordinates(parser),
                [prefix.len() as i64, 0, 1, prefix.len() as i64]
            );
            assert_eq!(
                XML_Parse(parser, suffix.as_ptr().cast(), suffix.len() as c_int, 1),
                OK
            );
            XML_ParserFree(parser);
        }

        let parser = XML_ParserCreate(ptr::null());
        let mut state = State {
            parser,
            trigger: "pi",
            inside: None,
        };
        XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
        XML_SetProcessingInstructionHandler(parser, Some(pi));
        assert_eq!(XML_Parse(parser, c"<?pi data?>".as_ptr(), 11, 0), SUSPENDED);
        assert_eq!(state.inside, Some([0, 11, 1, 0]));
        assert_eq!(coordinates(parser), [0, 11, 1, 11]);
        assert_eq!(XML_ResumeParser(parser), OK);
        assert_eq!(coordinates(parser), [11, 0, 1, 11]);
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        XML_ParserFree(parser);
    }
}
