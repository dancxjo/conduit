//! Scan controller ownership helpers; scheduling stays in the existing kernel.
use super::*;
impl BoundedScanActivationHost {
    pub(super) fn remaining_children(&self) -> usize {
        self.reusable
            .as_ref()
            .map_or(self.ready.len(), |pool| pool.remaining())
    }
    pub(super) fn retire_child(&mut self, host: KernelCompositeHost) {
        if let Some(pool) = &mut self.reusable {
            pool.retire(host);
        } else {
            self.receipts.push(host);
        }
    }
    pub(super) fn observed_child(&self) -> Option<&KernelCompositeHost> {
        self.active
            .as_ref()
            .or_else(|| self.reusable.as_ref().and_then(|pool| pool.host()))
            .or_else(|| self.receipts.last())
            .or_else(|| self.ready.last())
    }
}

impl From<crate::reusable_value_pool::ValueReuseError> for BoundedScanError {
    fn from(error: crate::reusable_value_pool::ValueReuseError) -> Self {
        use crate::reusable_value_pool::ValueReuseError;
        match error {
            ValueReuseError::InvalidBound => Self::PlannedContractMismatch,
            ValueReuseError::InvalidLifecycle => Self::InvalidLifecycle,
            ValueReuseError::Child(error) => Self::Refused(error),
        }
    }
}
