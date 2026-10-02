use super::{sign, BodyLifecycleRealization, BodyLifecycleSession, BodyLifecycleSessionError};
use crate::{BodyMaskTopology, BodyPlan, BodyPlotPlan, BodyState};
use alloc::vec::Vec;
use conduit_core::{BootId, HostId};

impl BodyLifecycleSession {
    /// Seal the complete workset before publishing a Wake. The host still
    /// prepares and starts the exact proposal before recording a Play.
    pub fn propose(
        &mut self,
        plots: Vec<BodyPlotPlan>,
        host: &HostId,
        boot: &BootId,
    ) -> Result<&BodyLifecycleRealization, BodyLifecycleSessionError> {
        self.propose_with_masks(plots, Vec::new(), host, boot)
    }

    /// Workload and Mask participants must all be exact current members.
    /// Refusal preserves evidence, archive obligations and realization together.
    pub fn propose_with_masks(
        &mut self,
        plots: Vec<BodyPlotPlan>,
        masks: Vec<BodyMaskTopology>,
        host: &HostId,
        boot: &BootId,
    ) -> Result<&BodyLifecycleRealization, BodyLifecycleSessionError> {
        self.require_mutable()?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(BodyLifecycleSessionError::NotLulled);
        }
        self.require_host(host, boot)?;
        for plan in plots.iter().map(|partition| &partition.plan).chain(
            masks
                .iter()
                .flat_map(|topology| topology.chains.iter().map(|chain| &chain.plan)),
        ) {
            for fragment in &plan.fragments {
                self.require_host(&fragment.host_id, &fragment.boot_id)?;
            }
        }
        let mut next = self.clone();
        // Reserve Woke and the eventual retained lull before publishing either.
        next.make_lifecycle_room(2, 1)?;
        let sequence = next.next_sequence()?;
        let (body, wake) = next
            .evidence
            .body
            .wake(sequence, sign(host, boot, sequence))
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let plan = BodyPlan::seal_with_masks(&wake, plots, masks)
            .map_err(BodyLifecycleSessionError::Plan)?;
        next.evidence
            .append_wake(body, wake.clone(), sequence)
            .map_err(BodyLifecycleSessionError::Biography)?;
        next.realization = Some(BodyLifecycleRealization {
            wake,
            plan,
            play: None,
        });
        *self = next;
        Ok(self.realization.as_ref().expect("published realization"))
    }
}
