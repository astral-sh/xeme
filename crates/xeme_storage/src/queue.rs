//! An amortized constant-time queue using allocator-aware storage.

use crate::{AllocError, Allocator, TryClone, Vec, try_push};

#[derive(Debug)]
pub struct Queue<T> {
    items: Vec<Option<T>>,
    head: usize,
}
impl<T> Queue<T> {
    #[must_use]
    pub fn new_in(allocator: Allocator) -> Self {
        Self {
            items: Vec::new_in(allocator),
            head: 0,
        }
    }
    /// Append a value, periodically compacting consumed slots to reuse capacity.
    /// Allocation failure leaves the live values intact.
    pub fn try_push_back(&mut self, value: T) -> Result<(), AllocError> {
        if self.head > 0 && self.head >= self.items.len() / 2 {
            self.items.drain(..self.head);
            self.head = 0;
        }
        try_push(&mut self.items, Some(value))
    }
    pub fn push_back(&mut self, value: T) -> Result<(), AllocError> {
        self.try_push_back(value)
    }
    pub fn pop_front(&mut self) -> Option<T> {
        let item = self.items.get_mut(self.head)?.take();
        self.head += 1;
        if self.head == self.items.len() {
            self.clear();
        }
        item
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len() - self.head
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn clear(&mut self) {
        self.items.clear();
        self.head = 0;
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(self.head.checked_add(index)?)?.as_ref()
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.items.get_mut(self.head.checked_add(index)?)?.as_mut()
    }
    pub fn front(&self) -> Option<&T> {
        self.get(0)
    }
    pub fn front_mut(&mut self) -> Option<&mut T> {
        self.get_mut(0)
    }
    pub fn back(&self) -> Option<&T> {
        self.items.last()?.as_ref()
    }
    pub fn back_mut(&mut self) -> Option<&mut T> {
        self.items.last_mut()?.as_mut()
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.items[self.head..]
            .iter()
            .map(|item| item.as_ref().expect("live queue slot"))
    }
    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> + ExactSizeIterator {
        self.items[self.head..]
            .iter_mut()
            .map(|item| item.as_mut().expect("live queue slot"))
    }
    #[must_use]
    pub fn allocator(&self) -> Allocator {
        *self.items.allocator()
    }
}
impl<T: TryClone> TryClone for Queue<T> {
    fn try_clone(&self) -> Result<Self, AllocError> {
        let mut output = Self::new_in(self.allocator());
        output.items.try_reserve_exact(self.len())?;
        for item in self.iter() {
            output.items.push(Some(item.try_clone()?));
        }
        Ok(output)
    }
}
