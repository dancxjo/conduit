//! A cancelled dispatch cannot replay under its unfinished capability.
use conduit_core::*;
pub(super) struct Pending<'a> {
    operation: &'a mut BoundBaseOperation,
    lease: Option<BaseOperationLease>,
}
impl<'a> Pending<'a> {
    pub fn new(operation: &'a mut BoundBaseOperation, lease: BaseOperationLease) -> Self {
        Self {
            operation,
            lease: Some(lease),
        }
    }
    pub fn scope(&self) -> &BaseCapabilityScope {
        &self.operation.inspection().scope
    }
    pub fn complete(mut self, bytes: u32) -> Result<(), BaseCapabilityRefusal> {
        self.operation
            .complete(self.lease.as_ref().expect("pending").clone(), bytes)?;
        self.lease = None;
        Ok(())
    }
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if self.lease.is_some() {
            let _ = self.operation.revoke();
        }
    }
}
