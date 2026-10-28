//! One private Boot-local arbitration domain, injected into the actual providers.
//! The queue is initially empty. Its duration ceiling is a scheduling bound,
//! never a claim that any flash operation has qualified physical timing.
use crate::{
    native_flash_jobs::{Mailbox, RootSubmitter},
    native_flash_wait::WaitLane,
    native_flash_window::Gate,
    native_observed_archive::custody_journal::Error,
    native_observed_reclaim_job::ReclaimJob,
};
use alloc::boxed::Box;
type Job = ReclaimJob<crate::native_evidence_region::ObservationEpoch>;
pub(crate) type Submitter = RootSubmitter<'static, Job, (), Error>;
pub(crate) type Lane = WaitLane<'static, Job, (), Error>;
struct Domain {
    gate: Gate,
    jobs: Mailbox<Job, (), Error>,
}
// Only the Box pointer occupies permanent static memory. Its complete finite
// backing comes from the existing Root arena; no allocator grant is enlarged.
static DOMAIN: static_cell::StaticCell<Box<Domain>> = static_cell::StaticCell::new();
#[inline(never)]
pub(crate) fn prepare() -> Result<(Submitter, Lane, &'static Gate), &'static str> {
    let memory = esp_alloc::HEAP.stats();
    if memory.size.saturating_sub(memory.current_usage) < core::mem::size_of::<Domain>() + 8192 {
        return Err("flash-domain-capacity");
    }
    let domain = DOMAIN
        .try_init(Box::new(Domain {
            gate: Gate::new(60_000, 60, 500).map_err(|_| "flash-domain-bounds")?,
            jobs: Mailbox::new(),
        }))
        .ok_or("flash-domain-already-owned")?;
    let domain: &'static mut Domain = domain.as_mut();
    let gate = &domain.gate;
    let (submitter, executor) = domain.jobs.split().map_err(|_| "flash-domain-endpoints")?;
    Ok((submitter, WaitLane::new(gate, executor), gate))
}
