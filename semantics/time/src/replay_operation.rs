//! Finite port-facing composition of replay timeline, control, and output values.

use crate::{
    decode_replay_command, decode_replay_timeline, encode_replay_event_into,
    encode_replay_state_into, BoundedReplayController, ReplayCommandCodecRefusal, ReplayPolicy,
    ReplayRefusal, ReplayState, ReplayTimelineCodecRefusal, MAXIMUM_REPLAY_DURATION_SECONDS,
    MAXIMUM_REPLAY_EVENT_BYTES, MAXIMUM_REPLAY_RATE_TERM, MAXIMUM_REPLAY_STATE_BYTES,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ReplayOperationOutput {
    pub event_bytes: Option<usize>,
    pub state_bytes: Option<usize>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplayOperationRefusal {
    InvalidPolicy,
    InvalidDurationLimit,
    TimelineWhileActive,
    Timeline(ReplayTimelineCodecRefusal),
    Command(ReplayCommandCodecRefusal),
    MissingTimeline,
    EventOutputTooSmall,
    StateOutputTooSmall,
    Replay(ReplayRefusal),
}

/// A reusable semantic operation over the exact value contracts named by the
/// replay-control Form. All storage remains caller-owned and bounded.
pub struct BoundedReplayOperation {
    policy: ReplayPolicy,
    maximum_duration_seconds: u64,
    controller: Option<BoundedReplayController>,
}

impl BoundedReplayOperation {
    pub fn new(policy: ReplayPolicy) -> Result<Self, ReplayOperationRefusal> {
        Self::new_with_maximum_duration(policy, MAXIMUM_REPLAY_DURATION_SECONDS)
    }

    pub fn new_with_maximum_duration(
        policy: ReplayPolicy,
        maximum_duration_seconds: u64,
    ) -> Result<Self, ReplayOperationRefusal> {
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
                return Err(ReplayOperationRefusal::InvalidPolicy);
            }
        }
        if maximum_duration_seconds == 0
            || maximum_duration_seconds > MAXIMUM_REPLAY_DURATION_SECONDS
        {
            return Err(ReplayOperationRefusal::InvalidDurationLimit);
        }
        Ok(Self {
            policy,
            maximum_duration_seconds,
            controller: None,
        })
    }

    pub fn load_timeline(&mut self, encoded: &[u8]) -> Result<(), ReplayOperationRefusal> {
        if self.controller.as_ref().is_some_and(|controller| {
            matches!(
                controller.state(),
                ReplayState::Running | ReplayState::Paused
            )
        }) {
            return Err(ReplayOperationRefusal::TimelineWhileActive);
        }
        let entries = decode_replay_timeline(encoded).map_err(ReplayOperationRefusal::Timeline)?;
        let controller = BoundedReplayController::new_with_maximum_duration(
            &entries,
            self.policy,
            self.maximum_duration_seconds,
        )
        .map_err(ReplayOperationRefusal::Replay)?;
        self.controller = Some(controller);
        Ok(())
    }

    pub fn apply_command(
        &mut self,
        encoded: &[u8],
        playback_ticks: u64,
        event_output: &mut [u8],
        state_output: &mut [u8],
    ) -> Result<ReplayOperationOutput, ReplayOperationRefusal> {
        Self::validate_output_capacity(event_output, state_output)?;
        let command = decode_replay_command(encoded).map_err(ReplayOperationRefusal::Command)?;
        let controller = self
            .controller
            .as_mut()
            .ok_or(ReplayOperationRefusal::MissingTimeline)?;
        let emission = controller
            .apply(command, playback_ticks)
            .map_err(ReplayOperationRefusal::Replay)?;
        let event_bytes = emission.map(|event| {
            encode_replay_event_into(event, event_output)
                .expect("validated replay entries fit the admitted maximum output")
        });
        let state_bytes = encode_replay_state_into(controller.state(), state_output)
            .expect("every replay state fits the admitted maximum output");
        Ok(ReplayOperationOutput {
            event_bytes,
            state_bytes: Some(state_bytes),
        })
    }

    pub fn poll(
        &mut self,
        playback_ticks: u64,
        event_output: &mut [u8],
        state_output: &mut [u8],
    ) -> Result<ReplayOperationOutput, ReplayOperationRefusal> {
        Self::validate_output_capacity(event_output, state_output)?;
        let controller = self
            .controller
            .as_mut()
            .ok_or(ReplayOperationRefusal::MissingTimeline)?;
        let Some(emission) = controller
            .poll(playback_ticks)
            .map_err(ReplayOperationRefusal::Replay)?
        else {
            return Ok(ReplayOperationOutput {
                event_bytes: None,
                state_bytes: None,
            });
        };
        let event_bytes = encode_replay_event_into(emission, event_output)
            .expect("validated replay entries fit the admitted maximum output");
        let state_bytes = encode_replay_state_into(controller.state(), state_output)
            .expect("every replay state fits the admitted maximum output");
        Ok(ReplayOperationOutput {
            event_bytes: Some(event_bytes),
            state_bytes: Some(state_bytes),
        })
    }

    pub fn state(&self) -> Option<ReplayState> {
        self.controller.as_ref().map(BoundedReplayController::state)
    }

    pub fn cursor(&self) -> Option<usize> {
        self.controller
            .as_ref()
            .map(BoundedReplayController::cursor)
    }

    fn validate_output_capacity(
        event_output: &[u8],
        state_output: &[u8],
    ) -> Result<(), ReplayOperationRefusal> {
        if event_output.len() < MAXIMUM_REPLAY_EVENT_BYTES {
            return Err(ReplayOperationRefusal::EventOutputTooSmall);
        }
        if state_output.len() < MAXIMUM_REPLAY_STATE_BYTES {
            return Err(ReplayOperationRefusal::StateOutputTooSmall);
        }
        Ok(())
    }
}
