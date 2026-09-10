//! Allocation-failure and allocator-routing checks for the complete safe core.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr;

use oriole::{Config, Error, ErrorKind, EventKind, Parser};
use oriole_storage::{Allocator, MemorySuite};

thread_local! {
    static TRACK_GLOBAL: Cell<bool> = const { Cell::new(false) };
    static GLOBAL_CALLS: Cell<usize> = const { Cell::new(0) };
    static CALLS: Cell<usize> = const { Cell::new(0) };
    static FAIL_AT: Cell<usize> = const { Cell::new(0) };
    static LIVE: Cell<usize> = const { Cell::new(0) };
}

struct CheckedGlobal;
// SAFETY: Allocation and deallocation always delegate to the same System allocator;
// the thread-local counters do not allocate or modify the returned allocations.
unsafe impl GlobalAlloc for CheckedGlobal {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_GLOBAL.get() {
            GLOBAL_CALLS.set(GLOBAL_CALLS.get() + 1);
        }
        // SAFETY: The GlobalAlloc caller supplies a valid nonzero layout.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The pointer and layout came from this allocator's System delegation.
        unsafe {
            System.dealloc(pointer, layout);
        }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK_GLOBAL.get() {
            GLOBAL_CALLS.set(GLOBAL_CALLS.get() + 1);
        }
        // SAFETY: The caller supplies a live System allocation and valid new size.
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static GLOBAL: CheckedGlobal = CheckedGlobal;

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void;
    fn free(pointer: *mut c_void);
}

fn fail_allocation() -> bool {
    CALLS.set(CALLS.get() + 1);
    FAIL_AT.get() != 0 && CALLS.get() >= FAIL_AT.get()
}
unsafe extern "C" fn checked_malloc(size: usize) -> *mut c_void {
    if fail_allocation() {
        return ptr::null_mut();
    }
    // SAFETY: libc malloc accepts any allocation size and reports failure with NULL.
    let pointer = unsafe { malloc(size) };
    if !pointer.is_null() {
        LIVE.set(LIVE.get() + 1);
    }
    pointer
}
unsafe extern "C" fn checked_realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    if fail_allocation() {
        return ptr::null_mut();
    }
    // SAFETY: The storage allocator passes a live block from this allocation suite.
    let result = unsafe { realloc(pointer, size) };
    if pointer.is_null() && !result.is_null() {
        LIVE.set(LIVE.get() + 1);
    }
    result
}
unsafe extern "C" fn checked_free(pointer: *mut c_void) {
    if !pointer.is_null() {
        LIVE.set(LIVE.get() - 1);
    }
    // SAFETY: The storage allocator returns the original suite pointer exactly once.
    unsafe {
        free(pointer);
    }
}

fn workload(allocator: Allocator) -> Result<(), Error> {
    let mut parser = Parser::try_new_with_encoding_in(
        Config {
            namespace_separator: Some('|'),
            namespace_triplets: true,
            ..Config::default()
        },
        None,
        allocator,
    )?;
    parser.feed(b"<!DOCTYPE r [<!ENTITY internal '<p:n/>'><!ENTITY external SYSTEM 'child'><!ATTLIST r a NMTOKENS ' a  b '>]><r xmlns:p='urn:p' p:attr='v'>&internal;&external;<!--c--><![CDATA[x]]></r>", true)?;
    while let Some(event) = parser.next_event()? {
        if let EventKind::ExternalEntityReference { context, .. } = event.kind {
            let mut child = parser.external_child_with_encoding(context.as_deref(), None)?;
            child.feed(b"<?xml encoding='UTF-8'?><p:x a='value'/>text", true)?;
            while child.next_event()?.is_some() {}
        }
    }
    let mut parser =
        Parser::try_new_with_encoding_in(Config::default(), Some("custom"), allocator)?;
    parser.feed(b"<r>\x80</r>", true)?;
    match parser.next_event() {
        Err(error) if error.kind == ErrorKind::UnknownEncoding => {
            let mut map = std::array::from_fn(|index| index as i32);
            map[128] = 0x20ac;
            parser.set_encoding_map("custom", map)?;
        }
        Err(error) => return Err(error),
        _ => panic!("custom encoding unexpectedly resolved without its map"),
    }
    while parser.next_event()?.is_some() {}
    Ok(())
}

#[test]
fn every_allocation_can_fail_and_all_memory_uses_the_selected_suite() {
    // SAFETY: The callbacks use a complete libc-backed suite with failure injection;
    // every pointer remains valid until realloc succeeds or its matching free call.
    let allocator = unsafe {
        Allocator::from_callbacks(MemorySuite {
            malloc: Some(checked_malloc),
            realloc: Some(checked_realloc),
            free: Some(checked_free),
        })
        .unwrap()
    };
    FAIL_AT.set(0);
    CALLS.set(0);
    LIVE.set(0);
    GLOBAL_CALLS.set(0);
    TRACK_GLOBAL.set(true);
    let result = workload(allocator);
    TRACK_GLOBAL.set(false);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(LIVE.get(), 0);
    assert_eq!(
        GLOBAL_CALLS.get(),
        0,
        "parser bypassed the selected allocator"
    );
    let allocation_count = CALLS.get();
    assert!(
        allocation_count > 100,
        "workload must exercise substantial allocation"
    );
    for failure in 1..=allocation_count {
        FAIL_AT.set(failure);
        CALLS.set(0);
        GLOBAL_CALLS.set(0);
        TRACK_GLOBAL.set(true);
        let result = workload(allocator);
        TRACK_GLOBAL.set(false);
        assert_eq!(
            result.unwrap_err().kind,
            ErrorKind::NoMemory,
            "allocation {failure}"
        );
        assert_eq!(LIVE.get(), 0, "leaked allocation at failure {failure}");
        assert_eq!(
            GLOBAL_CALLS.get(),
            0,
            "global allocation at failure {failure}"
        );
    }
    FAIL_AT.set(0);
}
