//! Shared support for fuzz harnesses.

pub mod allocator;

/// Decode an optional failure at any of the first 512 allocator requests.
/// Bit zero enables failure; the next nine bits select an independent ordinal.
pub fn allocation_failure_ordinal(control: u16) -> usize {
    if control & 1 == 0 {
        0
    } else {
        usize::from((control >> 1) & 511) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::allocation_failure_ordinal;

    #[test]
    fn every_failure_ordinal_and_the_disabled_mode_are_reachable() {
        for ordinal in 1..=512 {
            let selection = ((ordinal - 1) * 2) as u16;
            assert_eq!(allocation_failure_ordinal(selection), 0);
            assert_eq!(allocation_failure_ordinal(selection | 1), ordinal);
        }
    }
}
