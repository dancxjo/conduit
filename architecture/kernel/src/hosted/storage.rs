//! Exact requested payload capacities for the existing fixed hosted store.
use super::*;
use core::mem::size_of;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostedValueStoreStorageReceipt {
    pub item_capacity: u16,
    pub maximum_value_bytes: u32,
    pub logical_byte_capacity: u32,
    pub slot_array_requested_bytes: usize,
    pub slot_payload_requested_bytes: usize,
    pub preparation_requested_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostedValueStorePreparationRefusal {
    Capacity,
    Storage(StorageError),
}
impl HostedValueStore {
    /// Allocation-free full reservation. Every slot reserves its maximum even
    /// when logical byte_capacity is smaller. Root/allocator/stack excluded.
    pub fn storage_reservation(
        item_capacity: u16,
        maximum_value_bytes: u32,
        byte_capacity: u32,
    ) -> Result<HostedValueStoreStorageReceipt, HostedValueStorePreparationRefusal> {
        use HostedValueStorePreparationRefusal as Error;
        let maximum = validate_budget(item_capacity, maximum_value_bytes, byte_capacity)
            .map_err(Error::Storage)?;
        let slots = usize::from(item_capacity);
        let array = slots
            .checked_mul(size_of::<HostedValueSlot>())
            .ok_or(Error::Capacity)?;
        let payload = slots.checked_mul(maximum).ok_or(Error::Capacity)?;
        let total = array.checked_add(payload).ok_or(Error::Capacity)?;
        Ok(HostedValueStoreStorageReceipt {
            item_capacity,
            maximum_value_bytes,
            logical_byte_capacity: byte_capacity,
            slot_array_requested_bytes: array,
            slot_payload_requested_bytes: payload,
            preparation_requested_bytes_bound: total,
            retained_heap_bytes_bound: total,
        })
    }
    pub fn new_with_storage_limits(
        item_capacity: u16,
        maximum_value_bytes: u32,
        byte_capacity: u32,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_heap_bytes: usize,
    ) -> Result<(Self, HostedValueStoreStorageReceipt), HostedValueStorePreparationRefusal> {
        use HostedValueStorePreparationRefusal as Error;
        let r = Self::storage_reservation(item_capacity, maximum_value_bytes, byte_capacity)?;
        if r.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || r.retained_heap_bytes_bound > maximum_retained_heap_bytes
        {
            return Err(Error::Capacity);
        }
        Ok((
            Self::new(item_capacity, maximum_value_bytes, byte_capacity).map_err(Error::Storage)?,
            r,
        ))
    }
    /// Actual slot-array and all per-slot byte capacities, including spare space.
    pub fn owned_heap_bytes(&self) -> Result<usize, HostedValueStorePreparationRefusal> {
        use HostedValueStorePreparationRefusal as Error;
        self.slots.iter().try_fold(
            self.slots
                .capacity()
                .checked_mul(size_of::<HostedValueSlot>())
                .ok_or(Error::Capacity)?,
            |sum, slot| {
                sum.checked_add(slot.bytes.capacity())
                    .ok_or(Error::Capacity)
            },
        )
    }
}
