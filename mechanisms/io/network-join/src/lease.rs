//! A cancelled dispatch cannot replay under its unfinished capability.
use conduit_core::*;
pub(super) struct Pending<'a> {
    table: &'a mut BaseCapabilityTable,
    handle: &'a mut BaseCapabilityHandle,
    lease: Option<BaseOperationLease>,
}
impl<'a> Pending<'a> {
    pub fn new(
        table: &'a mut BaseCapabilityTable,
        handle: &'a mut BaseCapabilityHandle,
        lease: BaseOperationLease,
    ) -> Self {
        Self {
            table,
            handle,
            lease: Some(lease),
        }
    }
    pub fn complete(mut self, bytes: u32) -> Result<(), BaseCapabilityRefusal> {
        self.table.complete(
            self.handle,
            self.lease.as_ref().expect("pending").clone(),
            bytes,
        )?;
        self.lease = None;
        Ok(())
    }
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if self.lease.is_some() {
            let _ = self.table.revoke(self.handle);
        }
    }
}
