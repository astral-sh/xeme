//! Allocation-free, nonblocking exclusive access for shared parser state.

use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};

/// An exclusive lock that makes one acquisition attempt and never allocates.
///
/// Contention and reentry return `None`; there is no waiting, spinning, or
/// poisoning. The guard releases the lock even when unwinding after a panic.
pub struct TryLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

impl<T> TryLock<T> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    /// Return exclusive access immediately, or `None` if another guard is live.
    pub fn try_lock(&self) -> Option<TryLockGuard<'_, T>> {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()?;
        Some(TryLockGuard {
            lock: self,
            exclusive: PhantomData,
        })
    }
}

// SAFETY: A successful acquire operation creates the only live guard. Releasing
// that guard publishes its writes before another thread acquires the value.
// Exclusive cross-thread access requires T: Send, as moving an owned T does.
unsafe impl<T: Send> Sync for TryLock<T> {}

impl<T: std::fmt::Debug> std::fmt::Debug for TryLock<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.try_lock() {
            Some(value) => formatter.debug_tuple("TryLock").field(&*value).finish(),
            None => formatter.write_str("TryLock(<locked>)"),
        }
    }
}

/// A borrow that keeps its lock alive and releases exclusive access on drop.
///
/// The guard can only be shared between threads when its value supports shared
/// access, even though the lock itself supports exclusive access to `Send` values.
///
/// ```compile_fail
/// use std::cell::Cell;
/// use xeme_storage::TryLock;
/// fn require_sync<T: Sync>(_: &T) {}
/// let lock = TryLock::new(Cell::new(0));
/// require_sync(&lock.try_lock().unwrap());
/// ```
pub struct TryLockGuard<'a, T> {
    lock: &'a TryLock<T>,
    // A shared reference to TryLock<T> alone would incorrectly make the guard
    // Sync for every Send T. This borrow requires T: Sync for shared guards.
    exclusive: PhantomData<&'a mut T>,
}

impl<T> Deref for TryLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: This guard was created after the successful acquire operation
        // and keeps exclusive ownership of the value until its Drop releases it.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for TryLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: The unique guard owns the value, and its mutable borrow prevents
        // simultaneous access through another reference to this guard.
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for TryLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::TryLock;
    use std::cell::Cell;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn contention_returns_without_waiting_and_guard_drop_releases_access() {
        let lock = TryLock::new(Cell::new(41));
        let rendezvous = Barrier::new(2);
        let held = lock.try_lock().unwrap();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                assert!(lock.try_lock().is_none());
                rendezvous.wait();
                rendezvous.wait();
                let value = lock.try_lock().unwrap();
                assert_eq!(value.get(), 42);
                value.set(43);
            });
            held.set(42);
            rendezvous.wait();
            drop(held);
            rendezvous.wait();
        });
        assert_eq!(lock.try_lock().unwrap().get(), 43);
    }

    #[test]
    fn guards_can_move_between_threads_and_release_during_unwinding() {
        let lock = TryLock::new(0);
        let mut guard = lock.try_lock().unwrap();
        std::thread::scope(|scope| scope.spawn(move || *guard = 7).join().unwrap());
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut guard = lock.try_lock().unwrap();
            *guard = 9;
            panic!("unwind while holding the guard");
        }));
        assert!(panic.is_err());
        assert_eq!(*lock.try_lock().unwrap(), 9);
    }

    #[test]
    fn guard_drops_do_not_destroy_the_owned_value() {
        struct CountDrop<'a>(&'a AtomicUsize);
        impl Drop for CountDrop<'_> {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let drops = AtomicUsize::new(0);
        let lock = TryLock::new(CountDrop(&drops));
        drop(lock.try_lock().unwrap());
        drop(lock.try_lock().unwrap());
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        drop(lock);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }
}
