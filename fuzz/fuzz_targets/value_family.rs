#![no_main]

//! Callback-driven external value children with bounded parser slots and recursion.
//! Thirty-two control bytes precede separately owned p/q input slices.

use std::cell::Cell;
use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use libfuzzer_sys::fuzz_target;
use oriole_expat::*;

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void;
    fn free(pointer: *mut c_void);
}

thread_local! {
    static CALLS: Cell<usize> = const { Cell::new(0) };
    static FAIL_AT: Cell<usize> = const { Cell::new(0) };
    static LIVE: Cell<usize> = const { Cell::new(0) };
}

fn allocation_fails(size: usize) -> bool {
    let call = CALLS.get() + 1;
    CALLS.set(call);
    call == FAIL_AT.get() || size > 8 * 1024 * 1024
}

unsafe extern "C" fn allocate(size: usize) -> *mut c_void {
    if allocation_fails(size) {
        return ptr::null_mut();
    }
    let pointer = unsafe { malloc(size.max(1)) };
    if !pointer.is_null() {
        LIVE.set(LIVE.get() + 1);
    }
    pointer
}

unsafe extern "C" fn reallocate(pointer: *mut c_void, size: usize) -> *mut c_void {
    if allocation_fails(size) {
        return ptr::null_mut();
    }
    let replacement = unsafe { realloc(pointer, size.max(1)) };
    if pointer.is_null() && !replacement.is_null() {
        LIVE.set(LIVE.get() + 1);
    }
    replacement
}

unsafe extern "C" fn deallocate(pointer: *mut c_void) {
    if !pointer.is_null() {
        LIVE.set(
            LIVE.get()
                .checked_sub(1)
                .expect("unowned custom allocation"),
        );
        unsafe { free(pointer) };
    }
}

struct State {
    family: *mut Family,
    depth: usize,
    callbacks: usize,
    parser: XML_Parser,
    handlers: usize,
    releases: usize,
    active: bool,
    conversions: usize,
    action: u8,
    mode: u8,
    value: i32,
    map_mode: u8,
}

impl State {
    fn new(data: &[u8], action: u8) -> Self {
        Self {
            family: ptr::null_mut(),
            depth: 0,
            callbacks: 0,
            parser: ptr::null_mut(),
            handlers: 0,
            releases: 0,
            active: false,
            conversions: 0,
            action,
            mode: data[4] & 7,
            value: i32::from(u16::from_le_bytes([data[6], data[7]])),
            map_mode: data[10] & 15,
        }
    }
}

unsafe extern "C" fn convert(data: *mut c_void, bytes: *const c_char) -> c_int {
    let state = data.cast::<State>();
    // Copy fields before entering the API. No Rust reference to callback state
    // crosses a call that could itself invoke another callback.
    unsafe {
        assert!((*state).active);
        (*state).conversions += 1;
        let parser = (*state).parser;
        let action = if (*state).conversions == 1 {
            (*state).action & 7
        } else {
            0
        };
        let mode = (*state).mode;
        let value = (*state).value;
        let lead = *bytes.cast::<u8>();
        let width = match lead {
            0x80 => 2,
            0x81 => 3,
            0x82 => 4,
            _ => panic!("converter called for an unmapped lead byte"),
        };
        // Read exactly the number of bytes promised by the lead-byte map. ASan
        // checks that even one-byte input feeds provide a complete wire character.
        let wire = std::slice::from_raw_parts(bytes.cast::<u8>(), width);
        let scalar = match mode {
            0 => [0xe9, 0x4e2d, 0x6f22][usize::from(lead - 0x80)],
            1 => value,
            2 => -1,
            3 => 0xd800 + (value & 0x7ff),
            4 => 0x10000 + value,
            5 => value & 0x7f,
            6 => wire[1..]
                .iter()
                .fold(0_i32, |scalar, byte| (scalar << 8) | i32::from(*byte)),
            _ => 0xfffe + (value & 1),
        };
        if action == 1 || action == 4 {
            let error = XML_GetErrorCode(parser);
            assert_eq!(XML_Parse(parser, c"<recursive/>".as_ptr(), 12, 1), 0);
            assert_eq!(XML_ParseBuffer(parser, 0, 0), 0);
            assert!(XML_GetBuffer(parser, 1).is_null());
            assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
            // Callback-time Free is ignored: the outer driver still owns this
            // handle and frees it after parsing (or resuming) has returned.
            XML_ParserFree(parser);
            assert_eq!(XML_GetErrorCode(parser), error);
        }
        match action {
            2 | 4 => {
                // Value children are external parameter parsers; resumable stops
                // are rejected while aborts remain supported.
                assert_eq!(XML_StopParser(parser, 1), 0);
                assert_eq!(XML_GetErrorCode(parser), 37);
            }
            3 => assert_eq!(XML_StopParser(parser, 0), 1),
            5 => {
                assert_eq!(XML_StopParser(parser, 0), 1);
                exercise_encoding_setter(state);
            }
            _ => {}
        }
        scalar
    }
}

/// Exercise the finished-state setter without borrowing a callback-owned slot.
unsafe fn exercise_encoding_setter(state: *mut State) {
    // SAFETY: Callers retain the live slot, parser, and family for this serialized
    // operation. Only copied scalars and raw pointers survive the C API calls.
    unsafe {
        let parser = (*state).parser;
        let controls = (*(*state).family).controls;
        let active = (*state).active;
        let handlers = (*state).handlers;
        let releases = (*state).releases;
        let encoding = match controls[30] & 3 {
            0 => ptr::null(),
            1 => c"UTF-8".as_ptr(),
            2 => c"multibyte".as_ptr(),
            _ => c"unused-completed-protocol-name".as_ptr(),
        };
        let mut status = XML_ParsingStatus {
            parsing: -1,
            finalBuffer: 0,
        };
        XML_GetParsingStatus(parser, &mut status);
        let error = XML_GetErrorCode(parser);
        let index = XML_GetCurrentByteIndex(parser);
        // Scope an additional failure to this setter; retain the original
        // per-input failure schedule for every subsequent parse and destructor.
        let force_failure = controls[29] & 4 != 0 && !encoding.is_null();
        let previous_failure = FAIL_AT.get();
        if force_failure {
            FAIL_AT.set(CALLS.get() + 1);
        }
        let result = XML_SetEncoding(parser, encoding);
        FAIL_AT.set(previous_failure);
        assert!(matches!(result, 0 | 1));
        if !matches!(status.parsing, 0 | 2) || force_failure {
            assert_eq!(result, 0);
        } else if encoding.is_null() {
            assert_eq!(result, 1);
        }
        assert_eq!(XML_GetErrorCode(parser), error);
        assert_eq!(XML_GetCurrentByteIndex(parser), index);
        assert_eq!((*state).active, active);
        assert_eq!((*state).handlers, handlers);
        assert_eq!((*state).releases, releases);
        if controls[31] & 1 != 0 {
            // Clearing requires no allocation and must work after a failed copy
            // whenever the parser state permits protocol metadata changes.
            assert_eq!(
                XML_SetEncoding(parser, ptr::null()),
                c_int::from(matches!(status.parsing, 0 | 2))
            );
            assert_eq!(XML_GetErrorCode(parser), error);
        }
    }
}

unsafe extern "C" fn release(data: *mut c_void) {
    // Each handler invocation transfers exactly one instance. The stack-owned
    // state remains live until this parser and its independent child are freed.
    unsafe {
        let state = data.cast::<State>();
        assert!((*state).active, "encoding instance released more than once");
        (*state).active = false;
        (*state).releases += 1;
        assert!((*state).releases <= (*state).handlers);
    }
}

unsafe extern "C" fn encoding(
    data: *mut c_void,
    _: *const c_char,
    info: *mut XML_Encoding,
) -> c_int {
    unsafe {
        let state = data.cast::<State>();
        assert!(
            !(*state).active,
            "encoding instance replaced without release"
        );
        (*state).active = true;
        (*state).handlers += 1;
        (*info).map = std::array::from_fn(|byte| if byte < 128 { byte as i32 } else { -1 });
        (*info).map[0x80] = if (*state).map_mode == 1 { -5 } else { -2 };
        (*info).map[0x81] = -3;
        (*info).map[0x82] = -4;
        if (*state).map_mode == 3 {
            (*info).map[usize::from(b'<')] = 0xe9;
        }
        (*info).data = data;
        (*info).convert = if (*state).map_mode == 2 {
            None
        } else {
            Some(convert)
        };
        (*info).release = Some(release);
    }
    1
}

struct Family {
    states: [State; 8],
    controls: [u8; 32],
    payload: [(*const u8, usize); 2],
    used: usize,
    requests: usize,
}

unsafe fn callback(state: *mut State) {
    // No reference to mutable family or slot storage survives a callback API.
    unsafe {
        (*state).callbacks += 1;
        if (*state).callbacks != 1 {
            return;
        }
        let family = (*state).family;
        if (*state).depth != usize::from((*family).controls[28] % 5) {
            return;
        }
        let parser = (*state).parser;
        match (*family).controls[24] & 7 {
            1 => {
                assert_eq!(XML_Parse(parser, c"<recursive/>".as_ptr(), 12, 1), 0);
                assert!(XML_GetBuffer(parser, 1).is_null());
                assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
                XML_ParserFree(parser);
            }
            2 => XML_SetEntityDeclHandler(parser, None),
            3 => XML_SetEntityDeclHandler(parser, Some(entity)),
            4 => {
                XML_StopParser(parser, 0);
            }
            5 => {
                let root = (*family).states[0].parser;
                if !root.is_null() {
                    XML_StopParser(root, 1);
                }
            }
            6 => XML_SetDefaultHandler(parser, None),
            7 => XML_SetDefaultHandlerExpand(parser, Some(text)),
            _ => {}
        }
    }
}

unsafe extern "C" fn text(data: *mut c_void, bytes: *const c_char, length: c_int) {
    assert!(length >= 0);
    if length != 0 {
        // C callback data is borrowed only for this invocation.
        let bytes = unsafe { std::slice::from_raw_parts(bytes.cast::<u8>(), length as usize) };
        assert!(std::str::from_utf8(bytes).is_ok());
    }
    unsafe { callback(data.cast()) };
}

unsafe extern "C" fn entity(
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
    unsafe {
        assert!(std::ffi::CStr::from_ptr(name).to_str().is_ok());
        if !value.is_null() {
            assert!(length >= 0);
            let bytes = std::slice::from_raw_parts(value.cast::<u8>(), length as usize);
            assert!(std::str::from_utf8(bytes).is_ok());
        }
        callback(data.cast());
    }
}

unsafe extern "C" fn declaration(data: *mut c_void, _: *const c_char, _: *const c_char, _: c_int) {
    unsafe { callback(data.cast()) };
}

unsafe fn configure(state: *mut State) {
    unsafe {
        let parser = (*state).parser;
        let controls = (*(*state).family).controls;
        XML_SetUserData(parser, state.cast());
        XML_SetUnknownEncodingHandler(parser, Some(encoding), state.cast());
        XML_SetExternalEntityRefHandler(parser, Some(external));
        XML_SetCharacterDataHandler(parser, Some(text));
        XML_SetEntityDeclHandler(
            parser,
            if controls[11] & 1 != 0 {
                Some(entity)
            } else {
                None
            },
        );
        XML_SetDefaultHandler(
            parser,
            if controls[11] & 2 != 0 {
                Some(text)
            } else {
                None
            },
        );
        XML_SetXmlDeclHandler(
            parser,
            if controls[11] & 4 != 0 {
                Some(declaration)
            } else {
                None
            },
        );
        XML_SetReparseDeferralEnabled(parser, controls[9] & 1);
        XML_SetParamEntityParsing(parser, c_int::from(controls[12] % 3));
    }
}

unsafe fn document(
    parser: XML_Parser,
    input: &[u8],
    buffered: bool,
    width: usize,
    final_input: bool,
) -> c_int {
    let mut position = 0;
    for _ in 0..4096 {
        let end = input.len().min(position + width);
        let count = end - position;
        let last = end == input.len();
        let mut status = unsafe {
            if buffered {
                let buffer = XML_GetBuffer(parser, count as c_int);
                if buffer.is_null() {
                    return 0;
                }
                ptr::copy_nonoverlapping(input.as_ptr().add(position), buffer.cast(), count);
                XML_ParseBuffer(parser, count as c_int, c_int::from(last && final_input))
            } else {
                XML_Parse(
                    parser,
                    input.as_ptr().add(position).cast(),
                    count as c_int,
                    c_int::from(last && final_input),
                )
            }
        };
        for _ in 0..8 {
            if status != 2 {
                break;
            }
            status = unsafe { XML_ResumeParser(parser) };
        }
        if status != 1 || last {
            return status;
        }
        position = end;
    }
    // Bounded partial input is a legal unfinished parser, even when the driver
    // did not reach the final chunk within its operation budget.
    1
}

unsafe extern "C" fn external(
    parser: XML_Parser,
    context: *const c_char,
    _: *const c_char,
    system: *const c_char,
    _: *const c_char,
) -> c_int {
    unsafe {
        let parent = XML_GetUserData(parser).cast::<State>();
        let family = (*parent).family;
        (*family).requests += 1;
        if (*family).requests > 64 {
            return 0;
        }
        let depth = (*parent).depth + 1;
        if depth > 4 || (*family).used == 8 {
            return 1;
        }
        // Reserve the unique slot before constructing or parsing a child. Any
        // nested callback sees the updated index and cannot overwrite its owner.
        let index = (*family).used;
        (*family).used += 1;
        let controls = (*family).controls;
        let value = !system.is_null() && std::ffi::CStr::from_ptr(system).to_bytes() != b"d";
        let action = if value { controls[16 + index] & 7 } else { 0 };
        if action == 1 {
            return 1;
        }
        let state = ptr::addr_of_mut!((*family).states[index]);
        (*state).depth = depth;
        let protocol = if value && controls[0] & 64 != 0 {
            c"multibyte".as_ptr()
        } else {
            ptr::null()
        };
        (*state).parser = XML_ExternalEntityParserCreate(parser, context, protocol);
        let child = (*state).parser;
        if child.is_null() {
            return 0;
        }
        configure(state);
        if action == 2 {
            return 1;
        }
        if action == 3 {
            XML_Parse(child, ptr::null(), 0, 0);
            return 1;
        }
        let input: &[u8] = if value {
            let which = usize::from(std::ffi::CStr::from_ptr(system).to_bytes() == b"q");
            let (bytes, length) = (*family).payload[which];
            // Both spans refer to the fuzzer-owned input, which outlives this
            // complete parser family. They do not borrow mutable Family storage.
            std::slice::from_raw_parts(bytes, length)
        } else {
            match controls[25] & 7 {
                0 => b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY % i 'I'><!ENTITY e 'L%p;R'><!ENTITY after 'A'>",
                1 => b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY e 'L%missing;%p;R'><!ENTITY after 'A'>",
                2 => b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY e 'FIRST'><!ENTITY e 'L%p;R'><!ENTITY after 'A'>",
                3 => b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY % e 'L%p;R'><!ENTITY after 'A'>",
                4 => br#"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY % quoted "'L&#37;p;R'"><!ENTITY e %quoted;><!ENTITY after 'A'>"#,
                5 => b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY e %missing;'L%p;R'><!ENTITY after 'A'>",
                6 => br#"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY % quoted "'L&#37;p;R'"><!ENTITY e %quoted;%missing;><!ENTITY after 'A'>"#,
                _ => br#"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY % quoted "'L&#37;p;R'"><!ENTITY e 'FIRST'><!ENTITY e %missing;%quoted;%missing;><!ENTITY after 'A'>"#,
            }
        };
        let status = document(
            child,
            input,
            controls[0] & 2 != 0,
            usize::from(controls[3]) + 1,
            action != 7,
        );
        if !matches!(action, 6 | 7) {
            XML_ParserFree(child);
            (*state).parser = ptr::null_mut();
        }
        match action {
            4 | 7 => 1,
            5 => 0,
            _ => c_int::from(status == 1),
        }
    }
}

fuzz_target!(|data: &[u8]| {
    if !(32..=65_536).contains(&data.len()) {
        return;
    }
    assert_eq!(LIVE.get(), 0);
    CALLS.set(0);
    FAIL_AT.set(if data[0] & 1 == 0 {
        0
    } else {
        usize::from(u16::from_le_bytes([data[1], data[2]])) % 2048 + 1
    });
    let controls: [u8; 32] = data[..32].try_into().unwrap();
    let body = &data[32..];
    let split = usize::from(u16::from_le_bytes([data[13], data[14]])) % (body.len() + 1);
    let mut family = Family {
        states: std::array::from_fn(|_| State::new(data, data[8])),
        controls,
        payload: [
            (body[..split].as_ptr(), split),
            (body[split..].as_ptr(), body.len() - split),
        ],
        used: 1,
        requests: 0,
    };
    let suite = XML_Memory_Handling_Suite {
        malloc_fcn: Some(allocate),
        realloc_fcn: Some(reallocate),
        free_fcn: Some(deallocate),
    };
    let family_pointer = ptr::from_mut(&mut family);
    for state in &mut family.states {
        state.family = family_pointer;
    }
    unsafe {
        family.states[0].parser = XML_ParserCreate_MM(
            ptr::null(),
            &suite,
            if data[0] & 32 != 0 {
                c"|".as_ptr()
            } else {
                ptr::null()
            },
        );
        let root = family.states[0].parser;
        if !root.is_null() {
            configure(ptr::addr_of_mut!(family.states[0]));
            XML_SetParamEntityParsing(root, 2);
            let input: &[u8] = if data[26] & 1 != 0 {
                b"<?xml version='1.0' standalone='yes'?><!DOCTYPE r SYSTEM 'd'><r/>"
            } else {
                b"<!DOCTYPE r SYSTEM 'd'><r/>"
            };
            document(
                root,
                input,
                data[0] & 2 != 0,
                usize::from(data[3]) + 1,
                true,
            );
            if data[29] & 1 != 0 {
                exercise_encoding_setter(ptr::addr_of_mut!(family.states[0]));
            }
            if data[15] & 1 != 0 {
                XML_ParserFree(root);
                family.states[0].parser = ptr::null_mut();
            } else if data[15] & 2 != 0 {
                XML_ParserReset(root, ptr::null());
            }
        }
        // Retained, unfinished children own their channels and encoding state.
        // Disabling new loads keeps this destruction/lifetime phase nonrecursive.
        for index in 1..family.used {
            let parser = family.states[index].parser;
            if !parser.is_null() {
                XML_SetExternalEntityRefHandler(parser, None);
                let which = index & 1;
                let (bytes, length) = family.payload[which];
                let input = std::slice::from_raw_parts(bytes, length);
                document(
                    parser,
                    input,
                    data[0] & 2 == 0,
                    usize::from(data[27]) + 1,
                    true,
                );
                if data[29] & 2 != 0 {
                    exercise_encoding_setter(ptr::addr_of_mut!(family.states[index]));
                }
            }
        }
        for offset in 0..family.used {
            let index = if data[15] & 4 != 0 {
                family.used - offset - 1
            } else {
                offset
            };
            let parser = family.states[index].parser;
            if !parser.is_null() {
                XML_ParserFree(parser);
                family.states[index].parser = ptr::null_mut();
            }
        }
    }
    for state in &family.states {
        assert!(!state.active, "encoding registration was not released");
        assert_eq!(state.handlers, state.releases);
    }
    assert_eq!(
        LIVE.get(),
        0,
        "external value family leaked custom allocations"
    );
});
