//! Parent-Play binding and receipt-correlated scan child Signs.
use super::*;
use conduit_core::{HostId, PlanId};

/// One exact child kernel's retained Signs under the admitted parent Play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanChildSignReceipt {
    pub parent_active_play_id: ActivePlayId,
    pub selected_plan_id: PlanId,
    pub invocation: u16,
    pub child_host_id: HostId,
    pub child_active_play_id: ActivePlayId,
    pub events: Vec<conduit_kernel::KernelEvent>,
}

impl BoundedScanActivationHost {
    pub fn signs(
        &self,
    ) -> alloc::collections::BTreeMap<conduit_core::HostId, Vec<conduit_kernel::KernelEvent>> {
        let mut signs = alloc::collections::BTreeMap::new();
        for receipt in self.receipts.iter().chain(self.active.iter()) {
            for (host, events) in receipt.signs() {
                signs.entry(host).or_insert_with(Vec::new).extend(events);
            }
        }
        signs
    }
    /// Bind all finite prepared invocations before the first child starts.
    /// The ready pool is popped from the end, so identities follow acceptance
    /// order rather than allocation order.
    pub fn bind_parent_play(&mut self, parent: &ActivePlayId) -> Result<(), BoundedScanError> {
        if self.state != BoundedScanState::Idle
            || self.closing
            || self.queued_ready
            || self.output_ready
            || self.parent_play.is_some()
            || self.active.is_some()
            || !self.receipts.is_empty()
            || self.ready.len() != usize::from(self.planned.limits.maximum_items)
        {
            return Err(BoundedScanError::InvalidLifecycle);
        }
        for (index, child) in self.ready.iter_mut().rev().enumerate() {
            let invocation =
                u16::try_from(index).map_err(|_| BoundedScanError::PlannedContractMismatch)?;
            child
                .bind_parent_play(parent, &self.planned.activation_id, invocation)
                .map_err(BoundedScanError::Refused)?;
        }
        self.parent_play = Some(parent.clone());
        Ok(())
    }

    pub fn child_sign_receipts(&self) -> Result<Vec<ScanChildSignReceipt>, BoundedScanError> {
        let parent = self
            .parent_play
            .as_ref()
            .ok_or(BoundedScanError::InvalidLifecycle)?;
        let mut result = Vec::new();
        for (index, child) in self.receipts.iter().chain(self.active.iter()).enumerate() {
            let invocation =
                u16::try_from(index).map_err(|_| BoundedScanError::PlannedContractMismatch)?;
            if child.parent_play_binding() != Some((parent, invocation)) {
                return Err(BoundedScanError::PlannedContractMismatch);
            }
            for (host, events) in child.signs() {
                let child_active_play_id = child
                    .active_plays()
                    .get(&host)
                    .ok_or(BoundedScanError::PlannedContractMismatch)?
                    .clone();
                result.push(ScanChildSignReceipt {
                    parent_active_play_id: parent.clone(),
                    selected_plan_id: self.planned.selected_plan_id.clone(),
                    invocation,
                    child_host_id: host,
                    child_active_play_id,
                    events,
                });
            }
        }
        Ok(result)
    }
}
