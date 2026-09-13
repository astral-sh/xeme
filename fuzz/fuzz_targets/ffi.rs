#![no_main]

//! The first eight bytes choose operations; the remaining bytes are XML input.
//! Handles and allocations remain valid even when callbacks request deletion.

use std::cell::Cell;
use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use libfuzzer_sys::fuzz_target;
use xeme_expat::*;

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
    parser: XML_Parser,
    actions: [u8; 4],
    callbacks: usize,
    models: Vec<*mut XML_Content>,
}

unsafe fn callback(arg: *mut c_void) {
    // No State reference crosses an API call: changing default handlers can invoke
    // future callbacks, and the fuzzer must not itself introduce aliasing errors.
    let state = arg.cast::<State>();
    let (parser, action) = unsafe {
        let action = (*state).actions[(*state).callbacks % 4] & 31;
        (*state).callbacks += 1;
        ((*state).parser, action)
    };
    unsafe {
        match action {
            0 => {
                XML_StopParser(parser, 1);
            }
            1 => {
                XML_StopParser(parser, 0);
            }
            2 => {
                XML_ParserFree(parser);
            }
            3 => XML_SetCharacterDataHandler(parser, None),
            4 => XML_SetDefaultHandlerExpand(parser, Some(text)),
            5 => {
                XML_Parse(parser, c"<recursive/>".as_ptr(), 12, 1);
            }
            6 => {
                XML_ParserReset(parser, ptr::null());
            }
            7 => {
                XML_SetBase(parser, c"fuzz:base".as_ptr());
                XML_SetCommentHandler(parser, Some(string));
            }
            _ => {}
        }
    }
}

unsafe extern "C" fn start(arg: *mut c_void, _: *const c_char, _: *const *const c_char) {
    unsafe { callback(arg) };
}

unsafe extern "C" fn string(arg: *mut c_void, _: *const c_char) {
    unsafe { callback(arg) };
}

unsafe extern "C" fn text(arg: *mut c_void, _: *const c_char, _: c_int) {
    unsafe { callback(arg) };
}

unsafe extern "C" fn model(arg: *mut c_void, _: *const c_char, model: *mut XML_Content) {
    let state = arg.cast::<State>();
    unsafe {
        // Keep callback-owned models until after parser destruction. A model's
        // custom allocator and tracker must remain alive independently.
        (*state).models.push(model);
        callback(arg);
    }
}

unsafe fn configure(state: *mut State, options: u8) {
    unsafe {
        let parser = (*state).parser;
        XML_SetUserData(parser, state.cast());
        XML_SetElementHandler(parser, Some(start), Some(string));
        XML_SetCharacterDataHandler(parser, Some(text));
        XML_SetCommentHandler(parser, Some(string));
        XML_SetElementDeclHandler(parser, Some(model));
        XML_SetReturnNSTriplet(parser, c_int::from(options & 1 != 0));
        XML_SetReparseDeferralEnabled(parser, u8::from(options & 2 != 0));
        XML_SetParamEntityParsing(parser, c_int::from(options & 4 != 0));
        if options & 8 != 0 {
            XML_SetDefaultHandler(parser, Some(text));
        }
        if options & 16 != 0 {
            XML_SetEncoding(parser, c"UTF-8".as_ptr());
        }
    }
}

unsafe fn document(state: *mut State, input: &[u8], options: u8, chunk: usize) {
    let mut position = 0;
    let mut iterations = 0;
    while position < input.len() || iterations == 0 {
        let end = input.len().min(position + chunk);
        let count = end - position;
        let final_input = c_int::from(end == input.len());
        let parser = unsafe { (*state).parser };
        let mut status = unsafe {
            if options & 32 != 0 {
                let buffer = XML_GetBuffer(parser, count as c_int);
                if buffer.is_null() {
                    return;
                }
                ptr::copy_nonoverlapping(input.as_ptr().add(position), buffer.cast(), count);
                XML_ParseBuffer(parser, count as c_int, final_input)
            } else {
                XML_Parse(
                    parser,
                    input.as_ptr().add(position).cast(),
                    count as c_int,
                    final_input,
                )
            }
        };
        while status == 2 && iterations < 1024 {
            status = unsafe { XML_ResumeParser(parser) };
            iterations += 1;
        }
        if status != 1 {
            return;
        }
        position = end;
        iterations += 1;
        if iterations >= 1024 {
            return;
        }
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 || data.len() > 65_536 {
        return;
    }
    assert_eq!(LIVE.get(), 0);
    CALLS.set(0);
    FAIL_AT.set(xeme_fuzz::allocation_failure_ordinal(u16::from_le_bytes([
        data[1], data[2],
    ])));
    let suite = XML_Memory_Handling_Suite {
        malloc_fcn: Some(allocate),
        realloc_fcn: Some(reallocate),
        free_fcn: Some(deallocate),
    };
    unsafe {
        let separator = if data[0] & 64 != 0 {
            c"|".as_ptr()
        } else {
            ptr::null()
        };
        let encoding = match (data[1] >> 1) & 7 {
            1 => c"UTF-8".as_ptr(),
            2 => c"UTF-16".as_ptr(),
            3 => c"UTF-16LE".as_ptr(),
            4 => c"UTF-16BE".as_ptr(),
            5 => c"ISO-8859-1".as_ptr(),
            6 => c"US-ASCII".as_ptr(),
            _ => ptr::null(),
        };
        let parser = XML_ParserCreate_MM(encoding, &suite, separator);
        if !parser.is_null() {
            let mut state = State {
                parser,
                actions: data[4..8].try_into().unwrap(),
                callbacks: 0,
                models: Vec::new(),
            };
            configure(&mut state, data[0]);
            let allocation = XML_MemMalloc(parser, usize::from(data[2]) + 1);
            if !allocation.is_null() {
                let resized = XML_MemRealloc(parser, allocation, usize::from(data[3]) + 1);
                XML_MemFree(
                    parser,
                    if resized.is_null() {
                        allocation
                    } else {
                        resized
                    },
                );
            }
            document(&mut state, &data[8..], data[0], usize::from(data[3]) + 1);
            if data[0] & 128 != 0 && XML_ParserReset(parser, ptr::null()) != 0 {
                configure(&mut state, data[0]);
                document(&mut state, &data[8..], data[0] ^ 32, 1 + data[2] as usize);
            }
            // Callback-time Free is ignored; the driver retains ownership.
            XML_ParserFree(parser);
            for model in state.models {
                XML_FreeContentModel(ptr::null_mut(), model);
            }
        }
    }
    assert_eq!(
        LIVE.get(),
        0,
        "parser family leaked custom-suite allocations"
    );
});
