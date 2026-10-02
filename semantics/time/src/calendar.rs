//! Portable finite calendar meaning over the shared temporal substrate.

use crate::{
    AvailabilityBasis, CalendarEvent, CalendarEventTime, CalendarRefusal, InvitationEvidence,
    Participant, ParticipantAvailability, ReminderOccurrence, ReminderSpecification,
    TemporalInstant, TemporalRelation, TemporalWindowRefusal, TimedCalendarSpan,
    MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

pub const MAXIMUM_CALENDAR_TEXT_BYTES: usize = 1_024;
pub const MAXIMUM_EVENT_PARTICIPANTS: usize = 64;
pub const MAXIMUM_EVENT_REMINDERS: usize = 16;

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
        if let InvitationEvidence::Observed(observed) = &self.invitation {
            observed
                .observed_at()
                .validate()
                .map_err(|_| CalendarRefusal::InvalidInvitationEvidence)?;
            identity(observed.source_identity())
                .map_err(|_| CalendarRefusal::InvalidInvitationEvidence)?;
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
            Self::Timed(span) => {
                let span = span.value();
                span.local_start()
                    .validate()
                    .map_err(|_| CalendarRefusal::InvalidTime)?;
                span.local_end()
                    .validate()
                    .map_err(|_| CalendarRefusal::InvalidTime)?;
                span.zone()
                    .validate()
                    .map_err(|_| CalendarRefusal::InvalidTime)?;
                span.instant().validate().map_err(map_window)
            }
            Self::AllDay(value) => {
                let start = value.start();
                let end_exclusive = value.end_exclusive();
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
                .iter()
                .zip(self.participants.iter().skip(1))
                .any(|(left, right)| left.identity >= right.identity)
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

impl ReminderOccurrence {
    pub fn validate(&self) -> Result<(), CalendarRefusal> {
        identity(&self.identity).map_err(|_| CalendarRefusal::InvalidReminder)?;
        identity(&self.reminder_identity).map_err(|_| CalendarRefusal::InvalidReminder)?;
        identity(&self.event_identity).map_err(|_| CalendarRefusal::InvalidReminder)?;
        identity(&self.delivery_kind).map_err(|_| CalendarRefusal::InvalidReminder)
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
        for (left, right) in self.intervals.iter().zip(self.intervals.iter().skip(1)) {
            let relation = right
                .interval
                .start()
                .relation_to(left.interval.end())
                .map_err(|_| CalendarRefusal::IncomparableTime)?;
            match relation {
                TemporalRelation::Past { .. } | TemporalRelation::Indeterminate => {
                    return Err(CalendarRefusal::InvalidAvailability)
                }
                TemporalRelation::Present
                    if *left.interval.end_boundary() == crate::TemporalBoundary::Inclusive
                        && *right.interval.start_boundary()
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
