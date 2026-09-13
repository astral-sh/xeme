//! Owned membership indexes for live and inherited entity sources.

use xeme_storage::{AllocError, Allocator, HashMap, SaltedRandomState, String};

use crate::{Error, string};

/// General and parameter names share storage, but have independent lifetimes.
/// Counts distinguish inherited context from source frames: literal provenance
/// removes only names still present in the latter. Entries own names because
/// entity tables can change when external children return declarations.
#[derive(Debug)]
pub(crate) struct ActiveEntities {
    pub(crate) names: HashMap<String, [usize; 4]>,
}

impl ActiveEntities {
    pub(crate) fn new(allocator: Allocator, hasher: &SaltedRandomState) -> Self {
        Self {
            names: HashMap::with_hasher_in(hasher.clone(), allocator),
        }
    }

    pub(crate) fn contains(&self, name: &str, parameter: bool) -> bool {
        let offset = usize::from(parameter);
        self.names
            .get(name)
            .is_some_and(|counts| counts[offset] != 0 || counts[offset + 2] != 0)
    }

    /// Attribute recursion includes inherited names, but not the current content
    /// source. Preserve that distinction without copying the chain per attribute.
    pub(crate) fn inherited_contains(&self, name: &str, parameter: bool) -> bool {
        self.names
            .get(name)
            .is_some_and(|counts| counts[usize::from(parameter) + 2] != 0)
    }

    pub(crate) fn source_contains(&self, name: &str, parameter: bool) -> bool {
        self.names
            .get(name)
            .is_some_and(|counts| counts[usize::from(parameter)] != 0)
    }

    /// Allocate a new name before changing membership; repeated inherited names
    /// reuse the entry. The caller reserves its frame/chain before this operation.
    pub(crate) fn insert(
        &mut self,
        name: &str,
        parameter: bool,
        source: bool,
    ) -> Result<(), Error> {
        let offset = usize::from(parameter) + if source { 0 } else { 2 };
        if let Some(counts) = self.names.get_mut(name) {
            counts[offset] += 1;
        } else {
            self.names.try_reserve(1).map_err(AllocError::from)?;
            let name = string(name, *self.names.allocator())?;
            let mut counts = [0; 4];
            counts[offset] = 1;
            self.names.insert(name, counts);
        }
        Ok(())
    }

    /// Preserve the existing source-name representation exactly: general
    /// recursion compares the entire name, while parameter recursion strips
    /// one leading percent sign. Two views also preserve custom-decoded names.
    pub(crate) fn insert_source_name(&mut self, name: &str, source: bool) -> Result<(), Error> {
        self.insert(name, false, source)?;
        if let Some(parameter) = name.strip_prefix('%')
            && let Err(error) = self.insert(parameter, true, source)
        {
            self.remove(name, false, source);
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn remove_source_name(&mut self, name: &str) {
        self.remove_source(name, false);
        if let Some(parameter) = name.strip_prefix('%') {
            self.remove_source(parameter, true);
        }
    }

    pub(crate) fn remove_source(&mut self, name: &str, parameter: bool) {
        self.remove(name, parameter, true);
    }

    fn remove(&mut self, name: &str, parameter: bool, source: bool) {
        let counts = self.names.get_mut(name).expect("indexed entity source");
        let count = &mut counts[usize::from(parameter) + if source { 0 } else { 2 }];
        debug_assert!(*count != 0);
        *count -= 1;
        if *counts == [0; 4] {
            self.names.remove(name);
        }
    }
}
