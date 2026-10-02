use conduit_plot::rust_binding::NativeRustBinding;
use conduit_time::{
    CalendarRefusal, MeetingProposalRefusal, RecurrenceRefusal, ScheduledIntentRefusal,
    TemporalWindowRefusal,
};

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn calendar_and_meeting_refusals_are_native() {
    for value in [
        CalendarRefusal::InvalidIdentity,
        CalendarRefusal::InvalidText,
        CalendarRefusal::InvalidTime,
        CalendarRefusal::InvalidParticipants,
        CalendarRefusal::InvalidInvitationEvidence,
        CalendarRefusal::InvalidReminder,
        CalendarRefusal::InvalidRecurrence,
        CalendarRefusal::InvalidAvailability,
        CalendarRefusal::StaleAvailability,
        CalendarRefusal::IncomparableTime,
    ] {
        round_trip(value);
    }
    for value in [
        MeetingProposalRefusal::InvalidRequest,
        MeetingProposalRefusal::InvalidAvailability,
        MeetingProposalRefusal::StaleAvailability,
        MeetingProposalRefusal::MissingParticipant,
        MeetingProposalRefusal::NoCommonAvailability,
        MeetingProposalRefusal::IncomparableTime,
    ] {
        round_trip(value);
    }
}

#[test]
fn recurrence_schedule_and_window_refusals_are_native() {
    for value in [
        RecurrenceRefusal::InvalidIdentity,
        RecurrenceRefusal::InvalidRule,
        RecurrenceRefusal::InvalidLimit,
        RecurrenceRefusal::InvalidExceptions,
        RecurrenceRefusal::InvalidWindow,
        RecurrenceRefusal::IncomparableWindow,
        RecurrenceRefusal::WrongWindowKind,
        RecurrenceRefusal::WorkLimitExceeded,
        RecurrenceRefusal::ArithmeticOverflow,
        RecurrenceRefusal::CivilResolutionRequired,
        RecurrenceRefusal::InvalidCivilResolution,
        RecurrenceRefusal::CivilResolutionMismatch,
    ] {
        round_trip(value);
    }
    for value in [
        ScheduledIntentRefusal::InvalidIdentity,
        ScheduledIntentRefusal::InvalidOccurrence,
        ScheduledIntentRefusal::TriggerOccurrenceMismatch,
        ScheduledIntentRefusal::InvalidWindow,
        ScheduledIntentRefusal::IncomparableObservation,
        ScheduledIntentRefusal::WrongObservationProfile,
        ScheduledIntentRefusal::InvalidLatePolicy,
    ] {
        round_trip(value);
    }
    for value in [
        TemporalWindowRefusal::InvalidInstant,
        TemporalWindowRefusal::Incomparable,
        TemporalWindowRefusal::IntervalOverflow,
        TemporalWindowRefusal::Reversed,
        TemporalWindowRefusal::IndeterminateBoundaryOrder,
        TemporalWindowRefusal::Empty,
    ] {
        round_trip(value);
    }
}
