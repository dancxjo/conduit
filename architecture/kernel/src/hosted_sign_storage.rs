//! Exact full slot-array reservation for the hosted finite sign sink.
//! Logical sign byte limits do not describe allocation size; vacant Option slots
//! and both local/remote arrays are charged in full. Root/allocator/stack separate.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostedSignLogStorageReceipt {
    pub preparation_requested_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostedSignLogPreparationRefusal {
    Capacity,
    Budget(SignError),
}
impl HostedSignLog {
    pub fn storage_reservation(
        item_capacity: u16,
        byte_capacity: u32,
        remote_item_capacity: u16,
        remote_byte_capacity: u32,
    ) -> Result<HostedSignLogStorageReceipt, HostedSignLogPreparationRefusal> {
        let physical_bytes = usize::from(item_capacity)
            .checked_mul(size_of::<KernelEvent>())
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(HostedSignLogPreparationRefusal::Budget(
                SignError::InvalidBudget,
            ))?;
        let remote_physical_bytes = remote_sign_storage_bytes(remote_item_capacity).ok_or(
            HostedSignLogPreparationRefusal::Budget(SignError::InvalidBudget),
        )?;
        if item_capacity == 0
            || byte_capacity == 0
            || byte_capacity > physical_bytes
            || remote_item_capacity > item_capacity
            || (remote_item_capacity == 0 && remote_byte_capacity != 0)
            || (remote_item_capacity != 0
                && (remote_byte_capacity == 0 || remote_byte_capacity > remote_physical_bytes))
        {
            return Err(HostedSignLogPreparationRefusal::Budget(
                SignError::InvalidBudget,
            ));
        }
        let bytes = usize::from(item_capacity)
            .checked_mul(size_of::<Option<KernelEvent>>())
            .and_then(|v| {
                v.checked_add(
                    usize::from(remote_item_capacity)
                        .checked_mul(size_of::<Option<RemoteLifecycleSign>>())?,
                )
            })
            .ok_or(HostedSignLogPreparationRefusal::Capacity)?;
        Ok(HostedSignLogStorageReceipt {
            preparation_requested_bytes_bound: bytes,
            retained_heap_bytes_bound: bytes,
        })
    }
    pub fn new_with_storage_limits(
        item_capacity: u16,
        byte_capacity: u32,
        remote_item_capacity: u16,
        remote_byte_capacity: u32,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_heap_bytes: usize,
    ) -> Result<(Self, HostedSignLogStorageReceipt), HostedSignLogPreparationRefusal> {
        let receipt = Self::storage_reservation(
            item_capacity,
            byte_capacity,
            remote_item_capacity,
            remote_byte_capacity,
        )?;
        if receipt.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || receipt.retained_heap_bytes_bound > maximum_retained_heap_bytes
        {
            return Err(HostedSignLogPreparationRefusal::Capacity);
        }
        let owner = Self::new_with_remote_storage(
            item_capacity,
            byte_capacity,
            remote_item_capacity,
            remote_byte_capacity,
        )
        .map_err(HostedSignLogPreparationRefusal::Budget)?;
        Ok((owner, receipt))
    }
    pub fn owned_heap_bytes(&self) -> usize {
        self.entries
            .capacity()
            .saturating_mul(size_of::<Option<KernelEvent>>())
            .saturating_add(
                self.remote_entries
                    .capacity()
                    .saturating_mul(size_of::<Option<RemoteLifecycleSign>>()),
            )
    }
}
