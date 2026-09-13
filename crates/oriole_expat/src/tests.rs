use super::*;

#[test]
fn header_implementation_version_matches_package_version() {
    let definition = format!("#  define ORIOLE_VERSION \"{}\"", env!("CARGO_PKG_VERSION"));
    assert!(
        include_str!("../../../include/expat.h")
            .lines()
            .any(|line| line == definition)
    );
}

#[test]
fn feature_metadata_and_null_attribute_count_match_expat() {
    // SAFETY: The feature table is static and terminated by XML_FEATURE_END.
    unsafe {
        assert_eq!(XML_GetSpecifiedAttributeCount(ptr::null_mut()), -1);
        let mut features = XML_GetFeatureList();
        let mut enabled = Vec::new();
        while (*features).feature != 0 {
            enabled.push((
                (*features).feature,
                CStr::from_ptr((*features).name),
                (*features).value,
            ));
            features = features.add(1);
        }
        assert!(enabled.contains(&(3, c"XML_DTD", 0)));
        assert!(enabled.contains(&(13, c"XML_GE", 0)));
        assert!(!enabled.iter().any(|(feature, _, _)| *feature == 10));
    }
}

#[test]
fn entity_amplification_controls_apply_after_suspension_and_reset_to_defaults() {
    unsafe extern "C" fn suspend(data: *mut c_void, _: *const c_char, _: *const *const c_char) {
        // SAFETY: Parser-as-handler-argument supplies the active test parser.
        unsafe { assert_eq!(XML_StopParser(data.cast(), 1), OK) };
    }
    // SAFETY: Buffers and the parser remain live through each synchronous API call.
    unsafe {
        let document = format!("<!DOCTYPE r [<!ENTITY e '{}'>]><r>&e;</r>", "a".repeat(100));
        for buffered in [false, true] {
            let parser = XML_ParserCreate(ptr::null());
            assert!(!parser.is_null());
            assert_eq!(
                XML_SetBillionLaughsAttackProtectionMaximumAmplification(ptr::null_mut(), 2.0),
                0
            );
            assert_eq!(
                XML_SetBillionLaughsAttackProtectionActivationThreshold(ptr::null_mut(), 0),
                0
            );
            for invalid in [f32::NAN, f32::NEG_INFINITY, -1.0, 0.0, 0.5] {
                assert_eq!(
                    XML_SetBillionLaughsAttackProtectionMaximumAmplification(parser, invalid),
                    0
                );
            }
            XML_UseParserAsHandlerArg(parser);
            XML_SetStartElementHandler(parser, Some(suspend));
            let status = if buffered {
                let buffer = XML_GetBuffer(parser, document.len() as c_int);
                assert!(!buffer.is_null());
                ptr::copy_nonoverlapping(document.as_ptr(), buffer.cast(), document.len());
                XML_ParseBuffer(parser, document.len() as c_int, 1)
            } else {
                XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1)
            };
            assert_eq!(status, SUSPENDED);
            assert_eq!(
                XML_SetBillionLaughsAttackProtectionMaximumAmplification(parser, 1.0),
                1
            );
            assert_eq!(
                XML_SetBillionLaughsAttackProtectionActivationThreshold(parser, 0),
                1
            );
            assert_eq!(XML_ResumeParser(parser), ERROR);
            assert_eq!(XML_GetErrorCode(parser), 43);
            assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
            assert_eq!(
                XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
                OK
            );
            XML_ParserFree(parser);
        }
    }
}

#[derive(Default)]
struct State {
    parser: XML_Parser,
    events: Vec<String>,
    nested_status: c_int,
    releases: usize,
}

unsafe extern "C" fn ignore_entity_declaration(
    _: *mut c_void,
    _: *const c_char,
    _: c_int,
    _: *const c_char,
    _: c_int,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
) {
}

unsafe extern "C" fn ignore_attlist_declaration(
    _: *mut c_void,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: c_int,
) {
}

fn declaration_default_text(
    input: &str,
    standalone: bool,
    mode: c_int,
    handlers: bool,
    width: usize,
) -> String {
    // SAFETY: All handles, buffers and callback state are test-owned until cleanup.
    unsafe {
        let root = XML_ParserCreate(ptr::null());
        let declaration = if standalone {
            c"<?xml version='1.0' standalone='yes'?>"
        } else {
            c"<?xml version='1.0'?>"
        };
        assert_eq!(
            XML_Parse(
                root,
                declaration.as_ptr(),
                declaration.to_bytes().len() as c_int,
                0
            ),
            OK
        );
        let child = XML_ExternalEntityParserCreate(root, ptr::null(), ptr::null());
        assert!(!child.is_null());
        let mut state = State::default();
        XML_SetUserData(child, ptr::from_mut(&mut state).cast());
        XML_SetDefaultHandlerExpand(child, Some(text));
        assert_eq!(XML_SetParamEntityParsing(child, mode), 1);
        if handlers {
            XML_SetEntityDeclHandler(child, Some(ignore_entity_declaration));
            XML_SetAttlistDeclHandler(child, Some(ignore_attlist_declaration));
        }
        let count = input.len().div_ceil(width);
        for (index, bytes) in input.as_bytes().chunks(width).enumerate() {
            assert_eq!(
                XML_Parse(
                    child,
                    bytes.as_ptr().cast(),
                    bytes.len() as c_int,
                    c_int::from(index + 1 == count)
                ),
                OK,
                "{input}"
            );
        }
        XML_ParserFree(child);
        XML_ParserFree(root);
        state
            .events
            .iter()
            .filter_map(|event| event.strip_prefix("text:"))
            .collect()
    }
}

#[test]
fn skipped_declaration_references_deliver_each_default_byte_once() {
    for (input, skipped) in [
        ("<!ENTITY e %missing;\"X\">", "%missing;\"X\">"),
        (
            "<!ATTLIST r %missing;a CDATA \"X\">",
            "%missing;a CDATA \"X\">",
        ),
        (
            "<!ATTLIST r a CDATA \"X\" %missing; b CDATA \"Y\">",
            "%missing; b CDATA \"Y\">",
        ),
    ] {
        for standalone in [false, true] {
            for handlers in [false, true] {
                for mode in [0, 2] {
                    for width in 1..=input.len() {
                        let expected = if !handlers {
                            input
                        } else if standalone {
                            "%missing;"
                        } else {
                            skipped
                        };
                        assert_eq!(
                            declaration_default_text(input, standalone, mode, handlers, width),
                            expected
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn duplicate_entities_preserve_handler_dependent_default_tokens() {
    for (second, sparse) in [
        ("<!ENTITY e 'second'>", "e'second'"),
        ("<!ENTITY e SYSTEM 'sys'>", "e'sys'>"),
        ("<!ENTITY e PUBLIC 'pub' 'sys'>", "e'pub''sys'>"),
        ("<!ENTITY e SYSTEM 'sys' NDATA n>", "e'sys'n"),
        ("<!ENTITY e 'L%missing;R'>", "e'L%missing;R'>"),
    ] {
        let input = format!("<!ENTITY e 'first'>{second}");
        for handlers in [false, true] {
            for width in 1..=input.len() {
                assert_eq!(
                    declaration_default_text(&input, false, 2, handlers, width),
                    if handlers { sparse } else { &input }
                );
            }
        }
    }
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
        let limits = (*parser).config.limits.clone();
        (*parser).core.set_limits(limits).unwrap();
        assert_eq!(XML_Parse(parser, c"<root>".as_ptr(), 6, 0), OK);
        assert!(XML_GetBuffer(parser, 5).is_null());
        assert_eq!(XML_GetErrorCode(parser), 43);
        XML_ParserFree(parser);
    }
}

#[test]
fn source_bound_preflight_preserves_family_and_input_storage() {
    // SAFETY: The handles and buffers belong to this test. Lower only the core
    // allowance so the per-source preflight rejects before family mutation.
    unsafe {
        for buffered in [false, true] {
            let parser = XML_ParserCreate(ptr::null());
            assert!(!parser.is_null());
            let mut limits = (*parser).config.limits.clone();
            limits.max_total_bytes = 3;
            (*parser).core.set_limits(limits).unwrap();
            if buffered {
                let buffer = XML_GetBuffer(parser, 3);
                assert!(!buffer.is_null());
                ptr::copy_nonoverlapping(b"<r>".as_ptr(), buffer.cast(), 3);
                assert_eq!(XML_ParseBuffer(parser, 3, 0), OK);
            } else {
                assert_eq!(XML_Parse(parser, c"<r>".as_ptr(), 3, 0), OK);
            }
            let direct = (*parser).tracker.direct_bytes();
            let input = {
                let family = &(*parser).family;
                family.input_bytes.get()
            };
            let context = (*parser).core.input_context().0.to_vec();
            let context_start = (*parser).core.input_context().1;
            let buffer_capacity = (*parser).buffer.capacity();
            assert_eq!((*parser).core.input_bytes_remaining(), 0);
            assert!(XML_GetBuffer(parser, 1).is_null());
            assert_eq!(XML_GetErrorCode(parser), 43);
            assert_eq!(XML_Parse(parser, c"x".as_ptr(), 1, 1), ERROR);
            assert_eq!(XML_GetErrorCode(parser), 43);
            assert_eq!((*parser).tracker.direct_bytes(), direct);
            {
                let family = &(*parser).family;
                assert_eq!(family.input_bytes.get(), input);
            }
            assert_eq!((*parser).core.input_context().0, context);
            assert_eq!((*parser).core.input_context().1, context_start);
            assert_eq!((*parser).buffer.capacity(), buffer_capacity);
            assert!(!(*parser).final_buffer);
            XML_ParserFree(parser);
        }
    }
}

#[test]
fn request_bound_and_public_positions_do_not_depend_on_lifetime_quota() {
    // SAFETY: No large allocation is attempted; the single-request gate fires
    // before reservation. Scalar coordinates are seeded to test public widths.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert!(!parser.is_null());
        (*parser).config.limits.max_total_bytes = c_long::MAX as usize;
        (*parser)
            .core
            .set_limits((*parser).config.limits.clone())
            .unwrap();
        let live = (*parser).tracker.live_bytes();
        assert!(XML_GetBuffer(parser, (MAX_INPUT_BYTES + 1) as c_int).is_null());
        assert_eq!(XML_GetErrorCode(parser), 43);
        assert_eq!((*parser).tracker.live_bytes(), live);
        {
            let family = &(*parser).family;
            assert_eq!(family.input_bytes.get(), 0);
        }
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        (*parser).position.byte_index = c_long::MAX as usize;
        (*parser).position.line = c_long::MAX as usize + 1;
        (*parser).position.column = c_long::MAX as usize;
        assert_eq!(XML_GetCurrentByteIndex(parser), c_long::MAX);
        assert_eq!(XML_GetCurrentLineNumber(parser), c_long::MAX as c_ulong + 1);
        assert_eq!(XML_GetCurrentColumnNumber(parser), c_long::MAX as c_ulong);
        XML_ParserFree(parser);
    }
}

#[test]
fn consumed_work_credit_survives_old_children_but_not_root_reset() {
    unsafe extern "C" fn suspend(arg: *mut c_void, _: *const c_char, _: *const *const c_char) {
        // SAFETY: Parser-as-handler-argument supplies the active test parser.
        unsafe { assert_eq!(XML_StopParser(arg.cast(), 1), OK) };
    }
    // SAFETY: Each handle is owned by this test. Scalar reads and seeded work
    // counters occur between API calls; no parser borrow crosses a callback.
    unsafe {
        let parent = XML_ParserCreate(ptr::null());
        assert!(!parent.is_null());
        assert_eq!((*parent).config.limits.max_work_amplification, Some(100));
        assert_eq!(
            (*parent).core.input_bytes_remaining(),
            (c_long::MAX as usize).min(isize::MAX as usize)
        );
        XML_UseParserAsHandlerArg(parent);
        XML_SetElementHandler(parent, Some(suspend), None);
        let document = format!("<r>{}</r>", "x".repeat(700 * 1024));
        assert_eq!(
            XML_Parse(parent, document.as_ptr().cast(), document.len() as c_int, 1),
            SUSPENDED
        );
        // Only the opening tag has been consumed. The large buffered suffix and
        // allocation tracker's whole-feed credit must not enlarge work credit.
        assert_eq!((*parent).tracker.direct_bytes(), document.len() as u64);
        assert_eq!((*parent).core.work_bytes_limit(0), 300);
        let general = XML_ExternalEntityParserCreate(parent, c"".as_ptr(), ptr::null());
        let parameter = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
        assert!(!general.is_null());
        assert!(!parameter.is_null());
        assert_eq!((*general).core.work_bytes_limit(0), 300);
        assert_eq!((*parameter).core.work_bytes_limit(0), 300);
        XML_SetElementHandler(parent, None, None);
        assert_eq!(XML_ResumeParser(parent), OK);
        let earned = 100 * document.len();
        assert!(earned > INITIAL_CALLBACK_BYTES);
        assert_eq!((*parent).core.work_bytes_limit(0), earned);
        assert_eq!((*general).core.work_bytes_limit(0), earned);
        assert_eq!((*parameter).core.work_bytes_limit(0), earned);
        assert_eq!(XML_ParserReset(parent, ptr::null()), 1);
        assert_eq!((*parent).config.limits.max_work_amplification, Some(100));
        assert_eq!((*parent).core.work_bytes_limit(0), 0);
        assert_eq!((*general).core.work_bytes_limit(0), earned);
        assert_eq!((*parameter).core.work_bytes_limit(0), earned);
        {
            let family = &(*general).family;
            family.callback_bytes.set(INITIAL_CALLBACK_BYTES);
        }
        XML_SetElementHandler(general, None, None);
        assert_eq!(XML_Parse(general, c"<c/>".as_ptr(), 4, 1), OK);
        {
            let family = &(*general).family;
            assert_eq!(family.callback_bytes.get(), INITIAL_CALLBACK_BYTES + 2);
        }
        assert_eq!((*general).core.work_bytes_limit(0), earned);
        assert_eq!((*parent).core.work_bytes_limit(0), 0);
        XML_ParserFree(parent);
        assert_eq!((*parameter).core.work_bytes_limit(0), earned);
        XML_ParserFree(general);
        XML_ParserFree(parameter);
    }
}

#[test]
fn family_input_statistic_cannot_overflow_or_grant_work_credit() {
    // SAFETY: Only the test's shared statistic is seeded; all actual input is
    // a small valid slice, and the parser is freed after the checked rejection.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert!(!parser.is_null());
        {
            let family = &(*parser).family;
            family.input_bytes.set(usize::MAX - 2);
        }
        assert_eq!((*parser).core.work_bytes_limit(0), 0);
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 43);
        {
            let family = &(*parser).family;
            assert_eq!(family.input_bytes.get(), usize::MAX - 2);
        }
        assert_eq!((*parser).tracker.direct_bytes(), 0);
        assert_eq!((*parser).core.work_bytes_limit(0), 0);
        assert!((*parser).core.input_context().0.is_empty());
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
fn nul_namespace_separator_ignores_triplet_mode() {
    // SAFETY: Callback state and input remain live until the parser is freed.
    unsafe {
        let mut state = State::default();
        let parser = XML_ParserCreateNS(ptr::null(), 0);
        state.parser = parser;
        XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
        XML_SetElementHandler(parser, Some(start), Some(end));
        XML_SetReturnNSTriplet(parser, 1);
        let document = b"<p:r xmlns:p='a' p:b='1' ab='2'/>";
        assert_eq!(
            XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
            OK,
        );
        assert_eq!(state.events, ["start:ar", "ab=1", "ab=2", "end:ar"]);
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
fn external_declaration_errors_keep_their_codes_in_each_child_context() {
    struct DeclarationCase {
        input: &'static [u8],
        width: usize,
        expected: c_int,
        parameter: bool,
        calls: usize,
    }
    unsafe extern "C" fn external(
        parent: XML_Parser,
        context: *const c_char,
        _: *const c_char,
        _: *const c_char,
        _: *const c_char,
    ) -> c_int {
        // SAFETY: State and input are test-owned, and the child is freed before
        // returning to its parent. No borrowed state is held across parser calls.
        unsafe {
            let state = XML_GetUserData(parent).cast::<DeclarationCase>();
            let (input, width, expected, parameter) = (
                (*state).input,
                (*state).width,
                (*state).expected,
                (*state).parameter,
            );
            assert_eq!(context.is_null(), parameter);
            let child = XML_ExternalEntityParserCreate(parent, context, ptr::null());
            assert!(!child.is_null());
            let mut status = OK;
            for (index, bytes) in input.chunks(width).enumerate() {
                status = XML_Parse(
                    child,
                    bytes.as_ptr().cast(),
                    bytes.len() as c_int,
                    c_int::from((index + 1) * width >= input.len()),
                );
                if status == ERROR {
                    break;
                }
            }
            assert_eq!(status, ERROR);
            assert_eq!(XML_GetErrorCode(child), expected);
            let position = (
                XML_GetCurrentByteIndex(child),
                XML_GetCurrentLineNumber(child),
                XML_GetCurrentColumnNumber(child),
            );
            assert_eq!(XML_Parse(child, c"".as_ptr(), 0, 1), ERROR);
            assert_eq!(XML_GetErrorCode(child), expected);
            assert_eq!(
                (
                    XML_GetCurrentByteIndex(child),
                    XML_GetCurrentLineNumber(child),
                    XML_GetCurrentColumnNumber(child)
                ),
                position
            );
            XML_ParserFree(child);
            (*state).calls += 1;
            ERROR
        }
    }
    let documents = [
        (
            b"<!DOCTYPE r [<!ENTITY e SYSTEM 'e'>]><r>&e;</r>".as_slice(),
            false,
        ),
        (b"<!DOCTYPE r SYSTEM 'e'><r/>", true),
        (b"<!DOCTYPE r [<!ENTITY % e SYSTEM 'e'>%e;]><r/>", true),
    ];
    for (input, expected) in [
        (b"<?xml version='1.0'?>".as_slice(), 31),
        (
            b"<?xml version='1.0' encoding='UTF-8' standalone='yes'?>",
            31,
        ),
        (b"<?xml version='1.0' encoding 'UTF-8'?>", 31),
        (b"<?XML encoding='UTF8'?>", 4),
        (b"<?xml encoding='UTF8' version='1.0'?>", 31),
        (b"<?xml version='1.0' encoding='UTF8' extra='x'?>", 31),
        (b"<?xml version='1.0' encoding='UTF8'?>", 18),
    ] {
        for (document, parameter) in documents {
            for width in [1, 7, input.len()] {
                let mut state = DeclarationCase {
                    input,
                    width,
                    expected,
                    parameter,
                    calls: 0,
                };
                // SAFETY: Callback state stays live through all parent/child calls.
                unsafe {
                    let parent = XML_ParserCreate(ptr::null());
                    assert!(!parent.is_null());
                    XML_SetUserData(parent, ptr::from_mut(&mut state).cast());
                    XML_SetExternalEntityRefHandler(parent, Some(external));
                    assert_eq!(XML_SetParamEntityParsing(parent, 2), 1);
                    assert_eq!(
                        XML_Parse(parent, document.as_ptr().cast(), document.len() as c_int, 1),
                        ERROR
                    );
                    assert_eq!(XML_GetErrorCode(parent), 21);
                    XML_ParserFree(parent);
                }
                assert_eq!(state.calls, 1);
            }
        }
    }
}

#[test]
fn unknown_encoding_callbacks_follow_complete_declaration_grammar() {
    #[derive(Default)]
    struct EncodingState {
        requested: Vec<String>,
        accept: bool,
        releases: usize,
        remapping: u8,
        conversions: usize,
    }
    unsafe extern "C" fn release(data: *mut c_void) {
        // SAFETY: Test state outlives every parser using this encoding instance.
        unsafe {
            (*data.cast::<EncodingState>()).releases += 1;
        }
    }
    unsafe extern "C" fn convert(data: *mut c_void, bytes: *const c_char) -> c_int {
        // SAFETY: The registered map requests exactly two bytes; the state lives
        // until the parser releases the encoding instance.
        unsafe {
            assert_eq!(std::slice::from_raw_parts(bytes.cast::<u8>(), 2), b"@$");
            (*data.cast::<EncodingState>()).conversions += 1;
        }
        i32::from(b'1')
    }
    unsafe extern "C" fn unknown(
        data: *mut c_void,
        name: *const c_char,
        info: *mut XML_Encoding,
    ) -> c_int {
        // SAFETY: The name and writable record belong to this callback; state
        // remains live until the parser and any child have both been destroyed.
        unsafe {
            let state = data.cast::<EncodingState>();
            (*state)
                .requested
                .push(CStr::from_ptr(name).to_str().unwrap().to_owned());
            if !(*state).accept {
                return ERROR;
            }
            (*info).map = std::array::from_fn(|index| index as i32);
            match (*state).remapping {
                1 => (*info).map[1] = 0x100,
                2 => {
                    (*info).map[usize::from(b'@')] = -2;
                    (*info).convert = Some(convert);
                }
                _ => {}
            }
            (*info).data = data;
            (*info).release = Some(release);
            OK
        }
    }
    // Unrequested ASCII, explicit UTF-8, explicit custom, sniffed UTF-16,
    // and ASCII bytes with a mismatching UTF-16 declaration.
    for mode in 0..5 {
        // Context: root, directly created general child, parameter/subset child.
        for context in 0..3 {
            for case in 0..4 {
                let encoding = if mode >= 3 { "UTF-16" } else { "UTF8" };
                let header = match case {
                    0 => format!("<?xml encoding='{encoding}' version='1.0'?>"),
                    1 => format!("<?xml version='1.0' encoding='{encoding}' extra='x'?>"),
                    2 => format!("<?xml version='1.0' encoding='{encoding}'?>"),
                    _ => format!("<?XML encoding='{encoding}'?>"),
                };
                let document = if context == 2 {
                    header.clone()
                } else {
                    format!("{header}<r/>")
                };
                for bom in [false, true] {
                    // Explicit custom encodings reject a BOM before grammar;
                    // UTF-16 uses its own BOM in its single iteration.
                    if bom && mode != 0 {
                        continue;
                    }
                    let bytes = if mode == 3 {
                        [
                            b"\xff\xfe".as_slice(),
                            &document
                                .encode_utf16()
                                .flat_map(u16::to_le_bytes)
                                .collect::<Vec<_>>(),
                        ]
                        .concat()
                    } else if bom {
                        [b"\xef\xbb\xbf".as_slice(), document.as_bytes()].concat()
                    } else {
                        document.as_bytes().to_vec()
                    };
                    for accept in [false, true] {
                        let calls = usize::from(mode == 2 || (mode == 0 && case == 2));
                        let grammar_error = if context == 0 { 30 } else { 31 };
                        let expected = if mode == 4 && case != 3 {
                            19
                        } else if calls != 0 && !accept {
                            18
                        } else if case == 3 {
                            4
                        } else if case < 2 {
                            grammar_error
                        } else {
                            0
                        };
                        let mut first_position = None;
                        for split in 0..=bytes.len() {
                            let mut state = EncodingState {
                                accept,
                                ..EncodingState::default()
                            };
                            // SAFETY: All handles, input spans and callback state
                            // are test-owned. No state borrow crosses a parser call.
                            unsafe {
                                let requested = match mode {
                                    1 => c"UTF-8".as_ptr(),
                                    2 => c"custom".as_ptr(),
                                    _ => ptr::null(),
                                };
                                let parent = XML_ParserCreate(if context == 0 {
                                    requested
                                } else {
                                    ptr::null()
                                });
                                assert!(!parent.is_null());
                                let parser = if context == 0 {
                                    parent
                                } else {
                                    XML_ExternalEntityParserCreate(
                                        parent,
                                        if context == 1 {
                                            c"".as_ptr()
                                        } else {
                                            ptr::null()
                                        },
                                        requested,
                                    )
                                };
                                assert!(!parser.is_null());
                                XML_SetUnknownEncodingHandler(
                                    parser,
                                    Some(unknown),
                                    ptr::from_mut(&mut state).cast(),
                                );
                                let mut status = OK;
                                for (part, final_input) in
                                    [(&bytes[..split], 0), (&bytes[split..], 1)]
                                {
                                    status = XML_Parse(
                                        parser,
                                        part.as_ptr().cast(),
                                        part.len() as c_int,
                                        final_input,
                                    );
                                    if status == ERROR {
                                        break;
                                    }
                                }
                                assert_eq!(
                                    status,
                                    if expected == 0 { OK } else { ERROR },
                                    "mode={mode}, context={context}, case={case}, split={split}, accept={accept}"
                                );
                                assert_eq!(XML_GetErrorCode(parser), expected);
                                if expected != 0 {
                                    let position = (
                                        XML_GetCurrentByteIndex(parser),
                                        XML_GetCurrentLineNumber(parser),
                                        XML_GetCurrentColumnNumber(parser),
                                    );
                                    assert_eq!(*first_position.get_or_insert(position), position);
                                }
                                if mode == 0 && case < 2 && context != 2 {
                                    assert_eq!((*parser).core.current_raw(), Some(header.as_str()));
                                }
                                if context != 0 {
                                    XML_ParserFree(parser);
                                }
                                XML_ParserFree(parent);
                            }
                            assert_eq!(state.requested.len(), calls);
                            if calls != 0 {
                                assert_eq!(
                                    state.requested,
                                    [if mode == 2 { "custom" } else { "UTF8" }]
                                );
                            }
                            assert_eq!(state.releases, usize::from(calls != 0 && accept));
                        }
                    }
                }
            }
        }
    }

    // ASCII bytes outside required markup may decode differently. Preserve the
    // existing custom-map path before deciding grammar, including a multibyte
    // sequence whose decoded document version is valid. TextDecl versions keep
    // their existing permissive grammar.
    for remapping in 1..=2 {
        let version = if remapping == 1 { "\u{1}" } else { "@$" };
        for context in 0..3 {
            for explicit in [false, true] {
                for malformed_tail in [false, true] {
                    let tail = if malformed_tail { " extra='x'" } else { "" };
                    let header = format!("<?xml version='{version}' encoding='X-CUSTOM'{tail}?>");
                    let document = if context == 2 {
                        header
                    } else {
                        format!("{header}<r/>")
                    };
                    let bytes = document.as_bytes();
                    for accept in [false, true] {
                        let expected = if !accept {
                            18
                        } else if malformed_tail || (remapping == 1 && context == 0) {
                            if context == 0 { 30 } else { 31 }
                        } else {
                            0
                        };
                        for split in 0..=bytes.len() {
                            let mut state = EncodingState {
                                accept,
                                remapping,
                                ..EncodingState::default()
                            };
                            // SAFETY: Handles and callback state remain live across
                            // both feeds; state is read only after freeing all handles.
                            unsafe {
                                let requested = if explicit {
                                    c"X-CUSTOM".as_ptr()
                                } else {
                                    ptr::null()
                                };
                                let parent = XML_ParserCreate(if context == 0 {
                                    requested
                                } else {
                                    ptr::null()
                                });
                                assert!(!parent.is_null());
                                let parser = if context == 0 {
                                    parent
                                } else {
                                    XML_ExternalEntityParserCreate(
                                        parent,
                                        if context == 1 {
                                            c"".as_ptr()
                                        } else {
                                            ptr::null()
                                        },
                                        requested,
                                    )
                                };
                                assert!(!parser.is_null());
                                XML_SetUnknownEncodingHandler(
                                    parser,
                                    Some(unknown),
                                    ptr::from_mut(&mut state).cast(),
                                );
                                let mut status = OK;
                                for (part, final_input) in
                                    [(&bytes[..split], 0), (&bytes[split..], 1)]
                                {
                                    status = XML_Parse(
                                        parser,
                                        part.as_ptr().cast(),
                                        part.len() as c_int,
                                        final_input,
                                    );
                                    if status == ERROR {
                                        break;
                                    }
                                }
                                assert_eq!(
                                    status,
                                    if expected == 0 { OK } else { ERROR },
                                    "remapping={remapping}, context={context}, explicit={explicit}, malformed_tail={malformed_tail}, accept={accept}, split={split}"
                                );
                                assert_eq!(XML_GetErrorCode(parser), expected);
                                if context != 0 {
                                    XML_ParserFree(parser);
                                }
                                XML_ParserFree(parent);
                            }
                            assert_eq!(state.requested, ["X-CUSTOM"]);
                            assert_eq!(state.releases, usize::from(accept));
                            assert_eq!(state.conversions, usize::from(accept && remapping == 2));
                        }
                    }
                }
            }
        }
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
fn family_charge_preserves_zero_overflow_and_limit_boundaries() {
    for (initial, amount, limit, success, final_value) in [
        (0, 0, 0, true, 0),
        (4, 0, 5, true, 4),
        (5, 0, 5, true, 5),
        (6, 0, 5, false, 6),
        (4, 1, 5, true, 5),
        (5, 1, 5, false, 5),
        (usize::MAX, 0, usize::MAX, true, usize::MAX),
        (usize::MAX, 1, usize::MAX, false, usize::MAX),
        (usize::MAX - 1, 2, usize::MAX, false, usize::MAX - 1),
    ] {
        let counter = Cell::new(initial);
        assert_eq!(
            charge(&counter, amount, limit),
            success,
            "initial={initial}, amount={amount}, limit={limit}"
        );
        assert_eq!(counter.get(), final_value);
    }
}

#[test]
fn family_budgets_survive_child_failure_and_root_reset() {
    // SAFETY: All handles and callback state stay live until cleanup. Counter
    // changes happen between API calls, without borrowing a parser in a callback.
    unsafe {
        let counters = |parser: XML_Parser| {
            let family = &(*parser).family;
            (
                family.input_bytes.get(),
                family.callback_bytes.get(),
                family.children.get(),
            )
        };
        let mut state = State::default();
        let parent = configured(&mut state);
        let child = XML_ExternalEntityParserCreate(parent, c"".as_ptr(), ptr::null());
        let old_child = XML_ExternalEntityParserCreate(parent, c"".as_ptr(), ptr::null());
        assert!(!child.is_null());
        assert!(!old_child.is_null());

        // A valid child attempt consumes its slot even when allocation fails.
        {
            let family = &(*parent).family;
            family.children.set(MAX_FAMILY_CHILDREN - 1);
        }
        let (live_bytes, direct_bytes) = {
            let tracker = &(*parent).tracker;
            (tracker.live_bytes(), tracker.direct_bytes())
        };
        assert!(live_bytes > 0);
        assert_eq!(direct_bytes, 0);
        assert_eq!(XML_SetAllocTrackerMaximumAmplification(parent, 1.0), 1);
        assert_eq!(XML_SetAllocTrackerActivationThreshold(parent, 0), 1);
        assert!(XML_ExternalEntityParserCreate(parent, c"".as_ptr(), ptr::null()).is_null());
        assert_eq!(counters(parent).2, MAX_FAMILY_CHILDREN);
        {
            let tracker = &(*parent).tracker;
            assert_eq!(tracker.live_bytes(), live_bytes);
        }
        assert_eq!(
            XML_SetAllocTrackerMaximumAmplification(
                parent,
                oriole_storage::MAXIMUM_AMPLIFICATION_DEFAULT
            ),
            1
        );
        assert_eq!(
            XML_SetAllocTrackerActivationThreshold(
                parent,
                oriole_storage::ACTIVATION_THRESHOLD_DEFAULT
            ),
            1
        );
        assert!(XML_ExternalEntityParserCreate(parent, c"".as_ptr(), ptr::null()).is_null());
        assert_eq!(XML_GetErrorCode(parent), 43);
        assert_eq!(counters(parent).2, MAX_FAMILY_CHILDREN);

        {
            let family = &(*parent).family;
            family.callback_bytes.set(INITIAL_CALLBACK_BYTES - 2);
        }
        assert_eq!(XML_Parse(child, c"a".as_ptr(), 1, 1), OK);
        assert_eq!(XML_Parse(parent, c"<r/>".as_ptr(), 4, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parent), 43);
        assert_eq!(state.events, ["text:a", "start:r"]);
        for parser in [parent, child, old_child] {
            assert_eq!(
                counters(parser),
                (5, INITIAL_CALLBACK_BYTES, MAX_FAMILY_CHILDREN)
            );
        }

        assert_eq!(XML_ParserReset(parent, ptr::null()), 1);
        assert_eq!(counters(parent), (0, 0, 0));
        assert_eq!(XML_Parse(old_child, c"b".as_ptr(), 1, 1), ERROR);
        assert_eq!(XML_GetErrorCode(old_child), 43);
        assert_eq!(
            counters(old_child),
            (6, INITIAL_CALLBACK_BYTES, MAX_FAMILY_CHILDREN)
        );
        assert_eq!(state.events, ["text:a", "start:r"]);

        XML_SetUserData(parent, ptr::from_mut(&mut state).cast());
        XML_SetElementHandler(parent, Some(start), Some(end));
        assert_eq!(XML_Parse(parent, c"<r/>".as_ptr(), 4, 1), OK);
        assert_eq!(counters(parent), (4, 2, 0));
        assert_eq!(state.events, ["text:a", "start:r", "start:r", "end:r"]);
        XML_ParserFree(old_child);
        XML_ParserFree(child);
        XML_ParserFree(parent);
    }
}

#[test]
fn external_children_have_source_allowances_and_shared_construction_bounds() {
    // SAFETY: Limits are lowered directly in this internal test to avoid large allocations.
    unsafe {
        let parent = XML_ParserCreate(ptr::null());
        (*parent).config.limits.max_total_bytes = 6;
        let limits = (*parent).config.limits.clone();
        (*parent).core.set_limits(limits).unwrap();
        let child = XML_ExternalEntityParserCreate(parent, c"".as_ptr(), ptr::null());
        assert!(!child.is_null());
        assert_eq!(XML_Parse(child, c"<x/>".as_ptr(), 4, 1), OK);
        assert_eq!(XML_Parse(parent, c"<r/>".as_ptr(), 4, 1), OK);
        {
            let family = &(*parent).family;
            assert_eq!(family.input_bytes.get(), 8);
        }
        assert_eq!((*parent).core.input_bytes_remaining(), 2);
        assert_eq!((*child).core.input_bytes_remaining(), 2);
        XML_ParserFree(child);
        assert_eq!(XML_ParserReset(parent, ptr::null()), 1);
        let family = &(*parent).family;
        family.children.set(MAX_FAMILY_CHILDREN);
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
fn public_memory_helpers_are_exempt_from_parser_amplification() {
    // SAFETY: Each pointer is used only through its original parser's memory API.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        let tracker = Shared::clone(&(*parser).tracker);
        let initial = tracker.live_bytes();
        assert_eq!(XML_SetAllocTrackerActivationThreshold(parser, 0), 1);
        let memory = XML_MemMalloc(parser, 1000);
        assert!(!memory.is_null());
        assert_eq!(tracker.live_bytes(), initial);
        let memory = XML_MemRealloc(parser, memory, 2000);
        assert!(!memory.is_null());
        assert_eq!(tracker.live_bytes(), initial);
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

            for length in [15, 16, 17, 65_537] {
                for split_crlf in [false, true] {
                    let mut state = State::default();
                    let parser = configured(&mut state);
                    if expand {
                        XML_SetDefaultHandlerExpand(parser, Some(text));
                    } else {
                        XML_SetDefaultHandler(parser, Some(text));
                    }
                    assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 0), OK);
                    state.events.clear();
                    let prefix: String = " \t".chars().cycle().take(length).collect();
                    let input = format!("{prefix}{}", if split_crlf { "\r" } else { "\r\n\t" });
                    assert_eq!(
                        XML_Parse(
                            parser,
                            input.as_ptr().cast(),
                            input.len() as c_int,
                            i32::from(!split_crlf)
                        ),
                        OK
                    );
                    if split_crlf {
                        assert_eq!(state.events, [format!("text:{prefix}")]);
                        assert_eq!(XML_Parse(parser, c"\n\t".as_ptr(), 2, 1), OK);
                    }
                    assert_eq!(
                        state.events,
                        [
                            format!("text:{prefix}"),
                            "text:\r\n".to_string(),
                            "text:\t".to_string()
                        ]
                    );
                    XML_ParserFree(parser);
                }
            }

            // A valid whitespace prefix is not always a separate token before
            // an error. Whole-feed uncertain suffixes must keep that distinction.
            for (suffix, code, emits_prefix, emits_newline) in [
                ("x", 9, false, false),
                ("é", 9, false, false),
                ("]]>", 9, false, false),
                ("\u{1}", 4, true, false),
                ("&missing;", 4, true, false),
                ("<extra/>", 9, true, false),
                ("\r\nx", 9, true, true),
            ] {
                for split in [false, true] {
                    let mut state = State::default();
                    let parser = configured(&mut state);
                    if expand {
                        XML_SetDefaultHandlerExpand(parser, Some(text));
                    } else {
                        XML_SetDefaultHandler(parser, Some(text));
                    }
                    assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 0), OK);
                    state.events.clear();
                    let prefix = " ".repeat(17);
                    let input = if split {
                        prefix.clone()
                    } else {
                        format!("{prefix}{suffix}")
                    };
                    let mut status = XML_Parse(
                        parser,
                        input.as_ptr().cast(),
                        input.len() as c_int,
                        i32::from(!split),
                    );
                    if split {
                        assert_eq!(status, OK);
                        status =
                            XML_Parse(parser, suffix.as_ptr().cast(), suffix.len() as c_int, 1);
                    }
                    assert_eq!(status, ERROR, "{suffix:?}, split {split}");
                    assert_eq!(XML_GetErrorCode(parser), code);
                    let mut expected = Vec::new();
                    let mut offset = 4;
                    if split || emits_prefix {
                        expected.push(format!("text:{prefix}"));
                        offset += prefix.len();
                    }
                    if emits_newline {
                        expected.push("text:\r\n".to_string());
                        offset += 2;
                    }
                    assert_eq!(state.events, expected, "{suffix:?}, split {split}");
                    assert_eq!(XML_GetCurrentByteIndex(parser), offset as c_long);
                    assert_eq!(
                        XML_GetCurrentLineNumber(parser),
                        if emits_newline { 2 } else { 1 }
                    );
                    assert_eq!(
                        XML_GetCurrentColumnNumber(parser),
                        if emits_newline { 0 } else { offset as c_ulong }
                    );
                    XML_ParserFree(parser);
                }
            }

            // The earlier prolog-quote limit still publishes whitespace before
            // diagnosing a quoted token, including across vector-sized feeds.
            let prefix = " ".repeat(17);
            let input = format!("{prefix}\"x\"y");
            for width in [1, 15, 16, 17, input.len()] {
                let mut state = State::default();
                let parser = configured(&mut state);
                assert_eq!(XML_SetReparseDeferralEnabled(parser, 0), 1);
                if expand {
                    XML_SetDefaultHandlerExpand(parser, Some(text));
                } else {
                    XML_SetDefaultHandler(parser, Some(text));
                }
                let mut status = OK;
                for (index, chunk) in input.as_bytes().chunks(width).enumerate() {
                    status = XML_Parse(
                        parser,
                        chunk.as_ptr().cast(),
                        chunk.len() as c_int,
                        i32::from((index + 1) * width >= input.len()),
                    );
                    if status == ERROR {
                        break;
                    }
                }
                assert_eq!(status, ERROR);
                assert_eq!(XML_GetErrorCode(parser), 4);
                assert_eq!(
                    XML_GetCurrentByteIndex(parser),
                    (prefix.len() + 3) as c_long
                );
                assert_eq!(
                    state
                        .events
                        .iter()
                        .map(|event| event.strip_prefix("text:").unwrap())
                        .collect::<String>(),
                    prefix
                );
                XML_ParserFree(parser);
            }
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
                .set(INITIAL_CALLBACK_BYTES - remaining);
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
fn arena_start_enforces_exact_callback_budget_with_default_fallback() {
    for start_handler in [false, true] {
        for remaining in [2, 3] {
            // SAFETY: All owners remain live; this test manually holds the same
            // dispatch guard as the adapter and never borrows the core in a callback.
            unsafe {
                let mut state = State::default();
                let parser = configured(&mut state);
                XML_SetStartElementHandler(parser, start_handler.then_some(start));
                XML_SetDefaultHandler(parser, Some(text));
                (*parser)
                    .core
                    .feed(b"<r><warm a='first'/><n a='v'>", false)
                    .unwrap();
                for _ in 0..3 {
                    (*parser).core.next_event().unwrap().unwrap();
                }
                let mut frame = (*parser).core.adapter_frame();
                let mut event = None;
                (*parser)
                    .core
                    .next_event_for_adapter_into(&mut event, &mut frame)
                    .unwrap()
                    .unwrap();
                assert!(frame.is_active());
                assert!(event.is_none());
                assert_eq!(frame.callback_bytes(), 3);
                let family = &(*parser).family;
                family
                    .callback_bytes
                    .set(INITIAL_CALLBACK_BYTES - remaining);
                (*parser).busy = true;
                dispatch_start_frame(parser, &frame).unwrap();
                (*parser).busy = false;
                let family = &(*parser).family;
                if remaining == 2 {
                    assert_eq!(XML_GetErrorCode(parser), 43);
                    assert!(state.events.is_empty());
                    assert_eq!(family.callback_bytes.get(), INITIAL_CALLBACK_BYTES - 2);
                } else {
                    assert_eq!(XML_GetErrorCode(parser), 0);
                    if start_handler {
                        assert_eq!(state.events, ["start:n", "a=v"]);
                    } else {
                        assert_eq!(state.events, ["text:<n a='v'>"]);
                    }
                    assert_eq!(family.callback_bytes.get(), INITIAL_CALLBACK_BYTES);
                }
                (*parser).core.finish_adapter_frame(frame);
                XML_ParserFree(parser);
            }
        }
    }
}

#[test]
fn detached_end_charges_the_name_before_handlers_or_default_fallback() {
    for end_handler in [false, true] {
        for default_handler in [false, true] {
            for remaining in [3, 4] {
                // SAFETY: The test owns State and the parser through all callbacks.
                unsafe {
                    let mut state = State::default();
                    let parser = configured(&mut state);
                    let opening = c"<root><name a='v'>";
                    assert_eq!(
                        XML_Parse(
                            parser,
                            opening.as_ptr(),
                            opening.to_bytes().len() as c_int,
                            0
                        ),
                        OK
                    );
                    state.events.clear();
                    XML_SetEndElementHandler(parser, end_handler.then_some(end));
                    XML_SetDefaultHandler(parser, default_handler.then_some(text));
                    {
                        let family = &(*parser).family;
                        family
                            .callback_bytes
                            .set(INITIAL_CALLBACK_BYTES - remaining);
                    }
                    let status = XML_Parse(parser, c"</name>".as_ptr(), 7, 0);
                    let charged = {
                        let family = &(*parser).family;
                        family.callback_bytes.get()
                    };
                    if remaining == 3 {
                        assert_eq!(status, ERROR);
                        assert_eq!(XML_GetErrorCode(parser), 43);
                        assert!(state.events.is_empty());
                        assert_eq!(charged, INITIAL_CALLBACK_BYTES - 3);
                    } else {
                        assert_eq!(status, OK);
                        assert_eq!(charged, INITIAL_CALLBACK_BYTES);
                        let expected = if end_handler {
                            vec!["end:name"]
                        } else if default_handler {
                            vec!["text:</name>"]
                        } else {
                            vec![]
                        };
                        assert_eq!(state.events, expected);
                        assert_eq!(
                            XML_GetCurrentByteIndex(parser),
                            opening.to_bytes().len() as c_long
                        );
                        assert_eq!(XML_GetCurrentByteCount(parser), 7);
                        assert_eq!(XML_GetSpecifiedAttributeCount(parser), 2);
                    }
                    XML_ParserFree(parser);
                }
            }
        }
    }
}

#[test]
fn detached_end_owner_survives_default_current_children_stop_and_reset_rejection() {
    unsafe fn check_raw_context(parser: XML_Parser, raw: &[u8]) {
        // SAFETY: Called during the guarded End callback; the checked window
        // is read completely before any nested parser operation.
        unsafe {
            let mut offset = 0;
            let mut size = 0;
            let context = XML_GetInputContext(parser, &mut offset, &mut size);
            assert!(!context.is_null() && offset >= 0 && size >= offset);
            assert!(raw.len() <= (size - offset) as usize);
            assert_eq!(
                std::slice::from_raw_parts(context.cast::<u8>().add(offset as usize), raw.len()),
                raw
            );
        }
    }
    unsafe extern "C" fn held_end(data: *mut c_void, name: *const c_char) {
        // SAFETY: Only scalar handles and independently owned name bytes cross
        // nested callbacks; each State access ends before another C API call.
        unsafe {
            let parser = (*data.cast::<State>()).parser;
            let before = CStr::from_ptr(name).to_bytes().to_vec();
            let raw = format!("</{}>", std::str::from_utf8(&before).unwrap());
            let position = (
                XML_GetCurrentByteIndex(parser),
                XML_GetCurrentByteCount(parser),
                XML_GetCurrentLineNumber(parser),
                XML_GetCurrentColumnNumber(parser),
            );
            assert_eq!(position.0 as usize, before.len() + 9);
            assert_eq!(position.1 as usize, raw.len());
            assert_eq!((position.2, position.3), (2, 0));
            check_raw_context(parser, raw.as_bytes());
            (*data.cast::<State>())
                .events
                .push(format!("held:{}", std::str::from_utf8(&before).unwrap()));
            XML_DefaultCurrent(parser);
            XML_ParserFree(parser);
            assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
            let child = XML_ExternalEntityParserCreate(parser, c"".as_ptr(), ptr::null());
            assert!(!child.is_null());
            XML_SetElementHandler(child, None, None);
            XML_SetDefaultHandler(child, None);
            assert_eq!(XML_Parse(child, c"<child/>".as_ptr(), 8, 1), OK);
            XML_ParserFree(child);
            XML_SetEndElementHandler(parser, Some(end));
            assert_eq!(XML_StopParser(parser, 1), OK);
            XML_DefaultCurrent(parser);
            assert_eq!(CStr::from_ptr(name).to_bytes(), before);
            check_raw_context(parser, raw.as_bytes());
            assert_eq!(
                (
                    XML_GetCurrentByteIndex(parser),
                    XML_GetCurrentByteCount(parser),
                    XML_GetCurrentLineNumber(parser),
                    XML_GetCurrentColumnNumber(parser),
                ),
                position
            );
        }
    }
    for name in ["n".to_owned(), "é".to_owned(), "n".repeat(4097)] {
        // SAFETY: Input, State and both callback functions outlive synchronous use.
        unsafe {
            let document = format!("\u{feff}<r><{name}>\r</{name}><e/></r>");
            let mut state = State::default();
            let parser = configured(&mut state);
            XML_SetEndElementHandler(parser, Some(held_end));
            XML_SetDefaultHandler(parser, Some(text));
            assert_eq!(
                XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
                SUSPENDED
            );
            assert!(state.events.ends_with(&[
                format!("held:{name}"),
                format!("text:</{name}>"),
                format!("text:</{name}>")
            ]));
            // Renew userdata after the inspection before callbacks write it.
            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
            assert_eq!(XML_ResumeParser(parser), OK);
            assert!(
                state
                    .events
                    .ends_with(&["start:e".into(), "end:e".into(), "end:r".into()])
            );
            assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
            state.events.clear();
            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
            XML_SetElementHandler(parser, Some(start), Some(end));
            assert_eq!(XML_Parse(parser, c"<new></new>".as_ptr(), 11, 1), OK);
            assert_eq!(state.events, ["start:new", "end:new"]);
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
            2 => EventKind::ExternalEntityReference(
                oriole_storage::try_box(
                    oriole::ExternalEntityReference {
                        context: Some(word()),
                        system_id: Some(word()),
                        public_id: Some(word()),
                    },
                    Allocator::System,
                )
                .unwrap(),
            ),
            3 => EventKind::StartDoctype(
                oriole_storage::try_box(
                    oriole::DoctypeDeclaration {
                        name: word(),
                        system_id: Some(word()),
                        public_id: Some(word()),
                        has_internal_subset: false,
                    },
                    Allocator::System,
                )
                .unwrap(),
            ),
            4 => EventKind::StartNamespace {
                prefix: Some(word()),
                uri: Some(word()),
            },
            5 => EventKind::EndNamespace {
                prefix: Some(word()),
            },
            6 => EventKind::EntityDeclaration(
                oriole_storage::try_box(
                    oriole::EntityDeclaration {
                        name: word(),
                        value: Some(word()),
                        parameter: false,
                        system_id: Some(word()),
                        public_id: Some(word()),
                        notation: Some(word()),
                    },
                    Allocator::System,
                )
                .unwrap(),
            ),
            7 => EventKind::AttlistDeclaration(
                oriole_storage::try_box(
                    oriole::AttributeDeclaration {
                        element: word(),
                        name: word(),
                        attribute_type: word(),
                        default: Some(word()),
                        required: false,
                    },
                    Allocator::System,
                )
                .unwrap(),
            ),
            8 => EventKind::NotationDeclaration(
                oriole_storage::try_box(
                    oriole::NotationDeclaration {
                        name: word(),
                        system_id: Some(word()),
                        public_id: Some(word()),
                    },
                    Allocator::System,
                )
                .unwrap(),
            ),
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
                        .set(INITIAL_CALLBACK_BYTES - remaining);
                    (*parser).core.feed(b"<r/>", true).unwrap();
                    let (_, recycling) =
                        (*parser).core.next_event_for_recycling().unwrap().unwrap();
                    (*parser).busy = true;
                    dispatch(parser, event(case), recycling).unwrap();
                    (*parser).busy = false;
                    assert_eq!(
                        XML_GetErrorCode(parser),
                        if remaining < bytes { 43 } else { 0 },
                        "metadata case {case}"
                    );
                    let family = &(*parser).family;
                    assert_eq!(
                        family.callback_bytes.get(),
                        if remaining < bytes {
                            INITIAL_CALLBACK_BYTES - remaining
                        } else {
                            INITIAL_CALLBACK_BYTES
                        }
                    );
                    XML_ParserFree(parser);
                }
            }
        }
    }
}

#[derive(Default)]
struct MultibyteState {
    parser: XML_Parser,
    handlers: usize,
    conversions: usize,
    releases: usize,
    value: i32,
    action: u8,
    invalid_map: u8,
}

unsafe extern "C" fn convert_multibyte(data: *mut c_void, _: *const c_char) -> c_int {
    // SAFETY: Tests retain the state and inspect it only between callback calls.
    // Raw field copies, rather than a State reference, survive reentrant API calls.
    unsafe {
        let state = data.cast::<MultibyteState>();
        (*state).conversions += 1;
        let parser = (*state).parser;
        match (*state).action {
            1 => {
                assert_eq!(XML_Parse(parser, c"<bad/>".as_ptr(), 6, 1), ERROR);
                assert_eq!(XML_ParseBuffer(parser, 0, 0), ERROR);
                assert!(XML_GetBuffer(parser, 1).is_null());
                assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
                XML_ParserFree(parser);
                assert_eq!(XML_GetErrorCode(parser), 0);
            }
            2 => assert_eq!(XML_StopParser(parser, 1), OK),
            3 => assert_eq!(XML_StopParser(parser, 0), OK),
            5 => {
                let pending = (*parser).core.encoding_conversion().unwrap();
                assert_eq!(XML_StopParser(parser, 0), OK);
                assert_eq!(XML_SetEncoding(parser, c"changed".as_ptr()), OK);
                assert_eq!((*parser).core.encoding_conversion().unwrap(), pending);
            }
            _ => {}
        }
        (*state).value
    }
}

unsafe extern "C" fn release_multibyte(data: *mut c_void) {
    // SAFETY: The callback owns one registered encoding instance; state outlives it.
    unsafe { (*data.cast::<MultibyteState>()).releases += 1 };
}

unsafe extern "C" fn encoding_multibyte(
    data: *mut c_void,
    _: *const c_char,
    info: *mut XML_Encoding,
) -> c_int {
    // SAFETY: Expat provides the writable encoding record for this callback.
    unsafe {
        let state = data.cast::<MultibyteState>();
        (*state).handlers += 1;
        (*info).map = std::array::from_fn(|index| index as i32);
        (*info).map[128] = if (*state).invalid_map == 2 { -5 } else { -2 };
        (*info).data = data;
        (*info).release = Some(release_multibyte);
        (*info).convert = if (*state).invalid_map == 1 {
            None
        } else {
            Some(convert_multibyte)
        };
        if (*state).action == 4 {
            assert_eq!(
                XML_SetAllocTrackerActivationThreshold((*state).parser, 0),
                1
            );
            assert_eq!(
                XML_SetAllocTrackerMaximumAmplification((*state).parser, 1.0),
                1
            );
        }
    }
    OK
}

unsafe fn configured_multibyte(state: &mut MultibyteState) -> XML_Parser {
    // SAFETY: The tests keep state live until all related parser handles are freed.
    unsafe {
        let parser = XML_ParserCreate(c"multibyte".as_ptr());
        assert!(!parser.is_null());
        state.parser = parser;
        XML_SetUnknownEncodingHandler(
            parser,
            Some(encoding_multibyte),
            ptr::from_mut(state).cast(),
        );
        parser
    }
}

#[test]
fn multibyte_converter_reentry_is_guarded_and_release_is_owned_once() {
    // SAFETY: Test-owned state, bytes and handles obey the C API lifetime contract.
    unsafe {
        for value in ['é', 'A', '<', ' '] {
            for buffered in [false, true] {
                let mut state = MultibyteState {
                    value: value as i32,
                    action: 1,
                    ..MultibyteState::default()
                };
                let parser = configured_multibyte(&mut state);
                let document: &[u8] = if matches!(value, 'é' | 'A') {
                    b"<\x80\0 a='\x80\0'>\x80\0</\x80\0>"
                } else {
                    b"<r a='\x80\0'>\x80\0</r>"
                };
                for chunk in document.chunks(1) {
                    if buffered {
                        let buffer = XML_GetBuffer(parser, 1).cast::<u8>();
                        assert!(!buffer.is_null());
                        buffer.write(chunk[0]);
                        assert_eq!(XML_ParseBuffer(parser, 1, 0), OK);
                    } else {
                        assert_eq!(XML_Parse(parser, chunk.as_ptr().cast(), 1, 0), OK);
                    }
                }
                assert_eq!(XML_Parse(parser, ptr::null(), 0, 1), OK);
                assert_eq!(state.handlers, 1);
                assert_eq!(
                    state.conversions,
                    if matches!(value, 'é' | 'A') { 4 } else { 2 }
                );
                assert_eq!(state.releases, 0);
                assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
                assert_eq!(state.releases, 1);
                XML_ParserFree(parser);
                assert_eq!(state.releases, 1);
            }
        }
    }
}

#[test]
fn multibyte_converter_can_suspend_or_abort_without_repeating_conversion() {
    // SAFETY: The converter acts only on its own active parser.
    unsafe {
        for action in [2, 3, 5] {
            let mut state = MultibyteState {
                value: 'é' as i32,
                action,
                ..MultibyteState::default()
            };
            let parser = configured_multibyte(&mut state);
            let document = b"<r>\x80\0</r>";
            let status = XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1);
            assert_eq!(status, if action == 2 { SUSPENDED } else { ERROR });
            if action == 2 {
                state.action = 0;
                assert_eq!(XML_ResumeParser(parser), OK);
            } else {
                assert_eq!(XML_GetErrorCode(parser), 35);
            }
            assert_eq!(state.conversions, 1);
            assert_eq!(state.releases, 0);
            if action == 5 {
                assert_eq!(XML_SetEncoding(parser, ptr::null()), OK);
                assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
                assert_eq!(state.releases, 1);
                assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
            }
            XML_ParserFree(parser);
            assert_eq!(state.releases, 1);
        }
    }
}

#[test]
fn external_child_requests_its_own_multibyte_encoding_after_parent_free() {
    // SAFETY: Encoding callback state outlives both independently owned parsers.
    unsafe {
        let mut state = MultibyteState {
            value: 'é' as i32,
            ..MultibyteState::default()
        };
        let parser = configured_multibyte(&mut state);
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 1), OK);
        let child = XML_ExternalEntityParserCreate(parser, c"".as_ptr(), c"multibyte".as_ptr());
        assert!(!child.is_null());
        XML_ParserFree(parser);
        assert_eq!(state.releases, 1);
        state.parser = child;
        let document = b"<\x80\0/>";
        assert_eq!(
            XML_Parse(child, document.as_ptr().cast(), document.len() as c_int, 1),
            OK
        );
        assert_eq!(state.handlers, 2);
        assert_eq!(state.conversions, 1);
        XML_ParserFree(child);
        assert_eq!(state.releases, 2);
    }
}

#[test]
fn invalid_multibyte_maps_and_conversion_results_release_once() {
    // SAFETY: All failures retain live test state until the parser is destroyed.
    unsafe {
        for (invalid_map, value, expected) in [
            (1, 65, 18),
            (2, 65, 18),
            (0, -1, 4),
            (0, 0xd800, 4),
            (0, 0x10000, 4),
        ] {
            let mut state = MultibyteState {
                value,
                invalid_map,
                ..MultibyteState::default()
            };
            let parser = configured_multibyte(&mut state);
            let document = b"<r>\x80\0</r>";
            assert_eq!(
                XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
                ERROR
            );
            assert_eq!(XML_GetErrorCode(parser), expected);
            assert_eq!(state.releases, usize::from(invalid_map != 0));
            XML_ParserFree(parser);
            assert_eq!(state.releases, 1);
        }
    }
}

#[test]
fn multibyte_map_allocation_failure_releases_the_callback_instance_once() {
    // SAFETY: The handler tightens the live parser's tracker before its map is
    // allocated; callback state remains live through failure and destruction.
    unsafe {
        let mut state = MultibyteState {
            value: 'é' as i32,
            action: 4,
            ..MultibyteState::default()
        };
        let parser = configured_multibyte(&mut state);
        let document = b"<r>\x80\0</r>";
        assert_eq!(
            XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
            ERROR
        );
        assert_eq!(XML_GetErrorCode(parser), 1);
        assert_eq!(state.handlers, 1);
        assert_eq!(state.conversions, 0);
        assert_eq!(state.releases, 1);
        assert_eq!(XML_Parse(parser, ptr::null(), 0, 1), ERROR);
        assert_eq!(state.handlers, 1);
        XML_ParserFree(parser);
        assert_eq!(state.releases, 1);
    }
}

#[test]
fn entity_value_parameters_use_buffer_input_and_merge_into_the_parent() {
    // SAFETY: Both parsers and callback state are live throughout each parse;
    // GetBuffer storage is filled only up to the requested length.
    unsafe {
        for mode in 0..=2 {
            let mut state = State::default();
            let parent = configured(&mut state);
            let child = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
            assert!(!child.is_null());
            assert_eq!(XML_SetParamEntityParsing(child, mode), 1);
            let dtd = b"<!ENTITY % p 'works'><!ENTITY e '%p;'>";
            for (index, byte) in dtd.iter().enumerate() {
                let buffer = XML_GetBuffer(child, 1).cast::<u8>();
                assert!(!buffer.is_null());
                buffer.write(*byte);
                assert_eq!(
                    XML_ParseBuffer(child, 1, c_int::from(index + 1 == dtd.len())),
                    OK
                );
            }
            XML_ParserFree(child);
            let document = b"<!DOCTYPE r SYSTEM 'test.dtd'><r>&e;</r>";
            assert_eq!(
                XML_Parse(parent, document.as_ptr().cast(), document.len() as c_int, 1),
                OK
            );
            assert_eq!(state.events, ["start:r", "text:works", "end:r"]);
            XML_ParserFree(parent);
        }
    }
}

#[test]
fn entity_value_parameter_context_has_the_expat_error_code() {
    // SAFETY: Parsers and input buffers are test-owned and freed once after parsing.
    unsafe {
        for (document, expected) in [
            (b"<!DOCTYPE r [<!ENTITY e '%missing;'>]><r/>".as_slice(), 10),
            (b"<!DOCTYPE r [<!ENTITY e '%missing'>]><r/>", 4),
        ] {
            let parser = XML_ParserCreate(ptr::null());
            assert!(!parser.is_null());
            assert_eq!(
                XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
                ERROR
            );
            assert_eq!(XML_GetErrorCode(parser), expected);
            XML_ParserFree(parser);
        }
    }
}

#[derive(Default)]
struct HeaderCallbackState {
    root: XML_Parser,
    action: u8,
    requests: usize,
    declarations: Vec<String>,
}

unsafe extern "C" fn header_entity_decl(
    data: *mut c_void,
    name: *const c_char,
    _: c_int,
    _: *const c_char,
    _: c_int,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
) {
    // SAFETY: The test owns callback data and Expat supplies a callback-lived name.
    unsafe {
        (*data.cast::<HeaderCallbackState>())
            .declarations
            .push(CStr::from_ptr(name).to_str().unwrap().to_owned());
    }
}

#[test]
fn parameter_delimiters_preserve_buffer_input_after_parent_free() {
    let dtd = br#"<!ENTITY % p "><!ENTITY e "><!ELEMENT r EMPTY %p;"works"><!ENTITY % h "INCLUDE["><![%h;<!ENTITY after "done">]]>"#;
    // SAFETY: Fixed callback state outlives the retained child. Every write uses
    // its GetBuffer allocation, and parent and child are each freed once.
    unsafe {
        for namespaces in [false, true] {
            for width in 1..=dtd.len() {
                let parent = if namespaces {
                    XML_ParserCreateNS(ptr::null(), b'|' as c_char)
                } else {
                    XML_ParserCreate(ptr::null())
                };
                assert!(!parent.is_null());
                let child = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
                assert!(!child.is_null());
                XML_ParserFree(parent);
                let mut state = HeaderCallbackState::default();
                XML_SetUserData(child, ptr::from_mut(&mut state).cast());
                XML_SetEntityDeclHandler(child, Some(header_entity_decl));
                assert_eq!(XML_SetParamEntityParsing(child, 2), 1);
                for (index, bytes) in dtd.chunks(width).enumerate() {
                    let buffer = XML_GetBuffer(child, bytes.len() as c_int);
                    assert!(!buffer.is_null());
                    ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast(), bytes.len());
                    assert_eq!(
                        XML_ParseBuffer(
                            child,
                            bytes.len() as c_int,
                            c_int::from((index + 1) * width >= dtd.len())
                        ),
                        OK,
                    );
                }
                assert_eq!(state.declarations, ["p", "e", "h", "after"]);
                XML_ParserFree(child);
            }
        }
    }
}

unsafe extern "C" fn header_external(
    parser: XML_Parser,
    context: *const c_char,
    _: *const c_char,
    system: *const c_char,
    _: *const c_char,
) -> c_int {
    // SAFETY: All parser handles and callback strings remain live. Raw state access
    // keeps no reference to callback state across parser calls that may invoke it.
    unsafe {
        let state = XML_GetUserData(parser).cast::<HeaderCallbackState>();
        let parameter = CStr::from_ptr(system).to_bytes() == b"p";
        if parameter {
            (*state).requests += 1;
            assert_eq!(XML_StopParser(parser, 1), ERROR);
            assert_eq!(XML_GetErrorCode(parser), 37);
            match (*state).action {
                0 => return 1,
                2 => assert_eq!(XML_StopParser((*state).root, 1), OK),
                3 => {
                    assert_eq!(XML_StopParser(parser, 0), OK);
                    return 1;
                }
                _ => {}
            }
        }
        let child = XML_ExternalEntityParserCreate(parser, context, ptr::null());
        assert!(!child.is_null());
        let input: &[u8] = if parameter {
            b"<!ENTITY % keyword 'INCLUDE'>"
        } else {
            b"<!ENTITY % p SYSTEM 'p'><![INCLUDE%p;[<!ENTITY e 'yes'>]]><!ENTITY after 'yes'>"
        };
        let mut status = OK;
        for (index, byte) in input.iter().enumerate() {
            let buffer = XML_GetBuffer(child, 1).cast::<u8>();
            assert!(!buffer.is_null());
            buffer.write(*byte);
            status = XML_ParseBuffer(child, 1, c_int::from(index + 1 == input.len()));
            if status != OK {
                break;
            }
        }
        XML_ParserFree(child);
        c_int::from(status == OK)
    }
}

#[test]
fn external_header_callbacks_preserve_buffer_input_abort_and_parent_suspension() {
    // SAFETY: Test-owned state outlives the root and every synchronous child. Each
    // successful child is freed exactly once after its buffer parse has returned.
    unsafe {
        for action in 0..=3 {
            let root = XML_ParserCreate(ptr::null());
            assert!(!root.is_null());
            let mut state = HeaderCallbackState {
                root,
                action,
                ..HeaderCallbackState::default()
            };
            XML_SetUserData(root, ptr::from_mut(&mut state).cast());
            XML_SetParamEntityParsing(root, 2);
            XML_SetEntityDeclHandler(root, Some(header_entity_decl));
            XML_SetExternalEntityRefHandler(root, Some(header_external));
            let input = b"<!DOCTYPE r SYSTEM 'd'><r/>";
            let status = XML_Parse(root, input.as_ptr().cast(), input.len() as c_int, 1);
            assert_eq!(state.requests, 1);
            match action {
                0 => {
                    assert_eq!(status, OK);
                    assert_eq!(state.declarations, ["p"]);
                }
                1 => {
                    assert_eq!(status, OK);
                    assert_eq!(state.declarations, ["p", "keyword", "e", "after"]);
                }
                2 => {
                    assert_eq!(status, SUSPENDED);
                    assert_eq!(XML_ResumeParser(root), OK);
                    assert_eq!(state.requests, 1);
                    assert_eq!(state.declarations, ["p", "keyword", "e", "after"]);
                }
                3 => {
                    assert_eq!(status, ERROR);
                    assert_eq!(XML_GetErrorCode(root), 21);
                    assert_eq!(state.declarations, ["p"]);
                }
                _ => unreachable!(),
            }
            XML_ParserFree(root);
        }
    }
}

#[test]
fn character_data_ownership_survives_nested_calls_and_suspension() {
    unsafe extern "C" fn callback(data: *mut c_void, bytes: *const c_char, len: c_int) {
        // SAFETY: Character data remains owned by the dispatch frame until this
        // callback returns. Nested calls use a separate, independently owned parser.
        unsafe {
            let state = &mut *data.cast::<State>();
            let expected = std::slice::from_raw_parts(bytes.cast::<u8>(), len as usize).to_vec();
            let nested = XML_ParserCreate(ptr::null());
            assert!(!nested.is_null());
            assert_eq!(XML_Parse(nested, c"<nested/>".as_ptr(), 9, 1), OK);
            XML_ParserFree(nested);
            if state.events.is_empty() {
                assert_eq!(XML_StopParser(state.parser, 1), OK);
            }
            let actual = std::slice::from_raw_parts(bytes.cast::<u8>(), len as usize);
            assert_eq!(actual, expected);
            state
                .events
                .push(std::str::from_utf8(actual).unwrap().to_owned());
        }
    }
    // SAFETY: Inputs and state remain alive until all suspended events resume;
    // event pointers are inspected only within their callback's lifetime.
    unsafe {
        for width in [1, 7, 4096] {
            let mut state = State::default();
            let parser = XML_ParserCreate(ptr::null());
            assert!(!parser.is_null());
            state.parser = parser;
            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
            XML_SetCharacterDataHandler(parser, Some(callback));
            let document = "<r>é中😀&lt;&#x1F600;<![CDATA[short]]>123456789012345678901234</r>";
            let mut chunks = document.as_bytes().chunks(width).peekable();
            while let Some(bytes) = chunks.next() {
                let mut status = XML_Parse(
                    parser,
                    bytes.as_ptr().cast(),
                    bytes.len() as c_int,
                    c_int::from(chunks.peek().is_none()),
                );
                while status == SUSPENDED {
                    status = XML_ResumeParser(parser);
                }
                assert_eq!(status, OK);
            }
            assert_eq!(
                state.events.concat(),
                "é中😀<😀short123456789012345678901234"
            );
            XML_ParserFree(parser);
        }
    }
}

#[derive(Default)]
struct ValueCallbackState {
    root: XML_Parser,
    action: u8,
    requests: usize,
    events: Vec<String>,
    encoding: MultibyteState,
}

unsafe extern "C" fn value_entity_decl(
    data: *mut c_void,
    name: *const c_char,
    _: c_int,
    value: *const c_char,
    length: c_int,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
) {
    // SAFETY: Callback state outlives the parser family; the value is readable
    // for exactly length bytes during this callback and is copied immediately.
    unsafe {
        if !value.is_null() {
            let name = CStr::from_ptr(name).to_str().unwrap();
            let value =
                std::str::from_utf8(std::slice::from_raw_parts(value.cast(), length as usize))
                    .unwrap();
            (*data.cast::<ValueCallbackState>())
                .events
                .push(format!("entity:{name}:{value}"));
        }
    }
}

unsafe extern "C" fn value_default(data: *mut c_void, text: *const c_char, length: c_int) {
    // SAFETY: Input is readable for length bytes and callback state stays live.
    unsafe {
        let text =
            std::str::from_utf8(std::slice::from_raw_parts(text.cast(), length as usize)).unwrap();
        (*data.cast::<ValueCallbackState>())
            .events
            .push(format!("raw:{text}"));
    }
}

unsafe extern "C" fn value_external(
    parser: XML_Parser,
    context: *const c_char,
    _: *const c_char,
    system: *const c_char,
    _: *const c_char,
) -> c_int {
    // SAFETY: Raw state access avoids retaining a reference across recursive
    // callbacks. Each child and encoding instance is freed exactly once.
    unsafe {
        let state = XML_GetUserData(parser).cast::<ValueCallbackState>();
        let value = CStr::from_ptr(system).to_bytes() == b"p";
        if value {
            (*state).requests += 1;
            (*state).events.push("external:p".into());
            assert_eq!(XML_StopParser(parser, 1), ERROR);
            assert_eq!(XML_GetErrorCode(parser), 37);
            match (*state).action {
                1 => return OK,
                2 => assert_eq!(XML_StopParser((*state).root, 1), OK),
                3 => {
                    assert_eq!(XML_StopParser(parser, 0), OK);
                    return OK;
                }
                4 => XML_SetEntityDeclHandler(parser, None),
                5 => XML_SetEntityDeclHandler(parser, Some(value_entity_decl)),
                _ => {}
            }
        }
        let encoding = if value {
            c"multibyte".as_ptr()
        } else {
            ptr::null()
        };
        let child = XML_ExternalEntityParserCreate(parser, context, encoding);
        assert!(!child.is_null());
        if value {
            (*state).encoding.parser = child;
            (*state).encoding.value = 'é' as i32;
            (*state).encoding.action = 1;
            XML_SetUnknownEncodingHandler(
                child,
                Some(encoding_multibyte),
                ptr::addr_of_mut!((*state).encoding).cast(),
            );
        }
        let bytes: &[u8] = if value {
            b"\"\x80\0&#13;\""
        } else if (*state).action >= 6 {
            b"<!ENTITY % p SYSTEM 'p'><!ENTITY e 'OLD'><!ENTITY e 'L%p;R'><!ENTITY after 'A'>"
        } else {
            b"<!ENTITY % p SYSTEM 'p'><!ENTITY e 'L%p;R'><!ENTITY after 'A'>"
        };
        let mut status = OK;
        for (index, byte) in bytes.iter().enumerate() {
            let buffer = XML_GetBuffer(child, 1).cast::<u8>();
            assert!(!buffer.is_null());
            buffer.write(*byte);
            status = XML_ParseBuffer(child, 1, c_int::from(index + 1 == bytes.len()));
            if status != OK {
                break;
            }
        }
        XML_ParserFree(child);
        c_int::from(status == OK)
    }
}

#[test]
fn external_values_keep_callback_order_encoding_ownership_and_stop_semantics() {
    // SAFETY: State remains fixed on the stack until all synchronous children
    // and the root are freed; input and buffer lengths obey the public API.
    unsafe {
        for action in 0..=7 {
            let root = XML_ParserCreate(ptr::null());
            assert!(!root.is_null());
            let mut state = ValueCallbackState {
                root,
                action,
                ..ValueCallbackState::default()
            };
            XML_SetUserData(root, ptr::from_mut(&mut state).cast());
            XML_SetParamEntityParsing(root, 2);
            XML_SetExternalEntityRefHandler(root, Some(value_external));
            XML_SetDefaultHandler(root, Some(value_default));
            if !matches!(action, 5 | 7) {
                XML_SetEntityDeclHandler(root, Some(value_entity_decl));
            }
            let bytes = b"<!DOCTYPE r SYSTEM 'd'><r/>";
            let status = XML_Parse(root, bytes.as_ptr().cast(), bytes.len() as c_int, 1);
            assert_eq!(state.requests, 1);
            if action == 3 {
                assert_eq!(status, ERROR);
                assert_eq!(XML_GetErrorCode(root), 21);
            } else {
                assert_eq!(status, if action == 2 { SUSPENDED } else { OK });
                if action == 2 {
                    assert_eq!(XML_ResumeParser(root), OK);
                    assert_eq!(state.requests, 1);
                }
                if action == 1 {
                    assert!(state.events.iter().any(|event| event == "entity:e:LR"));
                    assert!(!state.events.iter().any(|event| event == "entity:after:A"));
                } else if action == 4 {
                    assert!(
                        state
                            .events
                            .windows(2)
                            .any(|events| events == ["raw:'L%p;R'", "raw:>"])
                    );
                } else if action >= 6 {
                    let external = state
                        .events
                        .iter()
                        .position(|event| event == "external:p")
                        .unwrap();
                    let suffix = state
                        .events
                        .iter()
                        .position(|event| event == "raw:'L%p;R'")
                        .unwrap();
                    assert!(external < suffix);
                    if action == 6 {
                        assert!(
                            state.events[..external]
                                .iter()
                                .any(|event| event == "raw:e")
                        );
                        assert_eq!(
                            state
                                .events
                                .iter()
                                .filter(|event| event.starts_with("entity:e:"))
                                .collect::<Vec<_>>(),
                            ["entity:e:OLD"]
                        );
                    } else {
                        assert_eq!(state.events[suffix + 1], "raw:>");
                    }
                } else {
                    assert!(
                        state
                            .events
                            .iter()
                            .any(|event| event == "entity:e:L\"é\r\"R")
                    );
                    assert!(state.events.iter().any(|event| event == "entity:after:A"));
                }
                if action == 5 {
                    let external = state
                        .events
                        .iter()
                        .position(|event| event == "external:p")
                        .unwrap();
                    let before = state.events[..external]
                        .iter()
                        .filter_map(|event| event.strip_prefix("raw:"))
                        .collect::<String>();
                    assert!(before.ends_with("<!ENTITY e "));
                }
            }
            XML_ParserFree(root);
            let count = usize::from(!matches!(action, 1 | 3));
            assert_eq!(state.encoding.handlers, count);
            assert_eq!(state.encoding.conversions, count);
            assert_eq!(state.encoding.releases, count);
        }
    }
}

#[derive(Default)]
struct ForeignPolicy {
    parser: XML_Parser,
    events: Vec<&'static str>,
    action: u8,
}

unsafe extern "C" fn foreign_policy_load(
    parser: XML_Parser,
    context: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
) -> c_int {
    // SAFETY: Only short state borrows occur before invoking a child parser.
    unsafe {
        (*XML_GetUserData(parser).cast::<ForeignPolicy>())
            .events
            .push("external");
        let child = XML_ExternalEntityParserCreate(parser, context, ptr::null());
        assert!(!child.is_null());
        let result = XML_Parse(child, c"".as_ptr(), 0, 0);
        XML_ParserFree(child);
        result
    }
}

unsafe extern "C" fn foreign_policy_notify(data: *mut c_void) -> c_int {
    // SAFETY: The serialized callback owns access to this test state.
    unsafe {
        let state = &mut *data.cast::<ForeignPolicy>();
        state.events.push("not-standalone");
        if state.action == 2 {
            assert_eq!(XML_StopParser(state.parser, 1), OK);
        }
        c_int::from(state.action != 1)
    }
}

unsafe extern "C" fn foreign_policy_end(data: *mut c_void) {
    // SAFETY: The test state outlives every callback.
    unsafe { (*data.cast::<ForeignPolicy>()).events.push("end-doctype") }
}

unsafe extern "C" fn foreign_policy_start(
    data: *mut c_void,
    _: *const c_char,
    _: *const *const c_char,
) {
    // SAFETY: The test state outlives every callback.
    unsafe { (*data.cast::<ForeignPolicy>()).events.push("root") }
}

#[test]
fn foreign_dtd_policy_rejection_and_suspension_precede_document_callbacks() {
    // SAFETY: All handles, callback pointers, buffers, and state remain live and serialized.
    unsafe {
        for document in [b"<r/>".as_slice(), b"<!DOCTYPE r []><r/>"] {
            for action in 0..3 {
                for width in [1, document.len()] {
                    let parser = XML_ParserCreate(ptr::null());
                    let mut state = ForeignPolicy {
                        parser,
                        action,
                        ..ForeignPolicy::default()
                    };
                    XML_SetUserData(parser, (&raw mut state).cast());
                    XML_SetExternalEntityRefHandler(parser, Some(foreign_policy_load));
                    XML_SetNotStandaloneHandler(parser, Some(foreign_policy_notify));
                    XML_SetEndDoctypeDeclHandler(parser, Some(foreign_policy_end));
                    XML_SetStartElementHandler(parser, Some(foreign_policy_start));
                    assert_eq!(XML_UseForeignDTD(parser, 1), 0);
                    assert_eq!(XML_SetParamEntityParsing(parser, 2), 1);
                    for (index, chunk) in document.chunks(width).enumerate() {
                        let status = XML_Parse(
                            parser,
                            chunk.as_ptr().cast(),
                            chunk.len() as c_int,
                            c_int::from((index + 1) * width >= document.len()),
                        );
                        if status == ERROR {
                            assert_eq!(action, 1);
                            assert_eq!(XML_GetErrorCode(parser), 22);
                            break;
                        }
                        if status == SUSPENDED {
                            assert_eq!(action, 2);
                            assert_eq!(state.events, ["external", "not-standalone"]);
                            assert_eq!(XML_ResumeParser(parser), OK);
                        }
                    }
                    let mut expected = vec!["external", "not-standalone"];
                    if action != 1 {
                        if document.starts_with(b"<!DOCTYPE") {
                            expected.push("end-doctype");
                        }
                        expected.push("root");
                    }
                    assert_eq!(state.events, expected);
                    XML_ParserFree(parser);
                }
            }
        }
    }
}

#[test]
fn external_content_encoding_initialization_survives_buffer_input_and_parent_free() {
    // SAFETY: Handles and callback state remain live through each call; GetBuffer
    // provides each writable span, and both parsers are freed exactly once.
    unsafe {
        for namespaces in [false, true] {
            for setter in [false, true] {
                for (input, protocol, expected, error) in [
                    (&b"\xff\xfeL "[..], Some(c"ISO-8859-1"), "ÿþL ", 0),
                    (&b"\xfe\xff L"[..], Some(c"ISO-8859-1"), "þÿ L", 0),
                    (&b"\xef\xbb\xbfX"[..], Some(c"ISO-8859-1"), "ï»¿X", 0),
                    (&b"a\0b\0c\0"[..], None, "", 4),
                    (&b"<\0r\0/\0>\0"[..], None, "", 0),
                ] {
                    for width in 1..=input.len() {
                        let parent = if namespaces {
                            XML_ParserCreateNS(ptr::null(), b'|' as c_char)
                        } else {
                            XML_ParserCreate(ptr::null())
                        };
                        assert!(!parent.is_null());
                        let encoding = protocol.map_or(ptr::null(), CStr::as_ptr);
                        let child = XML_ExternalEntityParserCreate(
                            parent,
                            c"".as_ptr(),
                            if setter { ptr::null() } else { encoding },
                        );
                        assert!(!child.is_null());
                        XML_ParserFree(parent);
                        if setter {
                            assert_eq!(XML_SetEncoding(child, encoding), OK);
                        }
                        let mut state = State::default();
                        XML_SetUserData(child, ptr::from_mut(&mut state).cast());
                        XML_SetCharacterDataHandler(child, Some(text));
                        let mut status = OK;
                        for (index, bytes) in input.chunks(width).enumerate() {
                            let buffer = XML_GetBuffer(child, bytes.len() as c_int);
                            assert!(!buffer.is_null());
                            ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast(), bytes.len());
                            status = XML_ParseBuffer(
                                child,
                                bytes.len() as c_int,
                                c_int::from((index + 1) * width >= input.len()),
                            );
                            if status == ERROR {
                                break;
                            }
                        }
                        assert_eq!(XML_GetErrorCode(child), error);
                        if error == 0 {
                            assert_eq!(status, OK);
                            let text: String = state
                                .events
                                .iter()
                                .filter_map(|event| event.strip_prefix("text:"))
                                .collect();
                            assert_eq!(text, expected);
                        } else {
                            assert_eq!(status, ERROR);
                        }
                        XML_ParserFree(child);
                    }
                }
            }
        }
    }
}

#[test]
fn finished_encoding_updates_preserve_decoder_and_reset_ownership() {
    // SAFETY: Test state and every C string outlive all callbacks and owned handles.
    unsafe {
        let mut state = State::default();
        let parser = configured(&mut state);
        assert_eq!(XML_SetEncoding(parser, c"test-map".as_ptr()), OK);
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
        let index = XML_GetCurrentByteIndex(parser);
        for encoding in [c"unused-name".as_ptr(), ptr::null(), c"ISO-8859-1".as_ptr()] {
            assert_eq!(XML_SetEncoding(parser, encoding), OK);
            assert_eq!(XML_GetErrorCode(parser), 0);
            assert_eq!(XML_GetCurrentByteIndex(parser), index);
            assert_eq!(state.releases, 0);
        }
        // The finished metadata setter must not discard the active custom map.
        let child = XML_ExternalEntityParserCreate(parser, c"".as_ptr(), c"test-map".as_ptr());
        assert!(!child.is_null());
        assert_eq!(XML_Parse(child, b"\x80".as_ptr().cast(), 1, 1), OK);
        XML_ParserFree(child);
        assert_eq!(state.releases, 0);
        assert_eq!(XML_Parse(parser, ptr::null(), 0, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 36);
        assert_eq!(XML_SetEncoding(parser, ptr::null()), OK);
        assert_eq!(XML_GetErrorCode(parser), 36);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        assert_eq!(state.releases, 1);
        XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
        XML_SetCharacterDataHandler(parser, Some(text));
        let input = "<r>é</r>".as_bytes();
        assert_eq!(
            XML_Parse(parser, input.as_ptr().cast(), input.len() as c_int, 1),
            OK
        );
        assert_eq!(state.events.last().unwrap(), "text:é");
        XML_ParserFree(parser);
        assert_eq!(state.releases, 1);
    }
}

#[test]
fn encoding_setter_observes_abort_and_suspension_inside_callbacks() {
    unsafe extern "C" fn stop_and_set(
        data: *mut c_void,
        _: *const c_char,
        _: *const *const c_char,
    ) {
        // SAFETY: The test owns this state until the parser has finished or resumed.
        unsafe {
            let state = data.cast::<State>();
            let parser = (*state).parser;
            assert_eq!(XML_ParseBuffer(parser, 0, 1), ERROR);
            assert_eq!(XML_StopParser(parser, (*state).nested_status as u8), OK);
            assert_eq!(
                XML_SetEncoding(parser, c"next".as_ptr()),
                i32::from((*state).nested_status == 0)
            );
        }
    }
    // SAFETY: Each parser and its callback state are used serially and freed once.
    unsafe {
        for resumable in [0, 1] {
            let mut state = State {
                nested_status: resumable,
                ..State::default()
            };
            let parser = configured(&mut state);
            XML_SetElementHandler(parser, Some(stop_and_set), None);
            assert_eq!(
                XML_Parse(parser, c"<r/>".as_ptr(), 4, 1),
                if resumable == 0 { ERROR } else { SUSPENDED }
            );
            if resumable == 1 {
                assert_eq!(XML_SetEncoding(parser, ptr::null()), ERROR);
                assert_eq!(XML_ParseBuffer(parser, 0, 1), ERROR);
                assert_eq!(XML_GetErrorCode(parser), 33);
                assert_eq!(XML_ResumeParser(parser), OK);
            }
            assert_eq!(XML_SetEncoding(parser, c"UTF-8".as_ptr()), OK);
            assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
            assert_eq!(XML_Parse(parser, c"<r>".as_ptr(), 3, 1), ERROR);
            assert_eq!(XML_SetEncoding(parser, c"UTF-8".as_ptr()), ERROR);
            assert_eq!(XML_GetErrorCode(parser), 3);
            XML_ParserFree(parser);
        }
    }
}

#[test]
fn zero_length_parse_buffer_finishes_owned_input_without_a_reservation() {
    // SAFETY: Every input is readable for its explicit length; buffer writes stay
    // within a successful reservation, and each parser is freed exactly once.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert_eq!(XML_ParseBuffer(parser, 0, 0), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 42);
        assert_eq!(XML_Parse(parser, c"<r/>".as_ptr(), 4, 0), OK);
        assert_eq!(XML_ParseBuffer(parser, 1, 0), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 42);
        assert_eq!(XML_ParseBuffer(parser, 0, 1), OK);
        assert_eq!(XML_ParseBuffer(parser, 0, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 36);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        let input = b"<r/>\xe2\x82";
        assert_eq!(
            XML_Parse(parser, input.as_ptr().cast(), input.len() as c_int, 0),
            OK
        );
        assert_eq!(XML_ParseBuffer(parser, 0, 1), ERROR);
        assert_eq!(XML_GetErrorCode(parser), 6);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        let buffer = XML_GetBuffer(parser, 1).cast::<u8>();
        assert!(!buffer.is_null());
        buffer.write(b'<');
        assert_eq!(XML_ParseBuffer(parser, 2, 0), ERROR);
        assert_eq!(XML_GetErrorCode(parser), INVALID_ARGUMENT);
        assert_eq!(XML_ParseBuffer(parser, 1, 0), OK);
        assert_eq!(XML_Parse(parser, c"r/>".as_ptr(), 3, 0), OK);
        assert_eq!(XML_ParseBuffer(parser, 0, 1), OK);
        XML_ParserFree(parser);
    }
}

#[derive(Default)]
struct GrammarCallbackState {
    root: XML_Parser,
    action: u8,
    requests: usize,
    events: Vec<String>,
}

unsafe extern "C" fn grammar_attlist(
    data: *mut c_void,
    _: *const c_char,
    name: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: c_int,
) {
    // SAFETY: Expat-style callback strings and the test's stack state are live.
    unsafe {
        (*data.cast::<GrammarCallbackState>())
            .events
            .push(format!("attr:{}", CStr::from_ptr(name).to_str().unwrap()));
    }
}

unsafe extern "C" fn grammar_default(data: *mut c_void, text: *const c_char, length: c_int) {
    // SAFETY: The callback span is readable and test state outlives every child.
    unsafe {
        let text =
            std::str::from_utf8(std::slice::from_raw_parts(text.cast(), length as usize)).unwrap();
        (*data.cast::<GrammarCallbackState>())
            .events
            .push(format!("raw:{text}"));
    }
}

unsafe extern "C" fn grammar_external(
    parser: XML_Parser,
    context: *const c_char,
    _: *const c_char,
    system: *const c_char,
    _: *const c_char,
) -> c_int {
    // SAFETY: No reference to shared state crosses a callback-capable API.
    // Child handles and each GetBuffer span are used only while valid.
    unsafe {
        let state = XML_GetUserData(parser).cast::<GrammarCallbackState>();
        let leaf = !system.is_null() && CStr::from_ptr(system).to_bytes() == b"p";
        if leaf {
            (*state).requests += 1;
            (*state).events.push("external:p".into());
            assert_eq!(XML_StopParser(parser, 1), ERROR);
            assert_eq!(XML_GetErrorCode(parser), 37);
            match (*state).action {
                1 => return OK,
                2 => assert_eq!(XML_StopParser((*state).root, 1), OK),
                3 => {
                    assert_eq!(XML_StopParser(parser, 0), OK);
                    return OK;
                }
                4 => XML_SetAttlistDeclHandler(parser, None),
                5 => XML_SetAttlistDeclHandler(parser, Some(grammar_attlist)),
                _ => {}
            }
        }
        let child = XML_ExternalEntityParserCreate(parser, context, ptr::null());
        assert!(!child.is_null());
        let mut status = OK;
        if leaf {
            status = XML_Parse(child, ptr::null(), 0, 1);
        } else {
            let bytes = b"<!ENTITY % p SYSTEM 'p'><!ATTLIST r a CDATA 'A' %p; b CDATA 'B'>";
            for (index, byte) in bytes.iter().enumerate() {
                let buffer = XML_GetBuffer(child, 1).cast::<u8>();
                assert!(!buffer.is_null());
                buffer.write(*byte);
                status = XML_ParseBuffer(child, 1, c_int::from(index + 1 == bytes.len()));
                if status != OK {
                    break;
                }
            }
        }
        XML_ParserFree(child);
        c_int::from(status == OK)
    }
}

#[test]
fn external_grammar_keeps_early_callbacks_stop_state_and_handler_changes() {
    // SAFETY: All parsers are freed once before their callback state goes away.
    unsafe {
        for action in 0..=5 {
            for namespaces in [false, true] {
                let root = if namespaces {
                    XML_ParserCreateNS(ptr::null(), b'|' as c_char)
                } else {
                    XML_ParserCreate(ptr::null())
                };
                assert!(!root.is_null());
                let mut state = GrammarCallbackState {
                    root,
                    action,
                    ..GrammarCallbackState::default()
                };
                XML_SetUserData(root, ptr::from_mut(&mut state).cast());
                XML_SetParamEntityParsing(root, 2);
                XML_SetExternalEntityRefHandler(root, Some(grammar_external));
                XML_SetDefaultHandlerExpand(root, Some(grammar_default));
                if action != 5 {
                    XML_SetAttlistDeclHandler(root, Some(grammar_attlist));
                }
                let bytes = b"<!DOCTYPE r SYSTEM 'd'><r/>";
                let status = XML_Parse(root, bytes.as_ptr().cast(), bytes.len() as c_int, 1);
                assert_eq!(state.requests, 1);
                let external = state
                    .events
                    .iter()
                    .position(|event| event == "external:p")
                    .unwrap();
                if action != 5 {
                    let before = state
                        .events
                        .iter()
                        .position(|event| event == "attr:a")
                        .unwrap();
                    assert!(before < external);
                }
                if action == 3 {
                    assert_eq!(status, ERROR);
                    assert_eq!(XML_GetErrorCode(root), 21);
                } else {
                    assert_eq!(status, if action == 2 { SUSPENDED } else { OK });
                    if action == 2 {
                        assert_eq!(XML_ResumeParser(root), OK);
                        assert_eq!(state.requests, 1);
                    }
                    let after = state.events.iter().position(|event| event == "attr:b");
                    assert_eq!(after.is_some(), !matches!(action, 1 | 4));
                    if let Some(after) = after {
                        assert!(external < after);
                    }
                    if matches!(action, 1 | 4) {
                        let raw: String = state.events[external + 1..]
                            .iter()
                            .filter_map(|event| event.strip_prefix("raw:"))
                            .collect();
                        assert!(raw.contains("b CDATA"), "action={action}: {raw}");
                    }
                }
                XML_ParserFree(root);
            }
        }
    }
}

#[test]
fn external_grammar_continuation_outlives_its_ancestor() {
    // SAFETY: The external child independently owns its inherited state. The
    // ancestor is freed before parsing and is never used by these callbacks.
    unsafe {
        let root = XML_ParserCreate(ptr::null());
        assert!(!root.is_null());
        let child = XML_ExternalEntityParserCreate(root, ptr::null(), ptr::null());
        assert!(!child.is_null());
        XML_ParserFree(root);
        let mut state = GrammarCallbackState::default();
        XML_SetUserData(child, ptr::from_mut(&mut state).cast());
        XML_SetParamEntityParsing(child, 2);
        XML_SetExternalEntityRefHandler(child, Some(grammar_external));
        XML_SetAttlistDeclHandler(child, Some(grammar_attlist));
        let input = b"<!ENTITY % p SYSTEM 'p'><!ATTLIST r a CDATA 'A' %p; b CDATA 'B'>";
        for (index, byte) in input.iter().enumerate() {
            let buffer = XML_GetBuffer(child, 1).cast::<u8>();
            assert!(!buffer.is_null());
            buffer.write(*byte);
            assert_eq!(
                XML_ParseBuffer(child, 1, c_int::from(index + 1 == input.len())),
                OK
            );
        }
        assert_eq!(state.events, ["attr:a", "external:p", "attr:b"]);
        XML_ParserFree(child);
    }
}

#[test]
fn memory_helpers_clear_the_ambient_parser_tracking_scope() {
    // SAFETY: The helper blocks use their live parser's allocator. Nesting the
    // scope reproduces a helper call made by an event callback during parsing.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        let tracker = Shared::clone(&(*parser).tracker);
        assert_eq!(XML_SetAllocTrackerActivationThreshold(parser, 0), 1);
        let initial = tracker.live_bytes();
        with_tracking(&tracker, || {
            let malloc = XML_MemMalloc(parser, 1000);
            let realloc = XML_MemRealloc(parser, ptr::null_mut(), 1000);
            assert!(!malloc.is_null());
            assert!(!realloc.is_null());
            assert_eq!(tracker.live_bytes(), initial);
            XML_MemFree(parser, malloc);
            XML_MemFree(parser, realloc);
        });
        XML_ParserFree(parser);
        assert_eq!(tracker.live_bytes(), 0);
    }
}

#[derive(Default)]
struct ContextState {
    parser: XML_Parser,
    original: Vec<u8>,
    observations: Vec<(usize, Vec<u8>)>,
    suspend_next: bool,
}

unsafe extern "C" fn record_context(arg: *mut c_void) {
    // SAFETY: The caller retains state and input until parsing finishes. Context
    // pointers are inspected and copied inside this callback only.
    unsafe {
        let state = &mut *arg.cast::<ContextState>();
        let mut offset = -1;
        let mut size = -1;
        let bytes = XML_GetInputContext(state.parser, &mut offset, &mut size);
        assert!(!bytes.is_null());
        assert!(offset >= 0 && size >= offset);
        let index = XML_GetCurrentByteIndex(state.parser) as usize;
        let count = XML_GetCurrentByteCount(state.parser) as usize;
        let context = std::slice::from_raw_parts(bytes.cast::<u8>(), size as usize);
        let start = index - offset as usize;
        assert_eq!(context, &state.original[start..start + context.len()]);
        let raw = context[offset as usize..offset as usize + count].to_vec();
        state.observations.push((index, raw));
        if state.suspend_next {
            state.suspend_next = false;
            assert_eq!(XML_StopParser(state.parser, 1), OK);
        }
    }
}

unsafe extern "C" fn context_start(arg: *mut c_void, _: *const c_char, _: *const *const c_char) {
    // SAFETY: Forward the live callback state without retaining the callback data.
    unsafe { record_context(arg) };
}

unsafe extern "C" fn context_text(arg: *mut c_void, _: *const c_char, _: c_int) {
    // SAFETY: Forward the live callback state without retaining the callback data.
    unsafe { record_context(arg) };
}

#[test]
fn input_context_preserves_original_bytes_across_feeds_and_suspension() {
    let xml = format!(
        "<!DOCTYPE r [<!ENTITY e 'expanded'>]><r a='{}'>{}&e;<s/></r>",
        "attribute".repeat(300),
        "content".repeat(600)
    );
    let mut utf16 = vec![0xff, 0xfe];
    utf16.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    for original in [xml.into_bytes(), utf16] {
        for width in [1, 7, 4096, original.len()] {
            for buffered in [false, true] {
                // SAFETY: Input, callback state and parser are owned until cleanup.
                unsafe {
                    let parser = XML_ParserCreate(ptr::null());
                    let mut state = ContextState {
                        parser,
                        original: original.clone(),
                        suspend_next: true,
                        ..ContextState::default()
                    };
                    XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                    XML_SetStartElementHandler(parser, Some(context_start));
                    XML_SetCharacterDataHandler(parser, Some(context_text));
                    let mut offset = -1;
                    let mut size = -1;
                    assert!(XML_GetInputContext(parser, &mut offset, &mut size).is_null());
                    let chunks = original.len().div_ceil(width);
                    for (index, chunk) in original.chunks(width).enumerate() {
                        let final_input = c_int::from(index + 1 == chunks);
                        let status = if buffered {
                            let buffer = XML_GetBuffer(parser, chunk.len() as c_int);
                            assert!(!buffer.is_null());
                            ptr::copy_nonoverlapping(chunk.as_ptr(), buffer.cast(), chunk.len());
                            XML_ParseBuffer(parser, chunk.len() as c_int, final_input)
                        } else {
                            XML_Parse(
                                parser,
                                chunk.as_ptr().cast(),
                                chunk.len() as c_int,
                                final_input,
                            )
                        };
                        assert!(matches!(status, OK | SUSPENDED));
                        if status == SUSPENDED {
                            assert!(XML_GetInputContext(parser, &mut offset, &mut size).is_null());
                            assert_eq!(XML_ResumeParser(parser), OK);
                        }
                    }
                    assert!(state.observations.len() >= 4);
                    assert!(XML_GetInputContext(parser, &mut offset, &mut size).is_null());
                    assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
                    assert!(XML_GetInputContext(parser, &mut offset, &mut size).is_null());
                    assert_eq!(XML_GetCurrentByteIndex(parser), -1);
                    assert_eq!(XML_Parse(parser, ptr::null(), 0, 0), OK);
                    assert_eq!(XML_GetCurrentByteIndex(parser), 0);
                    XML_ParserFree(parser);
                }
            }
        }
    }
}

unsafe extern "C" fn release_without_input_context(data: *mut c_void) {
    // SAFETY: Encoding release owns live callback state and a live parser handle.
    unsafe {
        let state = &mut *data.cast::<State>();
        state.releases += 1;
        let mut offset = -1;
        let mut size = -1;
        assert!(XML_GetInputContext(state.parser, &mut offset, &mut size).is_null());
    }
}

#[test]
fn input_context_is_inactive_during_reset_and_destruction_callbacks() {
    for reset in [false, true] {
        // SAFETY: The release callback is installed with test-owned state, and
        // reset/free owns the handle until that callback returns.
        unsafe {
            let mut state = State::default();
            let parser = configured(&mut state);
            assert_eq!(XML_Parse(parser, c"<r>".as_ptr(), 3, 0), OK);
            (*parser).encoding_release = Some(release_without_input_context);
            (*parser).encoding_data = ptr::from_mut(&mut state).cast();
            if reset {
                assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
            }
            XML_ParserFree(parser);
            assert_eq!(state.releases, 1);
        }
    }
}

#[test]
fn input_context_discards_consumed_eventless_whitespace() {
    for dtd in [false, true] {
        // SAFETY: The parser owns all fed bytes; no callbacks retain its context.
        unsafe {
            let parser = XML_ParserCreate(ptr::null());
            if dtd {
                let prefix = b"<!DOCTYPE r [";
                assert_eq!(
                    XML_Parse(parser, prefix.as_ptr().cast(), prefix.len() as c_int, 0),
                    OK
                );
            }
            let whitespace = [b' '; 1024];
            for _ in 0..1024 {
                assert_eq!(
                    XML_Parse(
                        parser,
                        whitespace.as_ptr().cast(),
                        whitespace.len() as c_int,
                        0
                    ),
                    OK
                );
                assert!(
                    (*parser).core.input_context().0.len()
                        <= INPUT_CONTEXT_BYTES + whitespace.len() + 16
                );
            }
            let suffix = if dtd {
                b"]><r/>".as_slice()
            } else {
                b"<r/>".as_slice()
            };
            assert_eq!(
                XML_Parse(parser, suffix.as_ptr().cast(), suffix.len() as c_int, 1),
                OK
            );
            XML_ParserFree(parser);
        }
    }
}

unsafe extern "C" fn context_entity(
    arg: *mut c_void,
    _: *const c_char,
    _: c_int,
    _: *const c_char,
    _: c_int,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
) {
    // SAFETY: Forward the live callback state without retaining declaration data.
    unsafe { record_context(arg) };
}

#[test]
fn input_context_keeps_split_dtd_and_parameter_entity_anchors() {
    let original = format!(
        "<!ENTITY % p \"<!ENTITY e '{}'>\"><![INCLUDE[%p;<!ENTITY f '{}'>]]>",
        "first".repeat(700),
        "second".repeat(700)
    )
    .into_bytes();
    for width in [1, 7, 4096] {
        for defaults in [false, true] {
            // SAFETY: The child, parent, input and callback state stay live until
            // every event has been inspected and the child has been freed.
            unsafe {
                let parent = XML_ParserCreate(ptr::null());
                let parser = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
                let mut state = ContextState {
                    parser,
                    original: original.clone(),
                    ..ContextState::default()
                };
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                assert_eq!(XML_SetParamEntityParsing(parser, 2), 1);
                XML_SetEntityDeclHandler(parser, Some(context_entity));
                if defaults {
                    XML_SetDefaultHandlerExpand(parser, Some(context_text));
                }
                let chunks = original.len().div_ceil(width);
                for (index, chunk) in original.chunks(width).enumerate() {
                    assert_eq!(
                        XML_Parse(
                            parser,
                            chunk.as_ptr().cast(),
                            chunk.len() as c_int,
                            c_int::from(index + 1 == chunks)
                        ),
                        OK
                    );
                }
                assert!(state.observations.len() >= 3);
                XML_ParserFree(parser);
                XML_ParserFree(parent);
            }
        }
    }
}

#[test]
fn hash_salt_changes_root_configuration_and_preserves_owned_children() {
    // SAFETY: All handles are live, accessed serially, and freed once. Entropy
    // arrays and static XML bytes remain readable for their entire C calls.
    unsafe {
        let entropy = *b"0123456789abcdef";
        assert_eq!(XML_SetHashSalt(ptr::null_mut(), 1), 0);
        assert_eq!(XML_SetHashSalt16Bytes(ptr::null_mut(), entropy.as_ptr()), 0);
        let root = XML_ParserCreateNS(ptr::null(), b'|' as c_char);
        assert!(!root.is_null());
        assert_eq!(XML_SetHashSalt16Bytes(root, ptr::null()), 0);
        assert_eq!(XML_SetHashSalt16Bytes(root, entropy.as_ptr()), 1);
        assert_eq!(XML_SetHashSalt16Bytes(root, entropy.as_ptr()), 1);
        assert_eq!((*root).core.hash_salt(), entropy);
        let child = XML_ExternalEntityParserCreate(root, c"p=urn:p".as_ptr(), ptr::null());
        assert!(!child.is_null());
        assert_eq!((*child).core.hash_salt(), entropy);
        assert_eq!(XML_SetHashSalt(child, 0x12345678), 1);
        let mut integer_salt = [0; 16];
        integer_salt[8..].copy_from_slice(&0x12345678_u64.to_le_bytes());
        assert_eq!((*root).core.hash_salt(), integer_salt);
        assert_eq!((*child).core.hash_salt(), entropy);
        let xml = c"<p:r xml:lang='en'/>";
        assert_eq!(
            XML_Parse(child, xml.as_ptr(), xml.to_bytes().len() as c_int, 1),
            1
        );
        XML_ParserFree(child);
        let child = XML_ExternalEntityParserCreate(root, c"".as_ptr(), ptr::null());
        assert!(!child.is_null());
        assert_eq!((*child).core.hash_salt(), integer_salt);
        assert_eq!(XML_Parse(root, ptr::null(), 0, 0), 1);
        assert_eq!(XML_SetHashSalt(root, 9), 0);
        assert_eq!(XML_SetHashSalt16Bytes(child, entropy.as_ptr()), 0);
        assert_eq!((*root).core.hash_salt(), integer_salt);
        let xml = c"<!DOCTYPE r [<!ENTITY e 'ok'><!ATTLIST r a CDATA 'default'>]><r xml:lang='en'>&e;</r>";
        assert_eq!(
            XML_Parse(root, xml.as_ptr(), xml.to_bytes().len() as c_int, 1),
            1
        );
        // Expat permits configuration again after successful completion.
        assert_eq!(XML_SetHashSalt16Bytes(root, entropy.as_ptr()), 1);
        assert_eq!((*root).core.hash_salt(), entropy);
        assert_eq!(XML_Parse(root, ptr::null(), 0, 1), 0);
        assert_eq!(XML_ParserReset(root, ptr::null()), 1);
        assert_eq!((*root).core.hash_salt(), entropy);
        // Reset invalidates old parent lifetime tokens, even at the same address.
        assert_eq!(XML_SetHashSalt(child, 7), 0);
        XML_ParserFree(child);
        let xml = c"<r xml:lang='en'/>";
        assert_eq!(
            XML_Parse(root, xml.as_ptr(), xml.to_bytes().len() as c_int, 1),
            1
        );
        XML_ParserFree(root);
    }
}

unsafe extern "C" fn suspend_with_live_attribute_strings(
    data: *mut c_void,
    name: *const c_char,
    attributes: *const *const c_char,
) {
    // SAFETY: The outer test keeps State and its live parser on this thread. All
    // callback bytes remain readable until this function returns.
    unsafe {
        let state = &mut *data.cast::<State>();
        let value = CStr::from_ptr(*attributes.add(1));
        let before = value.to_bytes().to_vec();
        state.events.push(value.to_str().unwrap().to_owned());
        XML_ParserFree(state.parser);
        assert_eq!(XML_ParserReset(state.parser, ptr::null()), 0);
        assert_eq!(
            XML_Parse(state.parser, c"<recursive/>".as_ptr(), 12, 1),
            ERROR
        );
        assert_eq!(XML_StopParser(state.parser, 1), OK);
        assert_eq!(CStr::from_ptr(*attributes.add(1)).to_bytes(), before);
        assert_eq!(CStr::from_ptr(name).to_bytes(), b"n");
    }
}

#[test]
fn recycling_waits_for_callbacks_and_survives_suspend_resume_and_reset() {
    // SAFETY: Every parser, input and user-data pointer remains live through its
    // callbacks. Callback-time Free is ignored; final cleanup frees exactly once.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        let mut state = State {
            parser,
            ..State::default()
        };
        XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
        XML_SetStartElementHandler(parser, Some(suspend_with_live_attribute_strings));
        let input = c"<n a='first'><n a='second'/></n>";
        assert_eq!(
            XML_Parse(parser, input.as_ptr(), input.to_bytes().len() as c_int, 1),
            SUSPENDED
        );
        assert_eq!(state.events, ["first"]);
        assert_eq!(XML_ResumeParser(parser), SUSPENDED);
        assert_eq!(state.events, ["first", "second"]);
        assert_eq!(XML_ResumeParser(parser), OK);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
        XML_SetStartElementHandler(parser, Some(suspend_with_live_attribute_strings));
        let input = c"<n a='reset'/>";
        assert_eq!(
            XML_Parse(parser, input.as_ptr(), input.to_bytes().len() as c_int, 1),
            SUSPENDED
        );
        assert_eq!(XML_ResumeParser(parser), OK);
        assert_eq!(state.events, ["first", "second", "reset"]);
        XML_ParserFree(parser);
    }
}

#[test]
fn arena_start_preserves_raw_context_live_pointers_and_callback_switches() {
    #[derive(Default)]
    struct ArenaState {
        parser: XML_Parser,
        starts: usize,
        name: &'static [u8],
        raw: Vec<String>,
        ends: Vec<String>,
    }
    unsafe extern "C" fn raw(data: *mut c_void, value: *const c_char, length: c_int) {
        // SAFETY: The caller keeps state live and supplies callback-owned bytes.
        unsafe {
            let bytes = std::slice::from_raw_parts(value.cast(), length as usize);
            (*data.cast::<ArenaState>())
                .raw
                .push(String::from_utf8(bytes.to_vec()).unwrap());
        }
    }
    unsafe extern "C" fn end(data: *mut c_void, name: *const c_char) {
        // SAFETY: State and name remain live during this synchronous callback.
        unsafe {
            (*data.cast::<ArenaState>())
                .ends
                .push(CStr::from_ptr(name).to_str().unwrap().to_owned());
        }
    }
    unsafe extern "C" fn start(
        data: *mut c_void,
        name: *const c_char,
        attrs: *const *const c_char,
    ) {
        // SAFETY: All pointers are callback-lived. Raw state pointers prevent
        // retaining a mutable State reference across nested DefaultCurrent.
        unsafe {
            let state = data.cast::<ArenaState>();
            if CStr::from_ptr(name).to_bytes() != (*state).name {
                return;
            }
            let parser = (*state).parser;
            (*state).starts += 1;
            let value = CStr::from_ptr(*attrs.add(1)).to_bytes().to_vec();
            XML_DefaultCurrent(parser);
            let mut offset = 0;
            let mut size = 0;
            let context = XML_GetInputContext(parser, &mut offset, &mut size);
            assert!(!context.is_null());
            let context = std::slice::from_raw_parts(context.cast::<u8>(), size as usize);
            assert!(context[offset as usize..].starts_with(b"<n a="));
            assert_eq!(XML_SetBase(parser, c"changed".as_ptr()), OK);
            XML_ParserFree(parser);
            assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
            assert_eq!(XML_Parse(parser, c"<reenter/>".as_ptr(), 10, 1), ERROR);
            assert_eq!(CStr::from_ptr(name).to_bytes(), (*state).name);
            assert_eq!(CStr::from_ptr(*attrs.add(1)).to_bytes(), value);
            if (*state).starts == 2 {
                assert_eq!(XML_GetSpecifiedAttributeCount(parser), 4);
                assert_eq!(CStr::from_ptr(*attrs.add(3)).to_bytes(), b"more");
                XML_SetEndElementHandler(parser, Some(end));
                assert_eq!(XML_StopParser(parser, 1), OK);
                assert_eq!(CStr::from_ptr(*attrs.add(3)).to_bytes(), b"more");
            }
        }
    }
    // SAFETY: Each parser and state stays live through its callbacks and resumes.
    unsafe {
        for (input, namespace, name, expected_ends, second_raw) in [
            (
                b"<r><n a='first'/><n a='second' b='more'/></r>".as_slice(),
                false,
                b"n".as_slice(),
                ["n", "r"],
                "<n a='second' b='more'/>",
            ),
            (
                b"<r><n a='first'/><n a='second' b='more'></n></r>".as_slice(),
                false,
                b"n".as_slice(),
                ["n", "r"],
                "<n a='second' b='more'>",
            ),
            (
                b"<r><n a='first'/><n a='second' b='more'></n></r>".as_slice(),
                true,
                b"n".as_slice(),
                ["n", "r"],
                "<n a='second' b='more'>",
            ),
            (
                b"<r xmlns='urn'><n a='first'/><n a='second' b='more'/></r>".as_slice(),
                true,
                b"urn|n".as_slice(),
                ["urn|n", "urn|r"],
                "<n a='second' b='more'/>",
            ),
        ] {
            for width in [1, 7, input.len()] {
                let parser = if namespace {
                    XML_ParserCreateNS(ptr::null(), b'|' as c_char)
                } else {
                    XML_ParserCreate(ptr::null())
                };
                let mut state = ArenaState {
                    parser,
                    name,
                    ..ArenaState::default()
                };
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                XML_SetStartElementHandler(parser, Some(start));
                XML_SetDefaultHandler(parser, Some(raw));
                for (index, chunk) in input.chunks(width).enumerate() {
                    let status = XML_Parse(
                        parser,
                        chunk.as_ptr().cast(),
                        chunk.len() as c_int,
                        c_int::from((index + 1) * width >= input.len()),
                    );
                    if status == SUSPENDED {
                        assert_eq!((*parser).core.current_raw(), Some(second_raw));
                        assert_eq!(
                            XML_GetCurrentByteIndex(parser) as usize,
                            input
                                .windows(b"<n a='second'".len())
                                .position(|value| value == b"<n a='second'")
                                .unwrap()
                        );
                        let raw_count = state.raw.len();
                        XML_DefaultCurrent(parser);
                        assert_eq!(state.raw.len(), raw_count);
                        assert_eq!(XML_ResumeParser(parser), OK);
                    } else {
                        assert_eq!(status, OK);
                    }
                }
                assert_eq!(state.starts, 2);
                assert_eq!(
                    state
                        .raw
                        .iter()
                        .filter(|value| !value.is_empty())
                        .collect::<Vec<_>>(),
                    [&"<n a='first'/>".to_owned(), &second_raw.to_owned()]
                );
                assert_eq!(state.ends, expected_ends);
                XML_ParserFree(parser);
            }
        }
    }
}

unsafe extern "C" fn open_value_external(
    parser: XML_Parser,
    context: *const c_char,
    _: *const c_char,
    system: *const c_char,
    _: *const c_char,
) -> c_int {
    // SAFETY: Expat supplies callback-lived identifiers and a live parser. Each
    // child is owned until its synchronous parse finishes and is then freed once.
    unsafe {
        let child = XML_ExternalEntityParserCreate(parser, context, ptr::null());
        assert!(!child.is_null());
        let bytes: &[u8] = if CStr::from_ptr(system).to_bytes() == b"d" {
            b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b 'B'><!ENTITY n '%a;'>"
        } else {
            b"%b;"
        };
        let status = XML_Parse(child, bytes.as_ptr().cast(), bytes.len() as c_int, 1);
        XML_ParserFree(child);
        status
    }
}

#[test]
fn reset_discards_internal_parameter_entities_left_open_by_value_children() {
    // SAFETY: Input buffers remain live for each call, callbacks own their child
    // handles, and the root is reset only after synchronous parsing has finished.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert!(!parser.is_null());
        let bytes = b"<!DOCTYPE r SYSTEM 'd'><r>&n;</r>";
        for width in [1, 7, bytes.len()] {
            assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
            assert_eq!(XML_SetParamEntityParsing(parser, 2), 1);
            XML_SetExternalEntityRefHandler(parser, Some(open_value_external));
            for (index, part) in bytes.chunks(width).enumerate() {
                assert_eq!(
                    XML_Parse(
                        parser,
                        part.as_ptr().cast(),
                        part.len() as c_int,
                        c_int::from((index + 1) * width >= bytes.len()),
                    ),
                    OK
                );
            }
            let child = XML_ExternalEntityParserCreate(parser, ptr::null(), ptr::null());
            assert!(!child.is_null());
            let declaration = b"<!ENTITY m '%b;'>";
            assert_eq!(
                XML_Parse(
                    child,
                    declaration.as_ptr().cast(),
                    declaration.len() as c_int,
                    1
                ),
                ERROR
            );
            assert_eq!(XML_GetErrorCode(child), 12);
            XML_ParserFree(child);
        }
        XML_ParserFree(parser);
    }
}

#[test]
fn c_name_rules_match_fourth_edition_and_survive_reset_and_child_creation() {
    // SAFETY: Each parser and input remains live through synchronous parsing.
    unsafe {
        for namespaces in [false, true] {
            let parser = if namespaces {
                XML_ParserCreateNS(ptr::null(), b'|' as c_char)
            } else {
                XML_ParserCreate(ptr::null())
            };
            assert!(!parser.is_null());
            for (xml, accepted) in [
                ("<a\u{901}/>", true),
                ("<\u{901}/>", false),
                ("<a\u{10000}/>", false),
                ("<À/>", true),
            ] {
                assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
                assert_eq!(
                    XML_Parse(parser, xml.as_ptr().cast(), xml.len() as c_int, 1),
                    if accepted { OK } else { ERROR }
                );
                assert_eq!(XML_GetErrorCode(parser), if accepted { 0 } else { 4 });
            }
            assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
            for (context, xml) in [
                (c"".as_ptr(), "<\u{10000}/>"),
                (ptr::null(), "<!ENTITY \u{10000} 'v'>"),
            ] {
                let child = XML_ExternalEntityParserCreate(parser, context, ptr::null());
                assert!(!child.is_null());
                assert_eq!(
                    XML_Parse(child, xml.as_ptr().cast(), xml.len() as c_int, 1),
                    ERROR
                );
                assert_eq!(XML_GetErrorCode(child), 4);
                XML_ParserFree(child);
            }
            XML_ParserFree(parser);
        }
    }
}

#[test]
fn resetting_after_a_missing_parameter_restores_declaration_processing() {
    unsafe extern "C" fn text(data: *mut c_void, value: *const c_char, len: c_int) {
        // SAFETY: The test installs a live String as user data; callback bytes
        // remain readable for len bytes and are valid UTF-8 for these inputs.
        unsafe {
            let bytes = std::slice::from_raw_parts(value.cast(), len as usize);
            (*data.cast::<std::string::String>()).push_str(std::str::from_utf8(bytes).unwrap());
        }
    }
    // SAFETY: Each parser, callback state and input remain live until parsing ends.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert!(!parser.is_null());
        for (xml, expected) in [
            (
                "<!DOCTYPE r [%missing;<!ENTITY later 'L'>]><r>&later;</r>",
                "",
            ),
            ("<!DOCTYPE r [<!ENTITY later 'L'>]><r>&later;</r>", "L"),
        ] {
            assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
            assert_eq!(XML_SetParamEntityParsing(parser, 2), 1);
            let mut result = std::string::String::new();
            XML_SetUserData(parser, ptr::from_mut(&mut result).cast());
            XML_SetCharacterDataHandler(parser, Some(text));
            for (index, byte) in xml.as_bytes().iter().enumerate() {
                assert_eq!(
                    XML_Parse(
                        parser,
                        ptr::from_ref(byte).cast(),
                        1,
                        c_int::from(index + 1 == xml.len())
                    ),
                    OK
                );
            }
            assert_eq!(result, expected);
        }
        XML_ParserFree(parser);
    }
}

#[test]
fn doctype_closing_tokens_follow_each_handler_and_survive_mutation() {
    unsafe extern "C" fn element_start(_: *mut c_void, _: *const c_char, _: *const *const c_char) {}
    unsafe extern "C" fn element_end(_: *mut c_void, _: *const c_char) {}
    struct ClosingState {
        parser: XML_Parser,
        events: Vec<String>,
        action: u8,
    }
    unsafe extern "C" fn raw(data: *mut c_void, value: *const c_char, len: c_int) {
        // SAFETY: Test-owned callback data and bytes stay live for the callback.
        unsafe {
            let state = &mut *data.cast::<ClosingState>();
            let value = std::str::from_utf8(std::slice::from_raw_parts(value.cast(), len as usize))
                .unwrap();
            if let Some(previous) = state.events.last_mut().filter(|s| s.starts_with("raw:")) {
                previous.push_str(value);
            } else {
                state.events.push(format!("raw:{value}"));
            }
            let parser = state.parser;
            if state.action == 5 && value == "]" {
                state.action = 0;
                XML_SetStartDoctypeDeclHandler(parser, Some(start));
                assert_eq!(XML_StopParser(parser, 1), OK);
            }
        }
    }
    unsafe extern "C" fn start(
        data: *mut c_void,
        _: *const c_char,
        _: *const c_char,
        _: *const c_char,
        _: c_int,
    ) {
        // SAFETY: Copy callback controls before invoking the serialized C API.
        unsafe {
            let state = &mut *data.cast::<ClosingState>();
            state.events.push("doctype".into());
            let (parser, action) = (state.parser, state.action);
            if action == 1 {
                XML_SetStartDoctypeDeclHandler(parser, None);
                assert_eq!(XML_StopParser(parser, 1), OK);
            }
        }
    }
    unsafe extern "C" fn end(data: *mut c_void) {
        // SAFETY: The state is owned by this test and lives until parser free.
        unsafe {
            (*data.cast::<ClosingState>())
                .events
                .push("enddoctype".into())
        };
    }
    unsafe extern "C" fn external(
        parser: XML_Parser,
        context: *const c_char,
        _: *const c_char,
        _: *const c_char,
        _: *const c_char,
    ) -> c_int {
        // SAFETY: Related parser operations are serialized; child input finishes
        // before freeing it. No borrowed state is accessed across parser calls.
        unsafe {
            let data = XML_GetUserData(parser).cast::<ClosingState>();
            (*data).events.push("external".into());
            let action = (*data).action;
            match action {
                2 => XML_SetStartDoctypeDeclHandler(parser, Some(start)),
                3 => XML_SetStartDoctypeDeclHandler(parser, None),
                4 => XML_SetEndDoctypeDeclHandler(parser, Some(end)),
                _ => {}
            }
            let child = XML_ExternalEntityParserCreate(parser, context, ptr::null());
            assert!(!child.is_null());
            let status = XML_Parse(child, ptr::null(), 0, 1);
            XML_ParserFree(child);
            status
        }
    }
    type ClosingCase<'a> = (&'a [u8], bool, bool, u8, &'a [&'a str]);
    let cases: &[ClosingCase<'_>] = &[
        (b"<!DOCTYPE r>", true, false, 0, &["doctype"]),
        (b"<!DOCTYPE r>", true, false, 1, &["doctype"]),
        (
            b"<!DOCTYPE r>",
            false,
            true,
            0,
            &["raw:<!DOCTYPE r", "enddoctype"],
        ),
        (b"<!DOCTYPE r>", false, false, 0, &["raw:<!DOCTYPE r>"]),
        (b"<!DOCTYPE r [] \t>", true, false, 0, &["doctype", "raw:>"]),
        (
            b"<!DOCTYPE r [] \t>",
            true,
            true,
            0,
            &["doctype", "enddoctype"],
        ),
        (
            b"<!DOCTYPE r [] \t>",
            false,
            true,
            0,
            &["raw:<!DOCTYPE r [] \t", "enddoctype"],
        ),
        (
            b"<!DOCTYPE r [] \t>",
            false,
            false,
            5,
            &["raw:<!DOCTYPE r []>"],
        ),
        (
            b"<!DOCTYPE r SYSTEM 'dtd'>",
            true,
            false,
            3,
            &["doctype", "external"],
        ),
        (
            b"<!DOCTYPE r SYSTEM 'dtd'>",
            false,
            false,
            2,
            &["raw:<!DOCTYPE r SYSTEM 'dtd'", "external", "raw:>"],
        ),
        (
            b"<!DOCTYPE r SYSTEM 'dtd' [] \t>",
            false,
            false,
            2,
            &["raw:<!DOCTYPE r SYSTEM 'dtd' [] \t", "external", "raw:>"],
        ),
        (
            b"<!DOCTYPE r SYSTEM 'dtd' [] \t>",
            true,
            false,
            3,
            &["doctype", "external", "raw:>"],
        ),
        (
            b"<!DOCTYPE r SYSTEM 'dtd' [] \t>",
            true,
            false,
            4,
            &["doctype", "external", "enddoctype"],
        ),
    ];
    for &(declaration, has_start, has_end, action, expected) in cases {
        for width in 1..=declaration.len() + 4 {
            // SAFETY: Every buffer, handle and callback context remains owned
            // throughout parsing, any resume, reset and final cleanup.
            unsafe {
                let parser = XML_ParserCreate(ptr::null());
                assert!(!parser.is_null());
                let mut state = ClosingState {
                    parser,
                    events: Vec::new(),
                    action,
                };
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                XML_SetDefaultHandlerExpand(parser, Some(raw));
                XML_SetElementHandler(parser, Some(element_start), Some(element_end));
                XML_SetStartDoctypeDeclHandler(parser, has_start.then_some(start));
                XML_SetEndDoctypeDeclHandler(parser, has_end.then_some(end));
                XML_SetExternalEntityRefHandler(parser, Some(external));
                assert_eq!(XML_SetParamEntityParsing(parser, 2), 1);
                // Element callbacks consume the root's raw markup so every
                // default byte belongs to the declaration under review.
                let mut input = declaration.to_vec();
                input.extend_from_slice(b"<r/>");
                for (index, part) in input.chunks(width).enumerate() {
                    let mut status = XML_Parse(
                        parser,
                        part.as_ptr().cast(),
                        part.len() as c_int,
                        c_int::from((index + 1) * width >= input.len()),
                    );
                    while status == SUSPENDED {
                        status = XML_ResumeParser(parser);
                    }
                    assert_eq!(status, OK);
                }
                assert_eq!(state.events, expected, "{declaration:?}, width {width}");
                assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
                state.events.clear();
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                XML_SetDefaultHandlerExpand(parser, Some(raw));
                let reset_input = b"<!DOCTYPE r><r/>";
                assert_eq!(
                    XML_Parse(
                        parser,
                        reset_input.as_ptr().cast(),
                        reset_input.len() as c_int,
                        1
                    ),
                    OK
                );
                assert_eq!(state.events, ["raw:<!DOCTYPE r><r/>"]);
                XML_ParserFree(parser);
            }
        }
    }
}

#[test]
fn precreated_parameter_siblings_observe_skips_before_the_first_child_finishes() {
    struct State {
        precreate: bool,
        final_first: bool,
        text: std::string::String,
        later_declarations: usize,
    }
    unsafe extern "C" fn text(data: *mut c_void, value: *const c_char, len: c_int) {
        // SAFETY: State and the callback's UTF-8 input remain live for this call.
        unsafe {
            (*data.cast::<State>()).text.push_str(
                std::str::from_utf8(std::slice::from_raw_parts(value.cast(), len as usize))
                    .unwrap(),
            );
        }
    }
    unsafe extern "C" fn declaration(
        data: *mut c_void,
        name: *const c_char,
        _: c_int,
        _: *const c_char,
        _: c_int,
        _: *const c_char,
        _: *const c_char,
        _: *const c_char,
        _: *const c_char,
    ) {
        // SAFETY: The synchronous callback receives a terminated name and live State.
        unsafe {
            if CStr::from_ptr(name).to_bytes() == b"later" {
                (*data.cast::<State>()).later_declarations += 1;
            }
        }
    }
    unsafe extern "C" fn external(
        parent: XML_Parser,
        _: *const c_char,
        _: *const c_char,
        _: *const c_char,
        _: *const c_char,
    ) -> c_int {
        // SAFETY: Children remain live through parsing; scalar flags are copied
        // without keeping a State reference across nested callbacks.
        unsafe {
            let state = XML_GetUserData(parent).cast::<State>();
            let precreate = (*state).precreate;
            let final_first = (*state).final_first;
            let first = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
            assert!(!first.is_null());
            let mut second = if precreate {
                XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null())
            } else {
                ptr::null_mut()
            };
            assert_eq!(
                XML_Parse(first, c"%missing;".as_ptr(), 9, c_int::from(final_first)),
                OK
            );
            if second.is_null() {
                second = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
            }
            assert!(!second.is_null());
            let declaration = b"<!ENTITY later 'L'>";
            assert_eq!(
                XML_Parse(
                    second,
                    declaration.as_ptr().cast(),
                    declaration.len() as c_int,
                    1
                ),
                OK
            );
            if !final_first {
                assert_eq!(XML_Parse(first, ptr::null(), 0, 1), OK);
            }
            XML_ParserFree(second);
            XML_ParserFree(first);
            OK
        }
    }
    // SAFETY: State, input and both generations of parser live through each call.
    unsafe {
        for precreate in [false, true] {
            for final_first in [false, true] {
                let parser = XML_ParserCreate(ptr::null());
                assert!(!parser.is_null());
                let mut state = State {
                    precreate,
                    final_first,
                    text: std::string::String::new(),
                    later_declarations: 0,
                };
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                XML_SetParamEntityParsing(parser, 2);
                XML_SetExternalEntityRefHandler(parser, Some(external));
                XML_SetEntityDeclHandler(parser, Some(declaration));
                XML_SetCharacterDataHandler(parser, Some(text));
                let xml = b"<!DOCTYPE r SYSTEM 'd'><r>&later;</r>";
                assert_eq!(
                    XML_Parse(parser, xml.as_ptr().cast(), xml.len() as c_int, 1),
                    OK
                );
                assert!(state.text.is_empty());
                assert_eq!(state.later_declarations, 0);
                let old_child = XML_ExternalEntityParserCreate(parser, ptr::null(), ptr::null());
                assert!(!old_child.is_null());
                assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
                let old_declaration = b"<!ENTITY later 'old'>";
                assert_eq!(
                    XML_Parse(
                        old_child,
                        old_declaration.as_ptr().cast(),
                        old_declaration.len() as c_int,
                        1
                    ),
                    OK
                );
                assert_eq!(state.later_declarations, 0);
                XML_ParserFree(old_child);
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                XML_SetCharacterDataHandler(parser, Some(text));
                let fresh = b"<!DOCTYPE r [<!ENTITY later 'fresh'>]><r>&later;</r>";
                assert_eq!(
                    XML_Parse(parser, fresh.as_ptr().cast(), fresh.len() as c_int, 1),
                    OK
                );
                assert_eq!(state.text, "fresh");
                XML_ParserFree(parser);
            }
        }
    }
}

unsafe extern "C" fn suspend_entity_comment(data: *mut c_void, _: *const c_char) {
    // SAFETY: The test passes its live parser as user data and only suspends it
    // during this synchronous callback. The surrounding test owns final cleanup.
    unsafe {
        assert_eq!(XML_StopParser(data.cast(), 1), OK);
    }
}

#[test]
fn active_entity_membership_survives_suspend_and_is_discarded_on_reset() {
    // SAFETY: The parser and input bytes outlive each synchronous callback;
    // resumption/reset occur after callbacks return and the handle is freed once.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        assert!(!parser.is_null());
        let bytes = b"<!DOCTYPE r [<!ENTITY a '<!--pause-->text'>]><r>&a;&a;</r>";
        for reset_suspended in [false, true] {
            XML_SetUserData(parser, parser.cast());
            XML_SetCommentHandler(parser, Some(suspend_entity_comment));
            assert_eq!(
                XML_Parse(parser, bytes.as_ptr().cast(), bytes.len() as c_int, 1),
                SUSPENDED
            );
            if !reset_suspended {
                assert_eq!(XML_ResumeParser(parser), SUSPENDED);
                assert_eq!(XML_ResumeParser(parser), OK);
            }
            assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        }
        let cycle = b"<!DOCTYPE r [<!ENTITY a '&a;'>]><r>&a;</r>";
        assert_eq!(
            XML_Parse(parser, cycle.as_ptr().cast(), cycle.len() as c_int, 1),
            ERROR
        );
        assert_eq!(XML_GetErrorCode(parser), 12);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        let fresh = b"<!DOCTYPE r [<!ENTITY a 'new'>]><r>&a;</r>";
        assert_eq!(
            XML_Parse(parser, fresh.as_ptr().cast(), fresh.len() as c_int, 1),
            OK
        );
        XML_ParserFree(parser);
    }
}

fn policy_entity_declarations(count: usize, chain: usize, parameter: bool) -> String {
    use std::fmt::Write;
    let mut dtd = String::new();
    for index in 0..count {
        if parameter {
            if index == 0 {
                dtd.push_str("<!ENTITY % e0 '<!--leaf-->'>");
            } else if index < chain {
                write!(dtd, "<!ENTITY % e{index} '&#37;e{};'>", index - 1).unwrap();
            } else {
                write!(dtd, "<!ENTITY % e{index} ''>").unwrap();
            }
        } else if index == 0 {
            dtd.push_str("<!ENTITY e0 'leaf'>");
        } else if index < chain {
            write!(dtd, "<!ENTITY e{index} '&e{};'>", index - 1).unwrap();
        } else {
            write!(dtd, "<!ENTITY e{index} ''>").unwrap();
        }
    }
    dtd
}

unsafe extern "C" fn policy_text(data: *mut c_void, value: *const c_char, length: c_int) {
    // SAFETY: Tests retain their String and the callback buffer is valid for length.
    unsafe {
        let bytes = std::slice::from_raw_parts(value.cast(), length as usize);
        (*data.cast::<String>()).push_str(std::str::from_utf8(bytes).unwrap());
    }
}

unsafe extern "C" fn policy_comment(data: *mut c_void, _: *const c_char) {
    // SAFETY: Tests retain this counter throughout synchronous parsing.
    unsafe { *data.cast::<usize>() += 1 };
}

#[test]
fn c_entity_limits_survive_all_constructors_reset_and_both_child_modes() {
    let dtd = policy_entity_declarations(10_001, 64, false);
    let document = format!("<!DOCTYPE r [{dtd}]><r>&e63;&e10000;&e63;</r>");
    // SAFETY: All parser handles, strings and callback state remain test-owned.
    unsafe {
        for constructor in 0..3 {
            let parser = match constructor {
                0 => XML_ParserCreate(ptr::null()),
                1 => XML_ParserCreateNS(ptr::null(), b'|' as c_char),
                _ => XML_ParserCreate_MM(ptr::null(), ptr::null(), ptr::null()),
            };
            assert!(!parser.is_null());
            for reset in [false, true] {
                if reset {
                    assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
                }
                let mut text = String::new();
                XML_SetUserData(parser, ptr::from_mut(&mut text).cast());
                XML_SetCharacterDataHandler(parser, Some(policy_text));
                assert_eq!(
                    XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
                    OK
                );
                assert_eq!(text, "leafleaf");
            }
            let child = XML_ExternalEntityParserCreate(parser, c"".as_ptr(), ptr::null());
            assert!(!child.is_null());
            let mut text = String::new();
            XML_SetUserData(child, ptr::from_mut(&mut text).cast());
            assert_eq!(XML_Parse(child, c"&e63;&e10000;&e63;".as_ptr(), 18, 1), OK);
            assert_eq!(text, "leafleaf");
            XML_ParserFree(child);
            XML_ParserFree(parser);
        }
        let parent = XML_ParserCreate(ptr::null());
        let child = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
        assert!(!child.is_null());
        assert_eq!(XML_SetParamEntityParsing(child, 2), 1);
        let mut comments = 0_usize;
        XML_SetUserData(child, ptr::from_mut(&mut comments).cast());
        XML_SetCommentHandler(child, Some(policy_comment));
        let dtd = format!(
            "{}%e63;%e10000;%e63;",
            policy_entity_declarations(10_001, 64, true)
        );
        assert_eq!(
            XML_Parse(child, dtd.as_ptr().cast(), dtd.len() as c_int, 1),
            OK
        );
        assert_eq!(comments, 2);
        XML_ParserFree(child);
        XML_ParserFree(parent);
    }
    // The compatibility policy belongs to C constructors. Existing safe callers
    // still reject the same document at their conservative declaration ceiling.
    let mut parser = Parser::new(Config::default());
    parser.feed(document.as_bytes(), true).unwrap();
    loop {
        match parser.next_event() {
            Ok(Some(_)) => {}
            Err(error) => {
                assert_eq!(error.kind, ErrorKind::LimitExceeded);
                break;
            }
            Ok(None) => panic!("safe defaults unexpectedly accepted the large DTD"),
        }
    }
}

#[test]
fn c_entity_count_and_depth_boundaries_remain_finite() {
    // SAFETY: Each large case owns one bounded parser and frees it before the next.
    unsafe {
        for count in [100_000, 100_001] {
            let document = format!(
                "<!DOCTYPE r [{}]><r/>",
                policy_entity_declarations(count, 1, false)
            );
            let parser = XML_ParserCreate(ptr::null());
            assert!(!parser.is_null());
            let status = XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1);
            assert_eq!(status, if count == 100_000 { OK } else { ERROR });
            assert_eq!(
                XML_GetErrorCode(parser),
                if count == 100_000 { 0 } else { 43 }
            );
            XML_ParserFree(parser);
        }
        // A fresh, manually created parameter child uses one external depth level
        // without consuming a declaration slot. Delayed percent references retain
        // each frame until the leaf, independently testing depth100000 and100001.
        for internal_depth in [99_999, 100_000] {
            let parent = XML_ParserCreate(ptr::null());
            // Preserve the relative allocation guard while providing real direct
            // input as its denominator. Child-only input does not increase it.
            let prime = format!("<!--{}--><r/>", "p".repeat(6 * 1024 * 1024));
            assert_eq!(
                XML_Parse(parent, prime.as_ptr().cast(), prime.len() as c_int, 1),
                OK
            );
            let child = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
            assert!(!child.is_null());
            assert_eq!(XML_SetParamEntityParsing(child, 2), 1);
            let dtd = format!(
                "{}%e{};",
                policy_entity_declarations(internal_depth, internal_depth, true),
                internal_depth - 1
            );
            let mut comments = 0_usize;
            XML_SetUserData(child, ptr::from_mut(&mut comments).cast());
            XML_SetCommentHandler(child, Some(policy_comment));
            let status = XML_Parse(child, dtd.as_ptr().cast(), dtd.len() as c_int, 1);
            assert_eq!(status, if internal_depth == 99_999 { OK } else { ERROR });
            assert_eq!(
                XML_GetErrorCode(child),
                if internal_depth == 99_999 { 0 } else { 43 }
            );
            assert_eq!(comments, usize::from(internal_depth == 99_999));
            XML_ParserFree(child);
            XML_ParserFree(parent);
        }
    }
}

#[test]
fn wider_c_entity_limits_keep_cycle_and_work_guards() {
    // SAFETY: Tests own live parsers; the work limit is lowered before any input.
    unsafe {
        let parser = XML_ParserCreate(ptr::null());
        let cycle = c"<!DOCTYPE r [<!ENTITY e '&e;'>]><r>&e;</r>";
        assert_eq!(
            XML_Parse(parser, cycle.as_ptr(), cycle.to_bytes().len() as c_int, 1),
            ERROR
        );
        assert_eq!(XML_GetErrorCode(parser), 12);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        let empty = format!(
            "<!DOCTYPE r [<!ENTITY e ''>]><r>{}</r>",
            "&e;".repeat(100_000)
        );
        assert_eq!(
            XML_Parse(parser, empty.as_ptr().cast(), empty.len() as c_int, 1),
            OK
        );
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        let mut limits = (*parser).config.limits.clone();
        limits.max_entity_expansion_bytes = 7;
        limits.max_work_amplification = None;
        (*parser).core.set_limits(limits).unwrap();
        let work = c"<!DOCTYPE r [<!ENTITY e 'leaf'>]><r>&e;&e;</r>";
        assert_eq!(
            XML_Parse(parser, work.as_ptr(), work.to_bytes().len() as c_int, 1),
            ERROR
        );
        assert_eq!(XML_GetErrorCode(parser), 43);
        XML_ParserFree(parser);
    }
}

#[test]
fn unused_ordinary_attlist_payloads_do_not_consume_adapter_event_bytes() {
    // SAFETY: The test owns each parser, reads only scalar counters between API
    // calls, and releases every handle after its synchronous parse finishes.
    unsafe {
        let xml = b"<!DOCTYPE r [<!ATTLIST r a CDATA 'A' b CDATA 'B'>]><r/>";
        let mut totals = [0; 2];
        for (index, enabled) in [false, true].into_iter().enumerate() {
            let parser = XML_ParserCreate(ptr::null());
            assert!(!parser.is_null());
            if enabled {
                XML_SetAttlistDeclHandler(parser, Some(ignore_attlist_declaration));
            }
            assert_eq!(
                XML_Parse(parser, xml.as_ptr().cast(), xml.len() as c_int, 1),
                OK
            );
            let family = &(*parser).family;
            totals[index] = family.callback_bytes.get();
            XML_ParserFree(parser);
        }
        assert_eq!(totals[1] - totals[0], 2 * (1 + 1 + 5 + 1));
        // The retained StartElement/default-value payloads still enforce the
        // same family limit when no declaration callback is requested.
        for remaining in [totals[0] - 1, totals[0]] {
            let parser = XML_ParserCreate(ptr::null());
            assert!(!parser.is_null());
            let family = &(*parser).family;
            family
                .callback_bytes
                .set(INITIAL_CALLBACK_BYTES - remaining);
            let status = XML_Parse(parser, xml.as_ptr().cast(), xml.len() as c_int, 1);
            assert_eq!(status, if remaining == totals[0] { OK } else { ERROR });
            assert_eq!(XML_GetErrorCode(parser), if status == OK { 0 } else { 43 });
            XML_ParserFree(parser);
        }
    }
}

#[test]
fn ordinary_attlist_handlers_change_at_tokens_and_resume_between_attributes() {
    struct State {
        parser: XML_Parser,
        install_at_literal: bool,
        installed: bool,
        names: std::vec::Vec<std::string::String>,
        saw_empty_default_current: bool,
        suspended: bool,
    }
    unsafe extern "C" fn raw(data: *mut c_void, text: *const c_char, length: c_int) {
        // SAFETY: User data and callback bytes remain live for this synchronous
        // call; copy fields before the callback-capable setter is invoked.
        unsafe {
            let pointer = data.cast::<State>();
            let text = std::slice::from_raw_parts(text.cast::<u8>(), length as usize);
            if text.is_empty() {
                (*pointer).saw_empty_default_current = true;
            }
            let install = !(*pointer).installed
                && text
                    == if (*pointer).install_at_literal {
                        b"'A'".as_slice()
                    } else {
                        b"<!ATTLIST".as_slice()
                    };
            if install {
                (*pointer).installed = true;
                XML_SetAttlistDeclHandler((*pointer).parser, Some(attribute));
            }
        }
    }
    unsafe extern "C" fn attribute(
        data: *mut c_void,
        _: *const c_char,
        name: *const c_char,
        _: *const c_char,
        _: *const c_char,
        _: c_int,
    ) {
        // SAFETY: Only an owned name and scalar state cross nested DefaultCurrent;
        // no reference to mutable callback state is held during that API call.
        unsafe {
            let pointer = data.cast::<State>();
            let name = CStr::from_ptr(name).to_str().unwrap().to_owned();
            let first = name == "a";
            (*pointer).names.push(name);
            if first {
                let parser = (*pointer).parser;
                assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
                XML_ParserFree(parser);
                XML_DefaultCurrent(parser);
                assert!((*pointer).saw_empty_default_current);
                assert_eq!(XML_StopParser(parser, 1), OK);
                (*pointer).suspended = true;
            }
        }
    }
    // SAFETY: Every parser, callback state and input allocation remains live until
    // parsing/resuming has completed; reset and final free occur outside callbacks.
    unsafe {
        for utf16 in [false, true] {
            for width in [1, 7, 4096] {
                for install_at_literal in [false, true] {
                    let parser = XML_ParserCreate(ptr::null());
                    assert!(!parser.is_null());
                    let mut state = State {
                        parser,
                        install_at_literal,
                        installed: false,
                        names: std::vec::Vec::new(),
                        saw_empty_default_current: false,
                        suspended: false,
                    };
                    XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                    XML_SetDefaultHandlerExpand(parser, Some(raw));
                    let xml = if width == 4096 {
                        format!(
                            "<!--{}--><!DOCTYPE r [<!ATTLIST r a CDATA 'A' b CDATA 'B'>]>{}<r/>",
                            "x".repeat(70000),
                            " ".repeat(70000)
                        )
                    } else {
                        "<!DOCTYPE r [<!ATTLIST r a CDATA 'A' b CDATA 'B'>]><r/>".to_owned()
                    };
                    let mut bytes = std::vec::Vec::new();
                    if utf16 {
                        bytes.extend_from_slice(&[0xff, 0xfe]);
                        for unit in xml.encode_utf16() {
                            bytes.extend_from_slice(&unit.to_le_bytes());
                        }
                    } else {
                        bytes.extend_from_slice(xml.as_bytes());
                    }
                    for (index, part) in bytes.chunks(width).enumerate() {
                        let mut status = XML_Parse(
                            parser,
                            part.as_ptr().cast(),
                            part.len() as c_int,
                            c_int::from((index + 1) * width >= bytes.len()),
                        );
                        if status == SUSPENDED {
                            assert_eq!(state.names, ["a"]);
                            status = XML_ResumeParser(parser);
                        }
                        assert_eq!(status, OK);
                    }
                    if install_at_literal {
                        assert_eq!(state.names, ["b"]);
                    } else {
                        assert_eq!(state.names, ["a", "b"]);
                        assert!(state.suspended);
                    }
                    assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
                    assert!(!(*parser).attlist_dispatch);
                    XML_ParserFree(parser);
                }
            }
        }
    }
}

#[test]
fn arena_text_enforces_exact_callback_budget_with_default_fallback() {
    for value in ["abc", "abcdefghijklmnopqrstuvwxyz0123456789"] {
        for handler in [false, true] {
            for remaining in [value.len() - 1, value.len()] {
                // SAFETY: The independent frame owns callback bytes; the manual
                // busy interval matches run_events and state lives until free.
                unsafe {
                    let mut state = State::default();
                    let parser = configured(&mut state);
                    XML_SetCharacterDataHandler(parser, handler.then_some(text));
                    XML_SetDefaultHandler(parser, Some(text));
                    let input = format!("<r>{value}<");
                    (*parser).core.feed(input.as_bytes(), false).unwrap();
                    (*parser).core.next_event().unwrap().unwrap();
                    let mut frame = (*parser).core.adapter_frame();
                    let mut event = None;
                    (*parser)
                        .core
                        .next_event_for_adapter_into(&mut event, &mut frame)
                        .unwrap()
                        .unwrap();
                    assert!(event.is_none());
                    assert_eq!(frame.text_bytes(), Some(value.as_bytes()));
                    let family = &(*parser).family;
                    family
                        .callback_bytes
                        .set(INITIAL_CALLBACK_BYTES - remaining);
                    (*parser).busy = true;
                    dispatch_text_frame(parser, frame.text_bytes().unwrap()).unwrap();
                    (*parser).busy = false;
                    if remaining < value.len() {
                        assert_eq!(XML_GetErrorCode(parser), 43);
                        assert!(state.events.is_empty());
                    } else {
                        assert_eq!(XML_GetErrorCode(parser), 0);
                        assert_eq!(state.events, [format!("text:{value}")]);
                    }
                    (*parser).core.finish_adapter_frame(frame);
                    XML_ParserFree(parser);
                }
            }
        }
    }
}

#[test]
fn arena_text_keeps_bytes_raw_context_and_handlers_live_through_suspension() {
    #[derive(Default)]
    struct TextState {
        parser: XML_Parser,
        first: usize,
        later: usize,
        raw: Vec<String>,
    }
    unsafe extern "C" fn raw(data: *mut c_void, bytes: *const c_char, len: c_int) {
        // SAFETY: The caller owns this state; callback bytes remain live.
        unsafe {
            (*data.cast::<TextState>()).raw.push(
                String::from_utf8(std::slice::from_raw_parts(bytes.cast(), len as usize).to_vec())
                    .unwrap(),
            );
        }
    }
    unsafe extern "C" fn later(data: *mut c_void, _: *const c_char, _: c_int) {
        // SAFETY: State is live through the synchronous callback.
        unsafe {
            (*data.cast::<TextState>()).later += 1;
        }
    }
    unsafe extern "C" fn first(data: *mut c_void, bytes: *const c_char, len: c_int) {
        // SAFETY: Raw state access avoids a mutable reference across the nested
        // DefaultCurrent callback. Eligible Text bytes stay in the C input context,
        // which remains stable under the busy guard through callback return.
        unsafe {
            let state = data.cast::<TextState>();
            let parser = (*state).parser;
            (*state).first += 1;
            let original = std::slice::from_raw_parts(bytes.cast::<u8>(), len as usize).to_vec();
            XML_DefaultCurrent(parser);
            let mut offset = 0;
            let mut size = 0;
            let context = XML_GetInputContext(parser, &mut offset, &mut size);
            assert!(!context.is_null());
            let context = std::slice::from_raw_parts(context.cast::<u8>(), size as usize);
            assert!(context[offset as usize..].starts_with(&original));
            assert_eq!(XML_SetBase(parser, c"changed".as_ptr()), OK);
            XML_ParserFree(parser);
            assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
            assert_eq!(XML_Parse(parser, c"<bad/>".as_ptr(), 6, 1), ERROR);
            XML_SetCharacterDataHandler(parser, Some(later));
            assert_eq!(XML_StopParser(parser, 1), OK);
            assert_eq!(
                std::slice::from_raw_parts(bytes.cast::<u8>(), len as usize),
                original
            );
        }
    }
    // SAFETY: Parser and user state stay live through parse, nested callbacks,
    // suspended getters, resume, and the final parser free.
    unsafe {
        for value in ["abc", "abcdefghijklmnopqrstuvwxyz0123456789"] {
            let content = format!("{value}\nend");
            let input = format!("<r>{content}&amp;tail</r>");
            let parser = XML_ParserCreate(ptr::null());
            let mut state = TextState {
                parser,
                ..TextState::default()
            };
            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
            XML_SetCharacterDataHandler(parser, Some(first));
            XML_SetDefaultHandler(parser, Some(raw));
            assert_eq!(
                XML_Parse(parser, input.as_ptr().cast(), input.len() as c_int, 1),
                SUSPENDED
            );
            assert_eq!(XML_GetCurrentByteIndex(parser), 3);
            assert_eq!(XML_GetCurrentByteCount(parser), content.len() as c_int);
            assert_eq!((*parser).core.current_raw(), Some(content.as_str()));
            let raw_count = state.raw.len();
            XML_DefaultCurrent(parser);
            assert_eq!(state.raw.len(), raw_count);
            assert_eq!(XML_ResumeParser(parser), OK);
            assert_eq!((state.first, state.later), (1, 2));
            assert!(state.raw.iter().any(|raw| raw == &content));
            XML_ParserFree(parser);
        }
    }
}
