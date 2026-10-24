//! Actual retained backing capacities of the hosted Sign log.
impl crate::HostedSignLog {
    /// Canonical retained backing before allocation; constructor validity and
    /// logical Sign budgets remain separate checks.
    pub fn canonical_storage_bytes(items: u16, remote_items: u16) -> Option<usize> {
        usize::from(items)
            .checked_mul(core::mem::size_of::<Option<crate::KernelEvent>>())?
            .checked_add(
                usize::from(remote_items)
                    .checked_mul(core::mem::size_of::<Option<crate::RemoteLifecycleSign>>())?,
            )
    }

    pub fn reserved_storage_bytes(&self) -> usize {
        self.entries.capacity() * core::mem::size_of::<Option<crate::KernelEvent>>()
            + self.remote_entries.capacity()
                * core::mem::size_of::<Option<crate::RemoteLifecycleSign>>()
    }
}

#[cfg(test)]
mod admission_tests {
    #[test]
    fn sign_backing_estimate_matches_prepared_retained_capacity() {
        let signs = crate::HostedSignLog::new_with_remote_storage(
            8,
            8 * core::mem::size_of::<crate::KernelEvent>() as u32,
            4,
            crate::remote_sign_storage_bytes(4).unwrap(),
        )
        .unwrap();
        assert_eq!(
            crate::HostedSignLog::canonical_storage_bytes(8, 4),
            Some(signs.reserved_storage_bytes())
        );
    }
}
