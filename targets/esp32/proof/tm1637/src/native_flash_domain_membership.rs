//! Membership custody shares the existing finite flash domain. Start/codec/
//! replay/final verification require their own preparation evidence; the ROM
//! flash-operation witness cannot qualify them.
use super::*;
type Membership = Box<crate::native_body_join::membership::job::Job>;
impl Submitter {
    /// Check before moving an accepted request or taking the Root's Body.
    pub(crate) fn membership_ready(&self) -> bool {
        self.membership_preparation_qualified() && self.available()
    }
    pub(crate) fn membership_preparation_qualified(&self) -> bool {
        self.preparation.is_some()
    }
    pub(crate) fn submit_membership(
        &mut self,
        owner: Membership,
    ) -> Result<u64, (SubmissionRefusal, Membership)> {
        let Some(timing) = self.timing else {
            return Err((SubmissionRefusal::UnreviewedTiming, owner));
        };
        use crate::native_body_join::membership::job::Phase;
        use crate::native_restart::owned_checkpoint::Phase as CheckpointPhase;
        let budget = match owner.phase() {
            Phase::Checkpoint(CheckpointPhase::Physical(operation)) => {
                crate::native_evidence_region::checkpoint_access::budget_ms(timing, operation)
            }
            phase @ (Phase::Start
            | Phase::Checkpoint(
                CheckpointPhase::Prepare
                | CheckpointPhase::Capture
                | CheckpointPhase::FrozenChunk
                | CheckpointPhase::ReplayChunk
                | CheckpointPhase::Finish,
            )) => {
                let Some(budget) = self
                    .preparation
                    .and_then(|profile| profile.budget_ms(phase))
                else {
                    return Err((SubmissionRefusal::Unprepared, owner));
                };
                budget
            }
            _ => return Err((SubmissionRefusal::Terminal, owner)),
        };
        match self.inner.submit(DomainJob::Membership(owner), budget) {
            Ok(id) => Ok(id),
            Err((error, DomainJob::Membership(owner))) => {
                Err((SubmissionRefusal::Queue(error), owner))
            }
            Err(_) => unreachable!("submitted membership returned as another owner kind"),
        }
    }
    pub(crate) fn take_membership(
        &mut self,
        id: u64,
    ) -> Result<
        crate::native_flash_jobs::Completion<Membership, (), &'static str>,
        crate::native_flash_jobs::Refusal,
    > {
        self.check(id, Kind::Membership)?;
        let done = self.inner.take(id)?;
        let DomainJob::Membership(owner) = done.owner else {
            unreachable!("checked actual membership submission")
        };
        let result = match done.result {
            Ok(()) => Ok(()),
            Err(DomainError::Membership(error)) => Err(error),
            Err(_) => Err("flash-domain-owner-kind"),
        };
        Ok(crate::native_flash_jobs::Completion {
            id: done.id,
            step: done.step,
            budget: done.budget,
            owner,
            result,
            progress: done.progress,
        })
    }
    pub(crate) fn cancel_membership(
        &mut self,
        id: u64,
    ) -> Result<Membership, crate::native_flash_jobs::Refusal> {
        self.check(id, Kind::Membership)?;
        let owner = self.inner.cancel(id)?;
        let DomainJob::Membership(owner) = owner else {
            unreachable!("checked actual membership submission")
        };
        Ok(owner)
    }
}
