//! One admitted reminder-delivery effect over an already-ready occurrence.
//!
//! This adapter owns no clock, queue, retry loop, or scheduler. The shared
//! temporal contract decides readiness; this boundary checks delivery authority
//! and performs exactly one adapter call.

use conduit_core::AuthorityBinding;
use conduit_std_catalog::{REMINDER_DELIVERY_AUTHORITY, REMINDER_DELIVER_KIND};
use conduit_std_offers::REMINDER_DELIVER_OPERATION;
use conduit_time::{
    ReminderOccurrence, ScheduledIntent, ScheduledIntentRefusal, ScheduledOccurrenceDecision,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderDeliveryReceipt {
    pub reminder_occurrence_identity: String,
    pub event_identity: String,
    pub grant_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReminderDeliveryRefusal {
    InvalidSchedule(ScheduledIntentRefusal),
    InvalidReminder,
    NotReady(ScheduledOccurrenceDecision),
    MissingAuthority,
    AdapterFailure,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReminderAdapterError {
    DeliveryFailed,
}

pub trait HostedReminderAdapter {
    fn deliver(&mut self, reminder: &ReminderOccurrence) -> Result<(), ReminderAdapterError>;
}

pub fn deliver_ready_reminder(
    scheduled: &ScheduledIntent<ReminderOccurrence>,
    decision: ScheduledOccurrenceDecision,
    grant: Option<&AuthorityBinding>,
    adapter: &mut dyn HostedReminderAdapter,
) -> Result<ReminderDeliveryReceipt, ReminderDeliveryRefusal> {
    scheduled
        .validate()
        .map_err(ReminderDeliveryRefusal::InvalidSchedule)?;
    scheduled
        .payload
        .validate()
        .map_err(|_| ReminderDeliveryRefusal::InvalidReminder)?;
    if !matches!(decision, ScheduledOccurrenceDecision::Ready { .. }) {
        return Err(ReminderDeliveryRefusal::NotReady(decision));
    }
    let grant = grant.ok_or(ReminderDeliveryRefusal::MissingAuthority)?;
    if grant.grant_id.as_str().is_empty()
        || grant.contract_id.as_str() != REMINDER_DELIVERY_AUTHORITY
        || grant.host_operation_contract_id.as_str() != REMINDER_DELIVER_OPERATION
        || grant.subject_kind.as_str() != REMINDER_DELIVER_KIND
    {
        return Err(ReminderDeliveryRefusal::MissingAuthority);
    }
    adapter
        .deliver(&scheduled.payload)
        .map_err(|_| ReminderDeliveryRefusal::AdapterFailure)?;
    Ok(ReminderDeliveryReceipt {
        reminder_occurrence_identity: scheduled.payload.identity.clone(),
        event_identity: scheduled.payload.event_identity.clone(),
        grant_identity: grant.grant_id.as_str().into(),
    })
}
