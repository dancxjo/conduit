//! Finite replay sequencing with historical and playback time kept distinct.

use alloc::{string::String, vec::Vec};

pub const MAXIMUM_REPLAY_ENTRIES: usize = 64;
pub const MAXIMUM_REPLAY_IDENTITY_BYTES: usize = 128;
pub const MAXIMUM_REPLAY_RATE_TERM: u32 = 1_000;

pub const REPLAY_CONTROL_KIND: &str = "time/replay-control";
pub const REPLAY_CONTROL_CONTRACT_REVISION: &str = "conduit.time/replay-control@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalReplayEntry {
    pub identity: String,
    pub event_ticks: u64,
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
    pub historical_event_ticks: u64,
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
        validate_policy(policy)?;
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
            if index > 0 && entry.event_ticks < entries[index - 1].event_ticks {
                return Err(ReplayRefusal::ReorderedHistoricalTime);
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
            historical_event_ticks: entry.event_ticks,
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
            .event_ticks
            .checked_sub(self.entries[0].event_ticks)
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

#[cfg(feature = "form-catalog")]
pub fn replay_control_kind_definition() -> conduit_form::KindDefinition {
    use conduit_core::{
        kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
        StructuredInfoType,
    };
    let port = |name, value_kind, direction, temporal| PortDescriptor {
        port_id: port_id(name),
        value_kind: StructuredInfoType::leaf(kind_id(value_kind))
            .expect("reviewed replay value identity")
            .profile()
            .expect("reviewed replay value profile")
            .value_kind()
            .clone(),
        direction,
        temporal,
    };
    conduit_form::KindDefinition {
        kind_id: kind_id(REPLAY_CONTROL_KIND),
        kind_contract_revision: KindContractRevision::from(REPLAY_CONTROL_CONTRACT_REVISION),
        inputs: alloc::vec![
            port(
                "timeline",
                "history/replay-timeline@1",
                PortDirection::Input,
                PortTemporal::Value,
            ),
            port(
                "control",
                "history/replay-control@1",
                PortDirection::Input,
                PortTemporal::Flow { closes: true }
            ),
            port(
                "clock",
                "time/playback-tick@1",
                PortDirection::Input,
                PortTemporal::Flow { closes: true }
            ),
        ],
        outputs: alloc::vec![
            port(
                "event",
                "history/replay-event@1",
                PortDirection::Output,
                PortTemporal::Flow { closes: true }
            ),
            port(
                "state",
                "history/replay-state@1",
                PortDirection::Output,
                PortTemporal::Flow { closes: true }
            ),
        ],
        configuration: alloc::vec![],
    }
}
