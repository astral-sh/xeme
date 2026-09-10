//! Runtime allocation with explicit ownership and callback reentry detection.

use std::cell::Cell;
use std::ffi::c_void;
use std::mem::{align_of, size_of};
use std::ptr::{self, NonNull};

use allocator_api2::alloc::{AllocError as ApiError, Allocator as ApiAllocator, Global, Layout};

use crate::AllocError;

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
#[derive(Clone, Copy)]
struct Header {
    original: *mut u8,
    offset: usize,
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

    /// Call the C allocation API directly, preserving the suite's original pointer.
    /// This is for public XML_Mem* and content-model blocks, not Rust containers.
    ///
    /// # Safety
    /// The returned block must be freed through this same allocator's `free` method.
    pub unsafe fn malloc(self, size: usize) -> *mut c_void {
        match self {
            Self::System => {
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
            Self::System => {
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
            Self::System => {
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

    fn total_size(layout: Layout) -> Result<usize, ApiError> {
        layout
            .size()
            .checked_add(layout.align().max(align_of::<Header>()) - 1)
            .and_then(|size| size.checked_add(size_of::<Header>()))
            .filter(|size| *size <= isize::MAX as usize)
            .ok_or(ApiError)
    }

    unsafe fn aligned_pointer(original: *mut u8, layout: Layout) -> *mut u8 {
        // SAFETY: total_size reserved room for a header, alignment padding, and the
        // payload. Effective alignment also keeps the preceding Header aligned.
        unsafe {
            let first = original.add(size_of::<Header>());
            let padding = first.align_offset(layout.align().max(align_of::<Header>()));
            let pointer = first.add(padding);
            pointer
                .sub(size_of::<Header>())
                .cast::<Header>()
                .write(Header {
                    original,
                    offset: size_of::<Header>() + padding,
                });
            pointer
        }
    }

    unsafe fn resize(
        self,
        pointer: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, ApiError> {
        let size = Self::total_size(new_layout)?;
        // SAFETY: Every custom container allocation has our aligned Header directly
        // before the user pointer. Read its value before realloc can move the block.
        let header = unsafe {
            pointer
                .as_ptr()
                .sub(size_of::<Header>())
                .cast::<Header>()
                .read()
        };
        // When the alignment decreases, the new allocation may not retain bytes at
        // the old offset. Allocate/copy/free instead of reading beyond the new block.
        if new_layout.align() < old_layout.align() {
            let result = self.allocate(new_layout)?;
            // SAFETY: Both blocks are live, disjoint, and contain min(old,new) bytes.
            unsafe {
                ptr::copy_nonoverlapping(
                    pointer.as_ptr(),
                    result.as_ptr().cast::<u8>(),
                    old_layout.size().min(new_layout.size()),
                );
                self.deallocate(pointer, old_layout);
            }
            return Ok(result);
        }
        // SAFETY: This is the exact original pointer returned by this C suite.
        let original = unsafe { self.realloc(header.original.cast(), size) }.cast::<u8>();
        let Some(original) = NonNull::new(original) else {
            return Err(ApiError);
        };
        // Compute the new aligned address without writing its header yet: a moved
        // header could otherwise overwrite bytes waiting to be relocated.
        // SAFETY: The new block covers header, padding, and payload.
        unsafe {
            let first = original.as_ptr().add(size_of::<Header>());
            let padding = first.align_offset(new_layout.align().max(align_of::<Header>()));
            let destination = first.add(padding);
            ptr::copy(
                original.as_ptr().add(header.offset),
                destination,
                old_layout.size().min(new_layout.size()),
            );
            destination
                .sub(size_of::<Header>())
                .cast::<Header>()
                .write(Header {
                    original: original.as_ptr(),
                    offset: size_of::<Header>() + padding,
                });
            Ok(NonNull::slice_from_raw_parts(
                NonNull::new_unchecked(destination),
                new_layout.size(),
            ))
        }
    }
}

// SAFETY: System delegates to Rust's global allocator. Custom allocations track
// their exact original C pointer in an aligned header; all pointer arithmetic is
// bounded by checked sizes, and failed realloc leaves the old block untouched.
unsafe impl ApiAllocator for Allocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, ApiError> {
        if matches!(self, Self::System) {
            return Global.allocate(layout);
        }
        let size = Self::total_size(layout)?;
        // SAFETY: The constructor validated this suite's malloc contract.
        let original = unsafe { self.malloc(size) }.cast::<u8>();
        let Some(original) = NonNull::new(original) else {
            return Err(ApiError);
        };
        // SAFETY: The allocation has the checked header/padding/payload size.
        let pointer = unsafe { Self::aligned_pointer(original.as_ptr(), layout) };
        // SAFETY: The aligned payload lies within the non-null allocation.
        Ok(NonNull::slice_from_raw_parts(
            unsafe { NonNull::new_unchecked(pointer) },
            layout.size(),
        ))
    }

    unsafe fn deallocate(&self, pointer: NonNull<u8>, layout: Layout) {
        if matches!(self, Self::System) {
            // SAFETY: Caller supplies the original global allocation and layout.
            unsafe { Global.deallocate(pointer, layout) };
        } else {
            // SAFETY: This pointer was allocated by this adapter, so its preceding
            // header is live and contains the exact original C allocation.
            let header = unsafe {
                pointer
                    .as_ptr()
                    .sub(size_of::<Header>())
                    .cast::<Header>()
                    .read()
            };
            // SAFETY: The recovered pointer belongs to this suite and is freed once.
            unsafe { self.free(header.original.cast()) };
        }
    }

    unsafe fn grow(
        &self,
        pointer: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, ApiError> {
        if matches!(self, Self::System) {
            // SAFETY: Forward the allocator trait's pointer/layout contract.
            unsafe { Global.grow(pointer, old_layout, new_layout) }
        } else {
            // SAFETY: resize preserves all old bytes and ownership on failure.
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
            // SAFETY: Forward the allocator trait's pointer/layout contract.
            return unsafe { Global.grow_zeroed(pointer, old_layout, new_layout) };
        }
        // SAFETY: The grow contract guarantees new_size >= old_size, and resize
        // preserves the old prefix while leaving ownership untouched on failure.
        let result = unsafe { self.resize(pointer, old_layout, new_layout)? };
        // SAFETY: The returned block covers new_size bytes; only its newly grown
        // suffix is initialized here, preserving every byte in the old prefix.
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
            // SAFETY: Forward the allocator trait's pointer/layout contract.
            unsafe { Global.shrink(pointer, old_layout, new_layout) }
        } else {
            // SAFETY: resize preserves the new-size prefix and ownership on failure.
            unsafe { self.resize(pointer, old_layout, new_layout) }
        }
    }
}
