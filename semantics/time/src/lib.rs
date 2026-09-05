#![no_std]

extern crate alloc;

mod tick;
pub use tick::*;

mod calendar;
mod calendar_proposal;
mod historical_command;
mod historical_timeline;
mod historical_timeline_codec;
mod replay_codec;
mod replay_control;
mod replay_source;
mod temporal_recurrence;
mod temporal_recurrence_civil;
mod temporal_schedule;
mod temporal_window;

pub use calendar::*;
pub use calendar_proposal::*;
pub use historical_command::*;
pub use historical_timeline::*;
pub use historical_timeline_codec::*;
pub use replay_codec::*;
pub use replay_control::*;
pub use replay_source::*;
pub use temporal_recurrence::*;
pub use temporal_recurrence_civil::*;
pub use temporal_schedule::*;
pub use temporal_window::*;

pub use conduit_core::{
    CivilTimeBasis, CivilTimeRefusal, ClockCorrelation, LocalDate, LocalDateTime, LocalTime,
    MonotonicClockIdentity, MonotonicDeadline, MonotonicDuration, MonotonicInstant,
    MonotonicTimeRefusal, NamedTimeZone, TemporalInstant, TemporalRelation, TemporalRelationError,
    TemporalScale, UtcOffsetSeconds, ZonedResolution, MAXIMUM_TEMPORAL_IDENTITY_BYTES,
    UNIX_UTC_CLOCK_BASIS,
};

#[cfg(feature = "form-catalog")]
mod catalog;
#[cfg(feature = "form-catalog")]
pub use catalog::*;
