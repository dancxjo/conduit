//! Actual retained backing capacities of the hosted Sign log.
impl crate::HostedSignLog {
    /// Required backing before allocation, including each optional entry's
    /// representation. Byte-retention admission remains the constructor's gate.
    pub const fn canonical_storage_bytes(items: u16, remote_items: u16) -> Option<usize> {
        if items == 0 || remote_items > items {
            return None;
        }
        let Some(local) = (items as usize).checked_mul(core::mem::size_of::<
            Option<crate::packed_sign::PackedKernelEvent>,
        >()) else {
            return None;
        };
        let Some(remote) = (remote_items as usize)
            .checked_mul(core::mem::size_of::<Option<crate::RemoteLifecycleSign>>())
        else {
            return None;
        };
        local.checked_add(remote)
    }
    /// Declared retention capacities, distinct from packed allocation backing.
    /// Use both bounds when selecting a finite Host-funded evidence profile.
    pub const fn canonical_logical_storage_bytes(items: u16, remote_items: u16) -> Option<usize> {
        if items == 0 || remote_items > items {
            return None;
        }
        let Some(local) = (items as usize).checked_mul(core::mem::size_of::<crate::KernelEvent>())
        else {
            return None;
        };
        let Some(remote) =
            (remote_items as usize).checked_mul(core::mem::size_of::<crate::RemoteLifecycleSign>())
        else {
            return None;
        };
        local.checked_add(remote)
    }
    pub fn reserved_storage_bytes(&self) -> usize {
        self.entries.capacity()
            * core::mem::size_of::<Option<crate::packed_sign::PackedKernelEvent>>()
            + self.remote_entries.capacity()
                * core::mem::size_of::<Option<crate::RemoteLifecycleSign>>()
    }
}

#[cfg(test)]
mod tests {
    use crate::HostedSignLog;
    #[test]
    fn preparation_size_matches_both_allocated_entry_backings() {
        for (items, remote) in [(1, 0), (4, 2), (17, 17)] {
            let log = HostedSignLog::new_with_remote_storage(
                items,
                1,
                remote,
                crate::remote_sign_storage_bytes(remote).unwrap(),
            )
            .unwrap();
            assert_eq!(
                HostedSignLog::canonical_storage_bytes(items, remote),
                Some(log.reserved_storage_bytes())
            );
        }
        assert_eq!(HostedSignLog::canonical_storage_bytes(0, 0), None);
        assert_eq!(HostedSignLog::canonical_storage_bytes(1, 2), None);
    }
    #[test]
    fn const_logical_capacity_matches_declared_constructor_budgets() {
        const LOGICAL: Option<usize> = HostedSignLog::canonical_logical_storage_bytes(61, 6);
        let log = HostedSignLog::new_with_remote_storage(
            61,
            (61 * core::mem::size_of::<crate::KernelEvent>()) as u32,
            6,
            crate::remote_sign_storage_bytes(6).unwrap(),
        )
        .unwrap();
        assert_eq!(
            LOGICAL,
            Some(log.byte_capacity as usize + log.remote_byte_capacity as usize)
        );
        assert_eq!(HostedSignLog::canonical_logical_storage_bytes(0, 0), None);
        assert_eq!(HostedSignLog::canonical_logical_storage_bytes(1, 2), None);
    }
}
