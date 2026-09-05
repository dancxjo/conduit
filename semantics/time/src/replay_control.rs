//! Finite replay sequencing with historical and playback time kept distinct.

use alloc::{string::String, vec::Vec};
use conduit_core::TemporalInstant;

pub const MAXIMUM_REPLAY_ENTRIES: usize = 64;
pub const MAXIMUM_REPLAY_IDENTITY_BYTES: usize = 128;
pub const MAXIMUM_REPLAY_RATE_TERM: u32 = 1_000;
pub const MAXIMUM_REPLAY_DURATION_SECONDS: u64 = 86_400;
pub const REPLAY_MODE_STEP: &str = "step";
pub const REPLAY_MODE_ORIGINAL_TIMING: &str = "original-timing";
pub const REPLAY_MODE_RATE: &str = "rate";

pub const REPLAY_CONTROL_KIND: &str = "time/replay-control";
pub const REPLAY_CONTROL_CONTRACT_REVISION: &str = "conduit.time/replay-control@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalReplayEntry {
    pub identity: String,
    pub event_time: TemporalInstant,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplayPolicy {
    Step,
    OriginalTiming,
    Rate { numerator: u32, denominator: u32 },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplayState {
    Stopped,
    Running,
    Paused,
    Completed,
    Failed { code: u16 },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ReplayEmission<'a> {
    pub ordinal: usize,
    pub historical_identity: &'a str,
    pub historical_event_time: &'a TemporalInstant,
    pub playback_ticks: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplayRefusal {
    EmptyTimeline,
    TooManyEntries,
    EmptyIdentity,
    IdentityTooLong,
    DuplicateIdentity,
    ReorderedHistoricalTime,
    InvalidRate,
    InvalidDurationLimit,
    ReplayDurationExceeded,
    InvalidHistoricalTime,
    IncomparableHistoricalTime,
    InvalidState,
    PlaybackClockRegressed,
    ArithmeticOverflow,
}

/// A bounded controller over retained entry metadata. Payload ownership stays
/// with the history source; an emission names the exact retained ordinal.
pub struct BoundedReplayController {
    entries: Vec<HistoricalReplayEntry>,
    policy: ReplayPolicy,
    state: ReplayState,
    cursor: usize,
    playback_origin: u64,
    last_playback_ticks: u64,
    paused_at: Option<u64>,
    accumulated_pause: u64,
}

impl BoundedReplayController {
    pub fn new(
        entries: &[HistoricalReplayEntry],
        policy: ReplayPolicy,
    ) -> Result<Self, ReplayRefusal> {
        Self::new_with_maximum_duration(entries, policy, MAXIMUM_REPLAY_DURATION_SECONDS)
    }

    pub fn new_with_maximum_duration(
        entries: &[HistoricalReplayEntry],
        policy: ReplayPolicy,
        maximum_duration_seconds: u64,
    ) -> Result<Self, ReplayRefusal> {
        validate_policy(policy)?;
        if maximum_duration_seconds == 0
            || maximum_duration_seconds > MAXIMUM_REPLAY_DURATION_SECONDS
        {
            return Err(ReplayRefusal::InvalidDurationLimit);
        }
        if entries.is_empty() {
            return Err(ReplayRefusal::EmptyTimeline);
        }
        if entries.len() > MAXIMUM_REPLAY_ENTRIES {
            return Err(ReplayRefusal::TooManyEntries);
        }
        for (index, entry) in entries.iter().enumerate() {
            if entry.identity.is_empty() {
                return Err(ReplayRefusal::EmptyIdentity);
            }
            if entry.identity.len() > MAXIMUM_REPLAY_IDENTITY_BYTES {
                return Err(ReplayRefusal::IdentityTooLong);
            }
            entry
                .event_time
                .validate()
                .map_err(|_| ReplayRefusal::InvalidHistoricalTime)?;
            if index > 0
                && (entry.event_time.clock_basis != entries[0].event_time.clock_basis
                    || entry.event_time.scale != entries[0].event_time.scale)
            {
                return Err(ReplayRefusal::IncomparableHistoricalTime);
            }
            if index > 0 && entry.event_time.ticks < entries[index - 1].event_time.ticks {
                return Err(ReplayRefusal::ReorderedHistoricalTime);
            }
            if index > 0
                && entry.event_time.ticks - entries[0].event_time.ticks
                    > maximum_duration_ticks(entry.event_time.scale, maximum_duration_seconds)
            {
                return Err(ReplayRefusal::ReplayDurationExceeded);
            }
            if entries[..index]
                .iter()
                .any(|prior| prior.identity == entry.identity)
            {
                return Err(ReplayRefusal::DuplicateIdentity);
            }
        }
        let mut owned = Vec::with_capacity(entries.len());
        owned.extend_from_slice(entries);
        Ok(Self {
            entries: owned,
            policy,
            state: ReplayState::Stopped,
            cursor: 0,
            playback_origin: 0,
            last_playback_ticks: 0,
            paused_at: None,
            accumulated_pause: 0,
        })
    }

    pub fn start(&mut self, playback_ticks: u64) -> Result<(), ReplayRefusal> {
        if self.state != ReplayState::Stopped {
            return Err(ReplayRefusal::InvalidState);
        }
        self.playback_origin = playback_ticks;
        self.last_playback_ticks = playback_ticks;
        self.accumulated_pause = 0;
        self.paused_at = None;
        self.state = match self.policy {
            ReplayPolicy::Step => ReplayState::Paused,
            ReplayPolicy::OriginalTiming | ReplayPolicy::Rate { .. } => ReplayState::Running,
        };
        Ok(())
    }

    pub fn pause(&mut self, playback_ticks: u64) -> Result<(), ReplayRefusal> {
        if self.state != ReplayState::Running {
            return Err(ReplayRefusal::InvalidState);
        }
        self.validate_clock(playback_ticks)?;
        self.elapsed(playback_ticks)?;
        self.last_playback_ticks = playback_ticks;
        self.paused_at = Some(playback_ticks);
        self.state = ReplayState::Paused;
        Ok(())
    }

    pub fn resume(&mut self, playback_ticks: u64) -> Result<(), ReplayRefusal> {
        if self.state != ReplayState::Paused || self.policy == ReplayPolicy::Step {
            return Err(ReplayRefusal::InvalidState);
        }
        self.validate_clock(playback_ticks)?;
        let paused_at = self.paused_at.ok_or(ReplayRefusal::InvalidState)?;
        let paused_duration = playback_ticks
            .checked_sub(paused_at)
            .ok_or(ReplayRefusal::PlaybackClockRegressed)?;
        self.accumulated_pause = self
            .accumulated_pause
            .checked_add(paused_duration)
            .ok_or(ReplayRefusal::ArithmeticOverflow)?;
        self.last_playback_ticks = playback_ticks;
        self.paused_at = None;
        self.state = ReplayState::Running;
        Ok(())
    }

    pub fn poll(
        &mut self,
        playback_ticks: u64,
    ) -> Result<Option<ReplayEmission<'_>>, ReplayRefusal> {
        if self.state != ReplayState::Running {
            return Err(ReplayRefusal::InvalidState);
        }
        self.validate_clock(playback_ticks)?;
        let elapsed = self.elapsed(playback_ticks)?;
        let required = self.required_elapsed(self.cursor)?;
        self.last_playback_ticks = playback_ticks;
        if elapsed < required {
            return Ok(None);
        }
        self.emit(playback_ticks).map(Some)
    }

    pub fn step(&mut self, playback_ticks: u64) -> Result<ReplayEmission<'_>, ReplayRefusal> {
        if self.state != ReplayState::Paused || self.policy != ReplayPolicy::Step {
            return Err(ReplayRefusal::InvalidState);
        }
        self.validate_clock(playback_ticks)?;
        self.last_playback_ticks = playback_ticks;
        self.emit(playback_ticks)
    }

    fn emit(&mut self, playback_ticks: u64) -> Result<ReplayEmission<'_>, ReplayRefusal> {
        let ordinal = self.cursor;
        let entry = self
            .entries
            .get(ordinal)
            .ok_or(ReplayRefusal::InvalidState)?;
        self.cursor += 1;
        if self.cursor == self.entries.len() {
            self.state = ReplayState::Completed;
        }
        Ok(ReplayEmission {
            ordinal,
            historical_identity: &entry.identity,
            historical_event_time: &entry.event_time,
            playback_ticks,
        })
    }

    fn elapsed(&self, playback_ticks: u64) -> Result<u64, ReplayRefusal> {
        playback_ticks
            .checked_sub(self.playback_origin)
            .and_then(|elapsed| elapsed.checked_sub(self.accumulated_pause))
            .ok_or(ReplayRefusal::PlaybackClockRegressed)
    }

    fn validate_clock(&self, playback_ticks: u64) -> Result<(), ReplayRefusal> {
        if playback_ticks < self.last_playback_ticks {
            return Err(ReplayRefusal::PlaybackClockRegressed);
        }
        Ok(())
    }

    fn required_elapsed(&self, ordinal: usize) -> Result<u64, ReplayRefusal> {
        let historical = self.entries[ordinal]
            .event_time
            .ticks
            .checked_sub(self.entries[0].event_time.ticks)
            .ok_or(ReplayRefusal::ReorderedHistoricalTime)?;
        match self.policy {
            ReplayPolicy::Step => Err(ReplayRefusal::InvalidState),
            ReplayPolicy::OriginalTiming => Ok(historical),
            ReplayPolicy::Rate {
                numerator,
                denominator,
            } => historical
                .checked_mul(u64::from(denominator))
                .and_then(|scaled| scaled.checked_div(u64::from(numerator)))
                .ok_or(ReplayRefusal::ArithmeticOverflow),
        }
    }

    pub fn restart(&mut self) {
        self.state = ReplayState::Stopped;
        self.cursor = 0;
        self.playback_origin = 0;
        self.last_playback_ticks = 0;
        self.paused_at = None;
        self.accumulated_pause = 0;
    }

    pub fn stop(&mut self) -> Result<(), ReplayRefusal> {
        if !matches!(self.state, ReplayState::Running | ReplayState::Paused) {
            return Err(ReplayRefusal::InvalidState);
        }
        self.state = ReplayState::Stopped;
        self.playback_origin = 0;
        self.last_playback_ticks = 0;
        self.paused_at = None;
        self.accumulated_pause = 0;
        Ok(())
    }

    pub fn apply(
        &mut self,
        command: crate::ReplayCommand,
        playback_ticks: u64,
    ) -> Result<Option<ReplayEmission<'_>>, ReplayRefusal> {
        match command {
            crate::ReplayCommand::Start => self.start(playback_ticks).map(|()| None),
            crate::ReplayCommand::Stop => self.stop().map(|()| None),
            crate::ReplayCommand::Pause => self.pause(playback_ticks).map(|()| None),
            crate::ReplayCommand::Resume => self.resume(playback_ticks).map(|()| None),
            crate::ReplayCommand::Restart => {
                self.restart();
                Ok(None)
            }
            crate::ReplayCommand::Step => self.step(playback_ticks).map(Some),
            crate::ReplayCommand::Fail { code } => self.fail(code).map(|()| None),
        }
    }

    pub fn fail(&mut self, code: u16) -> Result<(), ReplayRefusal> {
        if matches!(
            self.state,
            ReplayState::Completed | ReplayState::Failed { .. }
        ) {
            return Err(ReplayRefusal::InvalidState);
        }
        self.state = ReplayState::Failed { code };
        Ok(())
    }

    pub const fn state(&self) -> ReplayState {
        self.state
    }

    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

const fn maximum_duration_ticks(scale: conduit_core::TemporalScale, seconds: u64) -> u64 {
    match scale {
        conduit_core::TemporalScale::Seconds => seconds,
        conduit_core::TemporalScale::Milliseconds => seconds * 1_000,
        conduit_core::TemporalScale::Microseconds => seconds * 1_000_000,
        conduit_core::TemporalScale::Nanoseconds => seconds * 1_000_000_000,
    }
}

fn validate_policy(policy: ReplayPolicy) -> Result<(), ReplayRefusal> {
    if let ReplayPolicy::Rate {
        numerator,
        denominator,
    } = policy
    {
        if numerator == 0
            || denominator == 0
            || numerator > MAXIMUM_REPLAY_RATE_TERM
            || denominator > MAXIMUM_REPLAY_RATE_TERM
        {
            return Err(ReplayRefusal::InvalidRate);
        }
    }
    Ok(())
}
