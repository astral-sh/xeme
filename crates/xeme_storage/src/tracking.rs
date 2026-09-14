//! Tracks live allocations for a root parser and its children.
//! Each allocation retains its tracker until freed.

use std::cell::Cell;
use std::ptr;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::{AllocError, Allocator, Shared};

pub const MAXIMUM_AMPLIFICATION_DEFAULT: f32 = 100.0;
pub const ACTIVATION_THRESHOLD_DEFAULT: u64 = 64 * 1024 * 1024;
/// Absolute family limit, including allocation metadata and pending reservations.
/// This ceiling also applies when the relative amplification factor is infinite.
pub const MAXIMUM_LIVE_BYTES: usize = 512 * 1024 * 1024;

/// Allocation amplification protection shared by a root parser and its children.
#[derive(Debug)]
pub struct AllocationTracker {
    live: AtomicUsize,
    peak: AtomicUsize,
    direct: AtomicU64,
    factor: AtomicU32,
    threshold: AtomicU64,
}
impl AllocationTracker {
    /// Allocate the tracker without tracking its own backing allocation.
    pub fn try_new_in(allocator: Allocator) -> Result<Shared<Self>, AllocError> {
        without_tracking(|| {
            Shared::try_new_in(
                Self {
                    live: AtomicUsize::new(0),
                    peak: AtomicUsize::new(0),
                    direct: AtomicU64::new(0),
                    factor: AtomicU32::new(MAXIMUM_AMPLIFICATION_DEFAULT.to_bits()),
                    threshold: AtomicU64::new(ACTIVATION_THRESHOLD_DEFAULT),
                },
                allocator.trackable(),
            )
        })
    }
    /// Live backing bytes plus reservations for allocator calls in progress.
    #[must_use]
    pub fn live_bytes(&self) -> usize {
        self.live.load(Ordering::Relaxed)
    }
    /// Highest live-byte reservation, including calls that later failed allocation.
    #[must_use]
    pub fn peak_bytes(&self) -> usize {
        self.peak.load(Ordering::Relaxed)
    }
    #[must_use]
    pub fn direct_bytes(&self) -> u64 {
        self.direct.load(Ordering::Relaxed)
    }
    /// Reset input accounting for a new document without discarding live allocations.
    pub fn reset_direct_bytes(&self) {
        self.direct.store(0, Ordering::Relaxed);
    }
    pub fn add_direct_bytes(&self, bytes: u64) -> bool {
        self.direct
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(bytes)
            })
            .is_ok()
    }
    pub fn set_maximum_amplification(&self, factor: f32) -> bool {
        if factor.is_nan() || factor < 1.0 {
            return false;
        }
        self.factor.store(factor.to_bits(), Ordering::Relaxed);
        true
    }
    pub fn set_activation_threshold(&self, threshold: u64) {
        self.threshold.store(threshold, Ordering::Relaxed);
    }
    pub(crate) fn charge(&self, amount: usize) -> Result<(), allocator_api2::alloc::AllocError> {
        if amount == 0 {
            return Ok(());
        }
        let threshold = self.threshold.load(Ordering::Relaxed);
        let factor = f32::from_bits(self.factor.load(Ordering::Relaxed));
        let direct = self.direct.load(Ordering::Relaxed);
        let previous = self
            .live
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |current| {
                let next = current.checked_add(amount)?;
                if next > MAXIMUM_LIVE_BYTES {
                    return None;
                }
                if next as u64 >= threshold && next as f32 / direct as f32 > factor {
                    None
                } else {
                    Some(next)
                }
            })
            .map_err(|_| allocator_api2::alloc::AllocError)?;
        self.peak.fetch_max(previous + amount, Ordering::Relaxed);
        Ok(())
    }
    pub(crate) fn release(&self, amount: usize) {
        let previous = self.live.fetch_sub(amount, Ordering::AcqRel);
        assert!(previous >= amount, "allocation accounting underflow");
    }
}

thread_local! {
    static CURRENT: Cell<*const Shared<AllocationTracker>> = const { Cell::new(ptr::null()) };
}
struct Restore(*const Shared<AllocationTracker>);
impl Drop for Restore {
    fn drop(&mut self) {
        CURRENT.with(|current| current.set(self.0));
    }
}

/// Select the tracker for new allocations made by trackable allocators in a closure.
/// Existing allocations always retain their original tracker on resize and free.
/// Restores the previous tracker when the closure returns or unwinds.
pub fn with_tracking<R>(tracker: &Shared<AllocationTracker>, operation: impl FnOnce() -> R) -> R {
    let previous = CURRENT.with(|current| current.replace(ptr::from_ref(tracker)));
    let _restore = Restore(previous);
    operation()
}

/// Run an operation without associating new allocations with any tracker.
pub fn without_tracking<R>(operation: impl FnOnce() -> R) -> R {
    let previous = CURRENT.with(|current| current.replace(ptr::null()));
    let _restore = Restore(previous);
    operation()
}

pub(crate) fn current_tracker() -> Option<Shared<AllocationTracker>> {
    CURRENT.with(|current| {
        let pointer = current.get();
        if pointer.is_null() {
            None
        } else {
            // SAFETY: Only the closure-based with_tracking function installs this
            // pointer. Its borrowed Shared owner remains live until restoration,
            // including nested scopes and unwinding. Clone is allocation-free.
            Some(unsafe { &*pointer }.clone())
        }
    })
}

pub(crate) struct Charge<'a> {
    tracker: Option<&'a AllocationTracker>,
    amount: usize,
}
impl<'a> Charge<'a> {
    pub(crate) fn reserve(
        tracker: Option<&'a AllocationTracker>,
        amount: usize,
    ) -> Result<Self, allocator_api2::alloc::AllocError> {
        if let Some(tracker) = tracker {
            tracker.charge(amount)?;
        }
        Ok(Self { tracker, amount })
    }
    pub(crate) fn commit(mut self) {
        self.amount = 0;
    }
}
impl Drop for Charge<'_> {
    fn drop(&mut self) {
        if self.amount != 0
            && let Some(tracker) = self.tracker
        {
            tracker.release(self.amount);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_family_ceiling_cannot_be_disabled_with_the_relative_limit() {
        let tracker = AllocationTracker::try_new_in(Allocator::System).unwrap();
        tracker.set_maximum_amplification(f32::INFINITY);
        tracker.set_activation_threshold(u64::MAX);
        tracker.add_direct_bytes(u64::MAX);
        assert!(tracker.charge(MAXIMUM_LIVE_BYTES + 1).is_err());
        assert_eq!(tracker.live_bytes(), 0);
        // Test accounting alone, without requesting a large backing allocation.
        tracker.charge(MAXIMUM_LIVE_BYTES).unwrap();
        assert!(tracker.charge(1).is_err());
        assert_eq!(tracker.live_bytes(), MAXIMUM_LIVE_BYTES);
        tracker.release(MAXIMUM_LIVE_BYTES);
        assert_eq!(tracker.live_bytes(), 0);
    }
}
