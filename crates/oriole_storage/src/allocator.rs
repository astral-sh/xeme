//! Runtime allocation with explicit ownership and callback reentry detection.

use std::cell::Cell;
use std::ffi::c_void;
use std::mem::{align_of, size_of};
use std::ptr::{self, NonNull};

use allocator_api2::alloc::{AllocError as ApiError, Allocator as ApiAllocator, Global, Layout};

use crate::tracking::{Charge, current_tracker};
use crate::{AllocError, AllocationTracker, Shared};

/// A copied Expat-compatible allocation suite. Functions remain valid for the
/// lifetime of every allocation and may not unwind across the C boundary.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemorySuite {
    pub malloc: Option<unsafe extern "C" fn(usize) -> *mut c_void>,
    pub realloc: Option<unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void>,
    pub free: Option<unsafe extern "C" fn(*mut c_void)>,
}

impl std::fmt::Debug for MemorySuite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemorySuite").finish_non_exhaustive()
    }
}

/// An opaque, validated C allocation suite.
#[derive(Clone, Copy, Debug)]
pub struct CustomAllocator {
    suite: MemorySuite,
}

/// An allocator copied into every container that owns its allocations.
#[derive(Clone, Copy, Debug, Default)]
pub enum Allocator {
    /// The process's selected Rust global allocator.
    #[default]
    System,
    /// Rust global allocations with metadata for per-family tracking.
    TrackedSystem,
    /// Construct with [`Allocator::from_callbacks`].
    Custom(CustomAllocator),
}

thread_local! {
    static CALLBACK_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Whether this thread is executing a foreign allocation callback.
///
/// C entry points must reject parser reentry during this interval before creating
/// references to parser state. Allocator callbacks can execute while a container
/// has an exclusive Rust borrow.
#[must_use]
pub fn in_allocator_callback() -> bool {
    CALLBACK_DEPTH.with(|depth| depth.get() != 0)
}

struct CallbackGuard;
impl CallbackGuard {
    fn enter() -> Self {
        CALLBACK_DEPTH.with(|depth| {
            depth.set(
                depth
                    .get()
                    .checked_add(1)
                    .expect("allocator callback recursion overflow"),
            )
        });
        Self
    }
}
impl Drop for CallbackGuard {
    fn drop(&mut self) {
        CALLBACK_DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

#[repr(C)]
struct Header {
    original: *mut u8,
    offset: usize,
    layout: Layout,
    tracker: Option<Shared<AllocationTracker>>,
}

unsafe extern "C" {
    #[link_name = "malloc"]
    fn c_malloc(size: usize) -> *mut c_void;
    #[link_name = "realloc"]
    fn c_realloc(pointer: *mut c_void, size: usize) -> *mut c_void;
    #[link_name = "free"]
    fn c_free(pointer: *mut c_void);
}

impl Allocator {
    /// Construct an allocator from a complete caller-supplied C suite.
    ///
    /// # Safety
    /// Callbacks must implement malloc/realloc/free semantics, return disjoint
    /// writable allocations of the requested size (or NULL), and accept their
    /// corresponding pointers exactly once for freeing. A failed realloc leaves
    /// the original allocation live. They must remain callable on any thread that
    /// uses this allocator or its containers, and must be safe for concurrent calls
    /// from multiple threads. They must not unwind, and must not reenter
    /// parser APIs while an allocation callback is active.
    pub unsafe fn from_callbacks(suite: MemorySuite) -> Result<Self, AllocError> {
        if suite.malloc.is_none() || suite.realloc.is_none() || suite.free.is_none() {
            return Err(AllocError::InvalidAllocator);
        }
        Ok(Self::Custom(CustomAllocator { suite }))
    }

    /// Call the C suite directly, preserving its original pointer.
    /// This is adapter plumbing; C API blocks use `tracked_malloc` instead.
    ///
    /// # Safety
    /// The returned block must be freed through this same allocator's `free` method.
    pub unsafe fn malloc(self, size: usize) -> *mut c_void {
        match self {
            Self::System | Self::TrackedSystem => {
                let _guard = CallbackGuard::enter();
                // SAFETY: libc malloc accepts any size and returns NULL on failure.
                unsafe { c_malloc(size) }
            }
            Self::Custom(custom) => {
                let suite = custom.suite;
                let _guard = CallbackGuard::enter();
                // SAFETY: Construction validated the callback and the constructor
                // contract requires malloc semantics for the requested size.
                unsafe { (suite.malloc.expect("validated malloc callback"))(size) }
            }
        }
    }

    /// Reallocate a direct C API block; failure preserves the original block.
    ///
    /// # Safety
    /// `pointer` must be NULL or a live block from this allocator's C API methods.
    pub unsafe fn realloc(self, pointer: *mut c_void, size: usize) -> *mut c_void {
        match self {
            Self::System | Self::TrackedSystem => {
                let _guard = CallbackGuard::enter();
                // SAFETY: Caller provides libc's original allocation pointer.
                unsafe { c_realloc(pointer, size) }
            }
            Self::Custom(custom) => {
                let suite = custom.suite;
                let _guard = CallbackGuard::enter();
                // SAFETY: The constructor and caller jointly guarantee realloc's contract.
                unsafe { (suite.realloc.expect("validated realloc callback"))(pointer, size) }
            }
        }
    }

    /// Free a direct C API block with its original allocator.
    ///
    /// # Safety
    /// `pointer` must be NULL or a live block from this allocator's C API methods.
    pub unsafe fn free(self, pointer: *mut c_void) {
        match self {
            Self::System | Self::TrackedSystem => {
                let _guard = CallbackGuard::enter();
                // SAFETY: Caller provides libc's original pointer or NULL.
                unsafe { c_free(pointer) }
            }
            Self::Custom(custom) => {
                let suite = custom.suite;
                let _guard = CallbackGuard::enter();
                // SAFETY: The constructor and caller jointly guarantee free's contract.
                unsafe { (suite.free.expect("validated free callback"))(pointer) }
            }
        }
    }

    /// Enable accounting metadata while leaving bare Rust System containers unchanged.
    #[must_use]
    pub fn trackable(self) -> Self {
        match self {
            Self::System => Self::TrackedSystem,
            other => other,
        }
    }

    /// Allocate a C API block with an owned tracking header and recorded layout.
    ///
    /// # Safety
    /// Free this pointer only with `tracked_free` using the same allocator.
    pub unsafe fn tracked_malloc(self, size: usize) -> *mut c_void {
        let Ok(layout) = Layout::from_size_align(size, align_of::<u128>().max(align_of::<usize>()))
        else {
            return ptr::null_mut();
        };
        self.trackable()
            .allocate(layout)
            .map_or(ptr::null_mut(), |block| block.as_ptr().cast::<u8>().cast())
    }

    /// Resize a C API block, preserving its original allocation tracker.
    ///
    /// # Safety
    /// `pointer` is NULL or a live `tracked_malloc`/`tracked_realloc` block from self.
    pub unsafe fn tracked_realloc(self, pointer: *mut c_void, size: usize) -> *mut c_void {
        if pointer.is_null() {
            // SAFETY: This preserves realloc(NULL, size) semantics.
            return unsafe { self.tracked_malloc(size) };
        }
        // SAFETY: Tracked C blocks always retain their original layout in the header.
        let old = unsafe {
            (*pointer
                .cast::<u8>()
                .sub(size_of::<Header>())
                .cast::<Header>())
            .layout
        };
        let Ok(new) = Layout::from_size_align(size, old.align()) else {
            return ptr::null_mut();
        };
        // SAFETY: The original pointer/layout match this allocator; resize checks
        // new sizes and preserves the old allocation on failure.
        unsafe {
            self.trackable()
                .resize(NonNull::new_unchecked(pointer.cast()), old, new)
        }
        .map_or(ptr::null_mut(), |block| block.as_ptr().cast::<u8>().cast())
    }

    /// Free a tracked C API block, even outside its original tracking scope.
    ///
    /// # Safety
    /// `pointer` is NULL or a live block from self's tracked C allocation methods.
    pub unsafe fn tracked_free(self, pointer: *mut c_void) {
        if pointer.is_null() {
            return;
        }
        // SAFETY: The caller supplies a live pointer with the adapter's header.
        let layout = unsafe {
            (*pointer
                .cast::<u8>()
                .sub(size_of::<Header>())
                .cast::<Header>())
            .layout
        };
        // SAFETY: The recorded layout and original allocator match this pointer.
        unsafe {
            self.trackable()
                .deallocate(NonNull::new_unchecked(pointer.cast()), layout)
        };
    }

    fn total_size(layout: Layout) -> Result<usize, ApiError> {
        layout
            .size()
            .checked_add(layout.align().max(align_of::<Header>()) - 1)
            .and_then(|size| size.checked_add(size_of::<Header>()))
            .filter(|size| Layout::from_size_align(*size, align_of::<Header>()).is_ok())
            .ok_or(ApiError)
    }
    fn raw_layout(size: usize) -> Layout {
        Layout::from_size_align(size, align_of::<Header>()).expect("checked allocation size")
    }
    fn raw_allocate(self, size: usize) -> Result<NonNull<u8>, ApiError> {
        if matches!(self, Self::TrackedSystem) {
            let _guard = CallbackGuard::enter();
            return Global
                .allocate(Self::raw_layout(size))
                .map(|block| block.cast());
        }
        // SAFETY: Custom construction validates malloc and its ownership contract.
        NonNull::new(unsafe { self.malloc(size) }.cast::<u8>()).ok_or(ApiError)
    }
    unsafe fn raw_deallocate(self, original: NonNull<u8>, size: usize) {
        if matches!(self, Self::TrackedSystem) {
            let _guard = CallbackGuard::enter();
            // SAFETY: The original global allocation used this recorded raw layout.
            unsafe { Global.deallocate(original, Self::raw_layout(size)) };
        } else {
            // SAFETY: This exact original pointer came from the custom C suite.
            unsafe { self.free(original.as_ptr().cast()) };
        }
    }
    unsafe fn aligned_pointer(original: *mut u8, layout: Layout) -> *mut u8 {
        // SAFETY: total_size reserved a header, full alignment padding, and payload.
        unsafe {
            let first = original.add(size_of::<Header>());
            first.add(first.align_offset(layout.align().max(align_of::<Header>())))
        }
    }
    fn allocate_tracked(self, layout: Layout) -> Result<NonNull<[u8]>, ApiError> {
        let size = Self::total_size(layout)?;
        let tracker = current_tracker();
        let charge = Charge::reserve(tracker.as_deref(), size)?;
        let original = self.raw_allocate(size)?;
        // No fallible operation follows commitment; ownership moves into the header.
        charge.commit();
        // SAFETY: This allocation has exactly the checked header/padding/payload size.
        unsafe {
            let pointer = Self::aligned_pointer(original.as_ptr(), layout);
            pointer
                .sub(size_of::<Header>())
                .cast::<Header>()
                .write(Header {
                    original: original.as_ptr(),
                    offset: pointer.offset_from(original.as_ptr()) as usize,
                    layout,
                    tracker,
                });
            Ok(NonNull::slice_from_raw_parts(
                NonNull::new_unchecked(pointer),
                layout.size(),
            ))
        }
    }

    unsafe fn resize(
        self,
        pointer: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, ApiError> {
        let new_size = Self::total_size(new_layout)?;
        // SAFETY: All trackable allocations have a live aligned Header. Copy scalar
        // metadata and clone its shared owner before a foreign realloc can move it.
        let (original, old_offset, layout, tracker) = unsafe {
            let header = &*pointer.as_ptr().sub(size_of::<Header>()).cast::<Header>();
            (
                header.original,
                header.offset,
                header.layout,
                header.tracker.clone(),
            )
        };
        let old_size = Self::total_size(layout)?;
        let charge = Charge::reserve(tracker.as_deref(), new_size.saturating_sub(old_size))?;
        let fresh = new_layout.align() < old_layout.align();
        let next = if fresh {
            self.raw_allocate(new_size)?
        } else if matches!(self, Self::TrackedSystem) {
            let _guard = CallbackGuard::enter();
            // SAFETY: The global backing allocation uses recorded raw layouts.
            let block = unsafe {
                if new_size >= old_size {
                    Global.grow(
                        NonNull::new_unchecked(original),
                        Self::raw_layout(old_size),
                        Self::raw_layout(new_size),
                    )
                } else {
                    Global.shrink(
                        NonNull::new_unchecked(original),
                        Self::raw_layout(old_size),
                        Self::raw_layout(new_size),
                    )
                }
            }?;
            block.cast()
        } else {
            // SAFETY: Realloc receives the exact original pointer and preserves it on failure.
            NonNull::new(unsafe { self.realloc(original.cast(), new_size) }.cast::<u8>())
                .ok_or(ApiError)?
        };
        // SAFETY: A successful realloc retained the entire old header (new alignment
        // is at least the old alignment), or a fresh allocation left the old block
        // untouched. Move the original owning header before overlapping payload copy.
        unsafe {
            let old_header = if fresh {
                pointer
                    .as_ptr()
                    .sub(size_of::<Header>())
                    .cast::<Header>()
                    .read()
            } else {
                next.as_ptr()
                    .add(old_offset - size_of::<Header>())
                    .cast::<Header>()
                    .read()
            };
            let destination = Self::aligned_pointer(next.as_ptr(), new_layout);
            let source = if fresh {
                pointer.as_ptr()
            } else {
                next.as_ptr().add(old_offset)
            };
            ptr::copy(
                source,
                destination,
                old_layout.size().min(new_layout.size()),
            );
            destination
                .sub(size_of::<Header>())
                .cast::<Header>()
                .write(Header {
                    original: next.as_ptr(),
                    offset: destination.offset_from(next.as_ptr()) as usize,
                    layout: new_layout,
                    tracker: old_header.tracker,
                });
            if fresh {
                self.raw_deallocate(NonNull::new_unchecked(original), old_size);
            }
            if old_size > new_size
                && let Some(tracker) = &tracker
            {
                tracker.release(old_size - new_size);
            }
            charge.commit();
            Ok(NonNull::slice_from_raw_parts(
                NonNull::new_unchecked(destination),
                new_layout.size(),
            ))
        }
    }
}

// SAFETY: Bare System delegates to Rust's global allocator. Trackable allocations
// own their original backing pointer, layout, and tracker in an aligned header.
// All sizes are checked and failed resize preserves both bytes and tracker ownership.
unsafe impl ApiAllocator for Allocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, ApiError> {
        if matches!(self, Self::System) {
            Global.allocate(layout)
        } else {
            self.allocate_tracked(layout)
        }
    }
    unsafe fn deallocate(&self, pointer: NonNull<u8>, layout: Layout) {
        if matches!(self, Self::System) {
            // SAFETY: Forward the caller's original global allocation and layout.
            unsafe { Global.deallocate(pointer, layout) };
            return;
        }
        // SAFETY: Move the sole header owner before releasing its backing memory.
        let header = unsafe {
            pointer
                .as_ptr()
                .sub(size_of::<Header>())
                .cast::<Header>()
                .read()
        };
        let size = Self::total_size(header.layout).expect("previously checked allocation size");
        if let Some(tracker) = &header.tracker {
            tracker.release(size);
        }
        // SAFETY: The header contains the exact original allocation pointer and layout.
        unsafe { self.raw_deallocate(NonNull::new_unchecked(header.original), size) };
        // Header drop releases its tracker after accounting/backing memory cleanup.
    }
    unsafe fn grow(
        &self,
        pointer: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, ApiError> {
        if matches!(self, Self::System) {
            // SAFETY: Forward the allocator trait's pointer and layout contract.
            unsafe { Global.grow(pointer, old_layout, new_layout) }
        } else {
            // SAFETY: resize preserves every old byte and ownership on failure.
            unsafe { self.resize(pointer, old_layout, new_layout) }
        }
    }
    unsafe fn grow_zeroed(
        &self,
        pointer: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, ApiError> {
        if matches!(self, Self::System) {
            // SAFETY: Forward the allocator trait's pointer and layout contract.
            return unsafe { Global.grow_zeroed(pointer, old_layout, new_layout) };
        }
        // SAFETY: Grow guarantees new_size >= old_size and preserves the old prefix.
        let result = unsafe { self.resize(pointer, old_layout, new_layout)? };
        // SAFETY: Only the newly allocated suffix is zeroed inside the new block.
        unsafe {
            result
                .as_ptr()
                .cast::<u8>()
                .add(old_layout.size())
                .write_bytes(0, new_layout.size() - old_layout.size());
        }
        Ok(result)
    }
    unsafe fn shrink(
        &self,
        pointer: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, ApiError> {
        if matches!(self, Self::System) {
            // SAFETY: Forward the allocator trait's pointer and layout contract.
            unsafe { Global.shrink(pointer, old_layout, new_layout) }
        } else {
            // SAFETY: resize preserves the retained prefix and ownership on failure.
            unsafe { self.resize(pointer, old_layout, new_layout) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huge_tracked_layouts_fail_without_panicking_or_allocating() {
        let allocator = Allocator::System.trackable();
        // These layouts fit isize::MAX as payloads, but their backing layouts do
        // not: rounding to Header's alignment would exceed the maximum size.
        for adjustment in 0..align_of::<Header>() - 1 {
            let size =
                isize::MAX as usize - size_of::<Header>() - (align_of::<Header>() - 1) - adjustment;
            let layout = Layout::from_size_align(size, 1).unwrap();
            assert!(allocator.allocate(layout).is_err());
        }
        let layout = Layout::from_size_align(isize::MAX as usize, 1).unwrap();
        assert!(allocator.allocate(layout).is_err());
    }
}
