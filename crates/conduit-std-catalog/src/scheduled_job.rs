//! Machine-job scheduling over the shared temporal substrate.
//!
//! A ready occurrence reveals the exact bounded `JobRequest`; it never grants
//! the executable resource or execution authority required by `process/run-bounded`.

use conduit_core::{ScheduledIntent, ScheduledIntentRefusal, ScheduledOccurrenceDecision};

use crate::{JobRequest, JobRequestRefusal};

pub type ScheduledJobIntent = ScheduledIntent<JobRequest>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduledJobRefusal {
    InvalidSchedule(ScheduledIntentRefusal),
    InvalidJob(JobRequestRefusal),
    NotReady(ScheduledOccurrenceDecision),
}

pub fn validate_scheduled_job(intent: &ScheduledJobIntent) -> Result<(), ScheduledJobRefusal> {
    intent
        .validate()
        .map_err(ScheduledJobRefusal::InvalidSchedule)?;
    intent
        .payload
        .validate()
        .map_err(ScheduledJobRefusal::InvalidJob)
}

pub fn ready_job_request(
    intent: &ScheduledJobIntent,
    decision: ScheduledOccurrenceDecision,
) -> Result<&JobRequest, ScheduledJobRefusal> {
    validate_scheduled_job(intent)?;
    match decision {
        ScheduledOccurrenceDecision::Ready { .. } => Ok(&intent.payload),
        other => Err(ScheduledJobRefusal::NotReady(other)),
    }
}
