//! Host-neutral normalization of a timed physical Morse key.

use alloc::{string::String, vec::Vec};

use crate::{
    MorseError, MorsePattern, MorseSegment, MAXIMUM_MORSE_SEGMENTS, MAXIMUM_MORSE_UNIT_MILLIS,
    MINIMUM_MORSE_UNIT_MILLIS,
};

pub const MAXIMUM_MORSE_CLOCK_BASIS_BYTES: usize = 96;
pub const MAXIMUM_MORSE_KEY_TRANSITIONS: u16 = (MAXIMUM_MORSE_SEGMENTS as u16) * 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MorseKeyPhase {
    Pressed,
    Released,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MorseKeyTransition {
    pub clock_basis: String,
    pub monotonic_micros: u64,
    pub phase: MorseKeyPhase,
    pub sequence: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MorseKeyRefusal {
    InvalidUnitMillis,
    InvalidTransitionCapacity,
    InvalidClockBasis,
    ClockBasisMismatch,
    DuplicateSequence,
    SequenceGap,
    NonMonotonicTime,
    WrongPhase,
    AmbiguousDuration,
    TransitionPressure,
    Empty,
    Incomplete,
    Cancelled,
    InvalidPattern(MorseError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MorseKeyInterpreter {
    clock_basis: String,
    unit_millis: u16,
    unit_micros: u64,
    maximum_transitions: u16,
    accepted_transitions: u16,
    next_sequence: u64,
    next_phase: MorseKeyPhase,
    last_micros: Option<u64>,
    segments: Vec<MorseSegment>,
    cancelled: bool,
}

impl MorseKeyInterpreter {
    pub fn new(
        clock_basis: impl Into<String>,
        unit_millis: u16,
        maximum_transitions: u16,
    ) -> Result<Self, MorseKeyRefusal> {
        if !(MINIMUM_MORSE_UNIT_MILLIS..=MAXIMUM_MORSE_UNIT_MILLIS).contains(&unit_millis) {
            return Err(MorseKeyRefusal::InvalidUnitMillis);
        }
        if maximum_transitions == 0 || maximum_transitions > MAXIMUM_MORSE_KEY_TRANSITIONS {
            return Err(MorseKeyRefusal::InvalidTransitionCapacity);
        }
        let clock_basis = clock_basis.into();
        if clock_basis.is_empty() || clock_basis.len() > MAXIMUM_MORSE_CLOCK_BASIS_BYTES {
            return Err(MorseKeyRefusal::InvalidClockBasis);
        }
        Ok(Self {
            clock_basis,
            unit_millis,
            unit_micros: u64::from(unit_millis) * 1_000,
            maximum_transitions,
            accepted_transitions: 0,
            next_sequence: 0,
            next_phase: MorseKeyPhase::Pressed,
            last_micros: None,
            segments: Vec::with_capacity(usize::from(maximum_transitions.saturating_sub(1))),
            cancelled: false,
        })
    }

    pub fn accept(&mut self, transition: &MorseKeyTransition) -> Result<(), MorseKeyRefusal> {
        if self.cancelled {
            return Err(MorseKeyRefusal::Cancelled);
        }
        if transition.clock_basis != self.clock_basis {
            return Err(MorseKeyRefusal::ClockBasisMismatch);
        }
        if transition.sequence < self.next_sequence {
            return Err(MorseKeyRefusal::DuplicateSequence);
        }
        if transition.sequence > self.next_sequence {
            return Err(MorseKeyRefusal::SequenceGap);
        }
        if self.accepted_transitions == self.maximum_transitions {
            return Err(MorseKeyRefusal::TransitionPressure);
        }
        if transition.phase != self.next_phase {
            return Err(MorseKeyRefusal::WrongPhase);
        }
        if self
            .last_micros
            .is_some_and(|previous| transition.monotonic_micros <= previous)
        {
            return Err(MorseKeyRefusal::NonMonotonicTime);
        }

        let segment = match (transition.phase, self.last_micros) {
            (MorseKeyPhase::Pressed, None) => None,
            (MorseKeyPhase::Pressed, Some(previous)) => Some(MorseSegment {
                level: false,
                units: self.classify(transition.monotonic_micros - previous, &[1, 3, 7])?,
            }),
            (MorseKeyPhase::Released, Some(previous)) => Some(MorseSegment {
                level: true,
                units: self.classify(transition.monotonic_micros - previous, &[1, 3])?,
            }),
            (MorseKeyPhase::Released, None) => return Err(MorseKeyRefusal::WrongPhase),
        };
        if let Some(segment) = segment {
            if self.segments.len() == MAXIMUM_MORSE_SEGMENTS {
                return Err(MorseKeyRefusal::TransitionPressure);
            }
            self.segments.push(segment);
        }
        self.last_micros = Some(transition.monotonic_micros);
        self.accepted_transitions += 1;
        self.next_sequence += 1;
        self.next_phase = match transition.phase {
            MorseKeyPhase::Pressed => MorseKeyPhase::Released,
            MorseKeyPhase::Released => MorseKeyPhase::Pressed,
        };
        Ok(())
    }

    pub fn finish(self) -> Result<MorsePattern, MorseKeyRefusal> {
        if self.cancelled {
            return Err(MorseKeyRefusal::Cancelled);
        }
        if self.accepted_transitions == 0 {
            return Err(MorseKeyRefusal::Empty);
        }
        if self.next_phase != MorseKeyPhase::Pressed || self.segments.is_empty() {
            return Err(MorseKeyRefusal::Incomplete);
        }
        let pattern = MorsePattern {
            unit_millis: self.unit_millis,
            segments: self.segments,
        };
        pattern.to_text().map_err(MorseKeyRefusal::InvalidPattern)?;
        Ok(pattern)
    }

    pub fn cancel(&mut self) {
        self.segments.clear();
        self.cancelled = true;
    }

    fn classify(&self, duration_micros: u64, allowed: &[u8]) -> Result<u8, MorseKeyRefusal> {
        let tolerance = self.unit_micros / 2;
        allowed
            .iter()
            .copied()
            .find(|units| {
                let target = self.unit_micros * u64::from(*units);
                duration_micros >= target - tolerance && duration_micros <= target + tolerance
            })
            .ok_or(MorseKeyRefusal::AmbiguousDuration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    const BASIS: &str = "fixture/boot-monotonic-us@1";

    fn transitions(pattern: &MorsePattern) -> Vec<MorseKeyTransition> {
        let mut values = Vec::new();
        let mut micros = 100_000_u64;
        let mut sequence = 0_u64;
        values.push(transition(sequence, micros, MorseKeyPhase::Pressed));
        for segment in &pattern.segments {
            micros += u64::from(pattern.unit_millis) * 1_000 * u64::from(segment.units);
            sequence += 1;
            values.push(transition(
                sequence,
                micros,
                if segment.level {
                    MorseKeyPhase::Released
                } else {
                    MorseKeyPhase::Pressed
                },
            ));
        }
        values
    }

    fn transition(sequence: u64, micros: u64, phase: MorseKeyPhase) -> MorseKeyTransition {
        MorseKeyTransition {
            clock_basis: BASIS.into(),
            monotonic_micros: micros,
            phase,
            sequence,
        }
    }

    #[test]
    fn sos_and_word_gap_normalize_to_the_same_canonical_pattern() {
        for text in ["SOS", "E E"] {
            let expected = MorsePattern::from_text(text, 200).unwrap();
            let values = transitions(&expected);
            let mut interpreter = MorseKeyInterpreter::new(BASIS, 200, 64).unwrap();
            for value in &values {
                interpreter.accept(value).unwrap();
            }
            let observed = interpreter.finish().unwrap();
            assert_eq!(observed, expected);
            assert_eq!(observed.to_text().unwrap(), text);
        }
    }

    #[test]
    fn timing_windows_normalize_jitter_but_refuse_ambiguous_values() {
        let mut accepted = MorseKeyInterpreter::new(BASIS, 200, 4).unwrap();
        accepted
            .accept(&transition(0, 1_000_000, MorseKeyPhase::Pressed))
            .unwrap();
        accepted
            .accept(&transition(1, 1_590_000, MorseKeyPhase::Released))
            .unwrap();
        assert_eq!(accepted.finish().unwrap().to_text().unwrap(), "T");

        let mut ambiguous = MorseKeyInterpreter::new(BASIS, 200, 4).unwrap();
        ambiguous
            .accept(&transition(0, 1_000_000, MorseKeyPhase::Pressed))
            .unwrap();
        assert_eq!(
            ambiguous.accept(&transition(1, 1_400_000, MorseKeyPhase::Released)),
            Err(MorseKeyRefusal::AmbiguousDuration)
        );
    }

    #[test]
    fn identity_order_pressure_completion_and_cancellation_fail_distinctly() {
        assert_eq!(
            MorseKeyInterpreter::new("", 200, 2),
            Err(MorseKeyRefusal::InvalidClockBasis)
        );
        assert_eq!(
            MorseKeyInterpreter::new(BASIS, 39, 2),
            Err(MorseKeyRefusal::InvalidUnitMillis)
        );
        assert_eq!(
            MorseKeyInterpreter::new(BASIS, 200, 0),
            Err(MorseKeyRefusal::InvalidTransitionCapacity)
        );
        assert_eq!(
            MorseKeyInterpreter::new(BASIS, 200, 2).unwrap().finish(),
            Err(MorseKeyRefusal::Empty)
        );

        let mut interpreter = MorseKeyInterpreter::new(BASIS, 200, 2).unwrap();
        let mut wrong_basis = transition(0, 1_000, MorseKeyPhase::Pressed);
        wrong_basis.clock_basis = "other/boot".into();
        assert_eq!(
            interpreter.accept(&wrong_basis),
            Err(MorseKeyRefusal::ClockBasisMismatch)
        );
        assert_eq!(
            interpreter.accept(&transition(1, 1_000, MorseKeyPhase::Pressed)),
            Err(MorseKeyRefusal::SequenceGap)
        );
        assert_eq!(
            interpreter.accept(&transition(0, 1_000, MorseKeyPhase::Released)),
            Err(MorseKeyRefusal::WrongPhase)
        );
        interpreter
            .accept(&transition(0, 1_000, MorseKeyPhase::Pressed))
            .unwrap();
        assert_eq!(
            interpreter.accept(&transition(0, 2_000, MorseKeyPhase::Released)),
            Err(MorseKeyRefusal::DuplicateSequence)
        );
        assert_eq!(
            interpreter.accept(&transition(1, 1_000, MorseKeyPhase::Released)),
            Err(MorseKeyRefusal::NonMonotonicTime)
        );
        assert_eq!(
            interpreter.clone().finish(),
            Err(MorseKeyRefusal::Incomplete)
        );
        interpreter
            .accept(&transition(1, 201_000, MorseKeyPhase::Released))
            .unwrap();
        assert_eq!(
            interpreter.accept(&transition(2, 401_000, MorseKeyPhase::Pressed)),
            Err(MorseKeyRefusal::TransitionPressure)
        );

        let mut cancelled = MorseKeyInterpreter::new(BASIS, 200, 2).unwrap();
        cancelled.cancel();
        assert_eq!(
            cancelled.accept(&transition(0, 1_000, MorseKeyPhase::Pressed)),
            Err(MorseKeyRefusal::Cancelled)
        );
        assert_eq!(cancelled.finish(), Err(MorseKeyRefusal::Cancelled));
    }

    #[test]
    fn structurally_canonical_but_unknown_symbol_is_not_accepted() {
        let pattern = MorsePattern {
            unit_millis: 200,
            segments: vec![
                MorseSegment {
                    level: true,
                    units: 1,
                },
                MorseSegment {
                    level: false,
                    units: 1,
                },
                MorseSegment {
                    level: true,
                    units: 1,
                },
                MorseSegment {
                    level: false,
                    units: 1,
                },
                MorseSegment {
                    level: true,
                    units: 3,
                },
                MorseSegment {
                    level: false,
                    units: 1,
                },
                MorseSegment {
                    level: true,
                    units: 3,
                },
            ],
        };
        let values = transitions(&pattern);
        let mut interpreter = MorseKeyInterpreter::new(BASIS, 200, 16).unwrap();
        for value in values {
            interpreter.accept(&value).unwrap();
        }
        assert!(matches!(
            interpreter.finish(),
            Err(MorseKeyRefusal::InvalidPattern(_))
        ));
    }
}
