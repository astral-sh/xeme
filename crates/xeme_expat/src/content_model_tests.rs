use super::*;
use std::cell::Cell;

thread_local! {
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    // SAFETY: The system C allocator implements the selected suite's contract.
    let pointer = unsafe { Allocator::System.malloc(size) };
    if !pointer.is_null() {
        LIVE.set(LIVE.get() + 1);
        ALLOCATIONS.set(ALLOCATIONS.get() + 1);
    }
    pointer
}

unsafe extern "C" fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    // SAFETY: The pointer is null or a live allocation from this suite.
    let result = unsafe { Allocator::System.realloc(pointer, size) };
    if pointer.is_null() && !result.is_null() {
        LIVE.set(LIVE.get() + 1);
        ALLOCATIONS.set(ALLOCATIONS.get() + 1);
    }
    result
}

unsafe extern "C" fn free(pointer: *mut c_void) {
    if !pointer.is_null() {
        LIVE.set(LIVE.get() - 1);
    }
    // SAFETY: The pointer is null or an allocation owned by this suite.
    unsafe { Allocator::System.free(pointer) };
}

struct ModelState {
    retain: bool,
    called: bool,
    model: *mut XML_Content,
}

unsafe extern "C" fn declaration(data: *mut c_void, _: *const c_char, model: *mut XML_Content) {
    // SAFETY: Callback state and the model are live. The reference ends before
    // ownership is returned through XML_FreeContentModel or retained by the test.
    unsafe {
        let state = data.cast::<ModelState>();
        let view = &mut *model;
        assert_eq!(view.kind, 6);
        assert_eq!(view.numchildren, 2);
        let reborrowed = ptr::from_mut(view);
        (*state).called = true;
        if (*state).retain {
            (*state).model = reborrowed;
        } else {
            XML_FreeContentModel(ptr::null_mut(), reborrowed);
        }
    }
}

#[test]
fn content_model_reborrowed_callback_and_retained_owners() {
    for custom in [false, true] {
        for retain in [false, true] {
            LIVE.set(0);
            ALLOCATIONS.set(0);
            let suite = XML_Memory_Handling_Suite {
                malloc_fcn: Some(malloc),
                realloc_fcn: Some(realloc),
                free_fcn: Some(free),
            };
            let mut state = ModelState {
                retain,
                called: false,
                model: ptr::null_mut(),
            };
            // SAFETY: Inputs, callbacks, and the complete suite remain valid for
            // each parser and model. Each transferred model is freed exactly once.
            unsafe {
                let parser = if custom {
                    XML_ParserCreate_MM(ptr::null(), &suite, ptr::null())
                } else {
                    XML_ParserCreate(ptr::null())
                };
                assert!(!parser.is_null());
                let tracker = Shared::clone(&(*parser).tracker);
                XML_SetUserData(parser, ptr::from_mut(&mut state).cast());
                XML_SetElementDeclHandler(parser, Some(declaration));
                let input = b"<!DOCTYPE r [<!ELEMENT r (a,b)>]><r/>";
                assert_eq!(
                    XML_Parse(parser, input.as_ptr().cast(), input.len() as c_int, 1),
                    OK
                );
                assert!(state.called);
                XML_ParserFree(parser);
                if retain {
                    assert!(tracker.live_bytes() > 0);
                    let view = &mut *state.model;
                    assert_eq!(view.numchildren, 2);
                    XML_FreeContentModel(ptr::null_mut(), ptr::from_mut(view));
                }
                assert_eq!(tracker.live_bytes(), 0);
                drop(tracker);
                XML_FreeContentModel(ptr::null_mut(), ptr::null_mut());
            }
            assert_eq!(LIVE.get(), 0);
            assert_eq!(ALLOCATIONS.get() > 0, custom);
        }
    }
}
