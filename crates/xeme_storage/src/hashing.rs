//! Caller salt augments, rather than replaces, the process's secret hash keys.

use std::collections::hash_map::{DefaultHasher, RandomState};
use std::hash::{BuildHasher, Hasher};

/// Randomized hashing with optional caller-provided salt mixed into every hash.
///
/// Keeping `RandomState` protects tables even when an application supplies a
/// predictable salt. The all-zero default keeps the original hash stream and
/// avoids extra hashing work for applications that do not configure a salt.
#[derive(Clone, Debug, Default)]
pub struct SaltedRandomState {
    random: RandomState,
    salt: [u8; 16],
}

impl SaltedRandomState {
    /// Retain the secret random keys while selecting a caller salt.
    #[must_use]
    pub fn with_salt(&self, salt: [u8; 16]) -> Self {
        Self {
            random: self.random.clone(),
            salt,
        }
    }

    /// Return the configured public salt, without exposing secret random keys.
    #[must_use]
    pub fn salt(&self) -> [u8; 16] {
        self.salt
    }
}

impl BuildHasher for SaltedRandomState {
    type Hasher = DefaultHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        let mut hasher = self.random.build_hasher();
        if self.salt != [0; 16] {
            hasher.write(&self.salt);
        }
        hasher
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_salt_changes_hashes_without_replacing_random_keys() {
        let state = SaltedRandomState::default();
        assert_eq!(
            state.hash_one("xml-name"),
            state.random.hash_one("xml-name")
        );
        let salted = state.with_salt(*b"0123456789abcdef");
        let mut expected = state.random.build_hasher();
        expected.write(b"0123456789abcdef");
        std::hash::Hash::hash("xml-name", &mut expected);
        assert_eq!(salted.hash_one("xml-name"), expected.finish());
        assert_ne!(salted.hash_one("xml-name"), state.hash_one("xml-name"));
        assert_ne!(
            salted.hash_one("xml-name"),
            state.with_salt([1; 16]).hash_one("xml-name")
        );
        assert_eq!(
            salted.with_salt([0; 16]).hash_one("xml-name"),
            state.hash_one("xml-name")
        );
    }
}
