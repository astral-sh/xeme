use std::cell::Cell;
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::AtomicUsize;

use super::*;

thread_local! {
    static CALLS: Cell<usize> = const { Cell::new(0) };
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static FAIL_AT: Cell<usize> = const { Cell::new(usize::MAX) };
}
unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void;
    fn free(pointer: *mut c_void);
}
fn fails() -> bool {
    assert!(in_allocator_callback());
    CALLS.with(|calls| {
        calls.set(calls.get() + 1);
        FAIL_AT.with(|fail| fail.get() == calls.get())
    })
}
unsafe extern "C" fn custom_malloc(size: usize) -> *mut c_void {
    if fails() {
        return ptr::null_mut();
    }
    // SAFETY: libc accepts the requested size and reports failures with NULL.
    let pointer = unsafe { malloc(size) };
    if !pointer.is_null() {
        LIVE.with(|live| live.set(live.get() + 1));
    }
    pointer
}
unsafe extern "C" fn custom_realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    if fails() {
        return ptr::null_mut();
    }
    // SAFETY: This fixture receives live libc allocations and positive sizes.
    let result = unsafe { realloc(pointer, size) };
    if pointer.is_null() && !result.is_null() {
        LIVE.with(|live| live.set(live.get() + 1));
    }
    result
}
unsafe extern "C" fn custom_free(pointer: *mut c_void) {
    assert!(in_allocator_callback());
    if !pointer.is_null() {
        LIVE.with(|live| live.set(live.get() - 1));
    }
    // SAFETY: The test adapter returns original libc pointers exactly once.
    unsafe { free(pointer) };
}
fn allocator(fail_at: usize) -> Allocator {
    assert_eq!(LIVE.with(Cell::get), 0);
    CALLS.with(|value| value.set(0));
    FAIL_AT.with(|value| value.set(fail_at));
    // SAFETY: Test callbacks implement libc allocation semantics and do not unwind
    // for any fixture-controlled call; their state is independent in each test thread.
    unsafe {
        Allocator::from_callbacks(MemorySuite {
            malloc: Some(custom_malloc),
            realloc: Some(custom_realloc),
            free: Some(custom_free),
        })
        .unwrap()
    }
}

#[test]
fn failure_at_every_allocation_preserves_ownership() {
    fn operation(alloc: Allocator) -> Result<(), AllocError> {
        let mut text = String::try_from_str_in("café", alloc)?;
        text.try_push_str("abcdefghijklmnop")?;
        let text = text.try_clone()?;
        let cstring = CString::try_from_string(text)?;
        assert_eq!(cstring.as_c_str().to_str().unwrap(), "caféabcdefghijklmnop");
        let mut queue = Queue::new_in(alloc);
        queue.try_push_back(cstring)?;
        let owner = Shared::try_new_in(AtomicUsize::new(0), alloc)?;
        let child = owner.clone();
        assert_eq!(owner.strong_count(), 2);
        drop(child);
        let mut map = hash_map(alloc);
        try_insert(&mut map, String::try_from_str_in("key", alloc)?, queue)?;
        Ok(())
    }
    operation(allocator(usize::MAX)).unwrap();
    let count = CALLS.with(Cell::get);
    assert!(count >= 7);
    assert_eq!(LIVE.with(Cell::get), 0);
    for fail_at in 1..=count {
        assert_eq!(operation(allocator(fail_at)), Err(AllocError::OutOfMemory));
        assert_eq!(LIVE.with(Cell::get), 0, "leaked at allocation {fail_at}");
        assert!(!in_allocator_callback());
    }
}

#[test]
fn over_aligned_reallocation_preserves_bytes_and_failure_ownership() {
    let alloc = allocator(usize::MAX);
    for alignment in [1, 8, 16, 64, 256, 4096] {
        let old = Layout::from_size_align(31, alignment).unwrap();
        let grown = Layout::from_size_align(8193, alignment).unwrap();
        let shrunk = Layout::from_size_align(7, alignment).unwrap();
        let block = alloc.allocate(old).unwrap();
        assert_eq!((block.as_ptr().cast::<u8>() as usize) % alignment, 0);
        // SAFETY: Operations stay within the requested live allocations and use
        // the exact preceding layout for each resize/deallocation.
        unsafe {
            block.as_ptr().cast::<u8>().write_bytes(0xA5, 31);
            FAIL_AT.with(|fail| fail.set(CALLS.with(Cell::get) + 1));
            assert!(alloc.grow(block.cast(), old, grown).is_err());
            for i in 0..31 {
                assert_eq!(*block.as_ptr().cast::<u8>().add(i), 0xA5);
            }
            FAIL_AT.with(|fail| fail.set(usize::MAX));
            let block = alloc.grow(block.cast(), old, grown).unwrap();
            assert_eq!((block.as_ptr().cast::<u8>() as usize) % alignment, 0);
            for i in 0..31 {
                assert_eq!(*block.as_ptr().cast::<u8>().add(i), 0xA5);
            }
            let block = alloc.shrink(block.cast(), grown, shrunk).unwrap();
            for i in 0..7 {
                assert_eq!(*block.as_ptr().cast::<u8>().add(i), 0xA5);
            }
            alloc.deallocate(block.cast(), shrunk);
        }
    }
    assert_eq!(LIVE.with(Cell::get), 0);
}

#[test]
fn alignment_changes_and_zero_sized_allocations_are_valid() {
    let alloc = allocator(usize::MAX);
    let first = Layout::from_size_align(20, 4096).unwrap();
    let second = Layout::from_size_align(10, 1).unwrap();
    let third = Layout::from_size_align(40, 8192).unwrap();
    // SAFETY: Resizes preserve the new-size prefix and receive original layouts.
    unsafe {
        let block = alloc.allocate(first).unwrap();
        block.as_ptr().cast::<u8>().write_bytes(0x3B, 20);
        let block = alloc.shrink(block.cast(), first, second).unwrap();
        let block = alloc.grow(block.cast(), second, third).unwrap();
        for i in 0..10 {
            assert_eq!(*block.as_ptr().cast::<u8>().add(i), 0x3B);
        }
        alloc.deallocate(block.cast(), third);
        let zero = Layout::from_size_align(0, 256).unwrap();
        let block = alloc.allocate(zero).unwrap();
        assert_eq!((block.as_ptr().cast::<u8>() as usize) % 256, 0);
        alloc.deallocate(block.cast(), zero);
    }
    assert_eq!(LIVE.with(Cell::get), 0);
}

#[test]
fn shared_owner_drops_value_once_and_can_cross_threads() {
    static DROPS: AtomicUsize = AtomicUsize::new(0);
    struct Tracked;
    impl Drop for Tracked {
        fn drop(&mut self) {
            DROPS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let value = Shared::try_new_in(Tracked, Allocator::System).unwrap();
    let other = value.clone();
    std::thread::spawn(move || drop(other)).join().unwrap();
    assert_eq!(DROPS.load(std::sync::atomic::Ordering::SeqCst), 0);
    drop(value);
    assert_eq!(DROPS.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn queue_compaction_and_utf8_edits_keep_invariants() {
    let alloc = allocator(usize::MAX);
    {
        let mut queue = Queue::new_in(alloc);
        for i in 0..100 {
            queue.try_push_back(i).unwrap();
        }
        for i in 0..70 {
            assert_eq!(queue.pop_front(), Some(i));
        }
        for i in 100..200 {
            queue.try_push_back(i).unwrap();
        }
        assert!(queue.iter().copied().eq(70..200));
        let mut text = String::try_from_str_in("éab😀cd", alloc).unwrap();
        text.drain(..2);
        text.drain(2..6);
        assert_eq!(text, "abcd");
        text.truncate(3);
        assert_eq!(text, "abc");
        assert_eq!(
            CString::try_from_string(String::try_from_str_in("a\0b", alloc).unwrap()).err(),
            Some(AllocError::InteriorNul)
        );
    }
    assert_eq!(LIVE.with(Cell::get), 0);
}

thread_local! {
    static TRACK_GLOBAL: Cell<bool> = const { Cell::new(false) };
    static ESCAPED: Cell<usize> = const { Cell::new(0) };
}
struct TrackingGlobal;
fn observe_global() {
    if TRACK_GLOBAL.try_with(Cell::get).unwrap_or(false) {
        let _ = ESCAPED.try_with(|count| count.set(count.get() + 1));
    }
}
// SAFETY: All operations forward the original allocation contract to System;
// observation uses allocation-free thread-local Cells and changes no pointers.
unsafe impl std::alloc::GlobalAlloc for TrackingGlobal {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        observe_global();
        // SAFETY: Forward the caller's valid layout unchanged.
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        observe_global();
        // SAFETY: Forward the caller's valid layout unchanged.
        unsafe { std::alloc::System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        observe_global();
        // SAFETY: Forward the live pointer, original layout, and new size unchanged.
        unsafe { std::alloc::System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
        // SAFETY: Forward the live pointer and its allocation layout unchanged.
        unsafe { std::alloc::System.dealloc(pointer, layout) };
    }
}
#[global_allocator]
static GLOBAL: TrackingGlobal = TrackingGlobal;

#[test]
fn custom_storage_never_uses_the_rust_global_allocator() {
    let alloc = allocator(usize::MAX);
    ESCAPED.with(|count| count.set(0));
    TRACK_GLOBAL.with(|enabled| enabled.set(true));
    let result = (|| -> Result<(), AllocError> {
        let mut names = hash_set(alloc);
        try_set_insert(&mut names, String::try_from_str_in("name", alloc)?)?;
        let text = try_format(alloc, format_args!("value {} {}", 42, "é"))?;
        let cstring = CString::try_from_string(text)?;
        let copy = cstring.try_clone()?;
        let mut queue = Queue::new_in(alloc);
        queue.try_push_back(copy)?;
        let shared = Shared::try_new_in(queue, alloc)?;
        let clone = shared.clone();
        assert_eq!(clone.len(), 1);
        let boxed = try_box([0_i32; 256], alloc)?;
        assert_eq!(boxed[0], 0);
        Ok(())
    })();
    TRACK_GLOBAL.with(|enabled| enabled.set(false));
    result.unwrap();
    assert_eq!(ESCAPED.with(Cell::get), 0);
    assert_eq!(LIVE.with(Cell::get), 0);
}

#[repr(C)]
struct MovingHeader {
    original: *mut c_void,
    size: usize,
}
thread_local! {
    static MOVING_SHIFT: Cell<usize> = const { Cell::new(16) };
}
unsafe fn moving_allocate(size: usize) -> *mut c_void {
    let Some(total) = size.checked_add(2 * 8192 + std::mem::size_of::<MovingHeader>()) else {
        return ptr::null_mut();
    };
    // SAFETY: libc reports allocation failure as NULL; extra space covers our
    // fixture header, alignment, and deliberate 16/80-byte displacement.
    let original = unsafe { malloc(total) }.cast::<u8>();
    if original.is_null() {
        return ptr::null_mut();
    }
    let shift = MOVING_SHIFT.with(|shift| {
        let previous = shift.get();
        shift.set(if previous == 16 { 80 } else { 16 });
        previous
    });
    // SAFETY: The allocated block covers header, padding, shift, and payload.
    // Chosen shifts preserve ordinary C malloc alignment but alter the offset
    // required by the Rust adapter for 256-byte aligned allocations.
    unsafe {
        let first = original.add(std::mem::size_of::<MovingHeader>());
        let pointer = first.add(first.align_offset(8192) + shift);
        pointer
            .sub(std::mem::size_of::<MovingHeader>())
            .cast::<MovingHeader>()
            .write(MovingHeader {
                original: original.cast(),
                size,
            });
        LIVE.with(|live| live.set(live.get() + 1));
        pointer.cast()
    }
}
unsafe extern "C" fn moving_malloc(size: usize) -> *mut c_void {
    if fails() {
        return ptr::null_mut();
    }
    // SAFETY: The fixture constructs a correctly sized malloc-compatible block.
    unsafe { moving_allocate(size) }
}
unsafe extern "C" fn moving_free(pointer: *mut c_void) {
    assert!(in_allocator_callback());
    if pointer.is_null() {
        return;
    }
    // SAFETY: Each live fixture pointer has its original libc pointer in the
    // preceding header, and the caller frees it exactly once.
    unsafe {
        let header = pointer
            .cast::<u8>()
            .sub(std::mem::size_of::<MovingHeader>())
            .cast::<MovingHeader>()
            .read();
        LIVE.with(|live| live.set(live.get() - 1));
        free(header.original);
    }
}
unsafe extern "C" fn moving_realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    if fails() {
        return ptr::null_mut();
    }
    if pointer.is_null() {
        // SAFETY: Realloc(NULL, size) has malloc semantics.
        return unsafe { moving_allocate(size) };
    }
    // SAFETY: The old fixture block stays live until the disjoint new block has
    // received its full retained prefix. Failure never touches the old pointer.
    unsafe {
        let old_size = (*pointer
            .cast::<u8>()
            .sub(std::mem::size_of::<MovingHeader>())
            .cast::<MovingHeader>())
        .size;
        let next = moving_allocate(size);
        if next.is_null() {
            return next;
        }
        ptr::copy_nonoverlapping(pointer.cast::<u8>(), next.cast::<u8>(), old_size.min(size));
        moving_free(pointer);
        next
    }
}

#[test]
fn forced_moving_realloc_changes_alignment_offsets_and_zeroes_growth() {
    let _ = allocator(usize::MAX);
    // SAFETY: The moving fixture implements the complete allocation contract;
    // each successful realloc changes the allocation and preserves its prefix.
    let alloc = unsafe {
        Allocator::from_callbacks(MemorySuite {
            malloc: Some(moving_malloc),
            realloc: Some(moving_realloc),
            free: Some(moving_free),
        })
        .unwrap()
    };
    let small = Layout::from_size_align(127, 256).unwrap();
    let large = Layout::from_size_align(1001, 256).unwrap();
    for initial_shift in [16, 80] {
        MOVING_SHIFT.with(|shift| shift.set(initial_shift));
        let block = alloc.allocate(small).unwrap();
        // SAFETY: Reads and writes remain within the relevant allocations; every
        // grow/shrink/deallocation uses the exact layout from its predecessor.
        unsafe {
            block.as_ptr().cast::<u8>().write_bytes(0xC7, small.size());
            let next = alloc.grow_zeroed(block.cast(), small, large).unwrap();
            assert_ne!(next.as_ptr().cast::<u8>(), block.as_ptr().cast::<u8>());
            assert_eq!((next.as_ptr().cast::<u8>() as usize) % 256, 0);
            for i in 0..127 {
                assert_eq!(*next.as_ptr().cast::<u8>().add(i), 0xC7);
            }
            for i in 127..1001 {
                assert_eq!(*next.as_ptr().cast::<u8>().add(i), 0);
            }
            let last = alloc.shrink(next.cast(), large, small).unwrap();
            assert_ne!(last.as_ptr().cast::<u8>(), next.as_ptr().cast::<u8>());
            for i in 0..127 {
                assert_eq!(*last.as_ptr().cast::<u8>().add(i), 0xC7);
            }
            alloc.deallocate(last.cast(), small);
        }
    }
    assert_eq!(LIVE.with(Cell::get), 0);
}
