//! Independent allocator routing checks across the public C boundary.

use std::cell::Cell;
use std::ffi::{c_char, c_void};
use std::ptr;

use oriole_expat::*;

thread_local! {
    static OBSERVE: Cell<bool> = const { Cell::new(false) };
    static ESCAPES: Cell<usize> = const { Cell::new(0) };
    static CALLBACK_PARSER: Cell<XML_Parser> = const { Cell::new(ptr::null_mut()) };
    static REENTRIES: Cell<usize> = const { Cell::new(0) };
}

struct Global;

fn observe() {
    if OBSERVE.try_with(Cell::get).unwrap_or(false) {
        let _ = ESCAPES.try_with(|value| value.set(value.get() + 1));
    }
}

// SAFETY: Instrumentation uses allocation-free thread-local cells; every operation
// forwards the unchanged pointer and layout contract to the system allocator.
unsafe impl std::alloc::GlobalAlloc for Global {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        observe();
        // SAFETY: Forward the caller's valid layout.
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        observe();
        // SAFETY: Forward the caller's valid layout.
        unsafe { std::alloc::System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        observe();
        // SAFETY: Forward the caller's live allocation and requested new size.
        unsafe { std::alloc::System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
        // SAFETY: Forward the caller's live allocation and original layout.
        unsafe { std::alloc::System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static GLOBAL: Global = Global;

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void;
    fn free(pointer: *mut c_void);
}

fn attempt_reentry() {
    CALLBACK_PARSER.with(|parser| {
        let parser = parser.get();
        if parser.is_null() {
            return;
        }
        REENTRIES.with(|count| count.set(count.get() + 1));
        // SAFETY: The live handle belongs to this thread. The outer operation may
        // hold Rust references, so all these allocation-callback calls must refuse
        // entry before touching its parser storage or calling the allocator again.
        unsafe {
            XML_ParserFree(parser);
            assert_eq!(XML_Parse(parser, c"<bad/>".as_ptr(), 6, 1), 0);
            assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
            assert!(XML_GetBuffer(parser, 1).is_null());
            assert_eq!(XML_SetBase(parser, c"wrong".as_ptr()), 0);
            assert!(XML_MemMalloc(parser, 1).is_null());
            XML_SetUserData(parser, ptr::null_mut());
        }
    });
}

unsafe extern "C" fn custom_malloc(size: usize) -> *mut c_void {
    attempt_reentry();
    // SAFETY: The C allocator accepts the requested size and returns NULL on failure.
    unsafe { malloc(size) }
}

unsafe extern "C" fn custom_realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    attempt_reentry();
    // SAFETY: Oriole returns this suite's original allocation and a positive size.
    unsafe { realloc(pointer, size) }
}

unsafe extern "C" fn custom_free(pointer: *mut c_void) {
    attempt_reentry();
    // SAFETY: Oriole returns this suite's original live allocation once.
    unsafe { free(pointer) };
}

unsafe extern "C" fn model(data: *mut c_void, _: *const c_char, content: *mut XML_Content) {
    // SAFETY: User data is the owning parser; ownership of this model was transferred
    // to this callback, which returns it through the corresponding public API.
    unsafe { XML_FreeContentModel(data.cast(), content) };
}

unsafe extern "C" fn convert(_: *mut c_void, _: *const c_char) -> i32 {
    0xe9
}

unsafe extern "C" fn unknown(_: *mut c_void, _: *const c_char, encoding: *mut XML_Encoding) -> i32 {
    // SAFETY: Expat provides a writable encoding descriptor for this callback.
    unsafe {
        (*encoding).map = std::array::from_fn(|byte| byte as i32);
        (*encoding).map[0x80] = -2;
        (*encoding).convert = Some(convert);
    }
    1
}

#[test]
fn custom_parser_never_uses_global_storage_and_blocks_allocator_reentry() {
    let suite = XML_Memory_Handling_Suite {
        malloc_fcn: Some(custom_malloc),
        realloc_fcn: Some(custom_realloc),
        free_fcn: Some(custom_free),
    };
    ESCAPES.with(|value| value.set(0));
    REENTRIES.with(|value| value.set(0));
    OBSERVE.with(|enabled| enabled.set(true));
    // SAFETY: Inputs are valid static C strings, the suite forwards to libc, and
    // handles are used serially on this thread and freed exactly once.
    unsafe {
        let parser = XML_ParserCreate_MM(c"UTF-8".as_ptr(), &suite, c"|".as_ptr());
        assert!(!parser.is_null());
        CALLBACK_PARSER.with(|value| value.set(parser));
        XML_SetUserData(parser, parser.cast());
        XML_SetElementDeclHandler(parser, Some(model));
        assert_eq!(XML_SetBase(parser, c"urn:base".as_ptr()), 1);
        let xml = c"<!DOCTYPE r [<!ELEMENT r (#PCDATA|n)*><!ELEMENT n EMPTY><!ENTITY e 'text'><!ATTLIST r a CDATA 'default'>]><r xmlns:p='urn:p' p:a='b'>&e;<n/></r>";
        let bytes = xml.to_bytes();
        let buffer = XML_GetBuffer(parser, bytes.len() as i32);
        assert!(!buffer.is_null());
        ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast(), bytes.len());
        assert_eq!(XML_ParseBuffer(parser, bytes.len() as i32, 1), 1);
        assert_eq!(XML_GetUserData(parser), parser.cast());
        let child = XML_ExternalEntityParserCreate(parser, c"".as_ptr(), ptr::null());
        assert!(!child.is_null());
        assert_eq!(XML_Parse(child, c"<n>&e;</n>".as_ptr(), 10, 1), 1);
        XML_ParserFree(child);
        assert_eq!(XML_ParserReset(parser, c"UTF-8".as_ptr()), 1);
        assert_eq!(XML_Parse(parser, c"<again/>".as_ptr(), 8, 1), 1);
        assert_eq!(XML_ParserReset(parser, c"multibyte".as_ptr()), 1);
        XML_SetUnknownEncodingHandler(parser, Some(unknown), ptr::null_mut());
        let encoded = b"<\x80\0 a='\x80\0'>\x80\0</\x80\0>";
        for (index, byte) in encoded.iter().enumerate() {
            assert_eq!(
                XML_Parse(
                    parser,
                    ptr::from_ref(byte).cast(),
                    1,
                    i32::from(index + 1 == encoded.len())
                ),
                1
            );
        }
        let child = XML_ExternalEntityParserCreate(parser, c"".as_ptr(), c"multibyte".as_ptr());
        assert!(!child.is_null());
        assert_eq!(XML_Parse(child, [0x80_u8, 0].as_ptr().cast(), 2, 1), 1);
        XML_ParserFree(child);
        XML_ParserFree(parser);
        CALLBACK_PARSER.with(|value| value.set(ptr::null_mut()));
    }
    OBSERVE.with(|enabled| enabled.set(false));
    assert_eq!(ESCAPES.with(Cell::get), 0);
    assert!(REENTRIES.with(Cell::get) > 20);
}
