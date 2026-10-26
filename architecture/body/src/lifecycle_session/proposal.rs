use super::{sign, BodyLifecycleRealization, BodyLifecycleSession, BodyLifecycleSessionError};
use crate::{
    BodyLifecycleOwner, BodyMaskTopology, BodyPlan, BodyPlanEvidence, BodyPlanSeal, BodyPlotPlan,
    BodyState, SealedBodyLifecycleSession, VerifiedBodyMaskTopology, VerifiedBodyPlotPlan,
};
use alloc::vec::Vec;
use conduit_core::{BodyTimeRequirement, BootId, HostId};

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
    /// Propose from opaque Core-verified preparation metadata without rebuilding
    /// executable configurations or a rich Core Plan. Membership and the full
    /// lifecycle transaction are identical to ordinary proposal; this issues no
    /// provider possession or permission to execute the resulting Plan.
    pub fn propose_verified(
        &mut self,
        plots: &[VerifiedBodyPlotPlan<'_>],
        masks: &[VerifiedBodyMaskTopology<'_>],
        body_time: Option<BodyTimeRequirement>,
        host: &HostId,
        boot: &BootId,
    ) -> Result<&BodyLifecycleRealization<BodyPlanSeal>, BodyLifecycleSessionError> {
        self.propose_from(
            host,
            boot,
            (plots, masks, body_time),
            |owner, (plots, masks, _)| {
                for plan in plots.iter().map(|plot| plot.plan).chain(
                    masks
                        .iter()
                        .flat_map(|mask| mask.chains.iter().map(|chain| chain.plan)),
                ) {
                    for fragment in &plan.preparation_metadata().fragments {
                        owner.require_host(&fragment.host_id, &fragment.boot_id)?;
                    }
                }
                Ok(())
            },
            |wake, (plots, masks, body_time)| {
                BodyPlanSeal::capture_verified(wake, plots, masks, body_time)
                    .map_err(BodyLifecycleSessionError::PlanSeal)
            },
        )
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
        self.propose_from(
            host,
            boot,
            (plots, masks),
            |owner, (plots, masks)| {
                for plan in plots.iter().map(|partition| &partition.plan).chain(
                    masks
                        .iter()
                        .flat_map(|topology| topology.chains.iter().map(|chain| &chain.plan)),
                ) {
                    for fragment in &plan.fragments {
                        owner.require_host(&fragment.host_id, &fragment.boot_id)?;
                    }
                }
                Ok(())
            },
            |wake, (plots, masks)| {
                let plan = BodyPlan::seal_with_masks(wake, plots, masks)
                    .map_err(BodyLifecycleSessionError::Plan)?;
                retain(plan)
            },
        )
    }
    fn propose_from<T>(
        &mut self,
        host: &HostId,
        boot: &BootId,
        input: T,
        validate: impl FnOnce(&Self, &T) -> Result<(), BodyLifecycleSessionError>,
        retain: impl FnOnce(&crate::Wake, T) -> Result<P, BodyLifecycleSessionError>,
    ) -> Result<&BodyLifecycleRealization<P>, BodyLifecycleSessionError> {
        self.require_mutable()?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(BodyLifecycleSessionError::NotLulled);
        }
        self.require_host(host, boot)?;
        validate(self, &input)?;
        let mut next = self.clone();
        // Reserve Woke and the eventual retained lull before publishing either.
        next.make_lifecycle_room(2, 1)?;
        let sequence = next.next_sequence()?;
        let prepared = next
            .evidence
            .body
            .prepare_wake(sequence, sign(host, boot, sequence))
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let plan = retain(prepared.wake(), input)?;
        let wake = next
            .evidence
            .append_prepared_wake_unpublished(prepared, sequence)
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
