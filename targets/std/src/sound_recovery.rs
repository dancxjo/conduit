//! Exact finite musical-state policy across hosted sound provider replacement.

use conduit_audio::NoteOccurrenceId;
use conduit_core::{ActivePlayId, PlanId};
use serde::{Deserialize, Serialize};

const MIDI_CHANNELS: usize = 16;
const CONTROL_SLOTS: usize = 32;
const CENTER_PITCH_BEND: u16 = 8_192;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SoundStateTransferPolicy {
    CancelWithoutReplay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SoundInterruptionReason {
    ProviderLost,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InterruptedSoundState {
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub reason: SoundInterruptionReason,
    pub active_notes_interrupted: u16,
    pub sustain_was_down: bool,
    pub pitch_channels_reset: u16,
    pub controller_values_reset: u16,
    pub state_transfer_policy: SoundStateTransferPolicy,
    pub device_note_off_confirmed: bool,
    pub drain_confirmed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoundRecoveryError {
    DuplicateOccurrence,
    UnknownOccurrence,
    NoteCapacity,
    InvalidChannel,
    ControllerCapacity,
    AlreadyInterrupted,
    ReplacementPlanReused,
    ReplacementPlayReused,
    CompletionAlreadyPending,
    StaleCompletion,
    UnknownCompletion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveSoundState<const NOTES: usize> {
    plan_id: PlanId,
    active_play_id: ActivePlayId,
    active_notes: [Option<NoteOccurrenceId>; NOTES],
    sustain_down: bool,
    pitch_bend: [u16; MIDI_CHANNELS],
    pitch_changed: [bool; MIDI_CHANNELS],
    controllers: [Option<ControllerState>; CONTROL_SLOTS],
    interrupted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ControllerState {
    channel: u8,
    controller: u8,
    value: u8,
}

impl<const NOTES: usize> ActiveSoundState<NOTES> {
    pub fn new(plan_id: PlanId, active_play_id: ActivePlayId) -> Self {
        Self {
            plan_id,
            active_play_id,
            active_notes: [None; NOTES],
            sustain_down: false,
            pitch_bend: [CENTER_PITCH_BEND; MIDI_CHANNELS],
            pitch_changed: [false; MIDI_CHANNELS],
            controllers: [None; CONTROL_SLOTS],
            interrupted: false,
        }
    }

    pub fn note_on(&mut self, occurrence: NoteOccurrenceId) -> Result<(), SoundRecoveryError> {
        self.require_active()?;
        if self
            .active_notes
            .iter()
            .flatten()
            .any(|item| *item == occurrence)
        {
            return Err(SoundRecoveryError::DuplicateOccurrence);
        }
        let slot = self
            .active_notes
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(SoundRecoveryError::NoteCapacity)?;
        *slot = Some(occurrence);
        Ok(())
    }

    pub fn note_off(&mut self, occurrence: NoteOccurrenceId) -> Result<(), SoundRecoveryError> {
        self.require_active()?;
        let slot = self
            .active_notes
            .iter_mut()
            .find(|slot| slot.is_some_and(|item| item == occurrence))
            .ok_or(SoundRecoveryError::UnknownOccurrence)?;
        *slot = None;
        Ok(())
    }

    pub fn set_sustain(&mut self, down: bool) -> Result<(), SoundRecoveryError> {
        self.require_active()?;
        self.sustain_down = down;
        Ok(())
    }

    pub fn set_pitch_bend(&mut self, channel: u8, value: u16) -> Result<(), SoundRecoveryError> {
        self.require_active()?;
        let index = self.channel_index(channel)?;
        self.pitch_bend[index] = value;
        self.pitch_changed[index] = value != CENTER_PITCH_BEND;
        Ok(())
    }

    pub fn set_controller(
        &mut self,
        channel: u8,
        controller: u8,
        value: u8,
    ) -> Result<(), SoundRecoveryError> {
        self.require_active()?;
        self.channel_index(channel)?;
        if let Some(state) = self
            .controllers
            .iter_mut()
            .flatten()
            .find(|state| state.channel == channel && state.controller == controller)
        {
            state.value = value;
            return Ok(());
        }
        let slot = self
            .controllers
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(SoundRecoveryError::ControllerCapacity)?;
        *slot = Some(ControllerState {
            channel,
            controller,
            value,
        });
        Ok(())
    }

    pub fn provider_lost(&mut self) -> Result<InterruptedSoundState, SoundRecoveryError> {
        self.require_active()?;
        let active_notes_interrupted = u16::try_from(self.active_notes.iter().flatten().count())
            .map_err(|_| SoundRecoveryError::NoteCapacity)?;
        let pitch_channels_reset = self
            .pitch_changed
            .iter()
            .filter(|changed| **changed)
            .count() as u16;
        let controller_values_reset = self.controllers.iter().flatten().count() as u16;
        let record = InterruptedSoundState {
            plan_id: self.plan_id.clone(),
            active_play_id: self.active_play_id.clone(),
            reason: SoundInterruptionReason::ProviderLost,
            active_notes_interrupted,
            sustain_was_down: self.sustain_down,
            pitch_channels_reset,
            controller_values_reset,
            state_transfer_policy: SoundStateTransferPolicy::CancelWithoutReplay,
            device_note_off_confirmed: false,
            drain_confirmed: false,
        };
        self.active_notes.fill(None);
        self.sustain_down = false;
        self.pitch_bend.fill(CENTER_PITCH_BEND);
        self.pitch_changed.fill(false);
        self.controllers.fill(None);
        self.interrupted = true;
        Ok(record)
    }

    pub fn active_note_count(&self) -> usize {
        self.active_notes.iter().flatten().count()
    }

    pub fn sustain_down(&self) -> bool {
        self.sustain_down
    }

    pub fn changed_pitch_channel_count(&self) -> usize {
        self.pitch_changed
            .iter()
            .filter(|changed| **changed)
            .count()
    }

    pub fn controller_value_count(&self) -> usize {
        self.controllers.iter().flatten().count()
    }

    fn require_active(&self) -> Result<(), SoundRecoveryError> {
        if self.interrupted {
            Err(SoundRecoveryError::AlreadyInterrupted)
        } else {
            Ok(())
        }
    }

    fn channel_index(&self, channel: u8) -> Result<usize, SoundRecoveryError> {
        let index = usize::from(channel);
        if index < MIDI_CHANNELS {
            Ok(index)
        } else {
            Err(SoundRecoveryError::InvalidChannel)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementSoundState<const NOTES: usize> {
    state: ActiveSoundState<NOTES>,
    pending_completion_sequence: Option<u32>,
}

impl<const NOTES: usize> ReplacementSoundState<NOTES> {
    pub fn start(
        interrupted: &InterruptedSoundState,
        plan_id: PlanId,
        active_play_id: ActivePlayId,
    ) -> Result<Self, SoundRecoveryError> {
        if plan_id == interrupted.plan_id {
            return Err(SoundRecoveryError::ReplacementPlanReused);
        }
        if active_play_id == interrupted.active_play_id {
            return Err(SoundRecoveryError::ReplacementPlayReused);
        }
        Ok(Self {
            state: ActiveSoundState::new(plan_id, active_play_id),
            pending_completion_sequence: None,
        })
    }

    pub fn state(&self) -> &ActiveSoundState<NOTES> {
        &self.state
    }

    pub fn expect_completion(&mut self, sequence: u32) -> Result<(), SoundRecoveryError> {
        if self.pending_completion_sequence.is_some() {
            return Err(SoundRecoveryError::CompletionAlreadyPending);
        }
        self.pending_completion_sequence = Some(sequence);
        Ok(())
    }

    pub fn accept_completion(
        &mut self,
        active_play_id: &ActivePlayId,
        sequence: u32,
    ) -> Result<(), SoundRecoveryError> {
        if active_play_id != &self.state.active_play_id {
            return Err(SoundRecoveryError::StaleCompletion);
        }
        if self.pending_completion_sequence != Some(sequence) {
            return Err(SoundRecoveryError::UnknownCompletion);
        }
        self.pending_completion_sequence = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loss_cancels_notes_and_sustain_without_claiming_device_cleanup() {
        let mut state =
            ActiveSoundState::<2>::new(PlanId::from("plan-a"), ActivePlayId::from("play-a"));
        state.note_on(NoteOccurrenceId(1)).unwrap();
        state.note_on(NoteOccurrenceId(2)).unwrap();
        state.set_sustain(true).unwrap();
        state.set_pitch_bend(0, 9_000).unwrap();
        state.set_controller(0, 1, 64).unwrap();
        let interrupted = state.provider_lost().unwrap();
        assert_eq!(interrupted.active_notes_interrupted, 2);
        assert!(interrupted.sustain_was_down);
        assert_eq!(interrupted.pitch_channels_reset, 1);
        assert_eq!(interrupted.controller_values_reset, 1);
        assert!(!interrupted.device_note_off_confirmed);
        assert!(!interrupted.drain_confirmed);
        assert_eq!(state.active_note_count(), 0);
        assert!(!state.sustain_down());
        assert_eq!(state.changed_pitch_channel_count(), 0);
        assert_eq!(state.controller_value_count(), 0);
        assert_eq!(
            state.note_on(NoteOccurrenceId(3)),
            Err(SoundRecoveryError::AlreadyInterrupted)
        );
    }

    #[test]
    fn replacement_starts_empty_and_rejects_old_play_completion() {
        let interrupted = InterruptedSoundState {
            plan_id: PlanId::from("plan-a"),
            active_play_id: ActivePlayId::from("play-a"),
            reason: SoundInterruptionReason::ProviderLost,
            active_notes_interrupted: 2,
            sustain_was_down: true,
            pitch_channels_reset: 1,
            controller_values_reset: 1,
            state_transfer_policy: SoundStateTransferPolicy::CancelWithoutReplay,
            device_note_off_confirmed: false,
            drain_confirmed: false,
        };
        let mut replacement = ReplacementSoundState::<8>::start(
            &interrupted,
            PlanId::from("plan-b"),
            ActivePlayId::from("play-b"),
        )
        .unwrap();
        assert_eq!(replacement.state().active_note_count(), 0);
        assert!(!replacement.state().sustain_down());
        assert_eq!(replacement.state().changed_pitch_channel_count(), 0);
        assert_eq!(replacement.state().controller_value_count(), 0);
        replacement.expect_completion(7).unwrap();
        assert_eq!(
            replacement.expect_completion(8),
            Err(SoundRecoveryError::CompletionAlreadyPending)
        );
        assert_eq!(
            replacement.accept_completion(&interrupted.active_play_id, 7),
            Err(SoundRecoveryError::StaleCompletion)
        );
        assert_eq!(
            replacement.accept_completion(&ActivePlayId::from("play-b"), 8),
            Err(SoundRecoveryError::UnknownCompletion)
        );
        replacement
            .accept_completion(&ActivePlayId::from("play-b"), 7)
            .unwrap();
    }
}
