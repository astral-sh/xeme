//! A small allocator-aware shared owner with allocation-free cloning.

use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering, fence};

use crate::{AllocError, Allocator, AllocatorApi, Layout, TryClone};

struct Inner<T> {
    references: AtomicUsize,
    allocator: Allocator,
    value: T,
}

pub struct Shared<T> {
    pointer: NonNull<Inner<T>>,
}
impl<T> Shared<T> {
    pub fn try_new_in(value: T, allocator: Allocator) -> Result<Self, AllocError> {
        let memory = allocator.allocate(Layout::new::<Inner<T>>())?;
        let pointer = memory.cast::<Inner<T>>();
        // SAFETY: The allocated block is aligned and sized for Inner<T>, and is
        // exclusively owned until this constructor returns its first reference.
        unsafe {
            pointer.as_ptr().write(Inner {
                references: AtomicUsize::new(1),
                allocator,
                value,
            });
        }
        Ok(Self { pointer })
    }
    pub fn try_clone(&self) -> Result<Self, AllocError> {
        // SAFETY: This strong reference keeps Inner live for the atomic increment.
        let references = unsafe { &self.pointer.as_ref().references };
        references
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                (count < isize::MAX as usize).then_some(count + 1)
            })
            .map_err(|_| AllocError::CapacityOverflow)?;
        Ok(Self {
            pointer: self.pointer,
        })
    }
    #[must_use]
    pub fn strong_count(&self) -> usize {
        // SAFETY: This strong reference keeps Inner live for the atomic load.
        unsafe { self.pointer.as_ref().references.load(Ordering::Acquire) }
    }
}
impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        // As with Arc, exhausting the reference counter is an unrecoverable
        // ownership failure. Clone performs no allocation and cannot report OOM.
        self.try_clone().unwrap_or_else(|_| std::process::abort())
    }
}
impl<T> TryClone for Shared<T> {
    fn try_clone(&self) -> Result<Self, AllocError> {
        Shared::try_clone(self)
    }
}
impl<T> Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: A Shared value owns a strong reference and exposes only &T.
        unsafe { &self.pointer.as_ref().value }
    }
}
impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        // SAFETY: This strong owner keeps the allocation live through fetch_sub.
        let inner = unsafe { self.pointer.as_ref() };
        if inner.references.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }
        fence(Ordering::Acquire);
        let allocator = inner.allocator;
        // SAFETY: The last strong owner destroys the value once, then deallocates
        // with the original allocator and exact layout. No reference is used later.
        unsafe {
            std::ptr::drop_in_place(self.pointer.as_ptr());
            allocator.deallocate(self.pointer.cast(), Layout::new::<Inner<T>>());
        }
    }
}
// SAFETY: Shared ownership exposes only immutable references. The value's bounds
// permit cross-thread access; allocator constructors require callable callbacks
// wherever their allocated containers are moved or dropped.
unsafe impl<T: Send + Sync> Send for Shared<T> {}
// SAFETY: Atomic reference counting plus T: Sync permits concurrent shared access.
unsafe impl<T: Send + Sync> Sync for Shared<T> {}
impl<T: std::fmt::Debug> std::fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.deref().fmt(f)
    }
}
