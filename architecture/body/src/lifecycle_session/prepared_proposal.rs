//! Private preparation custody lets callers retire verified Plot metadata before
//! the atomic biography publication. This is not a Body start or execution grant.
use super::*;
use crate::{VerifiedBodyMaskTopology, VerifiedBodyPlotPlan};
use sha2::{Digest, Sha256};

pub struct PreparedVerifiedBodyProposal {
    expected: [u8; 32],
    sequence: u64,
    host: HostId,
    boot: BootId,
    wake: crate::lifecycle::PreparedBodyWake,
    plan: BodyPlanSeal,
}
/// Immutable cold preparation custody from one exact complete lifecycle state.
/// It cannot publish, deserialize, clone, admit a Host or grant execution.
pub struct VerifiedBodyProposalContext {
    expected: [u8; 32],
    sequence: u64,
    host: HostId,
    boot: BootId,
    wake: crate::lifecycle::PreparedBodyWake,
    admitted: alloc::vec::Vec<(HostId, BootId)>,
}
impl VerifiedBodyProposalContext {
    fn require_verified_hosts(
        &self,
        plan: &conduit_core::VerifiedPlanMetadata,
    ) -> Result<(), BodyLifecycleSessionError> {
        for fragment in &plan.preparation_metadata().fragments {
            if !self
                .admitted
                .iter()
                .any(|(host, boot)| host == &fragment.host_id && boot == &fragment.boot_id)
            {
                return Err(BodyLifecycleSessionError::StaleHost);
            }
        }
        Ok(())
    }
    /// Retain one verified Plot's exact Core fingerprint and finite Body-facing
    /// facts so its rich preparation metadata may retire before other planning.
    pub fn prepare_plot(
        &self,
        plot: &VerifiedBodyPlotPlan<'_>,
    ) -> Result<crate::PreparedVerifiedBodyPlotPlan, BodyLifecycleSessionError> {
        self.require_verified_hosts(plot.plan)?;
        if !self.wake.wake().workset.contains(&plot.plot) {
            return Err(BodyLifecycleSessionError::PlanSeal(
                crate::BodyPlanSealRefusal::Invalid(crate::BodyPlanError::UnexpectedPlot),
            ));
        }
        crate::PreparedVerifiedBodyPlotPlan::capture(self.expected, plot)
            .map_err(BodyLifecycleSessionError::PlanSeal)
    }
    /// Check every actual verified fragment against the captured membership,
    /// then seal the complete ordinary Body workset/Mask topology. Publication
    /// still requires the unchanged full-state fence on the original owner.
    pub fn prepare_verified(
        self,
        plots: &[VerifiedBodyPlotPlan<'_>],
        masks: &[VerifiedBodyMaskTopology<'_>],
        body_time: Option<conduit_core::BodyTimeRequirement>,
    ) -> Result<PreparedVerifiedBodyProposal, BodyLifecycleSessionError> {
        self.prepare_verified_with_prepared(&[], plots, masks, body_time)
    }
    pub fn prepare_verified_with_prepared(
        self,
        prepared: &[crate::PreparedVerifiedBodyPlotPlan],
        plots: &[VerifiedBodyPlotPlan<'_>],
        masks: &[VerifiedBodyMaskTopology<'_>],
        body_time: Option<conduit_core::BodyTimeRequirement>,
    ) -> Result<PreparedVerifiedBodyProposal, BodyLifecycleSessionError> {
        if prepared
            .iter()
            .any(|plot| !plot.matches_context(&self.expected))
        {
            return Err(BodyLifecycleSessionError::StaleWorkload);
        }
        for plan in plots
            .iter()
            .map(|p| p.plan)
            .chain(masks.iter().flat_map(|m| m.chains.iter().map(|c| c.plan)))
        {
            self.require_verified_hosts(plan)?;
        }
        let plan = BodyPlanSeal::capture_prepared_verified(
            self.wake.wake(),
            prepared,
            plots,
            masks,
            body_time,
        )
        .map_err(BodyLifecycleSessionError::PlanSeal)?;
        Ok(PreparedVerifiedBodyProposal {
            expected: self.expected,
            sequence: self.sequence,
            host: self.host,
            boot: self.boot,
            wake: self.wake,
            plan,
        })
    }
}
struct StateHash(Sha256);
impl postcard::ser_flavors::Flavor for StateHash {
    type Output = [u8; 32];
    fn try_push(&mut self, byte: u8) -> postcard::Result<()> {
        self.0.update([byte]);
        Ok(())
    }
    fn finalize(self) -> postcard::Result<Self::Output> {
        Ok(self.0.finalize().into())
    }
}
impl SealedBodyLifecycleSession {
    /// Capture only the immutable ordinary proposal preconditions and exact
    /// full-state hash, so complete biography custody can be parked while Core
    /// verifies Plans. No lifecycle mutation or archival acknowledgement occurs.
    pub fn prepare_verified_proposal_context(
        &self,
        host: &HostId,
        boot: &BootId,
    ) -> Result<VerifiedBodyProposalContext, BodyLifecycleSessionError> {
        self.require_mutable()?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(BodyLifecycleSessionError::NotLulled);
        }
        self.require_host(host, boot)?;
        if !self.has_lifecycle_room(2, 1) {
            return Err(BodyLifecycleSessionError::ArchivePersistenceRequired);
        }
        let sequence = self.next_sequence()?;
        let wake = self
            .evidence
            .body
            .prepare_wake(sequence, super::sign(host, boot, sequence))
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let admitted = self
            .evidence
            .membership
            .parts
            .iter()
            .filter(|part| part.state == MembershipState::Admitted)
            .filter_map(|part| part.current.as_ref())
            .map(|current| (current.host_id.clone(), current.boot_id.clone()))
            .collect();
        Ok(VerifiedBodyProposalContext {
            expected: self.proposal_state_hash()?,
            sequence,
            host: host.clone(),
            boot: boot.clone(),
            wake,
            admitted,
        })
    }
    fn proposal_state_hash(&self) -> Result<[u8; 32], BodyLifecycleSessionError> {
        let mut hash = Sha256::new();
        hash.update(b"conduit.body/prepared-proposal-state@1");
        postcard::serialize_with_flavor(
            &(&self.evidence, &self.foreground, &self.pending_archives),
            StateHash(hash),
        )
        .map_err(|_| BodyLifecycleSessionError::Biography(BodyBiographyError::InvalidEvidence))
    }
    /// Complete unchanged ordinary local proposal validation, retaining only an
    /// opaque Wake/Plan seal. The returned value cannot deserialize or execute.
    /// Pending archival custody must be resolved before this preparation path.
    pub fn prepare_verified_proposal(
        &self,
        plots: &[VerifiedBodyPlotPlan<'_>],
        masks: &[VerifiedBodyMaskTopology<'_>],
        body_time: Option<conduit_core::BodyTimeRequirement>,
        host: &HostId,
        boot: &BootId,
    ) -> Result<PreparedVerifiedBodyProposal, BodyLifecycleSessionError> {
        self.require_mutable()?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(BodyLifecycleSessionError::NotLulled);
        }
        self.require_host(host, boot)?;
        for plan in plots
            .iter()
            .map(|p| p.plan)
            .chain(masks.iter().flat_map(|m| m.chains.iter().map(|c| c.plan)))
        {
            for fragment in &plan.preparation_metadata().fragments {
                self.require_host(&fragment.host_id, &fragment.boot_id)?;
            }
        }
        if !self.has_lifecycle_room(2, 1) {
            return Err(BodyLifecycleSessionError::ArchivePersistenceRequired);
        }
        let sequence = self.next_sequence()?;
        let wake = self
            .evidence
            .body
            .prepare_wake(sequence, super::sign(host, boot, sequence))
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let plan = BodyPlanSeal::capture_verified(wake.wake(), plots, masks, body_time)
            .map_err(BodyLifecycleSessionError::PlanSeal)?;
        Ok(PreparedVerifiedBodyProposal {
            expected: self.proposal_state_hash()?,
            sequence,
            host: host.clone(),
            boot: boot.clone(),
            wake,
            plan,
        })
    }
    /// Publish only against the exact unchanged complete lifecycle state. All
    /// failure is confined to the same unpublished candidate as ordinary propose.
    pub fn commit_verified_proposal(
        &mut self,
        prepared: PreparedVerifiedBodyProposal,
    ) -> Result<&BodyLifecycleRealization<BodyPlanSeal>, BodyLifecycleSessionError> {
        self.require_mutable()?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(BodyLifecycleSessionError::NotLulled);
        }
        self.require_host(&prepared.host, &prepared.boot)?;
        if self.next_sequence()? != prepared.sequence
            || self.proposal_state_hash()? != prepared.expected
        {
            return Err(BodyLifecycleSessionError::StaleWorkload);
        }
        let mut next = self.clone();
        let wake = next
            .evidence
            .append_prepared_wake_unpublished(prepared.wake, prepared.sequence)
            .map_err(BodyLifecycleSessionError::Biography)?;
        next.realization = Some(BodyLifecycleRealization {
            wake,
            plan: prepared.plan,
            play: None,
        });
        *self = next;
        Ok(self
            .realization
            .as_ref()
            .expect("published checked proposal"))
    }
}
