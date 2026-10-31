//! One private Boot-local arbitration domain, injected into the actual providers.
//! Root submits only operations covered by the actual reviewed flash profile.
//! Profile evidence grants neither a wait window nor signed reclamation custody.
use crate::{
    native_flash_jobs::{Mailbox, RootSubmitter},
    native_flash_wait::WaitLane,
    native_flash_window::Gate,
    native_observed_archive::custody_journal::Error,
    native_observed_reclaim_job::ReclaimJob,
};
use alloc::boxed::Box;
type Job = ReclaimJob<crate::native_evidence_region::ObservationEpoch>;
#[path = "native_flash_domain_job.rs"]
mod job;
pub(crate) use job::{advance, DomainError, DomainJob};
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Observed,
    #[cfg(feature = "body-persistence")]
    Pending,
    #[cfg(feature = "body-persistence")]
    Biography,
}
use crate::native_evidence_region::measurement::{ReviewedTiming, MAXIMUM_BUDGET_MS};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubmissionRefusal {
    UnreviewedTiming,
    Terminal,
    Unprepared,
    Queue(crate::native_flash_jobs::Refusal),
}
pub(crate) struct Submitter {
    inner: RootSubmitter<'static, DomainJob, (), DomainError>,
    timing: Option<ReviewedTiming>,
}
impl Submitter {
    pub(crate) fn available(&self) -> bool {
        self.timing.is_some() && self.inner.phase() == crate::native_flash_jobs::Phase::Idle
    }
    /// No received budget or step label enters the real storage queue.
    pub(crate) fn submit(&mut self, job: Job) -> Result<u64, (SubmissionRefusal, Job)> {
        let Some(timing) = self.timing else {
            return Err((SubmissionRefusal::UnreviewedTiming, job));
        };
        let Some(operation) = job.operation() else {
            return Err((SubmissionRefusal::Terminal, job));
        };
        match self
            .inner
            .submit(DomainJob::Observed(job), timing.budget_ms(operation))
        {
            Ok(id) => Ok(id),
            Err((error, DomainJob::Observed(job))) => Err((SubmissionRefusal::Queue(error), job)),
            #[cfg(feature = "body-persistence")]
            Err((_, DomainJob::Pending(_) | DomainJob::Biography(_))) => {
                unreachable!("submitted observed owner returned as pending")
            }
        }
    }
    #[cfg(feature = "body-persistence")]
    pub(crate) fn submit_pending(
        &mut self,
        job: Box<crate::native_restart::network_pending_job::PendingJob>,
    ) -> Result<
        u64,
        (
            SubmissionRefusal,
            Box<crate::native_restart::network_pending_job::PendingJob>,
        ),
    > {
        let Some(timing) = self.timing else {
            return Err((SubmissionRefusal::UnreviewedTiming, job));
        };
        let Some(operation) = job.operation() else {
            return Err((SubmissionRefusal::Terminal, job));
        };
        let budget = crate::native_evidence_region::checkpoint_access::budget_ms(timing, operation);
        match self.inner.submit(DomainJob::Pending(job), budget) {
            Ok(id) => Ok(id),
            Err((error, DomainJob::Pending(job))) => Err((SubmissionRefusal::Queue(error), job)),
            Err((_, DomainJob::Observed(_) | DomainJob::Biography(_))) => {
                unreachable!("submitted pending owner returned as observed")
            }
        }
    }

    pub(crate) fn phase(&self) -> crate::native_flash_jobs::Phase {
        self.inner.phase()
    }
    pub(crate) fn take(
        &mut self,
        id: u64,
    ) -> Result<
        crate::native_flash_jobs::Completion<Job, (), Error>,
        crate::native_flash_jobs::Refusal,
    > {
        self.check(id, Kind::Observed)?;
        let done = self.inner.take(id)?;
        let DomainJob::Observed(owner) = done.owner else {
            unreachable!("checked actual observed submission")
        };
        let result = match done.result {
            Ok(()) => Ok(()),
            Err(DomainError::Observed(error)) => Err(error),
            #[cfg(feature = "body-persistence")]
            Err(DomainError::Pending(_) | DomainError::Biography(_)) => Err(Error::Phase),
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
    pub(crate) fn cancel(&mut self, id: u64) -> Result<Job, crate::native_flash_jobs::Refusal> {
        self.check(id, Kind::Observed)?;
        let owner = self.inner.cancel(id)?;
        let DomainJob::Observed(owner) = owner else {
            unreachable!("checked actual observed submission")
        };
        Ok(owner)
    }
    fn check(&self, id: u64, kind: Kind) -> Result<(), crate::native_flash_jobs::Refusal> {
        let matches = match kind {
            Kind::Observed => self
                .inner
                .owner_matches(id, |owner| matches!(owner, DomainJob::Observed(_))),
            #[cfg(feature = "body-persistence")]
            Kind::Pending => self
                .inner
                .owner_matches(id, |owner| matches!(owner, DomainJob::Pending(_))),
            #[cfg(feature = "body-persistence")]
            Kind::Biography => self
                .inner
                .owner_matches(id, |owner| matches!(owner, DomainJob::Biography(_))),
        };
        if matches {
            Ok(())
        } else {
            Err(crate::native_flash_jobs::Refusal::Identity)
        }
    }
    #[cfg(feature = "body-persistence")]
    pub(crate) fn take_pending(
        &mut self,
        id: u64,
    ) -> Result<
        crate::native_flash_jobs::Completion<
            Box<crate::native_restart::network_pending_job::PendingJob>,
            (),
            &'static str,
        >,
        crate::native_flash_jobs::Refusal,
    > {
        self.check(id, Kind::Pending)?;
        let done = self.inner.take(id)?;
        let DomainJob::Pending(owner) = done.owner else {
            unreachable!("checked actual pending submission")
        };
        let result = match done.result {
            Ok(()) => Ok(()),
            Err(DomainError::Pending(error)) => Err(error),
            Err(DomainError::Observed(_) | DomainError::Biography(_)) => {
                Err("flash-domain-owner-kind")
            }
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
    #[cfg(feature = "body-persistence")]
    pub(crate) fn cancel_pending(
        &mut self,
        id: u64,
    ) -> Result<
        Box<crate::native_restart::network_pending_job::PendingJob>,
        crate::native_flash_jobs::Refusal,
    > {
        self.check(id, Kind::Pending)?;
        let owner = self.inner.cancel(id)?;
        let DomainJob::Pending(owner) = owner else {
            unreachable!("checked actual pending submission")
        };
        Ok(owner)
    }
}

pub(crate) type Lane = WaitLane<'static, DomainJob, (), DomainError>;
struct Domain {
    gate: Gate,
    jobs: Mailbox<DomainJob, (), DomainError>,
}
// Only the Box pointer occupies permanent static memory. Its complete finite
// backing comes from the existing Root arena; no allocator grant is enlarged.
static DOMAIN: static_cell::StaticCell<Box<Domain>> = static_cell::StaticCell::new();
#[inline(never)]
pub(crate) fn prepare(
    timing: Option<ReviewedTiming>,
) -> Result<(Submitter, Lane, &'static Gate), &'static str> {
    let memory = esp_alloc::HEAP.stats();
    if memory.size.saturating_sub(memory.current_usage) < core::mem::size_of::<Domain>() + 8192 {
        return Err("flash-domain-capacity");
    }
    let domain = DOMAIN
        .try_init(Box::new(Domain {
            gate: Gate::new(60_000, 60, MAXIMUM_BUDGET_MS).map_err(|_| "flash-domain-bounds")?,
            jobs: Mailbox::new(),
        }))
        .ok_or("flash-domain-already-owned")?;
    let domain: &'static mut Domain = domain.as_mut();
    let gate = &domain.gate;
    let (submitter, executor) = domain.jobs.split().map_err(|_| "flash-domain-endpoints")?;
    Ok((
        Submitter {
            inner: submitter,
            timing,
        },
        WaitLane::new(gate, executor),
        gate,
    ))
}

#[cfg(feature = "body-persistence")]
#[path = "native_flash_domain_biography.rs"]
mod biography;
