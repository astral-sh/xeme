//! Actual C getter calls at native publication and reset boundaries.
use super::*;

#[derive(Default)]
struct Coordinates {
    parser: XML_Parser,
    resolve: bool,
    events: Vec<(String, Position)>,
}

unsafe fn observe(data: *mut c_void, kind: String) {
    // SAFETY: The test owns the live state and parser. Only raw field projections
    // cross getter calls; callback payloads live outside the borrowed core field.
    unsafe {
        let state = data.cast::<Coordinates>();
        let parser = (*state).parser;
        let native = (*parser).position;
        assert!(matches!(native, AdapterLocation::Native(_)));
        let position = (*parser).core.position();
        for _ in 0..2 {
            assert_eq!(
                XML_GetCurrentByteIndex(parser),
                position.byte_index as c_long
            );
            assert_eq!(
                XML_GetCurrentByteCount(parser),
                position.byte_count as c_int
            );
            let mut offset = -1;
            let mut size = -1;
            assert!(!XML_GetInputContext(parser, &mut offset, &mut size).is_null());
            assert!(offset >= 0 && size >= offset + position.byte_count as c_int);
            assert_eq!((*parser).position, native);
        }
        if (*state).resolve {
            for _ in 0..2 {
                assert_eq!(XML_GetCurrentLineNumber(parser), position.line as c_ulong);
                assert_eq!(
                    XML_GetCurrentColumnNumber(parser),
                    position.column as c_ulong
                );
                assert_eq!(
                    XML_GetCurrentByteIndex(parser),
                    position.byte_index as c_long
                );
            }
            assert_eq!((*parser).position, AdapterLocation::Position(position));
        }
        (*state).events.push((kind, position));
    }
}

unsafe extern "C" fn start(data: *mut c_void, name: *const c_char, _: *const *const c_char) {
    // SAFETY: The callback supplies a live terminated name and test-owned state.
    unsafe {
        let name = CStr::from_ptr(name).to_str().unwrap();
        observe(data, format!("start:{name}"));
    }
}

unsafe extern "C" fn end(data: *mut c_void, name: *const c_char) {
    // SAFETY: The callback supplies a live terminated name and test-owned state.
    unsafe {
        let name = CStr::from_ptr(name).to_str().unwrap();
        observe(data, format!("end:{name}"));
    }
}

unsafe extern "C" fn text(data: *mut c_void, bytes: *const c_char, len: c_int) {
    // SAFETY: Text is callback-lived, including through coordinate getter reentry.
    unsafe {
        let original = std::slice::from_raw_parts(bytes.cast::<u8>(), len as usize).to_vec();
        observe(
            data,
            format!("text:{}", std::str::from_utf8(&original).unwrap()),
        );
        assert_eq!(
            std::slice::from_raw_parts(bytes.cast::<u8>(), len as usize),
            original
        );
    }
}

#[test]
fn native_coordinate_getters_preserve_bytes_and_resolve_exact_positions() {
    eprintln!(
        "coordinate layout: CParser={} Parser={} AdapterFrame={} AdapterLocation={}",
        size_of::<XML_ParserStruct>(),
        size_of::<Parser>(),
        size_of::<oriole::AdapterFrame>(),
        size_of::<AdapterLocation>(),
    );
    let input = "<r>\n<n>é\nx</n></r>";
    let expected = [
        ("start:r", 0, 3, 1, 0),
        ("text:\n", 3, 1, 1, 3),
        ("start:n", 4, 3, 2, 0),
        ("text:é\nx", 7, 4, 2, 3),
        ("end:n", 11, 4, 3, 1),
        ("end:r", 15, 4, 3, 5),
    ]
    .map(|(kind, byte_index, byte_count, line, column)| {
        (
            kind.to_owned(),
            Position {
                byte_index,
                byte_count,
                line,
                column,
            },
        )
    });
    for resolve in [false, true] {
        for buffered in [false, true] {
            // SAFETY: State, input and callback functions outlive synchronous use.
            unsafe {
                let parser = XML_ParserCreate(ptr::null());
                assert!(!parser.is_null());
                let mut state = Coordinates {
                    parser,
                    resolve,
                    ..Coordinates::default()
                };
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                XML_SetElementHandler(parser, Some(start), Some(end));
                XML_SetCharacterDataHandler(parser, Some(text));
                let status = if buffered {
                    let buffer = XML_GetBuffer(parser, input.len() as c_int);
                    assert!(!buffer.is_null());
                    ptr::copy_nonoverlapping(input.as_ptr(), buffer.cast(), input.len());
                    XML_ParseBuffer(parser, input.len() as c_int, 1)
                } else {
                    XML_Parse(parser, input.as_ptr().cast(), input.len() as c_int, 1)
                };
                assert_eq!(status, OK);
                assert_eq!(state.events, expected);
                assert_eq!(XML_GetCurrentLineNumber(parser), 3);
                assert_eq!(XML_GetCurrentColumnNumber(parser), 9);
                assert_eq!(XML_GetCurrentByteIndex(parser), input.len() as c_long);
                assert_eq!(XML_GetCurrentByteCount(parser), 0);
                XML_ParserFree(parser);
            }
        }
    }
}

#[derive(Default)]
struct ReleaseState {
    parser: XML_Parser,
    releases: usize,
}

unsafe extern "C" fn suspend_on_n(data: *mut c_void, name: *const c_char, _: *const *const c_char) {
    // SAFETY: The parser is the configured handler argument and stays busy.
    unsafe {
        if CStr::from_ptr(name).to_bytes() == b"n" {
            let parser: XML_Parser = data.cast();
            let before = (*parser).position;
            assert!(matches!(before, AdapterLocation::Native(_)));
            assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
            assert_eq!((*parser).position, before);
            assert_eq!(XML_StopParser(parser, 1), OK);
        }
    }
}

unsafe extern "C" fn release_at_n(data: *mut c_void) {
    // SAFETY: Reset owns the still-live old core until this release returns.
    unsafe {
        let state = data.cast::<ReleaseState>();
        let parser = (*state).parser;
        assert_eq!(XML_GetCurrentLineNumber(parser), 2);
        assert_eq!(XML_GetCurrentColumnNumber(parser), 0);
        assert_eq!(XML_GetCurrentByteIndex(parser), 4);
        assert_eq!(XML_GetCurrentByteCount(parser), 3);
        (*state).releases += 1;
    }
}

#[test]
fn suspended_coordinates_survive_rejected_reset_and_encoding_release() {
    for resolve_before_reset in [false, true] {
        // SAFETY: The test owns the parser and manually installed release state.
        unsafe {
            let parser = XML_ParserCreate(ptr::null());
            assert!(!parser.is_null());
            XML_UseParserAsHandlerArg(parser);
            XML_SetStartElementHandler(parser, Some(suspend_on_n));
            let input = c"<r>\n<n></n></r>";
            assert_eq!(
                XML_Parse(parser, input.as_ptr(), input.to_bytes().len() as c_int, 1),
                SUSPENDED
            );
            assert!(matches!((*parser).position, AdapterLocation::Native(_)));
            assert_eq!(XML_GetCurrentByteIndex(parser), 4);
            assert!(matches!((*parser).position, AdapterLocation::Native(_)));
            if resolve_before_reset {
                assert_eq!(XML_GetCurrentLineNumber(parser), 2);
                assert_eq!(XML_GetCurrentColumnNumber(parser), 0);
            }
            let mut state = ReleaseState {
                parser,
                releases: 0,
            };
            (*parser).encoding_release = Some(release_at_n);
            (*parser).encoding_data = ptr::from_mut(&mut state).cast();
            assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
            assert_eq!(state.releases, 1);
            assert_eq!(XML_GetCurrentLineNumber(parser), 1);
            assert_eq!(XML_GetCurrentColumnNumber(parser), 0);
            assert_eq!(XML_GetCurrentByteIndex(parser), -1);
            assert_eq!(XML_GetCurrentByteCount(parser), 0);
            XML_ParserFree(parser);
            assert_eq!(state.releases, 1);
        }
    }
}

#[derive(Default)]
struct StaleState {
    parser: XML_Parser,
    first: Option<AdapterLocation>,
    rejected: bool,
}

unsafe extern "C" fn stale_start(data: *mut c_void, _: *const c_char, _: *const *const c_char) {
    // SAFETY: Only this private test corrupts the scalar descriptor. The getter
    // must reject its identity before looking up coordinates in the current core.
    unsafe {
        let state = data.cast::<StaleState>();
        let parser = (*state).parser;
        let current = (*parser).position;
        assert!(matches!(current, AdapterLocation::Native(_)));
        if let Some(first) = (*state).first {
            assert_ne!(first, current);
            (*parser).position = first;
            assert_eq!(XML_GetCurrentLineNumber(parser), 0);
            assert_eq!(XML_GetCurrentColumnNumber(parser), 0);
            assert_eq!((*parser).parse_error, UNEXPECTED_STATE);
            (*state).rejected = true;
        } else {
            (*state).first = Some(current);
        }
    }
}

#[test]
fn stale_same_generation_c_coordinates_fail_closed() {
    // SAFETY: The test owns the parser and state through each callback and free.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert!(!parser.is_null());
        let mut state = StaleState {
            parser,
            ..StaleState::default()
        };
        XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
        XML_SetStartElementHandler(parser, Some(stale_start));
        let input = c"<r><n></n></r>";
        assert_eq!(
            XML_Parse(parser, input.as_ptr(), input.to_bytes().len() as c_int, 1),
            ERROR
        );
        assert!(state.rejected);
        assert_eq!(XML_GetErrorCode(parser), UNEXPECTED_STATE);
        XML_ParserFree(parser);
    }
}
