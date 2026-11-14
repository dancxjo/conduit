//! Private checked budgets and infallible fixed-sign field initialization.
use super::*;
pub(crate) struct FixedSignBudgets {
    pub(super) byte_capacity: u32,
    pub(super) remote_item_capacity: u16,
    pub(super) remote_byte_capacity: u32,
}
impl FixedSignBudgets {
    pub(crate) fn checked<const EVENTS: usize>(
        byte_capacity: u32,
        remote_item_capacity: u16,
        remote_byte_capacity: u32,
    ) -> Result<Self, SignError> {
        let physical_bytes = EVENTS
            .checked_mul(size_of::<KernelEvent>())
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(SignError::InvalidBudget)?;
        let remote_physical_bytes =
            remote_sign_storage_bytes(remote_item_capacity).ok_or(SignError::InvalidBudget)?;
        if EVENTS == 0
            || EVENTS > usize::from(u16::MAX)
            || byte_capacity == 0
            || byte_capacity > physical_bytes
            || usize::from(remote_item_capacity) > EVENTS
            || (remote_item_capacity == 0 && remote_byte_capacity != 0)
            || (remote_item_capacity != 0
                && (remote_byte_capacity == 0 || remote_byte_capacity > remote_physical_bytes))
        {
            return Err(SignError::InvalidBudget);
        }
        Ok(Self {
            byte_capacity,
            remote_item_capacity,
            remote_byte_capacity,
        })
    }
    /// SAFETY: destination is uniquely owned, aligned, writable storage for the
    /// exact log type, not yet initialized. Budgets were checked for EVENTS.
    /// No user code or fallible operation runs while fields are initialized.
    #[cfg(feature = "alloc")]
    pub(crate) unsafe fn initialize<const EVENTS: usize>(
        self,
        destination: *mut FixedSignLog<EVENTS>,
    ) {
        let _exhaustive = |log: &FixedSignLog<EVENTS>| {
            let FixedSignLog {
                entries: _,
                len: _,
                retained_items: _,
                byte_capacity: _,
                used_bytes: _,
                next_sequence: _,
                remote_entries: _,
                remote_item_capacity: _,
                remote_byte_capacity: _,
                remote_len: _,
                remote_used_bytes: _,
                retention_gap: _,
            } = log;
        };
        // SAFETY: raw field addresses form no references to the incomplete log.
        // Write each array element directly, avoiding a full inline log temporary.
        unsafe {
            let entries =
                core::ptr::addr_of_mut!((*destination).entries).cast::<Option<KernelEvent>>();
            let remote = core::ptr::addr_of_mut!((*destination).remote_entries)
                .cast::<Option<crate::remote_sign::RemoteLifecycleSign>>();
            for index in 0..EVENTS {
                entries.add(index).write(None);
                remote.add(index).write(None);
            }
            core::ptr::addr_of_mut!((*destination).len).write(0);
            core::ptr::addr_of_mut!((*destination).retained_items).write(0);
            core::ptr::addr_of_mut!((*destination).byte_capacity).write(self.byte_capacity);
            core::ptr::addr_of_mut!((*destination).used_bytes).write(0);
            core::ptr::addr_of_mut!((*destination).next_sequence).write(0);
            core::ptr::addr_of_mut!((*destination).remote_item_capacity)
                .write(self.remote_item_capacity);
            core::ptr::addr_of_mut!((*destination).remote_byte_capacity)
                .write(self.remote_byte_capacity);
            core::ptr::addr_of_mut!((*destination).remote_len).write(0);
            core::ptr::addr_of_mut!((*destination).remote_used_bytes).write(0);
            core::ptr::addr_of_mut!((*destination).retention_gap).write(None);
        }
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    #[test]
    fn field_initialized_log_matches_original_under_local_remote_and_retention_pressure() {
        for items in [1, 3, 64] {
            for remote_items in [0, 2, 12] {
                let bytes = items * size_of::<KernelEvent>() as u32;
                let remote_bytes = remote_sign_storage_bytes(remote_items).unwrap();
                let mut original =
                    FixedSignLog::<64>::new_with_remote_storage(bytes, remote_items, remote_bytes)
                        .unwrap();
                let budgets =
                    FixedSignBudgets::checked::<64>(bytes, remote_items, remote_bytes).unwrap();
                let mut target = core::mem::MaybeUninit::<FixedSignLog<64>>::uninit();
                // SAFETY: exact uniquely owned uninitialized destination and
                // budgets checked for the same 64-event physical log.
                let mut actual = unsafe {
                    budgets.initialize::<64>(target.as_mut_ptr());
                    target.assume_init()
                };
                for index in 0..200 {
                    assert_eq!(original.ensure_capacity(2), actual.ensure_capacity(2));
                    assert_eq!(
                        original.ensure_remote_capacity(2),
                        actual.ensure_remote_capacity(2)
                    );
                    let kind = if index % 11 == 0 {
                        KernelEventKind::BackCompleted
                    } else {
                        KernelEventKind::ValueStored
                    };
                    assert_eq!(
                        original.record(NodeId(0), Some(PortId(1)), None, kind),
                        actual.record(NodeId(0), Some(PortId(1)), None, kind)
                    );
                    if index % 7 == 0 {
                        let identity = RemoteLifecycleIdentity {
                            endpoint: RemoteEndpointId(0),
                            cord: CordId(0),
                            direction: RemoteCordDirection::Egress,
                            sequence: index,
                        };
                        assert_eq!(
                            original.record_remote(
                                NodeId(0),
                                PortId(1),
                                KernelEventKind::RemoteValueOffered,
                                identity
                            ),
                            actual.record_remote(
                                NodeId(0),
                                PortId(1),
                                KernelEventKind::RemoteValueOffered,
                                identity
                            )
                        );
                    }
                    assert_eq!(original.entries, actual.entries);
                    assert_eq!(original.remote_entries, actual.remote_entries);
                    assert_eq!(
                        (
                            original.len,
                            original.retained_items,
                            original.used_bytes,
                            original.next_sequence,
                            original.remote_len,
                            original.remote_used_bytes,
                            original.retention_gap
                        ),
                        (
                            actual.len,
                            actual.retained_items,
                            actual.used_bytes,
                            actual.next_sequence,
                            actual.remote_len,
                            actual.remote_used_bytes,
                            actual.retention_gap
                        )
                    );
                }
            }
        }
    }
}
