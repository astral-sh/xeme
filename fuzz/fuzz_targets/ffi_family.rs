#![no_main]

//! Valid external parser families, allocator faults, and parent/child lifetime order.

use std::cell::Cell;
use std::ffi::{c_char, c_void};
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

unsafe fn parse(parser: XML_Parser, bytes: &[u8], width: usize) {
    // Each pointer and slice stays live for its call; no handlers are installed.
    unsafe {
        for chunk in bytes.chunks(width) {
            if XML_Parse(parser, chunk.as_ptr().cast(), chunk.len() as i32, 0) != 1 {
                break;
            }
        }
        XML_Parse(parser, ptr::null(), 0, 1);
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 16 || data.len() > 65536 {
        return;
    }
    assert_eq!(LIVE.get(), 0);
    CALLS.set(0);
    FAIL_AT.set(if data[0] & 1 == 0 {
        0
    } else {
        usize::from(u16::from_le_bytes([data[1], data[2]])) % 512 + 1
    });
    let suite = XML_Memory_Handling_Suite {
        malloc_fcn: Some(allocate),
        realloc_fcn: Some(reallocate),
        free_fcn: Some(deallocate),
    };
    // Fixed stack slots own every live handle. A freed slot is cleared immediately,
    // and children keep their own lifetime token after a parent is freed/reset.
    unsafe {
        let mut parsers: [XML_Parser; 8] = [ptr::null_mut(); 8];
        parsers[0] = XML_ParserCreate_MM(ptr::null(), &suite, c"|".as_ptr());
        if !parsers[0].is_null() {
            let prefix = b"<!DOCTYPE r [<!ENTITY e 'value'><!ATTLIST r a CDATA 'default'>]><r>";
            XML_Parse(parsers[0], prefix.as_ptr().cast(), prefix.len() as i32, 0);
            XML_SetBase(parsers[0], c"https://example.invalid/base".as_ptr());
        }
        // Always attempt a child first; later control bytes vary descendants,
        // sibling copies, feeding, reset, and destruction order without recursion.
        if !parsers[0].is_null() {
            let context = if data[3] & 1 == 0 {
                c"".as_ptr()
            } else {
                ptr::null()
            };
            parsers[1] = XML_ExternalEntityParserCreate(parsers[0], context, ptr::null());
        }
        let bytes = &data[16..];
        for (step, control) in data[4..16].iter().copied().enumerate() {
            let slot = usize::from(control >> 5);
            let parser = parsers[slot];
            if parser.is_null() {
                continue;
            }
            match control & 7 {
                0 | 1 => {
                    let next = (slot + 1 + step) % parsers.len();
                    if parsers[next].is_null() {
                        let context: *const c_char = if control & 1 == 0 {
                            c"".as_ptr()
                        } else {
                            ptr::null()
                        };
                        parsers[next] =
                            XML_ExternalEntityParserCreate(parser, context, ptr::null());
                    }
                }
                2 => parse(parser, bytes, usize::from(data[3]) + 1),
                3 => {
                    XML_ParserReset(parser, ptr::null());
                }
                4 => {
                    XML_ParserFree(parser);
                    parsers[slot] = ptr::null_mut();
                }
                5 => {
                    let count = bytes.len().min(4096);
                    let buffer = XML_GetBuffer(parser, count as i32);
                    if !buffer.is_null() {
                        ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast(), count);
                        XML_ParseBuffer(parser, count as i32, 1);
                    }
                }
                6 => {
                    XML_StopParser(parser, u8::from(control & 8 != 0));
                    XML_ResumeParser(parser);
                }
                _ => {
                    XML_SetParamEntityParsing(parser, i32::from(control % 3));
                    XML_UseForeignDTD(parser, 1);
                }
            }
        }
        // Feed surviving children after arbitrary parent destruction/reset, then
        // free every remaining handle exactly once. This also exercises DTD merge.
        for parser in parsers.iter().copied().skip(1).filter(|p| !p.is_null()) {
            parse(parser, bytes, 1 + usize::from(data[3]));
        }
        for parser in parsers.into_iter().rev().filter(|p| !p.is_null()) {
            XML_ParserFree(parser);
        }
    }
    assert_eq!(
        LIVE.get(),
        0,
        "custom allocation leaked by external parser family"
    );
});
