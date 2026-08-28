//! Bounded cross-realization evidence at the portable musical seam.

use alloc::vec::Vec;
use conduit_audio::{Gate, MusicalNoteEvent};
use conduit_core::{BootId, HostId, ImplementationId, PlanId, TerminalDisposition};
use serde::{Deserialize, Serialize};

use super::IncompatibilityReason;

pub const MAXIMUM_NORMALIZED_SOUND_EVENTS: usize = 256;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormalizedGate {
    On,
    Off,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedNoteEvidence {
    pub occurrence: u64,
    pub gate: NormalizedGate,
    pub order: u32,
    pub requested_pitch_millihertz: u64,
    pub admitted_pitch_millihertz: u64,
}

impl NormalizedNoteEvidence {
    pub fn exact(event: MusicalNoteEvent) -> Self {
        Self::admitted(event, event.pitch.frequency_millihertz)
    }

    pub fn admitted(event: MusicalNoteEvent, admitted_pitch_millihertz: u64) -> Self {
        Self {
            occurrence: event.occurrence.0,
            gate: match event.gate {
                Gate::On => NormalizedGate::On,
                Gate::Off => NormalizedGate::Off,
            },
            order: event.order,
            requested_pitch_millihertz: event.pitch.frequency_millihertz,
            admitted_pitch_millihertz,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedSoundRealization {
    pub plan_id: PlanId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub implementation_id: ImplementationId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedSoundTrace {
    pub events: Vec<NormalizedNoteEvidence>,
    pub terminal: TerminalDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealizedSoundEvidence {
    pub selected: SelectedSoundRealization,
    pub trace: NormalizedSoundTrace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoundConformanceEvidence {
    Realized(RealizedSoundEvidence),
    Refused { reason: IncompatibilityReason },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SoundEvidenceError {
    EventCapacityExceeded,
    EventOrderNotIncreasing,
    GateLifecycleInvalid,
}

impl NormalizedSoundTrace {
    pub fn new(
        events: Vec<NormalizedNoteEvidence>,
        terminal: TerminalDisposition,
    ) -> Result<Self, SoundEvidenceError> {
        if events.len() > MAXIMUM_NORMALIZED_SOUND_EVENTS {
            return Err(SoundEvidenceError::EventCapacityExceeded);
        }
        if events.windows(2).any(|pair| pair[0].order >= pair[1].order) {
            return Err(SoundEvidenceError::EventOrderNotIncreasing);
        }
        for (index, event) in events.iter().enumerate() {
            let balance = events[..index]
                .iter()
                .filter(|prior| prior.occurrence == event.occurrence)
                .fold(0_i16, |balance, prior| match prior.gate {
                    NormalizedGate::On => balance + 1,
                    NormalizedGate::Off => balance - 1,
                });
            match event.gate {
                NormalizedGate::On if balance != 0 => {
                    return Err(SoundEvidenceError::GateLifecycleInvalid)
                }
                NormalizedGate::On => {}
                NormalizedGate::Off if balance != 1 => {
                    return Err(SoundEvidenceError::GateLifecycleInvalid)
                }
                NormalizedGate::Off => {}
            }
        }
        if terminal == TerminalDisposition::Completed
            && events.iter().enumerate().any(|(index, event)| {
                event.gate == NormalizedGate::On
                    && !events[index + 1..].iter().any(|later| {
                        later.occurrence == event.occurrence && later.gate == NormalizedGate::Off
                    })
            })
        {
            return Err(SoundEvidenceError::GateLifecycleInvalid);
        }
        Ok(Self { events, terminal })
    }

    /// Compares only portable event and terminal meaning. The selected exact
    /// realization remains visible on the containing evidence and is never
    /// flattened into semantic equality.
    pub fn semantically_matches(&self, other: &Self) -> bool {
        self == other
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use conduit_audio::{MusicalPitch, NoteOccurrenceId};
    use conduit_core::CancellationReason;

    use super::*;

    fn note(occurrence: u64, gate: Gate, order: u32) -> MusicalNoteEvent {
        MusicalNoteEvent::new(
            NoteOccurrenceId(occurrence),
            MusicalPitch::new(440_000, 440_000, 0).unwrap(),
            gate,
            u16::MAX,
            u64::from(order) * 1_000,
            order,
        )
        .unwrap()
    }

    #[test]
    fn exact_realizations_compare_semantics_without_erasing_selected_identity() {
        let events = vec![
            NormalizedNoteEvidence::exact(note(7, Gate::On, 1)),
            NormalizedNoteEvidence::exact(note(7, Gate::Off, 2)),
        ];
        let left = RealizedSoundEvidence {
            selected: selected("plan-opl", "opl-host", "opl-boot", "opl-implementation"),
            trace: NormalizedSoundTrace::new(events.clone(), TerminalDisposition::Completed)
                .unwrap(),
        };
        let right = RealizedSoundEvidence {
            selected: selected("plan-midi", "midi-host", "midi-boot", "midi-implementation"),
            trace: NormalizedSoundTrace::new(events, TerminalDisposition::Completed).unwrap(),
        };
        assert_ne!(left.selected, right.selected);
        assert!(left.trace.semantically_matches(&right.trace));
    }

    #[test]
    fn admitted_pitch_terminal_and_refusal_remain_distinct() {
        let requested = note(9, Gate::On, 1);
        let exact = NormalizedNoteEvidence::exact(requested);
        let quantized = NormalizedNoteEvidence::admitted(requested, 439_963);
        assert_ne!(exact, quantized);

        let completed = NormalizedSoundTrace::new(
            vec![exact, NormalizedNoteEvidence::exact(note(9, Gate::Off, 2))],
            TerminalDisposition::Completed,
        )
        .unwrap();
        let cancelled = NormalizedSoundTrace::new(
            vec![quantized],
            TerminalDisposition::Cancelled {
                reason: CancellationReason::OperatorRequested,
            },
        )
        .unwrap();
        assert!(!completed.semantically_matches(&cancelled));
        assert_ne!(
            SoundConformanceEvidence::Realized(RealizedSoundEvidence {
                selected: selected("plan", "host", "boot", "implementation"),
                trace: cancelled,
            }),
            SoundConformanceEvidence::Refused {
                reason: IncompatibilityReason::MicrotonalPitchUnsupported,
            }
        );
    }

    #[test]
    fn capacity_order_and_gate_lifecycle_fail_closed() {
        let oversized = (0..=MAXIMUM_NORMALIZED_SOUND_EVENTS)
            .map(|index| {
                NormalizedNoteEvidence::exact(note(index as u64 + 1, Gate::On, index as u32))
            })
            .collect();
        assert_eq!(
            NormalizedSoundTrace::new(oversized, TerminalDisposition::Completed),
            Err(SoundEvidenceError::EventCapacityExceeded)
        );
        assert_eq!(
            NormalizedSoundTrace::new(
                vec![NormalizedNoteEvidence::exact(note(1, Gate::Off, 1))],
                TerminalDisposition::Completed,
            ),
            Err(SoundEvidenceError::GateLifecycleInvalid)
        );
        assert_eq!(
            NormalizedSoundTrace::new(
                vec![
                    NormalizedNoteEvidence::exact(note(1, Gate::On, 2)),
                    NormalizedNoteEvidence::exact(note(1, Gate::Off, 1)),
                ],
                TerminalDisposition::Completed,
            ),
            Err(SoundEvidenceError::EventOrderNotIncreasing)
        );
    }

    fn selected(
        plan: &str,
        host: &str,
        boot: &str,
        implementation: &str,
    ) -> SelectedSoundRealization {
        SelectedSoundRealization {
            plan_id: PlanId::from(plan),
            host_id: HostId::from(host),
            boot_id: BootId::from(boot),
            implementation_id: ImplementationId::from(implementation),
        }
    }
}
