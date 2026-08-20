//! Finite scheduled-intent decisions over the shared temporal substrate.
//!
//! This module decides whether one already-expanded occurrence is ready. It
//! does not wait, enqueue, retry, or execute the typed payload. Downstream
//! effects retain their own resource and authority admission.

use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::{
    MonotonicDuration, MonotonicInstant, NamedTimeZone, OccurrenceInstant, RecurrenceOccurrence,
    TemporalInstant, TemporalRelation, TemporalScale, TemporalWindow, TemporalWindowPosition,
    MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledIntent<T> {
    pub identity: String,
    pub occurrence: RecurrenceOccurrence,
    pub trigger: TriggerProfile,
    pub missed: MissedOccurrencePolicy,
    pub payload: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerProfile {
    Elapsed(ElapsedTrigger),
    Civil(CivilTrigger),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElapsedTrigger {
    pub opens_at: MonotonicInstant,
    pub expires_at: MonotonicInstant,
    pub suspend: SuspendBehavior,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CivilTrigger {
    pub window: TemporalWindow,
    pub zone: NamedTimeZone,
    pub clock_change: ClockChangeBehavior,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuspendBehavior {
    ClockIncludesSuspend,
    ClockExcludesSuspend,
    RefuseAfterSuspend,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockChangeBehavior {
    ReevaluateWindow,
    RefuseAfterChange,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissedOccurrencePolicy {
    Expire,
    Skip,
    FireLate { maximum_lateness_ticks: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerObservation {
    Elapsed {
        now: MonotonicInstant,
        suspend_observed: bool,
    },
    Civil {
        now: TemporalInstant,
        clock_change_observed: bool,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScheduledOccurrenceDecision {
    Awaiting,
    Ready { lateness_ticks: u64 },
    Missed,
    Expired,
    Cancelled,
    Rebooted,
    Suspended,
    ClockChanged,
    ClockUncertain,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScheduledIntentRefusal {
    InvalidIdentity,
    InvalidOccurrence,
    TriggerOccurrenceMismatch,
    InvalidWindow,
    IncomparableObservation,
    WrongObservationProfile,
    InvalidLatePolicy,
}

impl<T> ScheduledIntent<T> {
    pub fn validate(&self) -> Result<(), ScheduledIntentRefusal> {
        if self.identity.is_empty() || self.identity.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES {
            return Err(ScheduledIntentRefusal::InvalidIdentity);
        }
        self.occurrence
            .validate()
            .map_err(|_| ScheduledIntentRefusal::InvalidOccurrence)?;
        if matches!(
            self.missed,
            MissedOccurrencePolicy::FireLate {
                maximum_lateness_ticks: 0
            }
        ) {
            return Err(ScheduledIntentRefusal::InvalidLatePolicy);
        }
        match (&self.trigger, &self.occurrence.at) {
            (TriggerProfile::Elapsed(trigger), OccurrenceInstant::Monotonic(at)) => {
                validate_elapsed(trigger)?;
                same_monotonic_clock(&trigger.opens_at, at)?;
            }
            (TriggerProfile::Civil(trigger), OccurrenceInstant::Civil { zone, instant, .. }) => {
                validate_civil(trigger)?;
                if &trigger.zone != zone || !same_wall_basis(trigger.window.start(), instant) {
                    return Err(ScheduledIntentRefusal::TriggerOccurrenceMismatch);
                }
            }
            (TriggerProfile::Civil(trigger), OccurrenceInstant::Wall(at)) => {
                validate_civil(trigger)?;
                if !same_wall_basis(trigger.window.start(), at) {
                    return Err(ScheduledIntentRefusal::TriggerOccurrenceMismatch);
                }
            }
            _ => return Err(ScheduledIntentRefusal::TriggerOccurrenceMismatch),
        }
        Ok(())
    }

    pub fn decide(
        &self,
        observation: &TriggerObservation,
        cancelled: bool,
    ) -> Result<ScheduledOccurrenceDecision, ScheduledIntentRefusal> {
        self.validate()?;
        if cancelled {
            return Ok(ScheduledOccurrenceDecision::Cancelled);
        }
        match (&self.trigger, observation) {
            (
                TriggerProfile::Elapsed(trigger),
                TriggerObservation::Elapsed {
                    now,
                    suspend_observed,
                },
            ) => decide_elapsed(trigger, self.missed, now, *suspend_observed),
            (
                TriggerProfile::Civil(trigger),
                TriggerObservation::Civil {
                    now,
                    clock_change_observed,
                },
            ) => decide_civil(trigger, self.missed, now, *clock_change_observed),
            _ => Err(ScheduledIntentRefusal::WrongObservationProfile),
        }
    }
}

fn validate_elapsed(trigger: &ElapsedTrigger) -> Result<(), ScheduledIntentRefusal> {
    trigger
        .opens_at
        .validate()
        .map_err(|_| ScheduledIntentRefusal::InvalidWindow)?;
    trigger
        .expires_at
        .validate()
        .map_err(|_| ScheduledIntentRefusal::InvalidWindow)?;
    same_monotonic_clock(&trigger.opens_at, &trigger.expires_at)?;
    if trigger.opens_at.ticks() > trigger.expires_at.ticks() {
        return Err(ScheduledIntentRefusal::InvalidWindow);
    }
    Ok(())
}

fn validate_civil(trigger: &CivilTrigger) -> Result<(), ScheduledIntentRefusal> {
    trigger
        .window
        .validate()
        .map_err(|_| ScheduledIntentRefusal::InvalidWindow)?;
    trigger
        .zone
        .validate()
        .map_err(|_| ScheduledIntentRefusal::InvalidWindow)
}

fn decide_elapsed(
    trigger: &ElapsedTrigger,
    missed: MissedOccurrencePolicy,
    now: &MonotonicInstant,
    suspend_observed: bool,
) -> Result<ScheduledOccurrenceDecision, ScheduledIntentRefusal> {
    if now.clock() != trigger.opens_at.clock() {
        return Ok(ScheduledOccurrenceDecision::Rebooted);
    }
    if suspend_observed && trigger.suspend == SuspendBehavior::RefuseAfterSuspend {
        return Ok(ScheduledOccurrenceDecision::Suspended);
    }
    if now.ticks() < trigger.opens_at.ticks() {
        return Ok(ScheduledOccurrenceDecision::Awaiting);
    }
    if now.ticks() <= trigger.expires_at.ticks() {
        return Ok(ScheduledOccurrenceDecision::Ready {
            lateness_ticks: now.ticks() - trigger.opens_at.ticks(),
        });
    }
    finish_missed(now.ticks() - trigger.expires_at.ticks(), missed)
}

fn decide_civil(
    trigger: &CivilTrigger,
    missed: MissedOccurrencePolicy,
    now: &TemporalInstant,
    clock_change_observed: bool,
) -> Result<ScheduledOccurrenceDecision, ScheduledIntentRefusal> {
    if clock_change_observed && trigger.clock_change == ClockChangeBehavior::RefuseAfterChange {
        return Ok(ScheduledOccurrenceDecision::ClockChanged);
    }
    if now.uncertainty_ticks > 0 {
        return Ok(ScheduledOccurrenceDecision::ClockUncertain);
    }
    match trigger
        .window
        .classify(now)
        .map_err(|_| ScheduledIntentRefusal::IncomparableObservation)?
    {
        TemporalWindowPosition::Before => Ok(ScheduledOccurrenceDecision::Awaiting),
        TemporalWindowPosition::Within => {
            let lateness_ticks = match now.relation_to(trigger.window.start()) {
                Ok(TemporalRelation::Future { minimum_ticks, .. }) => minimum_ticks,
                Ok(TemporalRelation::Present) => 0,
                _ => 0,
            };
            Ok(ScheduledOccurrenceDecision::Ready { lateness_ticks })
        }
        TemporalWindowPosition::After => {
            let lateness = now
                .ticks
                .checked_sub(trigger.window.end().ticks)
                .ok_or(ScheduledIntentRefusal::IncomparableObservation)?;
            finish_missed(lateness, missed)
        }
        TemporalWindowPosition::Indeterminate => Ok(ScheduledOccurrenceDecision::ClockUncertain),
    }
}

fn finish_missed(
    lateness_ticks: u64,
    policy: MissedOccurrencePolicy,
) -> Result<ScheduledOccurrenceDecision, ScheduledIntentRefusal> {
    Ok(match policy {
        MissedOccurrencePolicy::Expire => ScheduledOccurrenceDecision::Expired,
        MissedOccurrencePolicy::Skip => ScheduledOccurrenceDecision::Missed,
        MissedOccurrencePolicy::FireLate {
            maximum_lateness_ticks,
        } if lateness_ticks <= maximum_lateness_ticks => {
            ScheduledOccurrenceDecision::Ready { lateness_ticks }
        }
        MissedOccurrencePolicy::FireLate { .. } => ScheduledOccurrenceDecision::Missed,
    })
}

fn same_monotonic_clock(
    left: &MonotonicInstant,
    right: &MonotonicInstant,
) -> Result<(), ScheduledIntentRefusal> {
    if left.clock() == right.clock() {
        Ok(())
    } else {
        Err(ScheduledIntentRefusal::TriggerOccurrenceMismatch)
    }
}

fn same_wall_basis(left: &TemporalInstant, right: &TemporalInstant) -> bool {
    left.clock_basis == right.clock_basis && left.scale == right.scale
}

pub fn elapsed_trigger_window(
    opens_at: MonotonicInstant,
    duration: MonotonicDuration,
    suspend: SuspendBehavior,
) -> Result<ElapsedTrigger, ScheduledIntentRefusal> {
    let expires_at = opens_at
        .deadline_after(duration)
        .map_err(|_| ScheduledIntentRefusal::InvalidWindow)?
        .instant()
        .clone();
    let trigger = ElapsedTrigger {
        opens_at,
        expires_at,
        suspend,
    };
    validate_elapsed(&trigger)?;
    Ok(trigger)
}

pub const fn temporal_scale_name(scale: TemporalScale) -> &'static str {
    match scale {
        TemporalScale::Seconds => "seconds",
        TemporalScale::Milliseconds => "milliseconds",
        TemporalScale::Microseconds => "microseconds",
        TemporalScale::Nanoseconds => "nanoseconds",
    }
}
