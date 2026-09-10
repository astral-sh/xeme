use super::*;

#[derive(Default)]
struct State {
    parser: XML_Parser,
    events: Vec<String>,
    nested_status: c_int,
    releases: usize,
}

unsafe extern "C" fn start(data: *mut c_void, name: *const c_char, attrs: *const *const c_char) {
    // SAFETY: Tests install a State user pointer and receive valid callback strings.
    unsafe {
        let state = &mut *data.cast::<State>();
        state
            .events
            .push(format!("start:{}", CStr::from_ptr(name).to_str().unwrap()));
        let mut index = 0;
        while !(*attrs.add(index)).is_null() {
            state.events.push(format!(
                "{}={}",
                CStr::from_ptr(*attrs.add(index)).to_str().unwrap(),
                CStr::from_ptr(*attrs.add(index + 1)).to_str().unwrap()
            ));
            index += 2;
        }
    }
}

unsafe extern "C" fn end(data: *mut c_void, name: *const c_char) {
    // SAFETY: Tests install State and the callback receives a valid C string.
    unsafe {
        (*data.cast::<State>())
            .events
            .push(format!("end:{}", CStr::from_ptr(name).to_str().unwrap()));
    }
}

unsafe extern "C" fn text(data: *mut c_void, text: *const c_char, len: c_int) {
    // SAFETY: The callback input is readable for len bytes and data points to State.
    unsafe {
        let bytes = std::slice::from_raw_parts(text.cast::<u8>(), len as usize);
        (*data.cast::<State>())
            .events
            .push(format!("text:{}", std::str::from_utf8(bytes).unwrap()));
    }
}

unsafe fn configured(state: &mut State) -> XML_Parser {
    // SAFETY: Test-owned State lives until the parser has been freed.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert!(!parser.is_null());
        state.parser = parser;
        XML_SetUserData(parser, ptr::from_mut(state).cast());
        XML_SetElementHandler(parser, Some(start), Some(end));
        XML_SetCharacterDataHandler(parser, Some(text));
        parser
    }
}

#[test]
fn streaming_callbacks_and_user_data_layout() {
    // SAFETY: All handles, strings, callback data, and buffers are test-owned.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        assert_eq!(
            *parser.cast::<*mut c_void>(),
            ptr::from_mut(&mut state).cast()
        );
        assert_eq!(XML_GetUserData(parser), ptr::from_mut(&mut state).cast());
        assert_eq!(XML_Parse(parser, c"<r x='1'>hel".as_ptr(), 12, 0), OK);
        assert_eq!(XML_Parse(parser, c"lo</r>".as_ptr(), 6, 1), OK);
        assert_eq!(state.events.first().unwrap(), "start:r");
        assert_eq!(state.events[1], "x=1");
        assert_eq!(state.events.last().unwrap(), "end:r");
        assert_eq!(
            state
                .events
                .iter()
                .filter_map(|s| s.strip_prefix("text:"))
                .collect::<String>(),
            "hello"
        );
        XML_ParserFree(parser);
    }
}

#[test]
fn buffer_api_bounds_and_reset() {
    // SAFETY: Writes stay within the buffer returned for four bytes.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert_eq!(XML_ParseBuffer(parser, 0, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 42);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        let buffer = XML_GetBuffer(parser, 4);
        assert!(!buffer.is_null());
        ptr::copy_nonoverlapping(b"<r/>".as_ptr(), buffer.cast(), 4);
        assert_eq!(XML_ParseBuffer(parser, 5, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), INVALID_ARGUMENT);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        let buffer = XML_GetBuffer(parser, 4);
        ptr::copy_nonoverlapping(b"<r/>".as_ptr(), buffer.cast(), 4);
        assert_eq!(XML_ParseBuffer(parser, 4, 1), OK);
        assert_eq!(XML_Parse(parser, ptr::null(), 0, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 36);
        XML_ParserFree(parser);
    }
}

unsafe extern "C" fn stop_on_start(
    data: *mut c_void,
    _name: *const c_char,
    _attrs: *const *const c_char,
) {
    // SAFETY: The State handle is live during this callback.
    unsafe {
        let parser = (*data.cast::<State>()).parser;
        (*data.cast::<State>()).nested_status = XML_StopParser(parser, 1);
        assert_eq!(XML_ResumeParser(parser), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 0);
        XML_SetStartElementHandler(parser, Some(start));
    }
}

#[test]
fn suspended_parser_resumes_without_repeating_callback() {
    // SAFETY: Callbacks retain no pointers beyond the enclosing parse call.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetStartElementHandler(parser, Some(stop_on_start));
        assert_eq!(XML_Parse(parser, c"<r><x/></r>".as_ptr(), 11, 1), SUSPENDED);
        assert_eq!(state.nested_status, OK);
        assert!(state.events.is_empty());
        let mut status = XML_ParsingStatus {
            parsing: -1,
            finalBuffer: 0,
        };
        XML_GetParsingStatus(parser, &mut status);
        assert_eq!(status.parsing, 3);
        assert_eq!(status.finalBuffer, 1);
        assert_eq!(XML_ResumeParser(parser), OK);
        assert_eq!(state.events, ["start:x", "end:x", "end:r"]);
        XML_ParserFree(parser);
    }
}

unsafe extern "C" fn reenter_parse(
    data: *mut c_void,
    _name: *const c_char,
    _attrs: *const *const c_char,
) {
    // SAFETY: Test intentionally attempts supported detection of same-parser reentry.
    unsafe {
        let parser = (*data.cast::<State>()).parser;
        (*data.cast::<State>()).nested_status = XML_Parse(parser, c"<x/>".as_ptr(), 4, 1);
        assert!(XML_GetBuffer(parser, 16).is_null());
        assert_eq!(XML_ParseBuffer(parser, 0, 0), ERROR);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
        assert_eq!(XML_ResumeParser(parser), ERROR);
        XML_ParserFree(parser);
        assert_eq!(XML_GetErrorCode(parser), 0);
    }
}

#[test]
fn forbidden_callback_calls_preserve_the_outer_parse() {
    // SAFETY: Reentry must return before mutating the parser's borrowed core.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetStartElementHandler(parser, Some(reenter_parse));
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        assert_eq!(state.nested_status, ERROR);
        assert_eq!(XML_GetErrorCode(parser), 0);
        assert_eq!(state.events, ["end:r"]);
        XML_ParserFree(parser);
    }
}

unsafe extern "C" fn free_on_start(
    data: *mut c_void,
    _name: *const c_char,
    _attrs: *const *const c_char,
) {
    // SAFETY: Callback-time Free is ignored, including repeated calls during one callback.
    unsafe {
        let parser = (*data.cast::<State>()).parser;
        XML_ParserFree(parser);
        XML_ParserFree(parser);
        // The caller still owns the live handle after these ignored calls.
        assert_eq!(XML_GetUserData(parser), data);
        XML_SetCharacterDataHandler(parser, None);
    }
}

#[test]
fn callback_free_is_ignored_and_dispatch_continues() {
    // SAFETY: The caller retains ownership and frees after the outer parse returns.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetStartElementHandler(parser, Some(free_on_start));
        assert_eq!(XML_Parse(parser, c"<r>hello</r>".as_ptr(), 12, 1), OK);
        assert_eq!(state.events, ["end:r"]);
        assert_eq!(XML_GetErrorCode(parser), 0);
        XML_ParserFree(parser);
    }
}

unsafe extern "C" fn reset_on_start(
    data: *mut c_void,
    _name: *const c_char,
    _attrs: *const *const c_char,
) {
    // SAFETY: Reset is intentionally rejected during a live callback.
    unsafe {
        let parser = (*data.cast::<State>()).parser;
        (*data.cast::<State>()).nested_status = XML_ParserReset(parser, ptr::null()).into();
    }
}

#[test]
fn callback_reset_cannot_replace_live_core() {
    // SAFETY: Handle remains live until explicitly freed below.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetStartElementHandler(parser, Some(reset_on_start));
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        assert_eq!(state.nested_status, 0);
        assert_eq!(XML_GetErrorCode(parser), 0);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        XML_ParserFree(parser);
    }
}

unsafe extern "C" fn recursive_default(data: *mut c_void, _text: *const c_char, _len: c_int) {
    // SAFETY: The guard must reject this callback cycle without recursion.
    unsafe {
        XML_DefaultCurrent((*data.cast::<State>()).parser);
    }
}

#[test]
fn recursive_default_current_is_rejected() {
    // SAFETY: Test-owned State and parser remain live through the callback.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetElementHandler(parser, None, None);
        XML_SetDefaultHandlerExpand(parser, Some(recursive_default));
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), UNEXPECTED_STATE);
        XML_ParserFree(parser);
    }
}

#[test]
fn invalid_inputs_are_rejected_without_dereference() {
    // SAFETY: Intentionally invalid argument combinations are checked before use.
    unsafe {
        assert_eq!(XML_Parse(ptr::null_mut(), ptr::null(), 0, 1), ERROR);
        assert_eq!(XML_GetErrorCode(ptr::null_mut()), INVALID_ARGUMENT);
        XML_ParserFree(ptr::null_mut());
        let parser = XML_ParserCreate(ptr::null());
        assert_eq!(XML_Parse(parser, ptr::null(), 1, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), INVALID_ARGUMENT);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        assert!(XML_GetBuffer(parser, -1).is_null());
        XML_ParserFree(parser);
    }
}

#[test]
fn buffer_reservation_cannot_bypass_input_budget() {
    // SAFETY: The limit is checked before attempting a multi-gigabyte allocation.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert!(XML_GetBuffer(parser, c_int::MAX).is_null());
        assert_eq!(XML_GetErrorCode(parser), 43);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        (*parser).config.limits.max_total_bytes = 10;
        assert_eq!(XML_Parse(parser, c"<root>".as_ptr(), 6, 0), OK);
        assert!(XML_GetBuffer(parser, 5).is_null());
        assert_eq!(XML_GetErrorCode(parser), 43);
        XML_ParserFree(parser);
    }
}

unsafe extern "C" fn parse_independent_child(
    data: *mut c_void,
    _name: *const c_char,
    _attrs: *const *const c_char,
) {
    // SAFETY: A distinct parser can run while the parent callback is active.
    unsafe {
        let child = XML_ParserCreate(ptr::null());
        (*data.cast::<State>()).nested_status = XML_Parse(child, c"<child/>".as_ptr(), 8, 1);
        XML_ParserFree(child);
    }
}

#[test]
fn a_different_parser_can_run_from_a_callback() {
    // SAFETY: The outer test and callback each exclusively own their parser.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetStartElementHandler(parser, Some(parse_independent_child));
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        assert_eq!(state.nested_status, OK);
        assert_eq!(state.events, ["end:r"]);
        XML_ParserFree(parser);
    }
}

#[test]
fn incomplete_allocator_suite_is_rejected() {
    // SAFETY: The non-null suite is rejected without calling any function pointer.
    unsafe {
        let suite = XML_Memory_Handling_Suite {
            malloc_fcn: None,
            realloc_fcn: None,
            free_fcn: None,
        };
        assert!(XML_ParserCreate_MM(ptr::null(), &suite, ptr::null()).is_null());
        let parser = XML_ParserCreate_MM(ptr::null(), ptr::null(), c"|".as_ptr());
        assert!(!parser.is_null());
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        XML_ParserFree(parser);
    }
}

#[test]
fn base_can_be_set_from_its_existing_pointer() {
    // SAFETY: SetBase copies the incoming base before freeing existing storage.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert_eq!(XML_SetBase(parser, c"https://example.com/".as_ptr()), OK);
        assert_eq!(XML_SetBase(parser, XML_GetBase(parser)), OK);
        assert_eq!(CStr::from_ptr(XML_GetBase(parser)), c"https://example.com/");
        XML_ParserFree(parser);
    }
}

#[test]
fn namespace_constructors_reject_non_ascii_separator_bytes() {
    // SAFETY: Both constructors receive NULL encoding/suite pointers and a
    // readable separator byte; every successfully constructed handle is freed.
    unsafe {
        for separator in 0x80_u8..=0xff {
            let separator = separator as c_char;
            assert!(XML_ParserCreateNS(ptr::null(), separator).is_null());
            assert!(XML_ParserCreate_MM(ptr::null(), ptr::null(), &separator).is_null());
        }
        for separator in [0, b'\n', b'|', 0x7f] {
            let separator = separator as c_char;
            let parser = XML_ParserCreateNS(ptr::null(), separator);
            assert!(!parser.is_null());
            XML_ParserFree(parser);
            let parser = XML_ParserCreate_MM(ptr::null(), ptr::null(), &separator);
            assert!(!parser.is_null());
            XML_ParserFree(parser);
        }
    }
}

#[test]
fn namespace_triplets_and_specified_attributes() {
    // SAFETY: Callback pointers and state have the test's lifetime.
    unsafe {
        let mut state = State::default();
        let parser = XML_ParserCreateNS(ptr::null(), b'|' as c_char);
        state.parser = parser;
        XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
        XML_SetElementHandler(parser, Some(start), Some(end));
        XML_SetReturnNSTriplet(parser, 1);
        let document = b"<p:r xmlns:p='urn:r' p:x='value'/>";
        assert_eq!(
            XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
            OK
        );
        assert_eq!(
            state.events,
            ["start:urn:r|r|p", "urn:r|x|p=value", "end:urn:r|r|p"]
        );
        assert_eq!(XML_GetSpecifiedAttributeCount(parser), 2);
        XML_ParserFree(parser);
    }
}

#[test]
fn late_encoding_change_does_not_poison_incremental_parse() {
    // SAFETY: CPython performs this pattern for successive Unicode Parse calls.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert_eq!(XML_SetEncoding(parser, c"UTF-8".as_ptr()), OK);
        assert_eq!(XML_Parse(parser, c"<r>".as_ptr(), 3, 0), OK);
        assert_eq!(XML_SetEncoding(parser, c"UTF-8".as_ptr()), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 0);
        assert_eq!(XML_Parse(parser, c"hello</r>".as_ptr(), 9, 1), OK);
        XML_ParserFree(parser);
    }
}

unsafe extern "C" fn external_entity(
    parent: XML_Parser,
    context: *const c_char,
    _base: *const c_char,
    _system: *const c_char,
    _public: *const c_char,
) -> c_int {
    // SAFETY: The child owns a snapshot of its parent's environment and is distinct
    // from the active parent. Callback context remains live through construction.
    unsafe {
        let child = XML_ExternalEntityParserCreate(parent, context, ptr::null());
        if child.is_null() {
            return 0;
        }
        let document = b"<x/>&internal;tail";
        let result = XML_Parse(child, document.as_ptr().cast(), document.len() as c_int, 1);
        XML_ParserFree(child);
        result
    }
}

#[test]
fn external_callback_can_parse_an_inherited_fragment() {
    // SAFETY: Parent and child callbacks use the same serialized test-owned state.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetExternalEntityRefHandler(parser, Some(external_entity));
        let document = b"<!DOCTYPE r [<!ENTITY internal 'ok'><!ENTITY ext SYSTEM 'external.xml'>]><r>&ext;</r>";
        assert_eq!(
            XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
            OK
        );
        assert!(state.events.contains(&"start:x".to_owned()));
        assert_eq!(
            state
                .events
                .iter()
                .filter_map(|s| s.strip_prefix("text:"))
                .collect::<String>(),
            "oktail"
        );
        XML_ParserFree(parser);
    }
}

#[test]
fn child_survives_parent_deletion() {
    // SAFETY: An external child retains no pointers to its parent allocation.
    unsafe {
        let mut state = State::default();
        let parent = configured(&mut state);
        let child = XML_ExternalEntityParserCreate(parent, c"".as_ptr(), ptr::null());
        assert!(!child.is_null());
        XML_ParserFree(parent);
        state.parser = child;
        assert_eq!(XML_Parse(child, c"hello<a/><b/>".as_ptr(), 13, 1), OK);
        assert_eq!(
            state.events,
            ["text:hello", "start:a", "end:a", "start:b", "end:b"]
        );
        assert_eq!(XML_ParserReset(child, ptr::null()), 0);
        XML_ParserFree(child);
    }
}

#[test]
fn external_dtd_construction_and_parsing_do_not_taint_parent() {
    // SAFETY: The child parses DTD declarations and merges them into its live parent.
    unsafe {
        let parent = XML_ParserCreate(ptr::null());
        let child = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
        assert!(!child.is_null());
        assert_eq!(XML_GetErrorCode(parent), 0);
        assert_eq!(XML_Parse(child, c"<!ELEMENT r EMPTY>".as_ptr(), 18, 1), OK);
        assert_eq!(XML_GetErrorCode(parent), 0);
        XML_ParserFree(child);
        assert_eq!(XML_Parse(parent, c"<r/>".as_ptr(), 4, 1), OK);
        XML_ParserFree(parent);
    }
}

#[test]
fn external_children_share_input_and_construction_budgets() {
    // SAFETY: Limits are lowered directly in this internal test to avoid large allocations.
    unsafe {
        let parent = XML_ParserCreate(ptr::null());
        (*parent).config.limits.max_total_bytes = 6;
        let child = XML_ExternalEntityParserCreate(parent, c"".as_ptr(), ptr::null());
        assert!(!child.is_null());
        assert_eq!(XML_Parse(child, c"<x/>".as_ptr(), 4, 1), OK);
        assert_eq!(XML_Parse(parent, c"<r/>".as_ptr(), 4, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parent), 43);
        XML_ParserFree(child);
        assert_eq!(XML_ParserReset(parent, ptr::null()), 1);
        let family = &(*parent).family;
        family
            .children
            .store(MAX_FAMILY_CHILDREN, Ordering::Relaxed);
        assert!(XML_ExternalEntityParserCreate(parent, c"".as_ptr(), ptr::null()).is_null());
        assert_eq!(XML_GetErrorCode(parent), 43);
        XML_ParserFree(parent);
    }
}

#[test]
fn external_child_depth_is_bounded() {
    // SAFETY: Each child owns its snapshot; freeing ancestors does not reset depth.
    unsafe {
        let mut parser = XML_ParserCreate(ptr::null());
        for _ in 0..MAX_EXTERNAL_DEPTH {
            let child = XML_ExternalEntityParserCreate(parser, c"".as_ptr(), ptr::null());
            assert!(!child.is_null());
            XML_ParserFree(parser);
            parser = child;
        }
        assert!(XML_ExternalEntityParserCreate(parser, c"".as_ptr(), ptr::null()).is_null());
        assert_eq!(XML_GetErrorCode(parser), 43);
        XML_ParserFree(parser);
    }
}

unsafe extern "C" fn release_custom_encoding(data: *mut c_void) {
    // SAFETY: The test retains State through reset/free of the custom encoding.
    unsafe {
        (*data.cast::<State>()).releases += 1;
    }
}

unsafe extern "C" fn custom_encoding(
    data: *mut c_void,
    _name: *const c_char,
    info: *mut XML_Encoding,
) -> c_int {
    // SAFETY: Expat supplies a writable XML_Encoding for the callback duration.
    unsafe {
        for index in 0..256 {
            (*info).map[index] = index as i32;
        }
        (*info).map[128] = 0x20ac;
        (*info).data = data;
        (*info).release = Some(release_custom_encoding);
    }
    1
}

#[test]
fn custom_single_byte_encoding_and_release_ownership() {
    // SAFETY: Custom map callbacks retain only test-owned State.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetEncoding(parser, c"test-map".as_ptr());
        XML_SetUnknownEncodingHandler(
            parser,
            Some(custom_encoding),
            ptr::from_mut(&mut state).cast(),
        );
        let input = b"<r>\x80</r>";
        assert_eq!(
            XML_Parse(parser, input.as_ptr().cast(), input.len() as c_int, 1),
            OK
        );
        assert!(state.events.contains(&"text:€".to_owned()));
        assert_eq!(state.releases, 0);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        assert_eq!(state.releases, 1);
        XML_ParserFree(parser);
        assert_eq!(state.releases, 1);
    }
}

unsafe extern "C" fn reject_custom_encoding(
    data: *mut c_void,
    _name: *const c_char,
    info: *mut XML_Encoding,
) -> c_int {
    // SAFETY: A rejected callback's data is still released according to Expat.
    unsafe {
        (*info).data = data;
        (*info).release = Some(release_custom_encoding);
    }
    0
}

#[test]
fn rejected_custom_encoding_releases_data_once() {
    // SAFETY: The callback's State outlives the parser.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetEncoding(parser, c"rejected".as_ptr());
        XML_SetUnknownEncodingHandler(
            parser,
            Some(reject_custom_encoding),
            ptr::from_mut(&mut state).cast(),
        );
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 18);
        assert_eq!(state.releases, 1);
        XML_ParserFree(parser);
        assert_eq!(state.releases, 1);
    }
}

unsafe extern "C" fn free_custom_encoding(
    data: *mut c_void,
    name: *const c_char,
    info: *mut XML_Encoding,
) -> c_int {
    // SAFETY: Callback-time Free is ignored; the caller retains the live parser.
    unsafe {
        custom_encoding(data, name, info);
        XML_ParserFree((*data.cast::<State>()).parser);
    }
    1
}

#[test]
fn ignored_free_from_unknown_encoding_callback_preserves_map_ownership() {
    // SAFETY: The outer parse and eventual explicit free retain the encoding map ownership.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetEncoding(parser, c"free-map".as_ptr());
        XML_SetUnknownEncodingHandler(
            parser,
            Some(free_custom_encoding),
            ptr::from_mut(&mut state).cast(),
        );
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        assert_eq!(state.releases, 0);
        XML_ParserFree(parser);
        assert_eq!(state.releases, 1);
    }
}

unsafe extern "C" fn recursive_encoding_release(data: *mut c_void) {
    // SAFETY: Recursive Free during encoding release is ignored by the busy guard.
    unsafe {
        (*data.cast::<State>()).releases += 1;
        XML_ParserFree((*data.cast::<State>()).parser);
    }
}

#[test]
fn encoding_release_cannot_double_free_its_parser() {
    // SAFETY: Inject a test release callback with the same ownership as a map handler.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        (*parser).encoding_release = Some(recursive_encoding_release);
        (*parser).encoding_data = ptr::from_mut(&mut state).cast();
        XML_ParserFree(parser);
        assert_eq!(state.releases, 1);
    }
}

unsafe extern "C" fn stop_default_once(data: *mut c_void, value: *const c_char, len: c_int) {
    // SAFETY: Stop only after recording the fragment; resume must retain the tail.
    unsafe {
        text(data, value, len);
        let parser = (*data.cast::<State>()).parser;
        XML_SetDefaultHandlerExpand(parser, Some(text));
        XML_StopParser(parser, 1);
    }
}

#[test]
fn default_doctype_fragments_survive_suspension() {
    // SAFETY: All callback data and input are test-owned and serialized.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetElementHandler(parser, None, None);
        XML_SetDefaultHandlerExpand(parser, Some(stop_default_once));
        let input = b"<!DOCTYPE r SYSTEM 'some.dtd'><r/>";
        assert_eq!(
            XML_Parse(parser, input.as_ptr().cast(), input.len() as c_int, 1),
            SUSPENDED
        );
        assert_eq!(state.events, ["text:<!DOCTYPE"]);
        assert_eq!(XML_ResumeParser(parser), OK);
        assert_eq!(
            state
                .events
                .iter()
                .filter_map(|s| s.strip_prefix("text:"))
                .collect::<String>(),
            std::str::from_utf8(input).unwrap()
        );
        XML_ParserFree(parser);
    }
}

#[test]
fn external_dtd_declarations_are_available_to_the_parent() {
    // SAFETY: The child is parsed synchronously before the parent continues.
    unsafe {
        let mut state = State::default();
        let parent = configured(&mut state);
        let child = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
        assert!(!child.is_null());
        let dtd = b"<!ENTITY e 'works'><!ATTLIST r a CDATA 'default'>";
        assert_eq!(
            XML_Parse(child, dtd.as_ptr().cast(), dtd.len() as c_int, 1),
            OK
        );
        XML_ParserFree(child);
        let document = b"<!DOCTYPE r SYSTEM 'test.dtd'><r>&e;</r>";
        assert_eq!(
            XML_Parse(parent, document.as_ptr().cast(), document.len() as c_int, 1),
            OK
        );
        assert_eq!(
            state.events,
            ["start:r", "a=default", "text:works", "end:r"]
        );
        XML_ParserFree(parent);
    }
}

#[test]
fn manually_injected_dtd_requires_an_external_subset_context() {
    // SAFETY: Both parsers remain live and serialized. Like Expat, importing an
    // external declaration cannot make it an internal declaration of a document
    // that has no external subset or parameter entity references.
    unsafe {
        let parent = XML_ParserCreate(ptr::null());
        let child = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
        assert!(!child.is_null());
        let dtd = b"<!ENTITY e 'works'>";
        assert_eq!(
            XML_Parse(child, dtd.as_ptr().cast(), dtd.len() as c_int, 1),
            OK
        );
        XML_ParserFree(child);
        assert_eq!(XML_Parse(parent, c"<r>&e;</r>".as_ptr(), 10, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parent), 24);
        XML_ParserFree(parent);
    }
}

unsafe extern "C" fn foreign_dtd(
    parent: XML_Parser,
    context: *const c_char,
    _base: *const c_char,
    system: *const c_char,
    public: *const c_char,
) -> c_int {
    // SAFETY: UseForeignDTD supplies null identifiers for an implicit external DTD.
    unsafe {
        assert!(context.is_null());
        assert!(system.is_null());
        assert!(public.is_null());
        (*XML_GetUserData(parent).cast::<State>()).nested_status += 1;
        let child = XML_ExternalEntityParserCreate(parent, context, ptr::null());
        if child.is_null() {
            return 0;
        }
        let dtd = b"<!ENTITY supplied 'yes'>";
        let result = XML_Parse(child, dtd.as_ptr().cast(), dtd.len() as c_int, 1);
        XML_ParserFree(child);
        result
    }
}

#[test]
fn foreign_dtd_callback_runs_before_document_content() {
    // SAFETY: The callback creates and frees its own child while the parent pauses.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        assert_eq!(XML_UseForeignDTD(parser, 1), 0);
        assert_eq!(XML_SetParamEntityParsing(parser, 2), 1);
        XML_SetExternalEntityRefHandler(parser, Some(foreign_dtd));
        // CPython sets this immediately before the first Unicode Parse call.
        assert_eq!(XML_SetEncoding(parser, c"UTF-8".as_ptr()), OK);
        let document = b"<r>&supplied;</r>";
        assert_eq!(
            XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
            OK
        );
        assert_eq!(state.nested_status, 1);
        assert_eq!(state.events, ["start:r", "text:yes", "end:r"]);
        XML_ParserFree(parser);
    }
}

#[test]
fn allocation_tracker_enforces_live_memory_amplification() {
    // SAFETY: These limits exercise small allocations, not process exhaustion.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert_eq!(XML_SetAllocTrackerActivationThreshold(parser, 0), 1);
        assert_eq!(XML_SetAllocTrackerMaximumAmplification(parser, 1.0), 1);
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 1);
        XML_ParserFree(parser);

        let parser = XML_ParserCreate(ptr::null());
        assert_eq!(XML_SetAllocTrackerActivationThreshold(parser, 100_000), 1);
        assert_eq!(XML_SetAllocTrackerMaximumAmplification(parser, 1.0), 1);
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        XML_ParserFree(parser);
    }
}

#[test]
fn allocation_tracker_accepts_infinity_and_rejects_child_settings() {
    // SAFETY: Root and child remain live until their final frees below.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert_eq!(XML_SetAllocTrackerMaximumAmplification(parser, f32::NAN), 0);
        assert_eq!(XML_SetAllocTrackerMaximumAmplification(parser, 0.0), 0);
        assert_eq!(
            XML_SetAllocTrackerMaximumAmplification(parser, f32::INFINITY),
            1
        );
        assert_eq!(XML_SetAllocTrackerActivationThreshold(parser, 0), 1);
        let child = XML_ExternalEntityParserCreate(parser, c"".as_ptr(), ptr::null());
        assert!(!child.is_null());
        assert_eq!(XML_SetAllocTrackerMaximumAmplification(child, 2.0), 0);
        assert_eq!(XML_SetAllocTrackerActivationThreshold(child, 0), 0);
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        XML_ParserFree(child);
        XML_ParserFree(parser);
    }
}

#[test]
fn public_memory_helpers_account_for_and_release_live_bytes() {
    // SAFETY: Each pointer is used only through its original parser's memory API.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        let tracker = Shared::clone(&(*parser).tracker);
        let initial = tracker.live_bytes();
        let memory = XML_MemMalloc(parser, 1000);
        assert!(!memory.is_null());
        assert!(tracker.live_bytes() >= initial + 1000);
        let memory = XML_MemRealloc(parser, memory, 2000);
        assert!(!memory.is_null());
        assert!(tracker.live_bytes() >= initial + 2000);
        XML_MemFree(parser, memory);
        assert_eq!(tracker.live_bytes(), initial);
        XML_ParserFree(parser);
        assert_eq!(tracker.live_bytes(), 0);
    }
}

#[test]
fn content_model_outlives_parent_and_retains_its_tracker() {
    // SAFETY: A model owns its allocator and can be freed after its parser.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        let tracker = Shared::clone(&(*parser).tracker);
        let model = with_parser_tracking(parser, || {
            content_model::allocate("(a,b+)", (*parser).allocator)
        })
        .unwrap();
        XML_ParserFree(parser);
        assert!(tracker.live_bytes() > 0);
        assert_eq!((*model).numchildren, 2);
        XML_FreeContentModel(ptr::null_mut(), model);
        assert_eq!(tracker.live_bytes(), 0);
    }
}

#[test]
fn an_old_dtd_child_cannot_modify_a_reset_parent_document() {
    // SAFETY: The old child remains independently usable, but reset invalidates
    // its parent-generation token before constructing the new document state.
    unsafe {
        let parent = XML_ParserCreate(ptr::null());
        let child = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
        assert!(!child.is_null());
        assert_eq!(XML_ParserReset(parent, ptr::null()), 1);
        let dtd = b"<!ENTITY stale 'must not leak'>";
        assert_eq!(
            XML_Parse(child, dtd.as_ptr().cast(), dtd.len() as c_int, 1),
            OK
        );
        XML_ParserFree(child);
        let document = b"<r>&stale;</r>";
        assert_eq!(
            XML_Parse(parent, document.as_ptr().cast(), document.len() as c_int, 1),
            ERROR
        );
        assert_eq!(XML_GetErrorCode(parent), 11);
        XML_ParserFree(parent);
    }
}

#[test]
fn unresolved_external_general_entity_reaches_the_default_handler() {
    // SAFETY: The default callback owns no parser references and records raw input.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetDefaultHandlerExpand(parser, Some(text));
        let document = b"<!DOCTYPE r [<!ENTITY ext SYSTEM 'external'>]><r>&ext;</r>";
        assert_eq!(
            XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
            OK
        );
        assert!(state.events.contains(&"text:&ext;".to_owned()));
        XML_ParserFree(parser);
    }
}

#[test]
fn setting_encoding_preserves_default_handler_and_deferral_modes() {
    // SAFETY: The settings are applied before parsing, as CPython does for strings.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        XML_SetDefaultHandler(parser, Some(text));
        assert_eq!(XML_SetReparseDeferralEnabled(parser, 0), 1);
        assert_eq!(XML_SetEncoding(parser, c"UTF-8".as_ptr()), OK);
        assert!(!(*parser).core.reparse_deferral_enabled());
        let document = b"<!DOCTYPE r [<!ENTITY e 'expanded'>]><r>&e;</r>";
        assert_eq!(
            XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
            OK
        );
        assert!(state.events.contains(&"text:&e;".to_owned()));
        assert!(!state.events.contains(&"text:expanded".to_owned()));
        XML_ParserFree(parser);
    }
}

#[test]
fn default_handler_receives_outside_root_whitespace_without_character_data() {
    // SAFETY: Test-owned state remains live through callbacks and parser cleanup.
    unsafe {
        for expand in [false, true] {
            let mut state = State::default();
            let parser = configured(&mut state);
            if expand {
                XML_SetDefaultHandlerExpand(parser, Some(text));
            } else {
                XML_SetDefaultHandler(parser, Some(text));
            }
            let document = b" \r\n<r/>\t";
            assert_eq!(
                XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
                OK
            );
            assert_eq!(
                state
                    .events
                    .iter()
                    .filter_map(|event| event.strip_prefix("text:"))
                    .collect::<String>(),
                " \r\n\t"
            );
            assert_eq!(
                state
                    .events
                    .iter()
                    .filter(|event| !event.starts_with("text:"))
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                ["start:r", "end:r"]
            );
            assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
            state.events.clear();
            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
            XML_SetCharacterDataHandler(parser, Some(text));
            assert_eq!(
                XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
                OK
            );
            assert!(state.events.is_empty());
            XML_ParserFree(parser);
        }
    }
}

#[test]
fn invalid_api_requests_do_not_poison_subsequent_input() {
    // SAFETY: All handles and input buffers are test-owned; no invalid pointer is read.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert_eq!(XML_ParseBuffer(parser, 0, 0), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 42);
        assert!(!XML_GetBuffer(parser, 4).is_null());
        assert_eq!(XML_GetErrorCode(parser), 42);
        assert_eq!(XML_ParseBuffer(parser, 0, 0), OK);
        assert_eq!(XML_ParseBuffer(parser, 0, 0), OK);
        assert!(XML_GetBuffer(parser, -1).is_null());
        assert_eq!(XML_GetErrorCode(parser), 1);
        assert!(XML_GetBuffer(parser, c_int::MAX).is_null());
        assert!(!XML_GetBuffer(parser, 4).is_null());
        assert_eq!(XML_Parse(parser, ptr::null(), 1, 0), ERROR);
        assert_eq!(XML_GetErrorCode(parser), INVALID_ARGUMENT);
        assert_eq!(XML_ResumeParser(parser), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 34);
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        assert_eq!(XML_GetErrorCode(parser), 0);
        XML_ParserFree(parser);
    }
}

unsafe extern "C" fn suspend_twice(data: *mut c_void, _text: *const c_char, _length: c_int) {
    // SAFETY: The callback owns no parser reference across API calls.
    unsafe {
        let parser = (*data.cast::<State>()).parser;
        assert_eq!(XML_StopParser(parser, 1), OK);
        assert_eq!(XML_StopParser(parser, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 33);
    }
}

#[test]
fn rejected_suspended_operations_preserve_resumable_input() {
    // SAFETY: Test-owned state and parser stay live until explicit cleanup.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        assert_eq!(XML_StopParser(parser, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 44);
        XML_SetCharacterDataHandler(parser, Some(suspend_twice));
        assert_eq!(XML_Parse(parser, c"<r>text</r>".as_ptr(), 11, 1), SUSPENDED);
        assert_eq!(XML_GetErrorCode(parser), 0);
        assert_eq!(XML_Parse(parser, ptr::null(), 0, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 33);
        assert!(XML_GetBuffer(parser, 1).is_null());
        assert_eq!(XML_ParseBuffer(parser, 0, 0), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 33);
        XML_SetCharacterDataHandler(parser, None);
        assert_eq!(XML_ResumeParser(parser), OK);
        assert_eq!(XML_GetErrorCode(parser), 0);
        assert_eq!(state.events, ["start:r", "end:r"]);
        XML_ParserFree(parser);
    }
}

#[test]
fn syntax_errors_remain_terminal_after_successful_buffer_requests() {
    // SAFETY: All buffers are readable for their stated lengths.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        assert_eq!(XML_Parse(parser, c"<r></s>".as_ptr(), 7, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 7);
        let emitted = state.events.len();
        let buffer = XML_GetBuffer(parser, 4);
        assert!(!buffer.is_null());
        ptr::copy_nonoverlapping(b"<x/>".as_ptr(), buffer.cast(), 4);
        assert_eq!(XML_ParseBuffer(parser, 4, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 7);
        assert_eq!(state.events.len(), emitted);
        XML_ParserFree(parser);
    }
}

#[test]
fn default_whitespace_counts_toward_the_shared_event_budget() {
    // SAFETY: The test controls the parser family counter before parsing starts.
    unsafe {
        for remaining in [1, 2] {
            let mut state = State::default();
            let parser = configured(&mut state);
            XML_SetDefaultHandlerExpand(parser, Some(text));
            let family = &(*parser).family;
            family
                .callback_bytes
                .store(MAX_FAMILY_CALLBACK_BYTES - remaining, Ordering::Relaxed);
            let status = XML_Parse(parser, c"  ".as_ptr(), 2, 0);
            if remaining == 1 {
                assert_eq!(status, ERROR);
                assert_eq!(XML_GetErrorCode(parser), 43);
                assert!(state.events.is_empty());
            } else {
                assert_eq!(status, OK);
                assert_eq!(state.events, ["text:  "]);
            }
            XML_ParserFree(parser);
        }
    }
}

#[test]
fn metadata_payloads_enforce_exact_shared_event_budget_boundaries() {
    fn word() -> XmlString {
        XmlString::try_from_str_in("x", Allocator::System).unwrap()
    }
    fn event(case: usize) -> EventKind {
        match case {
            0 => EventKind::XmlDeclaration {
                version: word(),
                encoding: Some(word()),
                standalone: None,
            },
            1 => EventKind::TextDeclaration {
                version: Some(word()),
                encoding: word(),
            },
            2 => EventKind::ExternalEntityReference {
                context: Some(word()),
                system_id: Some(word()),
                public_id: Some(word()),
            },
            3 => EventKind::StartDoctype {
                name: word(),
                system_id: Some(word()),
                public_id: Some(word()),
                has_internal_subset: false,
            },
            4 => EventKind::StartNamespace {
                prefix: Some(word()),
                uri: Some(word()),
            },
            5 => EventKind::EndNamespace {
                prefix: Some(word()),
            },
            6 => EventKind::EntityDeclaration {
                name: word(),
                value: Some(word()),
                parameter: false,
                system_id: Some(word()),
                public_id: Some(word()),
                notation: Some(word()),
            },
            7 => EventKind::AttlistDeclaration {
                element: word(),
                name: word(),
                attribute_type: word(),
                default: Some(word()),
                required: false,
            },
            8 => EventKind::NotationDeclaration {
                name: word(),
                system_id: Some(word()),
                public_id: Some(word()),
            },
            9 => EventKind::SkippedEntity {
                name: word(),
                parameter: false,
            },
            _ => unreachable!(),
        }
    }
    // Each populated string carries one byte. Dispatch must account for every
    // metadata field exactly once, independently of whether a handler is installed.
    for (case, bytes) in [2, 2, 3, 3, 2, 1, 5, 4, 3, 1].into_iter().enumerate() {
        for base in [false, true] {
            let bytes = bytes + usize::from(base && matches!(case, 2 | 6 | 8));
            for remaining in [bytes - 1, bytes] {
                // SAFETY: The test owns a live handle and manually holds the dispatch
                // guard; no callback is installed and the event owns all its strings.
                unsafe {
                    let parser = XML_ParserCreate(ptr::null());
                    if base {
                        assert_eq!(XML_SetBase(parser, c"x".as_ptr()), OK);
                    }
                    let family = &(*parser).family;
                    family
                        .callback_bytes
                        .store(MAX_FAMILY_CALLBACK_BYTES - remaining, Ordering::Relaxed);
                    (*parser).busy = true;
                    dispatch(parser, event(case)).unwrap();
                    (*parser).busy = false;
                    assert_eq!(
                        XML_GetErrorCode(parser),
                        if remaining < bytes { 43 } else { 0 },
                        "metadata case {case}"
                    );
                    let family = &(*parser).family;
                    assert_eq!(
                        family.callback_bytes.load(Ordering::Relaxed),
                        if remaining < bytes {
                            MAX_FAMILY_CALLBACK_BYTES - remaining
                        } else {
                            MAX_FAMILY_CALLBACK_BYTES
                        }
                    );
                    XML_ParserFree(parser);
                }
            }
        }
    }
}
