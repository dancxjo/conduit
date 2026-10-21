//! Bounded pre-Play value storage using the production kernel store.
use super::PreparedExecutionRegions;
use alloc::vec::Vec;
use conduit_kernel::{HostedValueStore, StorageError};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RegionValueStorageLimits {
    pub maximum_retained_bytes: usize,
    /// Retained storage plus the largest temporary slot-capacity vector.
    /// These ceilings count backing capacities, excluding allocator overhead
    /// and borrowed Plan/preparation data.
    pub maximum_preparation_bytes: usize,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RegionValueStorageError {
    CapacityExceeded,
    UnknownRegion,
    Storage(StorageError),
}

/// Value-slot backing only. The borrowed preparation retains Plan truth;
/// scheduler queues, Signs, Back state, worker stacks and scratch must still
/// be admitted separately. This is not a worker or a resource reservation.
pub struct PreparedRegionValueStorage<'prepared, 'plan> {
    prepared: &'prepared PreparedExecutionRegions<'plan>,
    stores: Vec<Option<HostedValueStore>>,
    retained_bytes: usize,
    preparation_bytes_upper_bound: usize,
}
impl<'plan> PreparedExecutionRegions<'plan> {
    pub fn prepare_value_storage<'prepared>(
        &'prepared self,
        limits: RegionValueStorageLimits,
    ) -> Result<PreparedRegionValueStorage<'prepared, 'plan>, RegionValueStorageError> {
        use RegionValueStorageError::{CapacityExceeded, Storage};
        let inline = core::mem::size_of::<PreparedRegionValueStorage<'_, '_>>();
        let vector_bytes = self
            .regions()
            .len()
            .checked_mul(core::mem::size_of::<Option<HostedValueStore>>())
            .ok_or(CapacityExceeded)?;
        let mut projected = inline.checked_add(vector_bytes).ok_or(CapacityExceeded)?;
        let mut scratch_bytes = 0usize;
        // Preflight all physical payload and metadata before allocating any
        // value store. One queue's live byte quota is not its physical shape.
        for region in self.regions() {
            let cords = || {
                self.lowered()
                    .cords
                    .iter()
                    .filter(|c| region.cords().contains(&c.spec.cord))
            };
            let slots = cords()
                .try_fold(0usize, |n, c| {
                    n.checked_add(usize::from(c.spec.item_capacity))
                })
                .ok_or(CapacityExceeded)?;
            if slots == 0 {
                continue;
            }
            let capacities = cords().flat_map(|c| {
                core::iter::repeat_n(
                    c.spec.maximum_value_bytes,
                    usize::from(c.spec.item_capacity),
                )
            });
            let bytes = HostedValueStore::canonical_storage_bytes(capacities).map_err(Storage)?;
            projected = projected.checked_add(bytes).ok_or(CapacityExceeded)?;
            scratch_bytes = scratch_bytes.max(
                slots
                    .checked_mul(core::mem::size_of::<u32>())
                    .ok_or(CapacityExceeded)?,
            );
        }
        if projected > limits.maximum_retained_bytes
            || projected
                .checked_add(scratch_bytes)
                .ok_or(CapacityExceeded)?
                > limits.maximum_preparation_bytes
        {
            return Err(CapacityExceeded);
        }
        let mut stores = Vec::with_capacity(self.regions().len());
        let mut retained = inline
            .checked_add(
                stores
                    .capacity()
                    .checked_mul(core::mem::size_of::<Option<HostedValueStore>>())
                    .ok_or(CapacityExceeded)?,
            )
            .ok_or(CapacityExceeded)?;
        projected = projected
            .checked_sub(inline + vector_bytes)
            .and_then(|bytes| bytes.checked_add(retained))
            .ok_or(CapacityExceeded)?;
        for region in self.regions() {
            let slots = self
                .lowered()
                .cords
                .iter()
                .filter(|c| region.cords().contains(&c.spec.cord))
                .map(|c| usize::from(c.spec.item_capacity))
                .sum();
            let mut capacities = Vec::with_capacity(slots);
            capacities.extend(
                self.lowered()
                    .cords
                    .iter()
                    .filter(|c| region.cords().contains(&c.spec.cord))
                    .flat_map(|c| {
                        core::iter::repeat_n(
                            c.spec.maximum_value_bytes,
                            usize::from(c.spec.item_capacity),
                        )
                    }),
            );
            if capacities.is_empty() {
                stores.push(None);
                continue;
            }
            let scratch = capacities
                .capacity()
                .checked_mul(core::mem::size_of::<u32>())
                .ok_or(CapacityExceeded)?;
            scratch_bytes = scratch_bytes.max(scratch);
            if projected
                .checked_add(scratch_bytes)
                .ok_or(CapacityExceeded)?
                > limits.maximum_preparation_bytes
            {
                return Err(CapacityExceeded);
            }
            let store = HostedValueStore::new_with_slot_capacities(
                &capacities,
                region.admitted().requirements.cord_byte_capacity,
            )
            .map_err(Storage)?;
            let expected = HostedValueStore::canonical_storage_bytes(capacities.iter().copied())
                .map_err(Storage)?;
            projected = projected
                .checked_sub(expected)
                .and_then(|bytes| bytes.checked_add(store.reserved_storage_bytes()))
                .ok_or(CapacityExceeded)?;
            retained = retained
                .checked_add(store.reserved_storage_bytes())
                .ok_or(CapacityExceeded)?;
            if projected > limits.maximum_retained_bytes
                || projected
                    .checked_add(scratch_bytes)
                    .ok_or(CapacityExceeded)?
                    > limits.maximum_preparation_bytes
            {
                return Err(CapacityExceeded);
            }
            stores.push(Some(store));
        }
        let preparation_bytes_upper_bound = retained
            .checked_add(scratch_bytes)
            .ok_or(CapacityExceeded)?;
        if preparation_bytes_upper_bound > limits.maximum_preparation_bytes {
            return Err(CapacityExceeded);
        }
        Ok(PreparedRegionValueStorage {
            prepared: self,
            stores,
            retained_bytes: retained,
            preparation_bytes_upper_bound,
        })
    }
}
impl<'prepared, 'plan> PreparedRegionValueStorage<'prepared, 'plan> {
    pub fn prepared(&self) -> &'prepared PreparedExecutionRegions<'plan> {
        self.prepared
    }
    pub fn region_mut(
        &mut self,
        index: usize,
    ) -> Result<Option<&mut HostedValueStore>, RegionValueStorageError> {
        self.stores
            .get_mut(index)
            .map(Option::as_mut)
            .ok_or(RegionValueStorageError::UnknownRegion)
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn preparation_bytes_upper_bound(&self) -> usize {
        self.preparation_bytes_upper_bound
    }
}
