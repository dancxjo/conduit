//! Parent-Play binding and receipt-correlated scan child Signs.
use super::*;
use conduit_core::{HostId, PlanId};

/// One exact child kernel's retained Signs under the admitted parent Play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanChildSignReceipt {
    pub parent_active_play_id: ActivePlayId,
    pub activation_id: String,
    pub selected_plan_id: PlanId,
    pub invocation: u16,
    pub child_host_id: HostId,
    pub child_active_play_id: ActivePlayId,
    pub events: Vec<conduit_kernel::KernelEvent>,
    pub remote_lifecycle: Vec<(u32, conduit_kernel::RemoteLifecycleIdentity)>,
}

impl BoundedScanActivationHost {
    pub fn signs(
        &self,
    ) -> alloc::collections::BTreeMap<conduit_core::HostId, Vec<conduit_kernel::KernelEvent>> {
        let mut signs = alloc::collections::BTreeMap::new();
        if let Some(pool) = &self.reusable {
            for evidence in &pool.evidence {
                for host in evidence.active_plays.keys() {
                    signs
                        .entry(host.clone())
                        .or_insert_with(Vec::new)
                        .extend(evidence.signs.events());
                }
            }
        }
        for receipt in self
            .receipts
            .iter()
            .chain(self.active.iter())
            .chain(self.reusable.iter().filter_map(|pool| pool.drained()))
        {
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
            || (self.reusable.is_none()
                && self.ready.len() != usize::from(self.planned.limits.maximum_items))
        {
            return Err(BoundedScanError::InvalidLifecycle);
        }
        if let Some(pool) = &mut self.reusable {
            pool.bind(parent, &self.planned.activation_id)?;
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
        if let Some(pool) = &self.reusable {
            use conduit_kernel::SignQuery;
            for (index, evidence) in pool.evidence.iter().enumerate() {
                let invocation =
                    u16::try_from(index).map_err(|_| BoundedScanError::PlannedContractMismatch)?;
                if evidence.parent_play_binding() != (parent, invocation)
                    || evidence.activation_id() != self.planned.activation_id
                    || evidence.definition().internal_plan.plan_id != self.planned.selected_plan_id
                {
                    return Err(BoundedScanError::PlannedContractMismatch);
                }
                for (host, play) in evidence.active_plays() {
                    result.push(ScanChildSignReceipt {
                        parent_active_play_id: parent.clone(),
                        activation_id: self.planned.activation_id.clone(),
                        selected_plan_id: self.planned.selected_plan_id.clone(),
                        invocation,
                        child_host_id: host.clone(),
                        child_active_play_id: play.clone(),
                        events: evidence.signs().events().collect(),
                        remote_lifecycle: evidence
                            .signs()
                            .events()
                            .filter_map(|event| {
                                evidence
                                    .signs()
                                    .remote_identity(event.sequence)
                                    .map(|identity| (event.sequence, identity))
                            })
                            .collect(),
                    });
                }
            }
        }
        let offset = self.reusable.as_ref().map_or(0, |pool| pool.evidence.len());
        for (index, child) in self
            .receipts
            .iter()
            .chain(self.active.iter())
            .chain(self.reusable.iter().filter_map(|pool| pool.drained()))
            .enumerate()
        {
            let invocation = u16::try_from(index + offset)
                .map_err(|_| BoundedScanError::PlannedContractMismatch)?;
            if child.parent_play_binding() != Some((parent, invocation)) {
                return Err(BoundedScanError::PlannedContractMismatch);
            }
            let mut remote = child.remote_signs();
            for (host, events) in child.signs() {
                let child_active_play_id = child
                    .active_plays()
                    .get(&host)
                    .ok_or(BoundedScanError::PlannedContractMismatch)?
                    .clone();
                result.push(ScanChildSignReceipt {
                    parent_active_play_id: parent.clone(),
                    activation_id: self.planned.activation_id.clone(),
                    selected_plan_id: self.planned.selected_plan_id.clone(),
                    invocation,
                    remote_lifecycle: remote.remove(&host).unwrap_or_default(),
                    child_host_id: host,
                    child_active_play_id,
                    events,
                });
            }
        }
        Ok(result)
    }
}
