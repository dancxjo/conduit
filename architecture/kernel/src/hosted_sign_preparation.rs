//! Hosted Sign storage is reserved before execution. Allocation refusal is
//! distinct from a malformed finite Sign budget and publishes no evidence.
use super::*;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedSignPreparationError {
    Budget(SignError),
    Allocation,
}

fn validate_budget(
    item_capacity: u16,
    byte_capacity: u32,
    remote_item_capacity: u16,
    remote_byte_capacity: u32,
) -> Result<(), SignError> {
    let physical_bytes = usize::from(item_capacity)
        .checked_mul(size_of::<KernelEvent>())
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(SignError::InvalidBudget)?;
    let remote_physical_bytes =
        remote_sign_storage_bytes(remote_item_capacity).ok_or(SignError::InvalidBudget)?;
    if item_capacity == 0
        || byte_capacity == 0
        || byte_capacity > physical_bytes
        || remote_item_capacity > item_capacity
        || (remote_item_capacity == 0 && remote_byte_capacity != 0)
        || (remote_item_capacity != 0
            && (remote_byte_capacity == 0 || remote_byte_capacity > remote_physical_bytes))
    {
        return Err(SignError::InvalidBudget);
    }
    Ok(())
}

impl HostedSignLog {
    pub fn new(item_capacity: u16, byte_capacity: u32) -> Result<Self, SignError> {
        Self::new_with_remote_storage(item_capacity, byte_capacity, 0, 0)
    }

    pub fn new_with_remote_storage(
        item_capacity: u16,
        byte_capacity: u32,
        remote_item_capacity: u16,
        remote_byte_capacity: u32,
    ) -> Result<Self, SignError> {
        validate_budget(
            item_capacity,
            byte_capacity,
            remote_item_capacity,
            remote_byte_capacity,
        )?;
        let mut entries = alloc::vec::Vec::with_capacity(usize::from(item_capacity));
        entries.resize(usize::from(item_capacity), None);
        let mut remote_entries = alloc::vec::Vec::with_capacity(usize::from(remote_item_capacity));
        remote_entries.resize(usize::from(remote_item_capacity), None);
        Ok(Self {
            entries,
            len: 0,
            retained_items: 0,
            byte_capacity,
            used_bytes: 0,
            next_sequence: 0,
            remote_entries,
            remote_item_capacity,
            remote_byte_capacity,
            remote_len: 0,
            remote_used_bytes: 0,
            retention_gap: None,
        })
    }

    /// Reserve fresh finite storage without invoking the allocation error
    /// handler. Failure leaves no log or partial reservation in the caller.
    pub fn try_new(
        item_capacity: u16,
        byte_capacity: u32,
    ) -> Result<Self, HostedSignPreparationError> {
        Self::try_new_with_remote_storage(item_capacity, byte_capacity, 0, 0)
    }

    pub fn try_new_with_remote_storage(
        item_capacity: u16,
        byte_capacity: u32,
        remote_item_capacity: u16,
        remote_byte_capacity: u32,
    ) -> Result<Self, HostedSignPreparationError> {
        validate_budget(
            item_capacity,
            byte_capacity,
            remote_item_capacity,
            remote_byte_capacity,
        )
        .map_err(HostedSignPreparationError::Budget)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(usize::from(item_capacity))
            .map_err(|_| HostedSignPreparationError::Allocation)?;
        let mut remote_entries = Vec::new();
        remote_entries
            .try_reserve_exact(usize::from(remote_item_capacity))
            .map_err(|_| HostedSignPreparationError::Allocation)?;
        entries.resize(usize::from(item_capacity), None);
        remote_entries.resize(usize::from(remote_item_capacity), None);
        Ok(Self {
            entries,
            len: 0,
            retained_items: 0,
            byte_capacity,
            used_bytes: 0,
            next_sequence: 0,
            remote_entries,
            remote_item_capacity,
            remote_byte_capacity,
            remote_len: 0,
            remote_used_bytes: 0,
            retention_gap: None,
        })
    }
}
