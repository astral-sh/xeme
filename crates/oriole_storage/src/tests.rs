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
    static EXPECT_CALLBACK_GUARD: Cell<bool> = const { Cell::new(false) };
}
fn check_global_guard() {
    if EXPECT_CALLBACK_GUARD.try_with(Cell::get).unwrap_or(false) {
        assert!(
            in_allocator_callback(),
            "global allocator must guard C reentry"
        );
    }
}
struct TrackingGlobal;
fn observe_global() {
    check_global_guard();
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
        check_global_guard();
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

#[test]
fn tracking_scopes_nest_restore_and_exclude_the_tracker_itself() {
    let alloc = allocator(usize::MAX);
    let first = AllocationTracker::try_new_in(alloc).unwrap();
    let second = with_tracking(&first, || AllocationTracker::try_new_in(alloc)).unwrap();
    assert_eq!(first.live_bytes(), 0);
    assert_eq!(second.live_bytes(), 0);
    assert_eq!(first.strong_count(), 1);
    let (outer, inner, restored, untracked) = with_tracking(&first, || {
        let outer = String::try_from_str_in("outer", alloc).unwrap();
        let inner = with_tracking(&second, || String::try_from_str_in("inner", alloc)).unwrap();
        let restored = String::try_from_str_in("restored", alloc).unwrap();
        let untracked = without_tracking(|| String::try_from_str_in("untracked", alloc)).unwrap();
        (outer, inner, restored, untracked)
    });
    assert_eq!(first.strong_count(), 3);
    assert_eq!(second.strong_count(), 2);
    assert!(first.live_bytes() > second.live_bytes());
    drop((outer, restored));
    assert_eq!(first.live_bytes(), 0);
    drop(inner);
    assert_eq!(second.live_bytes(), 0);
    drop((first, second, untracked));
    assert_eq!(LIVE.with(Cell::get), 0);
}

#[test]
fn tracking_scope_restores_after_unwinding_and_leaves_bare_system_untracked() {
    let first = AllocationTracker::try_new_in(Allocator::System).unwrap();
    let second = AllocationTracker::try_new_in(Allocator::System).unwrap();
    with_tracking(&first, || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_tracking(&second, || panic!("fixture unwind"));
        }));
        assert!(result.is_err());
        let tracked = String::try_from_str_in("tracked", Allocator::System.trackable()).unwrap();
        let bare = String::try_from_str_in("bare", Allocator::System).unwrap();
        assert_eq!(first.strong_count(), 2);
        assert_eq!(second.strong_count(), 1);
        drop((tracked, bare));
    });
    let outside = String::try_from_str_in("outside", Allocator::System.trackable()).unwrap();
    assert_eq!(first.live_bytes(), 0);
    assert_eq!(second.live_bytes(), 0);
    drop(outside);
}

#[test]
fn tracking_headers_own_lifetimes_and_realloc_keeps_the_original_family() {
    let alloc = allocator(usize::MAX);
    let first = AllocationTracker::try_new_in(alloc).unwrap();
    let second = AllocationTracker::try_new_in(alloc).unwrap();
    let mut value = with_tracking(&first, || String::try_from_str_in("start", alloc)).unwrap();
    let initial = first.live_bytes();
    with_tracking(&second, || value.try_reserve(8192)).unwrap();
    assert!(first.live_bytes() > initial);
    assert_eq!(second.live_bytes(), 0);
    assert_eq!(first.strong_count(), 2);
    drop((first, second));
    // The buffer still owns its original tracker, including after root teardown.
    value.try_push_str(" still live").unwrap();
    assert_eq!(value, "start still live");
    drop(value);
    assert_eq!(LIVE.with(Cell::get), 0);
}

#[test]
fn tracking_failures_roll_back_reservations_and_preserve_old_blocks() {
    let alloc = allocator(usize::MAX);
    let tracker = AllocationTracker::try_new_in(alloc).unwrap();
    let mut value = with_tracking(&tracker, || String::try_from_str_in("prefix", alloc)).unwrap();
    let initial = tracker.live_bytes();
    let owners = tracker.strong_count();
    FAIL_AT.with(|fail| fail.set(CALLS.with(Cell::get) + 1));
    assert_eq!(value.try_reserve(4096), Err(AllocError::OutOfMemory));
    assert_eq!(value, "prefix");
    assert_eq!(tracker.live_bytes(), initial);
    assert_eq!(tracker.strong_count(), owners);
    FAIL_AT.with(|fail| fail.set(CALLS.with(Cell::get) + 1));
    assert!(with_tracking(&tracker, || String::try_from_str_in("failure", alloc)).is_err());
    assert_eq!(tracker.live_bytes(), initial);
    assert_eq!(tracker.strong_count(), owners);
    FAIL_AT.with(|fail| fail.set(usize::MAX));
    drop(value);
    assert_eq!(tracker.live_bytes(), 0);
    drop(tracker);
    assert_eq!(LIVE.with(Cell::get), 0);
    assert!(AllocationTracker::try_new_in(allocator(1)).is_err());
    assert_eq!(LIVE.with(Cell::get), 0);
}

#[test]
fn allocation_amplification_limits_are_shared_and_reset_preserves_live_bytes() {
    let alloc = allocator(usize::MAX);
    let root = AllocationTracker::try_new_in(alloc).unwrap();
    let child = root.clone();
    root.set_activation_threshold(0);
    assert!(root.set_maximum_amplification(1.0));
    assert!(with_tracking(&root, || String::try_from_str_in("zero input", alloc)).is_err());
    assert_eq!(root.live_bytes(), 0);
    assert!(root.add_direct_bytes(1024));
    let parent = with_tracking(&root, || String::try_from_str_in("parent", alloc)).unwrap();
    let initial = root.live_bytes();
    assert!(initial > 0);
    let mut child_data = with_tracking(&child, || String::try_from_str_in("child", alloc)).unwrap();
    assert!(root.live_bytes() > initial);
    let calls = CALLS.with(Cell::get);
    assert_eq!(child_data.try_reserve(2048), Err(AllocError::OutOfMemory));
    assert_eq!(
        CALLS.with(Cell::get),
        calls,
        "limit must reject before malloc"
    );
    let live = root.live_bytes();
    root.reset_direct_bytes();
    assert_eq!(root.live_bytes(), live);
    assert_eq!(root.direct_bytes(), 0);
    assert_eq!(child_data.try_reserve(100), Err(AllocError::OutOfMemory));
    assert!(root.add_direct_bytes(4096));
    child_data.try_reserve(100).unwrap();
    drop((parent, child_data));
    assert_eq!(root.live_bytes(), 0);
    assert!(root.peak_bytes() >= live);
    drop((root, child));
    assert_eq!(LIVE.with(Cell::get), 0);
}

#[test]
fn allocation_tracker_validates_factors_and_input_overflow() {
    let tracker = AllocationTracker::try_new_in(Allocator::System).unwrap();
    for invalid in [f32::NAN, f32::NEG_INFINITY, -1.0, 0.0, 0.999] {
        assert!(!tracker.set_maximum_amplification(invalid));
    }
    assert!(tracker.set_maximum_amplification(f32::INFINITY));
    tracker.set_activation_threshold(0);
    let value = with_tracking(&tracker, || {
        String::try_from_str_in("unlimited", Allocator::System.trackable())
    })
    .unwrap();
    assert!(tracker.live_bytes() > 0);
    assert!(tracker.add_direct_bytes(u64::MAX));
    assert!(!tracker.add_direct_bytes(1));
    assert_eq!(tracker.direct_bytes(), u64::MAX);
    drop(value);
    assert_eq!(tracker.live_bytes(), 0);
}

#[test]
fn tracked_c_blocks_resize_and_free_outside_the_scope() {
    let alloc = allocator(usize::MAX);
    let tracker = AllocationTracker::try_new_in(alloc).unwrap();
    // SAFETY: Each pointer comes from the same allocator's tracked C API. The
    // failed realloc leaves the old pointer valid; successful calls replace it.
    unsafe {
        let block = with_tracking(&tracker, || alloc.tracked_malloc(32)).cast::<u8>();
        assert!(!block.is_null());
        block.write_bytes(0xE1, 32);
        let initial = tracker.live_bytes();
        FAIL_AT.with(|fail| fail.set(CALLS.with(Cell::get) + 1));
        assert!(alloc.tracked_realloc(block.cast(), 4096).is_null());
        assert_eq!(tracker.live_bytes(), initial);
        FAIL_AT.with(|fail| fail.set(usize::MAX));
        let block = alloc.tracked_realloc(block.cast(), 4096).cast::<u8>();
        assert!(!block.is_null());
        assert!(tracker.live_bytes() > initial);
        for index in 0..32 {
            assert_eq!(*block.add(index), 0xE1);
        }
        let block = alloc.tracked_realloc(block.cast(), 8).cast::<u8>();
        assert!(!block.is_null());
        assert!(tracker.live_bytes() < initial);
        for index in 0..8 {
            assert_eq!(*block.add(index), 0xE1);
        }
        drop(tracker);
        alloc.tracked_free(block.cast());
        alloc.tracked_free(ptr::null_mut());
        let zero = alloc.tracked_realloc(ptr::null_mut(), 0);
        assert!(!zero.is_null());
        alloc.tracked_free(zero);
    }
    assert_eq!(LIVE.with(Cell::get), 0);
}

#[test]
fn forced_moving_realloc_preserves_tracking_owner_and_zero_sized_headers() {
    let _ = allocator(usize::MAX);
    // SAFETY: The fixture obeys the suite contract and forces disjoint reallocs.
    let alloc = unsafe {
        Allocator::from_callbacks(MemorySuite {
            malloc: Some(moving_malloc),
            realloc: Some(moving_realloc),
            free: Some(moving_free),
        })
        .unwrap()
    };
    let tracker = AllocationTracker::try_new_in(alloc).unwrap();
    let small = Layout::from_size_align(1, 256).unwrap();
    let large = Layout::from_size_align(1001, 256).unwrap();
    let zero = Layout::from_size_align(0, 256).unwrap();
    for initial_shift in [16, 80] {
        MOVING_SHIFT.with(|shift| shift.set(initial_shift));
        let block = with_tracking(&tracker, || alloc.allocate(small)).unwrap();
        assert_eq!(tracker.strong_count(), 2);
        // SAFETY: Every operation uses the allocation's preceding valid layout.
        unsafe {
            block.as_ptr().cast::<u8>().write(0xC7);
            let next = alloc.grow_zeroed(block.cast(), small, large).unwrap();
            assert_eq!(*next.as_ptr().cast::<u8>(), 0xC7);
            for index in 1..large.size() {
                assert_eq!(*next.as_ptr().cast::<u8>().add(index), 0);
            }
            assert_eq!(tracker.strong_count(), 2);
            let last = alloc.shrink(next.cast(), large, zero).unwrap();
            assert_eq!(tracker.strong_count(), 2);
            alloc.deallocate(last.cast(), zero);
        }
        assert_eq!(tracker.strong_count(), 1);
        assert_eq!(tracker.live_bytes(), 0);
    }
    drop(tracker);
    assert_eq!(LIVE.with(Cell::get), 0);
}

#[test]
fn tracked_global_calls_guard_against_c_api_reentry() {
    let alloc = Allocator::System.trackable();
    EXPECT_CALLBACK_GUARD.with(|expected| expected.set(true));
    let tracker = AllocationTracker::try_new_in(alloc).unwrap();
    let mut value = with_tracking(&tracker, || String::try_from_str_in("value", alloc)).unwrap();
    value.try_reserve(8192).unwrap();
    drop(value);
    drop(tracker);
    EXPECT_CALLBACK_GUARD.with(|expected| expected.set(false));
    assert!(!in_allocator_callback());
}

#[test]
fn concurrent_tracking_reservations_enforce_the_shared_family_limit() {
    let alloc = Allocator::System.trackable();
    let tracker = AllocationTracker::try_new_in(alloc).unwrap();
    tracker.set_activation_threshold(0);
    assert!(tracker.set_maximum_amplification(1.0));
    assert!(tracker.add_direct_bytes(3500));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads: std::vec::Vec<_> = (0..8)
        .map(|_| {
            let tracker = tracker.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                with_tracking(&tracker, || String::try_with_capacity_in(1000, alloc)).ok()
            })
        })
        .collect();
    let allocations: std::vec::Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(
        allocations.iter().filter(|value| value.is_some()).count(),
        3
    );
    assert!((3000..=3500).contains(&tracker.live_bytes()));
    drop(allocations);
    assert_eq!(tracker.live_bytes(), 0);
    assert_eq!(tracker.strong_count(), 1);
}

#[test]
fn custom_tracking_never_uses_the_rust_global_allocator() {
    let alloc = allocator(usize::MAX);
    ESCAPED.with(|count| count.set(0));
    TRACK_GLOBAL.with(|enabled| enabled.set(true));
    let tracker = AllocationTracker::try_new_in(alloc).unwrap();
    let mut value = with_tracking(&tracker, || String::try_from_str_in("value", alloc)).unwrap();
    value.try_reserve(8192).unwrap();
    // SAFETY: The direct tracked block is freed through the same allocator once.
    unsafe {
        let block = with_tracking(&tracker, || alloc.tracked_malloc(100));
        assert!(!block.is_null());
        alloc.tracked_free(block);
    }
    drop(value);
    assert_eq!(tracker.live_bytes(), 0);
    drop(tracker);
    TRACK_GLOBAL.with(|enabled| enabled.set(false));
    assert_eq!(ESCAPED.with(Cell::get), 0);
    assert_eq!(LIVE.with(Cell::get), 0);
}
