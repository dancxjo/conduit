//! Parent-correlated child evidence; unused future reservations are not Plays.
use super::*;
use crate::ScanChildSignReceipt;
use conduit_kernel::SignQuery;

impl BoundedActivationHost {
    pub fn child_sign_receipts(&self) -> Result<Vec<ScanChildSignReceipt>, BoundedActivationError> {
        let parent = self
            .parent_play
            .as_ref()
            .ok_or(BoundedActivationError::InvalidLifecycle)?;
        let activation = self
            .activation_id
            .as_deref()
            .ok_or(BoundedActivationError::InvalidLifecycle)?;
        let mut receipts = Vec::new();
        let mut offset = 0;
        if let Some(pool) = &self.reusable {
            for evidence in &pool.evidence {
                let invocation = u16::try_from(offset)
                    .map_err(|_| BoundedActivationError::PlannedContractMismatch)?;
                if evidence.parent_play_binding() != (parent, invocation)
                    || evidence.activation_id() != activation
                    || evidence.definition().internal_plan.plan_id != self.contract.selected_plan_id
                {
                    return Err(BoundedActivationError::PlannedContractMismatch);
                }
                for (host, play) in evidence.active_plays() {
                    receipts.push(ScanChildSignReceipt {
                        parent_active_play_id: parent.clone(),
                        activation_id: activation.into(),
                        selected_plan_id: self.contract.selected_plan_id.clone(),
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
                offset += 1;
            }
        }
        for child in self
            .receipts
            .iter()
            .chain(self.active.iter())
            .chain(self.reusable.iter().filter_map(|pool| pool.drained()))
        {
            let invocation = u16::try_from(offset)
                .map_err(|_| BoundedActivationError::PlannedContractMismatch)?;
            if child.parent_play_binding() != Some((parent, invocation))
                || child.definition().internal_plan.plan_id != self.contract.selected_plan_id
            {
                return Err(BoundedActivationError::PlannedContractMismatch);
            }
            let mut remote = child.remote_signs();
            for (host, events) in child.signs() {
                let play = child
                    .active_plays()
                    .get(&host)
                    .ok_or(BoundedActivationError::PlannedContractMismatch)?;
                receipts.push(ScanChildSignReceipt {
                    parent_active_play_id: parent.clone(),
                    activation_id: activation.into(),
                    selected_plan_id: self.contract.selected_plan_id.clone(),
                    invocation,
                    child_host_id: host.clone(),
                    child_active_play_id: play.clone(),
                    events,
                    remote_lifecycle: remote.remove(&host).unwrap_or_default(),
                });
            }
            offset += 1;
        }
        Ok(receipts)
    }
}
