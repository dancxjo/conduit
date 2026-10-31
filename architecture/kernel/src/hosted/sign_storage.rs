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
    /// Alignment of the actual local entry backing, for allocation diagnostics.
    /// This describes storage only and grants no Sign retention capacity.
    pub const fn canonical_local_storage_alignment() -> usize {
        core::mem::align_of::<Option<crate::packed_sign::PackedKernelEvent>>()
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
    fn local_alignment_matches_actual_entry_layout() {
        assert_eq!(
            HostedSignLog::canonical_local_storage_alignment(),
            core::alloc::Layout::array::<Option<crate::packed_sign::PackedKernelEvent>>(128)
                .unwrap()
                .align()
        );
    }
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
}
