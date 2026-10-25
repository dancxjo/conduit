use super::{sign, BodyLifecycleRealization, BodyLifecycleSession, BodyLifecycleSessionError};
use crate::{
    BodyLifecycleOwner, BodyMaskTopology, BodyPlan, BodyPlanEvidence, BodyPlanSeal, BodyPlotPlan,
    BodyState, SealedBodyLifecycleSession,
};
use alloc::vec::Vec;
use conduit_core::{BootId, HostId};

impl BodyLifecycleSession {
    pub fn propose(
        &mut self,
        plots: Vec<BodyPlotPlan>,
        host: &HostId,
        boot: &BootId,
    ) -> Result<&BodyLifecycleRealization, BodyLifecycleSessionError> {
        self.propose_with_masks(plots, Vec::new(), host, boot)
    }
    pub fn propose_with_masks(
        &mut self,
        plots: Vec<BodyPlotPlan>,
        masks: Vec<BodyMaskTopology>,
        host: &HostId,
        boot: &BootId,
    ) -> Result<&BodyLifecycleRealization, BodyLifecycleSessionError> {
        self.propose_using(plots, masks, host, boot, Ok)
    }
}
impl SealedBodyLifecycleSession {
    pub fn propose(
        &mut self,
        plots: Vec<BodyPlotPlan>,
        host: &HostId,
        boot: &BootId,
    ) -> Result<&BodyLifecycleRealization<BodyPlanSeal>, BodyLifecycleSessionError> {
        self.propose_with_masks(plots, Vec::new(), host, boot)
    }
    pub fn propose_with_masks(
        &mut self,
        plots: Vec<BodyPlotPlan>,
        masks: Vec<BodyMaskTopology>,
        host: &HostId,
        boot: &BootId,
    ) -> Result<&BodyLifecycleRealization<BodyPlanSeal>, BodyLifecycleSessionError> {
        self.propose_using(plots, masks, host, boot, |plan| {
            BodyPlanSeal::capture(&plan).map_err(BodyLifecycleSessionError::PlanSeal)
        })
    }
}
impl<P: BodyPlanEvidence> BodyLifecycleOwner<P> {
    /// One transaction preserves exact membership, Wake, workset and archive
    /// semantics. Only the immutable retained Plan evidence differs.
    fn propose_using(
        &mut self,
        plots: Vec<BodyPlotPlan>,
        masks: Vec<BodyMaskTopology>,
        host: &HostId,
        boot: &BootId,
        retain: impl FnOnce(BodyPlan) -> Result<P, BodyLifecycleSessionError>,
    ) -> Result<&BodyLifecycleRealization<P>, BodyLifecycleSessionError> {
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
        let plan = retain(plan)?;
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
