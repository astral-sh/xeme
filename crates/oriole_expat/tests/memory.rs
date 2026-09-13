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
    static FAIL_NEXT: Cell<bool> = const { Cell::new(false) };
    static END_CALLS: Cell<usize> = const { Cell::new(0) };
}

#[test]
fn detached_end_uses_the_selected_owner_and_default_failure_stays_terminal() {
    unsafe extern "C" fn suspend_inner(
        parser: *mut c_void,
        name: *const c_char,
        _: *const *const c_char,
    ) {
        // SAFETY: Parser-as-handler-arg supplies the live parser and callback name.
        unsafe {
            if std::ffi::CStr::from_ptr(name).to_bytes() == b"n" {
                assert_eq!(XML_StopParser(parser.cast(), 1), 1);
            }
        }
    }
    unsafe extern "C" fn count_end(_: *mut c_void, _: *const c_char) {
        END_CALLS.set(END_CALLS.get() + 1);
    }
    unsafe extern "C" fn reject_default(_: *mut c_void, _: *const c_char, _: i32) {
        panic!("the selected raw-copy allocation must fail before the callback");
    }
    let suite = XML_Memory_Handling_Suite {
        malloc_fcn: Some(custom_malloc),
        realloc_fcn: Some(custom_realloc),
        free_fcn: Some(custom_free),
    };
    for default_failure in [false, true] {
        ESCAPES.set(0);
        END_CALLS.set(0);
        OBSERVE.set(true);
        // SAFETY: The complete selected suite and static input outlive the parser;
        // callback reentry is deliberately checked by the existing allocator.
        unsafe {
            let parser = XML_ParserCreate_MM(ptr::null(), &suite, ptr::null());
            assert!(!parser.is_null());
            CALLBACK_PARSER.set(parser);
            XML_UseParserAsHandlerArg(parser);
            XML_SetStartElementHandler(parser, Some(suspend_inner));
            // Warm the element/name path before the resumed End/default check.
            // Composed matched Ends borrow raw context and need no token-owner warmup.
            let input = c"<r><w></w><n a='v'></n></r>";
            assert_eq!(
                XML_Parse(parser, input.as_ptr(), input.to_bytes().len() as i32, 1),
                2
            );
            if default_failure {
                XML_SetDefaultHandler(parser, Some(reject_default));
            } else {
                XML_SetEndElementHandler(parser, Some(count_end));
            }
            FAIL_NEXT.set(true);
            let status = XML_ResumeParser(parser);
            if default_failure {
                assert_eq!(status, 0);
                assert_eq!(XML_GetErrorCode(parser), 1);
                assert!(!FAIL_NEXT.get());
                assert_eq!(END_CALLS.get(), 0);
                assert_eq!(XML_Parse(parser, ptr::null(), 0, 1), 0);
                assert_eq!(XML_GetErrorCode(parser), 1);
            } else {
                assert_eq!(status, 1);
                assert_eq!(END_CALLS.get(), 2);
                assert!(FAIL_NEXT.get(), "eligible End dispatch allocated");
            }
            FAIL_NEXT.set(false);
            CALLBACK_PARSER.set(ptr::null_mut());
            XML_ParserFree(parser);
        }
        OBSERVE.set(false);
        assert_eq!(ESCAPES.get(), 0);
    }
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
            assert_eq!(XML_SetEncoding(parser, c"wrong".as_ptr()), 0);
            assert_eq!(XML_SetHashSalt(parser, 1), 0);
            assert_eq!(
                XML_SetHashSalt16Bytes(parser, b"0123456789abcdef".as_ptr()),
                0
            );
            assert!(XML_MemMalloc(parser, 1).is_null());
            XML_SetUserData(parser, ptr::null_mut());
        }
    });
}

unsafe extern "C" fn custom_malloc(size: usize) -> *mut c_void {
    attempt_reentry();
    if FAIL_NEXT.replace(false) {
        return ptr::null_mut();
    }
    // SAFETY: The C allocator accepts the requested size and returns NULL on failure.
    unsafe { malloc(size) }
}

unsafe extern "C" fn custom_realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    attempt_reentry();
    if FAIL_NEXT.replace(false) {
        return ptr::null_mut();
    }
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

unsafe extern "C" fn external_value(
    parser: XML_Parser,
    context: *const c_char,
    _: *const c_char,
    system: *const c_char,
    _: *const c_char,
) -> i32 {
    // SAFETY: The callback receives live handles and identifiers; each child is
    // used serially and freed exactly once. Static inputs outlive every parse.
    unsafe {
        let value = std::ffi::CStr::from_ptr(system).to_bytes() == b"p";
        let child = XML_ExternalEntityParserCreate(parser, context, ptr::null());
        assert!(!child.is_null());
        let input: &[u8] = if value {
            b"\"&#13;ok\""
        } else {
            b"<!ENTITY % p SYSTEM 'p'><!ENTITY e 'L%p;R'>"
        };
        let status = XML_Parse(child, input.as_ptr().cast(), input.len() as i32, 1);
        XML_ParserFree(child);
        status
    }
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
        FAIL_NEXT.set(true);
        assert_eq!(XML_SetHashSalt(parser, 1), 0);
        assert!(!FAIL_NEXT.get());
        assert_eq!(XML_GetErrorCode(parser), 0);
        assert_eq!(
            XML_SetHashSalt16Bytes(parser, b"0123456789abcdef".as_ptr()),
            1
        );
        XML_SetElementDeclHandler(parser, Some(model));
        assert_eq!(XML_SetBase(parser, c"urn:base".as_ptr()), 1);
        let xml = c"<!DOCTYPE r [<!ELEMENT r (#PCDATA|n)*><!ELEMENT n EMPTY><!ENTITY e 'text'><!ATTLIST r a CDATA 'default'>]><r xmlns:p='urn:p' p:a='b'>&e;<n/></r>";
        let bytes = xml.to_bytes();
        let buffer = XML_GetBuffer(parser, bytes.len() as i32);
        assert!(!buffer.is_null());
        ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast(), bytes.len());
        assert_eq!(XML_ParseBuffer(parser, bytes.len() as i32, 1), 1);
        assert_eq!(XML_GetUserData(parser), parser.cast());
        FAIL_NEXT.set(true);
        assert_eq!(XML_SetEncoding(parser, c"finished-metadata".as_ptr()), 0);
        assert!(!FAIL_NEXT.get());
        assert_eq!(XML_GetErrorCode(parser), 0);
        assert_eq!(XML_SetEncoding(parser, c"finished-metadata".as_ptr()), 1);
        assert_eq!(XML_SetEncoding(parser, ptr::null()), 1);
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
        assert_eq!(XML_ParserReset(parser, c"UTF-8".as_ptr()), 1);
        XML_SetExternalEntityRefHandler(parser, Some(external_value));
        assert_eq!(XML_SetParamEntityParsing(parser, 2), 1);
        let input = b"<!DOCTYPE r SYSTEM 'd'><r>&e;</r>";
        assert_eq!(
            XML_Parse(parser, input.as_ptr().cast(), input.len() as i32, 1),
            1
        );
        XML_ParserFree(parser);
        CALLBACK_PARSER.with(|value| value.set(ptr::null_mut()));
    }
    OBSERVE.with(|enabled| enabled.set(false));
    assert_eq!(ESCAPES.with(Cell::get), 0);
    assert!(REENTRIES.with(Cell::get) > 20);
}

#[test]
fn nonnamespace_children_and_reset_keep_selected_storage() {
    let suite = XML_Memory_Handling_Suite {
        malloc_fcn: Some(custom_malloc),
        realloc_fcn: Some(custom_realloc),
        free_fcn: Some(custom_free),
    };
    ESCAPES.set(0);
    REENTRIES.set(0);
    OBSERVE.set(true);
    // SAFETY: The suite delegates to libc. Each handle is used serially and freed
    // once; children own their context after reset and destruction of the parent.
    unsafe {
        let parser = XML_ParserCreate_MM(ptr::null(), &suite, ptr::null());
        assert!(!parser.is_null());
        CALLBACK_PARSER.set(parser);
        assert_eq!(XML_SetHashSalt(parser, 12345), 1);
        let child = XML_ExternalEntityParserCreate(parser, c"".as_ptr(), ptr::null());
        assert!(!child.is_null());
        FAIL_NEXT.set(true);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
        assert!(!FAIL_NEXT.get());
        assert_eq!(XML_Parse(parser, c"<p:r/>".as_ptr(), 6, 1), 1);
        assert_eq!(XML_ParserReset(parser, ptr::null()), 1);
        assert_eq!(XML_Parse(parser, c"<xml:r/>".as_ptr(), 8, 1), 1);
        let explicit =
            XML_ExternalEntityParserCreate(parser, c"xml=urn:custom".as_ptr(), ptr::null());
        assert!(!explicit.is_null());
        XML_ParserFree(parser);
        CALLBACK_PARSER.set(ptr::null_mut());
        assert_eq!(XML_Parse(child, c"<xml:r/>".as_ptr(), 8, 1), 1);
        assert_eq!(XML_Parse(explicit, c"<xml:r/>".as_ptr(), 8, 1), 1);
        XML_ParserFree(child);
        XML_ParserFree(explicit);
    }
    OBSERVE.set(false);
    assert_eq!(ESCAPES.get(), 0);
    assert!(REENTRIES.get() > 0);
}
