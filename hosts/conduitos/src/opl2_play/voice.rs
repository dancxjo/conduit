//! Finite occurrence-to-native-channel ownership for the OPL realization.

use conduit_core::{Gate, MusicalNoteEvent};

use super::{PreparedOpl2Execution, Voice};
use crate::{machine::Opl2Base, ordinary_plan::PreparationError};

pub(super) fn apply_event<B: Opl2Base>(
    execution: &mut PreparedOpl2Execution,
    base: &mut B,
    event: MusicalNoteEvent,
) -> Result<u64, PreparationError> {
    match event.gate {
        Gate::On => {
            if event.velocity != u16::MAX
                || execution
                    .voices
                    .iter()
                    .flatten()
                    .any(|voice| voice.occurrence == event.occurrence)
            {
                return Err(PreparationError::KernelRejected);
            }
            let channel = execution
                .voices
                .iter()
                .position(Option::is_none)
                .ok_or(PreparationError::KernelRejected)?;
            let pitch = base
                .key_on(channel as u8, event.pitch.frequency_millihertz)
                .map_err(|_| PreparationError::KernelRejected)?;
            execution.voices[channel] = Some(Voice {
                occurrence: event.occurrence,
                pitch,
            });
            execution.peak_voices = execution
                .peak_voices
                .max(execution.voices.iter().flatten().count() as u8);
            Ok(pitch.realized_millihertz)
        }
        Gate::Off => {
            let channel = execution
                .voices
                .iter()
                .position(|voice| voice.is_some_and(|voice| voice.occurrence == event.occurrence))
                .ok_or(PreparationError::KernelRejected)?;
            let admitted_pitch_millihertz = execution.voices[channel]
                .ok_or(PreparationError::KernelRejected)?
                .pitch
                .realized_millihertz;
            base.key_off(channel as u8)
                .map_err(|_| PreparationError::KernelRejected)?;
            execution.voices[channel] = None;
            Ok(admitted_pitch_millihertz)
        }
    }
}
