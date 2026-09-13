//! Shared byte accounting for entity amplification, separate from work limits.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

pub(crate) const MAXIMUM_AMPLIFICATION_DEFAULT: f32 = 100.0;
pub(crate) const ACTIVATION_THRESHOLD_DEFAULT: u64 = 8 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct EntityBudget {
    pub(crate) expanded: AtomicUsize,
    direct: AtomicU64,
    indirect: AtomicU64,
    factor: AtomicU32,
    threshold: AtomicU64,
}

impl EntityBudget {
    pub(crate) fn new() -> Self {
        Self {
            expanded: AtomicUsize::new(0),
            direct: AtomicU64::new(0),
            indirect: AtomicU64::new(0),
            factor: AtomicU32::new(MAXIMUM_AMPLIFICATION_DEFAULT.to_bits()),
            threshold: AtomicU64::new(ACTIVATION_THRESHOLD_DEFAULT),
        }
    }

    pub(crate) fn set_factor(&self, factor: f32) -> bool {
        if factor.is_nan() || factor < 1.0 {
            return false;
        }
        self.factor.store(factor.to_bits(), Ordering::Relaxed);
        true
    }

    pub(crate) fn set_threshold(&self, bytes: u64) {
        self.threshold.store(bytes, Ordering::Relaxed);
    }

    /// Bound cumulative work by consumed root bytes, retaining a fixed allowance
    /// for small documents. Saturation never exempts the work counter's overflow
    /// check. External bytes and input waiting in the decoder provide no credit.
    pub(crate) fn work_limit(&self, initial: usize, factor: Option<usize>) -> usize {
        let Some(factor) = factor else {
            return initial;
        };
        let direct = usize::try_from(self.direct.load(Ordering::Relaxed)).unwrap_or(usize::MAX);
        initial.max(direct.saturating_mul(factor))
    }

    /// Count bytes before processing their token. Predefined entities contribute
    /// one extra byte without checking immediately, matching Expat's contract.
    pub(crate) fn account(&self, bytes: usize, indirect: bool, enforce: bool) -> bool {
        if bytes == 0 {
            return true;
        }
        let Ok(bytes) = u64::try_from(bytes) else {
            return false;
        };
        let counter = if indirect {
            &self.indirect
        } else {
            &self.direct
        };
        if counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(bytes)
            })
            .is_err()
        {
            return false;
        }
        let direct = self.direct.load(Ordering::Relaxed);
        let indirect = self.indirect.load(Ordering::Relaxed);
        let Some(total) = direct.checked_add(indirect) else {
            return false;
        };
        if !enforce || total < self.threshold.load(Ordering::Relaxed) {
            return true;
        }
        // An independently parsed external entity may precede every root byte.
        // Expat uses the shortest external-entity declaration (22 bytes) as the
        // denominator in that case; real direct input is never replaced by it.
        let amplification = if direct == 0 {
            let Some(total) = indirect.checked_add(22) else {
                return false;
            };
            total as f32 / 22.0
        } else {
            total as f32 / direct as f32
        };
        amplification <= f32::from_bits(self.factor.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::EntityBudget;
    use std::sync::atomic::Ordering;

    #[test]
    fn work_threshold_uses_only_direct_credit_and_saturates() {
        let budget = EntityBudget::new();
        assert!(budget.account(1000, true, false));
        assert_eq!(budget.work_limit(7, Some(100)), 7);
        assert!(budget.account(3, false, true));
        assert_eq!(budget.work_limit(7, None), 7);
        assert_eq!(budget.work_limit(7, Some(100)), 300);
        assert_eq!(budget.work_limit(400, Some(100)), 400);
        assert!(budget.set_factor(f32::INFINITY));
        budget.set_threshold(u64::MAX);
        assert_eq!(budget.work_limit(7, Some(100)), 300);
        budget.direct.store(u64::MAX, Ordering::Relaxed);
        assert_eq!(budget.work_limit(7, Some(100)), usize::MAX);
        assert!(!budget.account(1, false, false));
    }

    #[test]
    fn counters_reject_overflow_even_when_relative_limits_are_disabled() {
        let budget = EntityBudget::new();
        assert!(budget.set_factor(f32::INFINITY));
        budget.set_threshold(u64::MAX);
        budget.direct.store(u64::MAX, Ordering::Relaxed);
        assert!(!budget.account(1, false, true));
        assert!(!budget.account(1, true, true));

        budget.direct.store(0, Ordering::Relaxed);
        budget.indirect.store(u64::MAX - 1, Ordering::Relaxed);
        assert!(!budget.account(1, true, true));
    }
}
