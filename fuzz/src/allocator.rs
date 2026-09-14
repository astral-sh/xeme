//! Fault-injecting C allocator shared by parser fuzz targets.

use std::cell::Cell;
use std::ffi::c_void;
use std::ptr;

use xeme_expat::XML_Memory_Handling_Suite;

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void;
    fn free(pointer: *mut c_void);
}

thread_local! {
    pub static CALLS: Cell<usize> = const { Cell::new(0) };
    pub static FAIL_AT: Cell<usize> = const { Cell::new(0) };
    pub static LIVE: Cell<usize> = const { Cell::new(0) };
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

/// C callbacks with per-thread allocation counts and a selected failure ordinal.
pub const SUITE: XML_Memory_Handling_Suite = XML_Memory_Handling_Suite {
    malloc_fcn: Some(allocate),
    realloc_fcn: Some(reallocate),
    free_fcn: Some(deallocate),
};
