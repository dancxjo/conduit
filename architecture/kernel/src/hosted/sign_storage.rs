//! Actual retained backing capacities of the hosted Sign log.
impl crate::HostedSignLog {
    pub fn reserved_storage_bytes(&self) -> usize {
        self.entries.capacity() * core::mem::size_of::<Option<crate::KernelEvent>>()
            + self.remote_entries.capacity()
                * core::mem::size_of::<Option<crate::RemoteLifecycleSign>>()
    }
}
