//! Small actual-ABI lifetime tests, including a Rust-backed MM suite for Miri.
use super::*;
use std::alloc::{Layout, alloc, dealloc, realloc};
use std::cell::RefCell;
use std::mem::{align_of, size_of};

#[repr(C, align(16))]
struct Header {
    layout: Layout,
}

thread_local! {
    static REQUESTS: RefCell<Vec<(bool, usize)>> = const { RefCell::new(Vec::new()) };
    static FAIL_AT: Cell<usize> = const { Cell::new(0) };
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static REENTRY: Cell<XML_Parser> = const { Cell::new(ptr::null_mut()) };
}

fn request(resize: bool, size: usize) -> bool {
    let ordinal = REQUESTS.with(|calls| {
        let mut calls = calls.borrow_mut();
        calls.push((resize, size));
        calls.len()
    });
    let parser = REENTRY.get();
    if !parser.is_null() {
        // SAFETY: The suite is called only while this live parser is guarded;
        // all these APIs must reject before borrowing parser storage.
        unsafe {
            assert!(in_allocator_callback());
            assert_eq!(XML_GetCurrentByteIndex(parser), -1);
            assert_eq!(XML_SetBase(parser, c"blocked".as_ptr()), ERROR);
            XML_SetCharacterDataHandler(parser, None);
            assert!(XML_GetBuffer(parser, 1).is_null());
            assert_eq!(XML_Parse(parser, ptr::null(), 0, 0), ERROR);
            XML_ParserFree(parser);
        }
    }
    FAIL_AT.get() == 0 || ordinal < FAIL_AT.get()
}

fn layout(size: usize) -> Option<Layout> {
    Layout::from_size_align(
        size.max(1).checked_add(size_of::<Header>())?,
        align_of::<Header>(),
    )
    .ok()
}

unsafe extern "C" fn mm_malloc(size: usize) -> *mut c_void {
    if !request(false, size) {
        return ptr::null_mut();
    }
    let Some(layout) = layout(size) else {
        return ptr::null_mut();
    };
    // SAFETY: Valid nonzero layout; aligned prefix records the matching layout.
    unsafe {
        let block = alloc(layout);
        if block.is_null() {
            return ptr::null_mut();
        }
        block.cast::<Header>().write(Header { layout });
        LIVE.set(LIVE.get() + 1);
        block.add(size_of::<Header>()).cast()
    }
}

unsafe extern "C" fn mm_realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    if pointer.is_null() {
        // SAFETY: A null realloc is a fresh allocation through the same suite.
        return unsafe { mm_malloc(size) };
    }
    if !request(true, size) {
        return ptr::null_mut();
    }
    let Some(new_layout) = layout(size) else {
        return ptr::null_mut();
    };
    // SAFETY: The pointer came from this suite; a failed realloc leaves it live.
    unsafe {
        let block = pointer.cast::<u8>().sub(size_of::<Header>());
        let old_layout = (*block.cast::<Header>()).layout;
        let replacement = realloc(block, old_layout, new_layout.size());
        if replacement.is_null() {
            return ptr::null_mut();
        }
        replacement
            .cast::<Header>()
            .write(Header { layout: new_layout });
        replacement.add(size_of::<Header>()).cast()
    }
}

unsafe extern "C" fn mm_free(pointer: *mut c_void) {
    if pointer.is_null() {
        return;
    }
    // SAFETY: One matching free for a live allocation from this suite.
    unsafe {
        let block = pointer.cast::<u8>().sub(size_of::<Header>());
        let layout = (*block.cast::<Header>()).layout;
        LIVE.set(LIVE.get() - 1);
        dealloc(block, layout);
    }
}

const SUITE: XML_Memory_Handling_Suite = XML_Memory_Handling_Suite {
    malloc_fcn: Some(mm_malloc),
    realloc_fcn: Some(mm_realloc),
    free_fcn: Some(mm_free),
};

fn clear_requests(fail_at: usize) {
    REQUESTS.with(|calls| calls.borrow_mut().clear());
    FAIL_AT.set(fail_at);
}

unsafe fn make_parser() -> XML_Parser {
    assert_eq!(LIVE.get(), 0);
    clear_requests(0);
    // SAFETY: Static callbacks implement allocation and alignment semantics.
    let parser = unsafe { XML_ParserCreate_MM(ptr::null(), &SUITE, ptr::null()) };
    assert!(!parser.is_null());
    parser
}

#[derive(Default)]
struct State {
    parser: XML_Parser,
    text: Vec<u8>,
    raw: Vec<u8>,
    calls: usize,
    action: u8,
    mutate: bool,
    expect_native: bool,
}

unsafe extern "C" fn default_text(data: *mut c_void, bytes: *const c_char, len: c_int) {
    // SAFETY: No callback-state reference survives a call back into the parser.
    unsafe {
        let state = data.cast::<State>();
        (*state)
            .raw
            .extend_from_slice(std::slice::from_raw_parts(bytes.cast::<u8>(), len as usize));
        XML_SetReparseDeferralEnabled((*state).parser, 1);
    }
}

unsafe extern "C" fn text(data: *mut c_void, bytes: *const c_char, len: c_int) {
    // SAFETY: Synchronous test callback; parser and State outlive every call.
    // Raw field projections, never an encompassing &mut State, cross nested calls.
    unsafe {
        let state = data.cast::<State>();
        let parser = (*state).parser;
        let original = std::slice::from_raw_parts(bytes.cast::<u8>(), len as usize).to_vec();
        (*state).calls += 1;
        (*state).text.extend_from_slice(&original);
        if (*state).expect_native {
            let mut offset = 0;
            let mut size = 0;
            let context = XML_GetInputContext(parser, &mut offset, &mut size);
            assert!(!context.is_null());
            assert!(offset >= 0 && len > 0 && offset <= size && len <= size - offset);
            assert_eq!(bytes, context.cast::<c_char>().add(offset as usize));
            let raw = (*parser).core.current_raw().unwrap();
            assert_eq!(raw.as_bytes(), original);
            assert_eq!(raw.as_ptr(), bytes.cast::<u8>());
        }
        if (*state).mutate {
            XML_SetDefaultHandlerExpand(parser, Some(default_text));
            XML_DefaultCurrent(parser);
            XML_SetCharacterDataHandler(parser, Some(text));
            XML_SetUserData(parser, data);
            XML_SetAttlistDeclHandler(parser, None);
            XML_SetNotationDeclHandler(parser, None);
            XML_SetReparseDeferralEnabled(parser, 0);
            assert_eq!(XML_SetBase(parser, c"changed/base".as_ptr()), OK);
            assert_eq!(CStr::from_ptr(XML_GetBase(parser)), c"changed/base");
            let block = XML_MemMalloc(parser, 7);
            assert!(!block.is_null());
            let block = XML_MemRealloc(parser, block, 19);
            assert!(!block.is_null());
            XML_MemFree(parser, block);
            let child = XML_ExternalEntityParserCreate(parser, c"".as_ptr(), ptr::null());
            assert!(!child.is_null());
            // Child storage is independent; avoid inheriting the parent test callback.
            XML_SetCharacterDataHandler(child, None);
            XML_SetDefaultHandler(child, None);
            assert_eq!(XML_Parse(child, c"<child/>".as_ptr(), 8, 1), OK);
            XML_ParserFree(child);
            assert_eq!(XML_Parse(parser, ptr::null(), 0, 0), ERROR);
            assert_eq!(XML_ParseBuffer(parser, 0, 0), ERROR);
            assert!(XML_GetBuffer(parser, 1).is_null());
            assert_eq!(XML_ResumeParser(parser), ERROR);
            assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
            XML_ParserFree(parser);
        }
        match (*state).action {
            1 => {
                (*state).action = 0;
                assert_eq!(XML_StopParser(parser, 1), OK);
            }
            2 => {
                (*state).action = 0;
                assert_eq!(XML_StopParser(parser, 0), OK);
                assert_eq!(XML_SetEncoding(parser, c"UTF-8".as_ptr()), OK);
            }
            _ => {}
        }
        if (*state).expect_native {
            assert_eq!((*parser).core.current_raw().unwrap().as_bytes(), original);
        }
        // This read is the alias/lifetime assertion, after every nested operation.
        assert_eq!(
            std::slice::from_raw_parts(bytes.cast::<u8>(), len as usize),
            original
        );
    }
}

unsafe fn parse(parser: XML_Parser, bytes: &[u8], final_input: bool, buffered: bool) -> c_int {
    // SAFETY: Copy to the caller-visible buffer before entering ParseBuffer;
    // neither source nor writable buffer reference survives into callbacks.
    unsafe {
        if buffered && !bytes.is_empty() {
            let buffer = XML_GetBuffer(parser, bytes.len() as c_int);
            assert!(!buffer.is_null());
            ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast(), bytes.len());
            XML_ParseBuffer(parser, bytes.len() as c_int, c_int::from(final_input))
        } else {
            XML_Parse(
                parser,
                bytes.as_ptr().cast(),
                bytes.len() as c_int,
                c_int::from(final_input),
            )
        }
    }
}

#[test]
fn context_text_pointer_survives_reentry_suspend_and_abort() {
    // SAFETY: All allocations, callbacks and raw state pointers are test-owned.
    unsafe {
        for buffered in [false, true] {
            for (prefix, value) in [
                ("", "abcdefghijklmnopqrstuvwxyz"),
                ("\u{feff}", "abcédefghijklmnopqrstuvwxyz"),
            ] {
                for action in [0, 1, 2] {
                    let parser = make_parser();
                    let mut state = State {
                        parser,
                        action,
                        mutate: true,
                        expect_native: true,
                        ..State::default()
                    };
                    XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                    XML_SetCharacterDataHandler(parser, Some(text));
                    REENTRY.set(parser);
                    let input = format!("{prefix}<r>{value}</r>");
                    let status = parse(parser, input.as_bytes(), true, buffered);
                    REENTRY.set(ptr::null_mut());
                    assert_eq!(state.text, value.as_bytes());
                    assert!(state.raw.starts_with(value.as_bytes()));
                    assert_eq!(state.calls, 1);
                    match action {
                        1 => {
                            assert_eq!(status, SUSPENDED);
                            assert_eq!((*parser).core.current_raw(), Some(value));
                            let raw_len = state.raw.len();
                            XML_DefaultCurrent(parser); // Outside a callback is a no-op.
                            assert_eq!(state.raw.len(), raw_len);
                            // Direct state reads ended the previous mutable reborrow.
                            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                            assert_eq!(XML_ResumeParser(parser), OK);
                        }
                        2 => {
                            assert_eq!(status, ERROR);
                            assert_eq!(XML_GetErrorCode(parser), 35);
                        }
                        _ => assert_eq!(status, OK),
                    }
                    XML_ParserFree(parser);
                    assert_eq!(LIVE.get(), 0);
                }
            }
        }
    }
}

#[test]
fn context_text_offsets_compaction_split_utf8_and_sizes() {
    // SAFETY: Context pointers are observed only inside text(), never after return.
    unsafe {
        for buffered in [false, true] {
            for count in [1, 23, 24, 4096, 4097] {
                let parser = make_parser();
                let mut state = State {
                    parser,
                    expect_native: count <= 4096,
                    ..State::default()
                };
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                XML_SetCharacterDataHandler(parser, Some(text));
                let input = format!("\u{feff}<r>{}</r>", "a".repeat(count));
                assert_eq!(parse(parser, input.as_bytes(), true, buffered), OK);
                assert_eq!(state.text, vec![b'a'; count]);
                XML_ParserFree(parser);
                assert_eq!(LIVE.get(), 0);
            }
            let parser = make_parser();
            let mut state = State {
                parser,
                expect_native: true,
                ..State::default()
            };
            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
            XML_SetCharacterDataHandler(parser, Some(text));
            let first = format!("<r>{}<n/>", "x".repeat(2048));
            assert_eq!(parse(parser, first.as_bytes(), false, buffered), OK);
            assert_eq!(parse(parser, b"\xc3", false, buffered), OK);
            assert!((*parser).core.input_context().1 > 0);
            assert_eq!(parse(parser, b"\xa9tail</r>", true, buffered), OK);
            assert_eq!(state.text, format!("{}étail", "x".repeat(2048)).as_bytes());
            XML_ParserFree(parser);
            assert_eq!(LIVE.get(), 0);
            for (chunks, expected, error) in [
                (
                    vec![b"\xef".as_slice(), b"\xbb", b"\xbf<r>ok</r>"],
                    b"ok".as_slice(),
                    0,
                ),
                (
                    vec![b"<r>abc".as_slice(), b"tail\xff</r>"],
                    b"abctail".as_slice(),
                    4,
                ),
                (
                    vec![b"<r>abc".as_slice(), b"tail\xc3"],
                    b"abctail".as_slice(),
                    6,
                ),
            ] {
                let parser = make_parser();
                let mut state = State {
                    parser,
                    expect_native: true,
                    ..State::default()
                };
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                XML_SetCharacterDataHandler(parser, Some(text));
                let input = chunks.concat();
                let mut result = OK;
                for (index, chunk) in chunks.iter().enumerate() {
                    result = parse(parser, chunk, index + 1 == chunks.len(), buffered);
                    if result != OK {
                        break;
                    }
                }
                assert_eq!(result, if error == 0 { OK } else { ERROR });
                assert_eq!(XML_GetErrorCode(parser), error);
                assert_eq!(state.text, expected);
                let (context, start) = (*parser).core.input_context();
                assert_eq!(context, &input[start..]);
                if error != 0 {
                    assert_eq!(
                        XML_GetCurrentByteIndex(parser),
                        3 + expected.len() as c_long
                    );
                }
                XML_ParserFree(parser);
                assert_eq!(LIVE.get(), 0);
            }
        }
    }
}

#[test]
fn context_text_default_and_excluded_paths_keep_exact_bytes() {
    // SAFETY: Parser handles and callback state are scoped to each full document.
    unsafe {
        for (input, expected) in [
            ("<r>a\r\nb</r>", "a\nb"),
            ("<r><![CDATA[abc]]></r>", "abc"),
            ("<r>&amp;</r>", "&"),
            ("<!DOCTYPE r [<!ENTITY e 'abc'>]><r>&e;</r>", "abc"),
        ] {
            let parser = make_parser();
            let mut state = State {
                parser,
                ..State::default()
            };
            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
            XML_SetCharacterDataHandler(parser, Some(text));
            assert_eq!(parse(parser, input.as_bytes(), true, false), OK);
            assert_eq!(state.text, expected.as_bytes());
            XML_ParserFree(parser);
            assert_eq!(LIVE.get(), 0);
        }
        let parser = make_parser();
        let mut state = State {
            parser,
            ..State::default()
        };
        XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
        XML_SetDefaultHandler(parser, Some(default_text));
        assert_eq!(parse(parser, b"<r>plain</r>", true, false), OK);
        assert_eq!(state.raw, b"<r>plain</r>");
        XML_ParserFree(parser);
        assert_eq!(LIVE.get(), 0);
    }
}

#[test]
fn context_text_frame_and_raw_oom_match_owned_mode() {
    // Use the same Rust MM suite with the safe core to compare allocation
    // requests and publication at every failure in the single Text delivery.
    for count in [1, 23, 24, 1024] {
        let mut success_requests = None;
        for fail_at in 0..=3 {
            let mut outcomes = Vec::new();
            for native in [false, true] {
                assert_eq!(LIVE.get(), 0);
                clear_requests(0);
                // SAFETY: The Rust-backed suite implements the documented contract.
                let allocator = unsafe {
                    Allocator::from_callbacks(MemorySuite {
                        malloc: Some(mm_malloc),
                        realloc: Some(mm_realloc),
                        free: Some(mm_free),
                    })
                    .unwrap()
                };
                let mut core = Parser::try_new_in(Config::default(), allocator).unwrap();
                let value = "x".repeat(count);
                core.feed(format!("<r>{value}</r>").as_bytes(), true)
                    .unwrap();
                core.next_event().unwrap().unwrap();
                let old_raw = core.current_raw().unwrap().to_owned();
                let mut frame = core.adapter_frame();
                let mut event = None;
                clear_requests(fail_at);
                let result = if native {
                    core.next_event_for_c_text_context_into(&mut event, &mut frame)
                } else {
                    core.next_event_for_adapter_into(&mut event, &mut frame)
                };
                let requests = REQUESTS.with(|calls| calls.borrow().clone());
                let error = result.err();
                if let Some(error) = error {
                    assert_eq!(error.kind, ErrorKind::NoMemory);
                    assert!(!frame.is_active() && event.is_none());
                    assert_eq!(core.current_raw(), Some(old_raw.as_str()));
                } else {
                    assert!(frame.is_active() && event.is_none());
                    assert_eq!(core.current_raw(), Some(value.as_str()));
                    assert_eq!(frame.native_text_range_for_c().is_some(), native);
                }
                outcomes.push((
                    requests,
                    error,
                    frame.is_active(),
                    core.position(),
                    core.current_raw().unwrap().to_owned(),
                ));
                FAIL_AT.set(0);
                core.finish_adapter_frame(frame);
                drop(core);
                assert_eq!(LIVE.get(), 0);
            }
            assert_eq!(outcomes[0], outcomes[1], "count={count}, failure={fail_at}");
            if fail_at == 0 {
                success_requests = Some(outcomes[0].0.len());
            }
        }
        assert_eq!(
            success_requests,
            Some(if count <= 23 {
                usize::from(count > 8)
            } else {
                2
            })
        );
    }
}

#[test]
fn context_text_invalid_host_range_fails_closed() {
    // SAFETY: Deliberately invalid scalar ranges are rejected before pointer use.
    unsafe {
        for case in 0..5 {
            let parser = make_parser();
            if case >= 3 {
                let input = format!("<r>{}<n/>", "x".repeat(2048));
                assert_eq!(parse(parser, input.as_bytes(), false, false), OK);
                assert_eq!(parse(parser, b"abc", false, false), OK);
            }
            let (context, context_start) = (*parser).core.input_context();
            let (start, count) = match case {
                0 => (0, 0),
                1 => (usize::MAX, 1),
                2 => (0, 4097),
                3 => (context_start.checked_sub(1).unwrap(), 1),
                _ => (context_start + context.len() - 1, 2),
            };
            let mut state = State {
                parser,
                ..State::default()
            };
            XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
            XML_SetCharacterDataHandler(parser, Some(text));
            (*parser).busy = true;
            (*parser).input_context_active = true;
            dispatch_context_text(parser, start, count).unwrap();
            assert_eq!((*parser).parse_error, UNEXPECTED_STATE);
            assert_eq!(state.calls, 0);
            (*parser).busy = false;
            XML_ParserFree(parser);
            assert_eq!(LIVE.get(), 0);
        }
    }
}

#[test]
fn context_text_warmed_reservations_do_not_allocate() {
    for markup_between in [false, true] {
        let mut outcomes = Vec::new();
        for native in [false, true] {
            assert_eq!(LIVE.get(), 0);
            clear_requests(0);
            // SAFETY: Same Rust allocation suite as the failure-order test.
            let allocator = unsafe {
                Allocator::from_callbacks(MemorySuite {
                    malloc: Some(mm_malloc),
                    realloc: Some(mm_realloc),
                    free: Some(mm_free),
                })
                .unwrap()
            };
            let mut core = Parser::try_new_in(Config::default(), allocator).unwrap();
            let value = "a".repeat(64);
            let input = if markup_between {
                format!("<r>{value}<n/>{value}</r>")
            } else {
                format!("<r>{value}")
            };
            core.feed(input.as_bytes(), markup_between).unwrap();
            core.next_event().unwrap().unwrap();
            let mut frame = core.adapter_frame();
            let mut event = None;
            for _ in 0..if markup_between { 3 } else { 1 } {
                if native {
                    core.next_event_for_c_text_context_into(&mut event, &mut frame)
                } else {
                    core.next_event_for_adapter_into(&mut event, &mut frame)
                }
                .unwrap()
                .unwrap();
            }
            if !markup_between {
                // Feed may allocate; arm the failure only for event delivery.
                // With no intervening markup, both Text owners retain capacity.
                core.feed(format!("{value}</r>").as_bytes(), true).unwrap();
            }
            clear_requests(1);
            let result = if native {
                core.next_event_for_c_text_context_into(&mut event, &mut frame)
            } else {
                core.next_event_for_adapter_into(&mut event, &mut frame)
            };
            let requests = REQUESTS.with(|calls| calls.borrow().clone());
            let error = if markup_between {
                // Markup swaps the large raw owner into token_scratch. Its
                // smaller replacement must regrow in both delivery modes.
                let error = result.unwrap_err();
                assert_eq!(error.kind, ErrorKind::NoMemory);
                assert_eq!(requests.len(), 1);
                assert!(!frame.is_active() && event.is_none());
                assert_eq!(core.current_raw(), None);
                Some(error)
            } else {
                result.unwrap().unwrap();
                assert!(requests.is_empty());
                assert_eq!(core.current_raw(), Some(value.as_str()));
                assert_eq!(frame.native_text_range_for_c().is_some(), native);
                None
            };
            outcomes.push((
                requests,
                error,
                core.position(),
                core.current_raw().map(str::to_owned),
            ));
            FAIL_AT.set(0);
            core.finish_adapter_frame(frame);
            drop(core);
            assert_eq!(LIVE.get(), 0);
        }
        assert_eq!(outcomes[0], outcomes[1], "markup_between={markup_between}");
    }
}

#[test]
fn context_text_late_handler_after_suspension_uses_eager_raw() {
    // SAFETY: The callback never retains its input pointer beyond its return.
    unsafe {
        let parser = make_parser();
        let mut state = State {
            parser,
            action: 1,
            expect_native: true,
            ..State::default()
        };
        XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
        XML_SetCharacterDataHandler(parser, Some(text));
        assert_eq!(parse(parser, b"<r>first<n/>", false, false), SUSPENDED);
        assert_eq!(state.text, b"first");
        assert_eq!((*parser).core.current_raw(), Some("first"));
        XML_SetCharacterDataHandler(parser, None);
        XML_SetDefaultHandlerExpand(parser, Some(default_text));
        XML_DefaultCurrent(parser);
        assert!(state.raw.is_empty());
        // Refresh the callback borrow after inspecting state between parses.
        XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
        assert_eq!(XML_ResumeParser(parser), OK);
        assert_eq!(parse(parser, b"", false, false), OK);
        assert_eq!(parse(parser, b"tail</r>", true, false), OK);
        assert_eq!(state.raw, b"<n/>tail</r>");
        assert_eq!(state.calls, 1);
        XML_ParserFree(parser);
        assert_eq!(LIVE.get(), 0);
    }
}

#[test]
fn context_input_allocation_failures_preserve_sticky_errors_and_owners() {
    // SAFETY: Parser and callback state are test-owned; failed allocations never
    // invalidate earlier owners, and every successful suite allocation is freed.
    unsafe {
        for (prefix, input, expected) in [
            (
                b"".as_slice(),
                b"<r>hello</r>".as_slice(),
                b"hello".as_slice(),
            ),
            (
                b"<r>seed<n/>".as_slice(),
                b"tail\xc3".as_slice(),
                b"tail".as_slice(),
            ),
            (
                b"<?xml version='1.0' ".as_slice(),
                b"encoding='ISO-8859-1'?><r>\xe9</r>".as_slice(),
                "é".as_bytes(),
            ),
        ] {
            let mut successful_requests = 0;
            for fail_at in 0..=64 {
                if fail_at != 0 && fail_at > successful_requests {
                    break;
                }
                let parser = make_parser();
                if !prefix.is_empty() {
                    assert_eq!(parse(parser, prefix, false, false), OK);
                }
                let mut state = State {
                    parser,
                    ..State::default()
                };
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                XML_SetCharacterDataHandler(parser, Some(text));
                clear_requests(fail_at);
                let partial = input.ends_with(b"\xc3");
                let result = parse(parser, input, !partial, false);
                if fail_at == 0 {
                    successful_requests = REQUESTS.with(|calls| calls.borrow().len());
                    assert!(successful_requests > 0 && successful_requests <= 64);
                    assert_eq!(result, OK);
                    if partial {
                        assert!(expected.starts_with(&state.text));
                        clear_requests(0);
                        assert_eq!(parse(parser, b"\xa9</r>", true, false), OK);
                        assert_eq!(state.text, "tailé".as_bytes());
                    } else {
                        assert_eq!(state.text, expected);
                    }
                } else {
                    assert_eq!(result, ERROR, "failure={fail_at}");
                    assert_eq!(XML_GetErrorCode(parser), 1);
                    assert!(expected.starts_with(&state.text));
                    let calls = state.calls;
                    assert_eq!(XML_Parse(parser, ptr::null(), 0, 1), ERROR);
                    assert_eq!(XML_GetErrorCode(parser), 1);
                    assert_eq!(state.calls, calls);
                    if fail_at == 1 {
                        assert_eq!(calls, 0);
                    }
                }
                clear_requests(0);
                XML_ParserFree(parser);
                assert_eq!(LIVE.get(), 0, "failure={fail_at}");
            }
        }
    }
}
