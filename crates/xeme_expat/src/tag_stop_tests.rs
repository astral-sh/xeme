use super::*;

#[derive(Default)]
struct State {
    parser: XML_Parser,
    events: std::vec::Vec<String>,
    stop_at: usize,
    resumable: u8,
    default_current: bool,
    replace_end: bool,
}

unsafe fn record(arg: *mut c_void, event: String) {
    // SAFETY: Tests retain State and its parser until parsing and callbacks end.
    // No reference to State survives a reentrant parser call.
    unsafe {
        let state = arg.cast::<State>();
        (*state).events.push(event);
        if (*state).events.len() == (*state).stop_at {
            assert_eq!(XML_StopParser((*state).parser, (*state).resumable), OK);
            if (*state).replace_end {
                XML_SetEndElementHandler((*state).parser, Some(replaced_end));
            }
        }
    }
}

unsafe extern "C" fn start(arg: *mut c_void, name: *const c_char, _: *const *const c_char) {
    // SAFETY: The callback owns a live name and the test-owned state.
    unsafe {
        record(
            arg,
            format!("start:{}", CStr::from_ptr(name).to_str().unwrap()),
        );
        if (*arg.cast::<State>()).default_current {
            XML_DefaultCurrent((*arg.cast::<State>()).parser);
        }
    }
}

unsafe extern "C" fn end(arg: *mut c_void, name: *const c_char) {
    // SAFETY: Callback strings remain live while recording their owned copy.
    unsafe {
        record(
            arg,
            format!("end:{}", CStr::from_ptr(name).to_str().unwrap()),
        )
    };
}

unsafe extern "C" fn replaced_end(arg: *mut c_void, name: *const c_char) {
    // SAFETY: Callback strings remain live while recording their owned copy.
    unsafe {
        record(
            arg,
            format!("replaced:{}", CStr::from_ptr(name).to_str().unwrap()),
        )
    };
}

unsafe extern "C" fn start_ns(arg: *mut c_void, prefix: *const c_char, _: *const c_char) {
    // SAFETY: All test namespace prefixes are non-null callback-scoped strings.
    unsafe {
        record(
            arg,
            format!("startns:{}", CStr::from_ptr(prefix).to_str().unwrap()),
        )
    };
}

unsafe extern "C" fn end_ns(arg: *mut c_void, prefix: *const c_char) {
    // SAFETY: All test namespace prefixes are non-null callback-scoped strings.
    unsafe {
        record(
            arg,
            format!("endns:{}", CStr::from_ptr(prefix).to_str().unwrap()),
        )
    };
}

unsafe extern "C" fn default(arg: *mut c_void, text: *const c_char, length: c_int) {
    // SAFETY: Expat supplies length readable callback-scoped bytes.
    unsafe {
        let text = std::slice::from_raw_parts(text.cast(), length as usize);
        record(
            arg,
            format!("default:{}", std::str::from_utf8(text).unwrap()),
        );
    }
}

#[test]
fn stop_finishes_the_current_tag_callbacks_before_returning() {
    // SAFETY: All parser handles, input bytes and callback states outlive calls.
    unsafe {
        for empty in [false, true] {
            let document = if empty {
                "<r xmlns:a='u' xmlns:b='v'/>"
            } else {
                "<r xmlns:a='u' xmlns:b='v'></r>"
            };
            let all = [
                "startns:a",
                "startns:b",
                "start:r",
                "end:r",
                "endns:b",
                "endns:a",
            ];
            for stop_at in 1..=all.len() {
                for resumable in [0, 1] {
                    for utf16 in [false, true] {
                        for buffer_api in [false, true] {
                            let input = if utf16 {
                                [0xff, 0xfe]
                                    .into_iter()
                                    .chain(document.encode_utf16().flat_map(u16::to_le_bytes))
                                    .collect::<std::vec::Vec<_>>()
                            } else {
                                document.as_bytes().to_vec()
                            };
                            let parser = XML_ParserCreateNS(ptr::null(), b'|' as c_char);
                            let mut state = State {
                                parser,
                                stop_at,
                                resumable,
                                ..State::default()
                            };
                            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                            XML_SetElementHandler(parser, Some(start), Some(end));
                            XML_SetNamespaceDeclHandler(parser, Some(start_ns), Some(end_ns));
                            let status = if buffer_api {
                                let buffer =
                                    XML_GetBuffer(parser, input.len() as c_int).cast::<u8>();
                                assert!(!buffer.is_null());
                                ptr::copy_nonoverlapping(input.as_ptr(), buffer, input.len());
                                XML_ParseBuffer(parser, input.len() as c_int, 1)
                            } else {
                                XML_Parse(parser, input.as_ptr().cast(), input.len() as c_int, 1)
                            };
                            let completed = if empty || stop_at > 3 { 6 } else { 3 };
                            assert_eq!(state.events, all[..completed]);
                            assert_eq!(status, if resumable == 1 { SUSPENDED } else { ERROR });
                            assert_eq!(
                                XML_GetErrorCode(parser),
                                if resumable == 1 { 0 } else { 35 }
                            );
                            if resumable == 1 {
                                assert_eq!(XML_ResumeParser(parser), OK);
                                assert_eq!(state.events, all);
                            }
                            XML_ParserFree(parser);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn stopped_tags_keep_default_current_and_handler_mutations() {
    // SAFETY: Test-owned State and document storage remain live through callbacks.
    unsafe {
        for resumable in [0, 1] {
            for default_current in [false, true] {
                let document = if default_current {
                    "<r/>"
                } else {
                    "<r xmlns:a='u'/>"
                };
                let parser = XML_ParserCreateNS(ptr::null(), b'|' as c_char);
                let mut state = State {
                    parser,
                    stop_at: if default_current { 2 } else { 1 },
                    resumable,
                    default_current,
                    replace_end: default_current,
                    ..State::default()
                };
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                XML_SetDefaultHandler(parser, Some(default));
                if default_current {
                    XML_SetElementHandler(parser, Some(start), Some(end));
                } else {
                    XML_SetNamespaceDeclHandler(parser, Some(start_ns), Some(end_ns));
                }
                let result =
                    XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1);
                let expected = if default_current {
                    vec!["start:r", "default:<r/>", "replaced:r"]
                } else {
                    vec!["startns:a", "default:<r xmlns:a='u'/>", "endns:a"]
                };
                assert_eq!(state.events, expected);
                assert_eq!(result, if resumable == 1 { SUSPENDED } else { ERROR });
                assert_eq!(
                    XML_GetErrorCode(parser),
                    if resumable == 1 { 0 } else { 35 }
                );
                if resumable == 1 {
                    assert_eq!(XML_ResumeParser(parser), OK);
                    assert_eq!(state.events, expected);
                }
                XML_ParserFree(parser);
            }
        }
    }
}

#[test]
fn streamed_empty_tags_finish_their_stopped_callback_group() {
    // SAFETY: State, parser, and each one-byte input remain live during callbacks.
    unsafe {
        for resumable in [0, 1] {
            let input = b"<r xmlns:a='u'/>";
            let parser = XML_ParserCreateNS(ptr::null(), b'|' as c_char);
            let mut state = State {
                parser,
                stop_at: 1,
                resumable,
                ..State::default()
            };
            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
            XML_SetElementHandler(parser, Some(start), Some(end));
            XML_SetNamespaceDeclHandler(parser, Some(start_ns), Some(end_ns));
            for byte in &input[..input.len() - 1] {
                assert_eq!(XML_Parse(parser, ptr::from_ref(byte).cast(), 1, 0), OK);
                assert!(state.events.is_empty());
            }
            assert_eq!(
                XML_Parse(parser, input[input.len() - 1..].as_ptr().cast(), 1, 1),
                if resumable == 1 { SUSPENDED } else { ERROR }
            );
            assert_eq!(state.events, ["startns:a", "start:r", "end:r", "endns:a"]);
            if resumable == 1 {
                assert_eq!(XML_ResumeParser(parser), OK);
                assert_eq!(state.events.len(), 4);
            }
            XML_ParserFree(parser);
        }
    }
}
