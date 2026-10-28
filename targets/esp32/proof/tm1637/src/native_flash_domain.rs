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
use crate::native_evidence_region::measurement::{ReviewedTiming, MAXIMUM_BUDGET_MS};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubmissionRefusal {
    UnreviewedTiming,
    Terminal,
    Queue(crate::native_flash_jobs::Refusal),
}
pub(crate) struct Submitter {
    inner: RootSubmitter<'static, Job, (), Error>,
    timing: Option<ReviewedTiming>,
}
impl Submitter {
    /// No received budget or step label enters the real storage queue.
    pub(crate) fn submit(&mut self, job: Job) -> Result<u64, (SubmissionRefusal, Job)> {
        let Some(timing) = self.timing else {
            return Err((SubmissionRefusal::UnreviewedTiming, job));
        };
        let Some(operation) = job.operation() else {
            return Err((SubmissionRefusal::Terminal, job));
        };
        self.inner
            .submit(job, timing.budget_ms(operation))
            .map_err(|(error, job)| (SubmissionRefusal::Queue(error), job))
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
        self.inner.take(id)
    }
    pub(crate) fn cancel(&mut self, id: u64) -> Result<Job, crate::native_flash_jobs::Refusal> {
        self.inner.cancel(id)
    }
}
pub(crate) type Lane = WaitLane<'static, Job, (), Error>;
struct Domain {
    gate: Gate,
    jobs: Mailbox<Job, (), Error>,
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
