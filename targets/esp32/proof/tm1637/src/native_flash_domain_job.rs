//! One affine owner across maintenance kinds; received labels cannot
//! select the physical operation, its step ordinal, or its timing profile.
use super::*;
use crate::native_flash_window::FlashPermit;
pub(crate) enum DomainJob {
    Observed(Job),
    #[cfg(feature = "body-persistence")]
    Pending(Box<crate::native_restart::network_pending_job::PendingJob>),
    #[cfg(feature = "body-persistence")]
    Biography(Box<crate::native_restart::biography::hot::Custody>),
    #[cfg(feature = "body-persistence")]
    Membership(Box<crate::native_body_join::membership::job::Job>),
}
pub(crate) enum DomainError {
    Observed(Error),
    #[cfg(feature = "body-persistence")]
    Pending(&'static str),
    #[cfg(feature = "body-persistence")]
    Biography(&'static str),
    #[cfg(feature = "body-persistence")]
    Membership(&'static str),
}
impl crate::native_flash_jobs::Job for DomainJob {
    fn step(&self) -> u32 {
        match self {
            Self::Observed(job) => crate::native_flash_jobs::Job::step(job),
            #[cfg(feature = "body-persistence")]
            Self::Pending(job) => job.step(),
            #[cfg(feature = "body-persistence")]
            Self::Biography(job) => job.step(),
            #[cfg(feature = "body-persistence")]
            Self::Membership(job) => job.step(),
        }
    }
}
pub(crate) fn advance(
    job: &mut DomainJob,
    permit: &FlashPermit<'_, '_>,
) -> Result<(), DomainError> {
    match job {
        DomainJob::Observed(job) => {
            crate::native_observed_archive::advance_reclaim_step(job, permit)
                .map_err(DomainError::Observed)
        }
        #[cfg(feature = "body-persistence")]
        DomainJob::Pending(job) => job
            .advance(&mut crate::native_evidence_region::checkpoint_access::Access::new(permit))
            .map(|_| ())
            .map_err(DomainError::Pending),
        #[cfg(feature = "body-persistence")]
        DomainJob::Membership(job) => job
            .advance(
                permit,
                &mut crate::native_evidence_region::checkpoint_access::Access::new(permit),
                embassy_time::Instant::now().as_millis(),
            )
            .map_err(DomainError::Membership),
        #[cfg(feature = "body-persistence")]
        DomainJob::Biography(job) => job
            .advance(&mut crate::native_evidence_region::checkpoint_access::Access::new(permit))
            .map(|_| ())
            .map_err(DomainError::Biography),
    }
}
