//! Portable finite calendar meaning over the shared temporal substrate.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{
    LocalDate, LocalDateTime, NamedTimeZone, RecurrenceDefinition, TemporalInstant,
    TemporalRelation, TemporalWindow, TemporalWindowRefusal, MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

pub const MAXIMUM_CALENDAR_TEXT_BYTES: usize = 1_024;
pub const MAXIMUM_EVENT_PARTICIPANTS: usize = 64;
pub const MAXIMUM_EVENT_REMINDERS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participant {
    pub identity: String,
    pub contact_reference: Option<String>,
    pub role: ParticipantRole,
    pub invitation: InvitationEvidence,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantRole {
    Organizer,
    Required,
    Optional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvitationEvidence {
    Unknown,
    Observed {
        state: InvitationState,
        observed_at: TemporalInstant,
        source_identity: String,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvitationState {
    NeedsAction,
    Accepted,
    Declined,
    Tentative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedCalendarSpan {
    pub local_start: LocalDateTime,
    pub local_end: LocalDateTime,
    pub zone: NamedTimeZone,
    pub instant: TemporalWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalendarEventTime {
    Timed(TimedCalendarSpan),
    AllDay {
        start: LocalDate,
        end_exclusive: LocalDate,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReminderSpecification {
    pub identity: String,
    pub before_start_ticks: u64,
    pub scale: crate::TemporalScale,
    pub delivery_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub identity: String,
    pub title: String,
    pub description: String,
    pub location: String,
    pub time: CalendarEventTime,
    pub participants: Vec<Participant>,
    pub recurrence: Option<RecurrenceDefinition>,
    pub reminders: Vec<ReminderSpecification>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AvailabilityState {
    Free,
    Tentative,
    Busy,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailabilityInterval {
    pub participant_identity: String,
    pub interval: TemporalWindow,
    pub state: AvailabilityState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailabilityBasis {
    pub identity: String,
    pub observed_at: TemporalInstant,
    pub usable_until: TemporalInstant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantAvailability {
    pub participant_identity: String,
    pub zone: NamedTimeZone,
    pub basis: AvailabilityBasis,
    pub intervals: Vec<AvailabilityInterval>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CalendarRefusal {
    InvalidIdentity,
    InvalidText,
    InvalidTime,
    InvalidParticipants,
    InvalidInvitationEvidence,
    InvalidReminder,
    InvalidRecurrence,
    InvalidAvailability,
    StaleAvailability,
    IncomparableTime,
}

impl Participant {
    pub fn validate(&self) -> Result<(), CalendarRefusal> {
        identity(&self.identity)?;
        if self
            .contact_reference
            .as_ref()
            .is_some_and(|value| identity(value).is_err())
        {
            return Err(CalendarRefusal::InvalidIdentity);
        }
        if let InvitationEvidence::Observed {
            observed_at,
            source_identity,
            ..
        } = &self.invitation
        {
            observed_at
                .validate()
                .map_err(|_| CalendarRefusal::InvalidInvitationEvidence)?;
            identity(source_identity).map_err(|_| CalendarRefusal::InvalidInvitationEvidence)?;
        }
        Ok(())
    }
}

impl TimedCalendarSpan {
    pub fn validate(&self) -> Result<(), CalendarRefusal> {
        self.local_start
            .validate()
            .map_err(|_| CalendarRefusal::InvalidTime)?;
        self.local_end
            .validate()
            .map_err(|_| CalendarRefusal::InvalidTime)?;
        self.zone
            .validate()
            .map_err(|_| CalendarRefusal::InvalidTime)?;
        self.instant.validate().map_err(map_window)?;
        Ok(())
    }
}

impl CalendarEventTime {
    pub fn validate(&self) -> Result<(), CalendarRefusal> {
        match self {
            Self::Timed(span) => span.validate(),
            Self::AllDay {
                start,
                end_exclusive,
            } => {
                start.validate().map_err(|_| CalendarRefusal::InvalidTime)?;
                end_exclusive
                    .validate()
                    .map_err(|_| CalendarRefusal::InvalidTime)?;
                ((start.year(), start.month(), start.day())
                    < (
                        end_exclusive.year(),
                        end_exclusive.month(),
                        end_exclusive.day(),
                    ))
                    .then_some(())
                    .ok_or(CalendarRefusal::InvalidTime)
            }
        }
    }
}

impl CalendarEvent {
    pub fn validate(&self) -> Result<(), CalendarRefusal> {
        identity(&self.identity)?;
        text(&self.title)?;
        text(&self.description)?;
        text(&self.location)?;
        self.time.validate()?;
        if self.participants.len() > MAXIMUM_EVENT_PARTICIPANTS
            || self
                .participants
                .iter()
                .any(|value| value.validate().is_err())
            || self
                .participants
                .windows(2)
                .any(|pair| pair[0].identity >= pair[1].identity)
        {
            return Err(CalendarRefusal::InvalidParticipants);
        }
        if self.reminders.len() > MAXIMUM_EVENT_REMINDERS
            || self.reminders.iter().any(|value| value.validate().is_err())
        {
            return Err(CalendarRefusal::InvalidReminder);
        }
        if self
            .recurrence
            .as_ref()
            .is_some_and(|value| value.validate().is_err())
        {
            return Err(CalendarRefusal::InvalidRecurrence);
        }
        Ok(())
    }
}

impl ReminderSpecification {
    pub fn validate(&self) -> Result<(), CalendarRefusal> {
        identity(&self.identity).map_err(|_| CalendarRefusal::InvalidReminder)?;
        identity(&self.delivery_kind).map_err(|_| CalendarRefusal::InvalidReminder)?;
        (self.before_start_ticks > 0)
            .then_some(())
            .ok_or(CalendarRefusal::InvalidReminder)
    }
}

impl AvailabilityBasis {
    pub fn validate_at(&self, reference: &TemporalInstant) -> Result<(), CalendarRefusal> {
        identity(&self.identity)?;
        self.observed_at
            .validate()
            .map_err(|_| CalendarRefusal::InvalidAvailability)?;
        self.usable_until
            .validate()
            .map_err(|_| CalendarRefusal::InvalidAvailability)?;
        match reference
            .relation_to(&self.observed_at)
            .map_err(|_| CalendarRefusal::IncomparableTime)?
        {
            TemporalRelation::Past { .. } => Err(CalendarRefusal::InvalidAvailability),
            TemporalRelation::Indeterminate => Err(CalendarRefusal::IncomparableTime),
            TemporalRelation::Present | TemporalRelation::Future { .. } => match reference
                .relation_to(&self.usable_until)
                .map_err(|_| CalendarRefusal::IncomparableTime)?
            {
                TemporalRelation::Past { .. } | TemporalRelation::Present => Ok(()),
                TemporalRelation::Future { .. } => Err(CalendarRefusal::StaleAvailability),
                TemporalRelation::Indeterminate => Err(CalendarRefusal::IncomparableTime),
            },
        }
    }
}

impl ParticipantAvailability {
    pub fn validate_at(&self, reference: &TemporalInstant) -> Result<(), CalendarRefusal> {
        identity(&self.participant_identity)?;
        self.zone
            .validate()
            .map_err(|_| CalendarRefusal::InvalidAvailability)?;
        self.basis.validate_at(reference)?;
        if self.intervals.is_empty() || self.intervals.len() > 256 {
            return Err(CalendarRefusal::InvalidAvailability);
        }
        for interval in &self.intervals {
            if interval.participant_identity != self.participant_identity {
                return Err(CalendarRefusal::InvalidAvailability);
            }
            interval.interval.validate().map_err(map_window)?;
        }
        for pair in self.intervals.windows(2) {
            let relation = pair[1]
                .interval
                .start()
                .relation_to(pair[0].interval.end())
                .map_err(|_| CalendarRefusal::IncomparableTime)?;
            match relation {
                TemporalRelation::Past { .. } | TemporalRelation::Indeterminate => {
                    return Err(CalendarRefusal::InvalidAvailability)
                }
                TemporalRelation::Present
                    if pair[0].interval.end_boundary() == crate::TemporalBoundary::Inclusive
                        && pair[1].interval.start_boundary()
                            == crate::TemporalBoundary::Inclusive =>
                {
                    return Err(CalendarRefusal::InvalidAvailability)
                }
                TemporalRelation::Present | TemporalRelation::Future { .. } => {}
            }
        }
        Ok(())
    }
}

fn identity(value: &str) -> Result<(), CalendarRefusal> {
    (!value.is_empty() && value.len() <= MAXIMUM_TEMPORAL_IDENTITY_BYTES)
        .then_some(())
        .ok_or(CalendarRefusal::InvalidIdentity)
}

fn text(value: &str) -> Result<(), CalendarRefusal> {
    (value.len() <= MAXIMUM_CALENDAR_TEXT_BYTES)
        .then_some(())
        .ok_or(CalendarRefusal::InvalidText)
}

fn map_window(_: TemporalWindowRefusal) -> CalendarRefusal {
    CalendarRefusal::InvalidTime
}
