//! Finite runtime binding ownership; payload inventories remain caller-owned.
extern crate alloc;
use alloc::vec::Vec;
use core::{borrow::Borrow, mem::size_of};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeTableRefusal {
    Capacity,
    Duplicate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeTableStorageReceipt {
    pub maximum_entries: usize,
    pub preparation_requested_bytes_bound: usize,
    pub retained_array_bytes_bound: usize,
}
/// Owns a sorted, preallocated array. It never grows after preparation. Keys
/// and values are moved exactly, and insertion refusal returns both originals.
/// Nested payloads/Arc headers/allocator bookkeeping/root/stack are not counted.
pub struct BoundedRuntimeTable<K, V> {
    entries: Vec<(K, V)>,
    maximum_entries: usize,
}
impl<K: Ord, V> BoundedRuntimeTable<K, V> {
    pub fn storage_reservation(
        maximum_entries: usize,
    ) -> Result<RuntimeTableStorageReceipt, RuntimeTableRefusal> {
        let bytes = maximum_entries
            .checked_mul(size_of::<(K, V)>())
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or(RuntimeTableRefusal::Capacity)?;
        Ok(RuntimeTableStorageReceipt {
            maximum_entries,
            preparation_requested_bytes_bound: bytes,
            retained_array_bytes_bound: bytes,
        })
    }
    pub fn with_storage_limits(
        maximum_entries: usize,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_array_bytes: usize,
    ) -> Result<(Self, RuntimeTableStorageReceipt), RuntimeTableRefusal> {
        let r = Self::storage_reservation(maximum_entries)?;
        if r.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || r.retained_array_bytes_bound > maximum_retained_array_bytes
        {
            return Err(RuntimeTableRefusal::Capacity);
        }
        Ok((
            Self {
                entries: Vec::with_capacity(maximum_entries),
                maximum_entries,
            },
            r,
        ))
    }
    pub fn try_insert(&mut self, key: K, value: V) -> Result<(), (RuntimeTableRefusal, K, V)> {
        let position = match self.entries.binary_search_by(|(k, _)| k.cmp(&key)) {
            Ok(_) => return Err((RuntimeTableRefusal::Duplicate, key, value)),
            Err(p) => p,
        };
        if self.entries.len() == self.maximum_entries {
            return Err((RuntimeTableRefusal::Capacity, key, value));
        }
        self.entries.insert(position, (key, value));
        Ok(())
    }
    pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.entries
            .binary_search_by(|(k, _)| k.borrow().cmp(key))
            .ok()
            .map(|i| &self.entries[i].1)
    }
    pub fn get_mut<Q: Ord + ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
    {
        self.entries
            .binary_search_by(|(k, _)| k.borrow().cmp(key))
            .ok()
            .map(|i| &mut self.entries[i].1)
    }
    pub fn contains_key<Q: Ord + ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.get(key).is_some()
    }
    pub fn remove<Q: Ord + ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        self.entries
            .binary_search_by(|(k, _)| k.borrow().cmp(key))
            .ok()
            .map(|i| self.entries.remove(i).1)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn array_capacity_bytes(&self) -> Result<usize, RuntimeTableRefusal> {
        self.entries
            .capacity()
            .checked_mul(size_of::<(K, V)>())
            .ok_or(RuntimeTableRefusal::Capacity)
    }
}
