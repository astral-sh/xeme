//! Fallible storage that carries its allocator through parser and callback lifetimes.

mod allocator;
mod queue;
mod shared;
mod string;
mod tracking;

pub use allocator::{Allocator, CustomAllocator, MemorySuite, in_allocator_callback};
pub use allocator_api2::alloc::{Allocator as AllocatorApi, Layout};
pub use queue::Queue;
pub use shared::Shared;
pub use string::{CString, String};
pub use tracking::{
    ACTIVATION_THRESHOLD_DEFAULT, AllocationTracker, MAXIMUM_AMPLIFICATION_DEFAULT,
    MAXIMUM_LIVE_BYTES, with_tracking, without_tracking,
};

/// An allocation failure that can be reported without allocating another object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocError {
    OutOfMemory,
    CapacityOverflow,
    InvalidAllocator,
    InteriorNul,
}
impl std::fmt::Display for AllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::OutOfMemory => "out of memory",
            Self::CapacityOverflow => "allocation size overflow",
            Self::InvalidAllocator => "incomplete memory handling suite",
            Self::InteriorNul => "string contains a NUL character",
        })
    }
}
impl std::error::Error for AllocError {}
impl From<allocator_api2::alloc::AllocError> for AllocError {
    fn from(_: allocator_api2::alloc::AllocError) -> Self {
        Self::OutOfMemory
    }
}
impl From<allocator_api2::collections::TryReserveError> for AllocError {
    fn from(error: allocator_api2::collections::TryReserveError) -> Self {
        match error.kind() {
            allocator_api2::collections::TryReserveErrorKind::CapacityOverflow => {
                Self::CapacityOverflow
            }
            allocator_api2::collections::TryReserveErrorKind::AllocError { .. } => {
                Self::OutOfMemory
            }
        }
    }
}
impl From<hashbrown::TryReserveError> for AllocError {
    fn from(error: hashbrown::TryReserveError) -> Self {
        match error {
            hashbrown::TryReserveError::CapacityOverflow => Self::CapacityOverflow,
            hashbrown::TryReserveError::AllocError { .. } => Self::OutOfMemory,
        }
    }
}

pub type Vec<T> = allocator_api2::vec::Vec<T, Allocator>;
pub type Box<T> = allocator_api2::boxed::Box<T, Allocator>;
pub type HashMap<K, V> =
    hashbrown::HashMap<K, V, std::collections::hash_map::RandomState, Allocator>;
pub type HashSet<K> = hashbrown::HashSet<K, std::collections::hash_map::RandomState, Allocator>;

#[must_use]
pub fn hash_map<K, V>(allocator: Allocator) -> HashMap<K, V> {
    HashMap::with_hasher_in(std::collections::hash_map::RandomState::new(), allocator)
}
#[must_use]
pub fn hash_set<K>(allocator: Allocator) -> HashSet<K> {
    HashSet::with_hasher_in(std::collections::hash_map::RandomState::new(), allocator)
}
pub fn try_box<T>(value: T, allocator: Allocator) -> Result<Box<T>, AllocError> {
    Box::try_new_in(value, allocator).map_err(Into::into)
}
pub fn try_push<T>(values: &mut Vec<T>, value: T) -> Result<(), AllocError> {
    values.try_reserve(1)?;
    values.push(value);
    Ok(())
}
pub fn try_extend_from_slice<T: Copy>(values: &mut Vec<T>, other: &[T]) -> Result<(), AllocError> {
    values.try_reserve(other.len())?;
    values.extend_from_slice(other);
    Ok(())
}
pub fn try_insert<K: Eq + std::hash::Hash, V>(
    map: &mut HashMap<K, V>,
    key: K,
    value: V,
) -> Result<Option<V>, AllocError> {
    map.try_reserve(1)?;
    Ok(map.insert(key, value))
}
pub fn try_set_insert<K: Eq + std::hash::Hash>(
    set: &mut HashSet<K>,
    key: K,
) -> Result<bool, AllocError> {
    set.try_reserve(1)?;
    Ok(set.insert(key))
}
pub fn try_format(
    allocator: Allocator,
    arguments: std::fmt::Arguments<'_>,
) -> Result<String, AllocError> {
    use std::fmt::Write;
    let mut output = String::new_in(allocator);
    output
        .write_fmt(arguments)
        .map_err(|_| AllocError::OutOfMemory)?;
    Ok(output)
}

/// Explicit fallible cloning, avoiding collection Clone implementations that abort.
pub trait TryClone: Sized {
    fn try_clone(&self) -> Result<Self, AllocError>;
}
impl TryClone for String {
    fn try_clone(&self) -> Result<Self, AllocError> {
        String::try_clone(self)
    }
}
impl<T: TryClone> TryClone for Option<T> {
    fn try_clone(&self) -> Result<Self, AllocError> {
        self.as_ref().map(TryClone::try_clone).transpose()
    }
}
impl<T: TryClone> TryClone for Vec<T> {
    fn try_clone(&self) -> Result<Self, AllocError> {
        let mut output = Self::new_in(*self.allocator());
        output.try_reserve_exact(self.len())?;
        for item in self {
            output.push(item.try_clone()?);
        }
        Ok(output)
    }
}
impl<A: TryClone, B: TryClone> TryClone for (A, B) {
    fn try_clone(&self) -> Result<Self, AllocError> {
        Ok((self.0.try_clone()?, self.1.try_clone()?))
    }
}
macro_rules! copy_clone {
    ($($ty:ty),* $(,)?) => {$(impl TryClone for $ty {
        fn try_clone(&self) -> Result<Self, AllocError> { Ok(*self) }
    })*};
}
copy_clone!(
    u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, bool, char
);

#[cfg(test)]
mod tests;
