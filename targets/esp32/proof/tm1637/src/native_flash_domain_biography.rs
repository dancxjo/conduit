//! Physical biography steps share the existing finite queue and reviewed SDK
//! budgets. Codec/read phases must finish under separately qualified quiet
//! windows before submission; this adapter never runs them inside advance.
use super::*;
type Biography = Box<crate::native_restart::biography::hot::Custody>;
impl Submitter {
    pub(crate) fn submit_biography(
        &mut self,
        owner: Biography,
    ) -> Result<u64, (SubmissionRefusal, Biography)> {
        let Some(timing) = self.timing else {
            return Err((SubmissionRefusal::UnreviewedTiming, owner));
        };
        if owner.needs_chunk() {
            return Err((SubmissionRefusal::Unprepared, owner));
        }
        let Some(operation) = owner.operation() else {
            return Err((SubmissionRefusal::Terminal, owner));
        };
        let budget = crate::native_evidence_region::checkpoint_access::budget_ms(timing, operation);
        match self.inner.submit(DomainJob::Biography(owner), budget) {
            Ok(id) => Ok(id),
            Err((error, DomainJob::Biography(owner))) => {
                Err((SubmissionRefusal::Queue(error), owner))
            }
            Err(_) => unreachable!("submitted biography returned as another owner kind"),
        }
    }
    pub(crate) fn take_biography(
        &mut self,
        id: u64,
    ) -> Result<
        crate::native_flash_jobs::Completion<Biography, (), &'static str>,
        crate::native_flash_jobs::Refusal,
    > {
        self.check(id, Kind::Biography)?;
        let done = self.inner.take(id)?;
        let DomainJob::Biography(owner) = done.owner else {
            unreachable!("checked actual biography submission")
        };
        let result = match done.result {
            Ok(()) => Ok(()),
            Err(DomainError::Biography(error)) => Err(error),
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
    pub(crate) fn cancel_biography(
        &mut self,
        id: u64,
    ) -> Result<Biography, crate::native_flash_jobs::Refusal> {
        self.check(id, Kind::Biography)?;
        let owner = self.inner.cancel(id)?;
        let DomainJob::Biography(owner) = owner else {
            unreachable!("checked actual biography submission")
        };
        Ok(owner)
    }
}
