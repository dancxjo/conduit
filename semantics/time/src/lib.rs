#![no_std]

extern crate alloc;

#[allow(dead_code, clippy::clone_on_copy)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));

    impl Copy for SynchronizationOutcomeAdjusted {}
    impl Copy for SynchronizationOutcome {}

    impl WeekdaySet {
        pub const MONDAY: Self = Self(1 << 0);
        pub const TUESDAY: Self = Self(1 << 1);
        pub const WEDNESDAY: Self = Self(1 << 2);
        pub const THURSDAY: Self = Self(1 << 3);
        pub const FRIDAY: Self = Self(1 << 4);
        pub const SATURDAY: Self = Self(1 << 5);
        pub const SUNDAY: Self = Self(1 << 6);
        pub const WEEKDAYS: Self = Self((1 << 5) - 1);

        pub const fn union(self, other: Self) -> Self {
            Self(self.0 | other.0)
        }

        pub const fn contains(self, weekday: Self) -> bool {
            self.0 & weekday.0 != 0
        }

        pub const fn bits(self) -> u8 {
            self.0
        }
    }
}
pub use generated::{
    AvailabilityBasis, AvailabilityInterval, AvailabilityState, CalendarEvent, CalendarEventTime,
    CalendarRefusal, CandidateConflict, CivilFoldPolicy, CivilGapPolicy, CivilResolutionChoice,
    CivilResolutionPolicy, CivilTrigger, ClockChangeBehavior, ElapsedTrigger,
    HistoricalEntryOrigin, HistoricalEntryOriginForm, HistoricalOverflowPolicy,
    HistoricalOverflowPolicyForm, HistoricalReplayEntry, HistoricalRetentionGap,
    HistoricalTimelineCommand, HistoricalTimelineCommandAppend, HistoricalTimelineCommandRemove,
    HistoricalTimelineEntry, HistoricalTimelineOutcome, HistoricalTimelineOutcomeAppended,
    HistoricalTimelineOutcomeCleared, IntervalSequence, InvitationEvidence, InvitationState,
    LocalDate, LocalDateTime, LocalTime, MeetingCandidate, MeetingProposal, MeetingProposalRefusal,
    MeetingProposalRequest, MissedOccurrencePolicy, MonotonicClockIdentity, MonotonicDuration,
    MonotonicInstant, NamedPatternTemplate, NamedPatternTemplateCommand,
    NamedPatternTemplateResult, NamedPatternTemplateSlot, NamedPatternTemplateSlots, NamedTimeZone,
    NormalizedDurationSequence, OccurrenceInstant, OwnedReplayEvent, Participant,
    ParticipantAvailability, ParticipantRole, PatternComparison, PatternComparisonRefusal,
    ProposedMeetingSlot, PulseObservation, RecurrenceDefinition, RecurrenceExpansion,
    RecurrenceOccurrence, RecurrenceRefusal, RecurrenceRule, RecurrenceUntil, RecurrenceWindow,
    RejectedMeetingSlot, ReminderOccurrence, ReminderSpecification, ReplayCommand,
    ReplayCommandFail, ReplayPolicy, ReplayPolicyRate, ReplayState, ReplayStateFailed, RhythmState,
    ScheduleAssessment, ScheduleEffectIntent, ScheduleRefusal, ScheduleWorkflowLifecycle,
    ScheduledIntentRefusal, ScheduledOccurrenceDecision, SequenceNormalizationRefusal,
    SuspendBehavior, SynchronizationOutcome, SynchronizationOutcomeAdjusted,
    TemplateCollectionRefusal, TemporalBoundary, TemporalInstant, TemporalScale, TemporalWindow,
    TemporalWindowPosition, TemporalWindowRefusal, TimedCalendarSpan, TimedEventSequence,
    TimedPatternRefusal, TriggerObservation, TriggerProfile, WeekdaySet, WorkflowLifecycle,
    WorkflowTimingOutcome, WorkflowTimingOutcomeClockUncertain, WorkflowTimingOutcomeLate,
};

mod timed_pattern_refusal;

mod tick;
pub use tick::*;

mod pulse_observation;
pub use pulse_observation::*;

mod rhythm;
pub use rhythm::*;

mod calendar;
mod calendar_proposal;
mod civil_deadline;
mod historical_command;
mod historical_configuration;
mod historical_operation;
mod historical_store;
mod historical_timeline;
mod historical_timeline_codec;
mod native_temporal;
mod playback_tick;
mod replay_codec;
mod replay_command;
mod replay_configuration;
mod replay_control;
mod replay_operation;
mod replay_output;
mod replay_source;
mod retention_gap_codec;
mod temporal_instant;
mod temporal_recurrence;
mod temporal_recurrence_civil;
mod temporal_schedule;
mod temporal_window;

pub use calendar::*;
pub use calendar_proposal::*;
pub use civil_deadline::*;
pub use generated::{
    TemporalInstant as NativeTemporalInstant, TemporalScale as NativeTemporalScale,
};
pub use historical_command::*;
pub use historical_configuration::*;
pub use historical_operation::*;
pub use historical_store::*;
pub use historical_timeline::*;
pub use historical_timeline_codec::*;
pub use playback_tick::*;
pub use replay_codec::*;
pub use replay_command::*;
pub use replay_configuration::*;
pub use replay_control::*;
pub use replay_operation::*;
pub use replay_output::*;
pub use replay_source::*;
pub use retention_gap_codec::*;
pub use temporal_instant::*;
pub use temporal_recurrence::*;
pub use temporal_recurrence_civil::*;
pub use temporal_schedule::*;

pub use conduit_core::{
    CivilTimeBasis, CivilTimeRefusal, ClockCorrelation, MonotonicDeadline, MonotonicTimeRefusal,
    TemporalRelation, TemporalRelationError, UtcOffsetSeconds, ZonedResolution,
    MAXIMUM_TEMPORAL_IDENTITY_BYTES, UNIX_UTC_CLOCK_BASIS,
};

#[cfg(feature = "plot-catalog")]
mod catalog;
#[cfg(feature = "plot-catalog")]
pub use catalog::*;

#[cfg(feature = "kernel-step")]
mod button_attempt_back;
#[cfg(feature = "kernel-step")]
pub use button_attempt_back::TimedButtonAttemptBack;

#[cfg(feature = "kernel-step")]
mod debounce_back;
#[cfg(feature = "kernel-step")]
pub use debounce_back::{DebouncePreparationError, TrailingDebounceBack};

#[cfg(feature = "kernel-step")]
mod sample_back;
#[cfg(feature = "kernel-step")]
pub use sample_back::CadenceSampleBack;

#[cfg(feature = "kernel-step")]
mod deadline_back;
#[cfg(feature = "kernel-step")]
pub use deadline_back::CancellationDeadlineBack;

#[cfg(feature = "kernel-step")]
mod window_back;
#[cfg(feature = "kernel-step")]
pub use window_back::ProcessingTimeWindowBack;

#[cfg(feature = "kernel-step")]
mod pulse_observation_back;
#[cfg(feature = "kernel-step")]
pub use pulse_observation_back::PulseObservationBack;

#[cfg(feature = "kernel-step")]
mod phase_synchronization_back;
#[cfg(feature = "kernel-step")]
pub use phase_synchronization_back::PhaseSynchronizationBack;
