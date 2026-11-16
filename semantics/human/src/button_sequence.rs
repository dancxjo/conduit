//! Bounded ordered button recognition, independent of Mask and action authority.
//! The borrowed pattern is admitted before use; recognition retains no event tape.
//! Consume one source's consecutive occurrence stream, not a merge of unrelated
//! counters or a filtered subset that conceals its intervening occurrences.
use crate::{InputButtonPhase, InputButtonTransition};

pub const MAXIMUM_BUTTON_SEQUENCE_STEPS: usize = 16;
pub const MAXIMUM_BUTTON_SEQUENCE_BUTTONS: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct ButtonSequenceStep<'a> {
    pub button_identity: &'a str,
    pub pressed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSequenceRefusal {
    Empty,
    Capacity,
    ButtonIdentity,
    UnbalancedPattern,
    UnexpectedButton,
    OutOfOrder,
    InputGap,
    UnmatchedRelease,
    Mismatch,
    AlreadyRecognized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSequenceOutcome {
    Pending { matched: usize },
    Recognized,
    Rejected(ButtonSequenceRefusal),
    Cancelled,
}

pub struct ButtonSequence<'a> {
    pattern: &'a [ButtonSequenceStep<'a>],
    buttons: [Option<&'a str>; MAXIMUM_BUTTON_SEQUENCE_BUTTONS],
    matched: usize,
    down: u8,
    draining: bool,
    last_sequence: Option<u64>,
    recognized: bool,
    continuity_failure: Option<ButtonSequenceRefusal>,
}

impl<'a> ButtonSequence<'a> {
    pub fn prepare(pattern: &'a [ButtonSequenceStep<'a>]) -> Result<Self, ButtonSequenceRefusal> {
        if pattern.is_empty() {
            return Err(ButtonSequenceRefusal::Empty);
        }
        if pattern.len() > MAXIMUM_BUTTON_SEQUENCE_STEPS {
            return Err(ButtonSequenceRefusal::Capacity);
        }
        let mut buttons = [None; MAXIMUM_BUTTON_SEQUENCE_BUTTONS];
        let mut down = 0_u8;
        for step in pattern {
            if step.button_identity.is_empty() || step.button_identity.len() > 256 {
                return Err(ButtonSequenceRefusal::ButtonIdentity);
            }
            let index = buttons
                .iter()
                .position(|value| *value == Some(step.button_identity))
                .or_else(|| buttons.iter().position(Option::is_none))
                .ok_or(ButtonSequenceRefusal::Capacity)?;
            buttons[index] = Some(step.button_identity);
            let bit = 1 << index;
            if (down & bit != 0) == step.pressed {
                return Err(ButtonSequenceRefusal::UnbalancedPattern);
            }
            down ^= bit;
        }
        if down != 0 {
            return Err(ButtonSequenceRefusal::UnbalancedPattern);
        }
        Ok(Self {
            pattern,
            buttons,
            matched: 0,
            down: 0,
            draining: false,
            last_sequence: None,
            recognized: false,
            continuity_failure: None,
        })
    }

    /// Abandon recognition, not an owner operation. Held controls must release
    /// before a fresh pattern can start; cancellation never rearms a held press.
    pub fn cancel(&mut self) -> ButtonSequenceOutcome {
        if self.recognized {
            return ButtonSequenceOutcome::Rejected(ButtonSequenceRefusal::AlreadyRecognized);
        }
        self.matched = 0;
        self.draining = self.down != 0;
        ButtonSequenceOutcome::Cancelled
    }

    fn reject(&mut self, reason: ButtonSequenceRefusal) -> ButtonSequenceOutcome {
        self.cancel();
        ButtonSequenceOutcome::Rejected(reason)
    }

    /// A gap or replay permanently fences this instance: missing observations
    /// cannot establish which controls are currently held. The caller must
    /// reestablish its input source before preparing a fresh recognizer.
    pub fn observe(&mut self, event: &InputButtonTransition) -> ButtonSequenceOutcome {
        if self.recognized {
            return ButtonSequenceOutcome::Rejected(ButtonSequenceRefusal::AlreadyRecognized);
        }
        if let Some(reason) = self.continuity_failure {
            return ButtonSequenceOutcome::Rejected(reason);
        }
        let sequence = *event.sequence();
        if let Some(previous) = self.last_sequence {
            if sequence <= previous {
                self.continuity_failure = Some(ButtonSequenceRefusal::OutOfOrder);
                return self.reject(ButtonSequenceRefusal::OutOfOrder);
            }
            if previous.checked_add(1) != Some(sequence) {
                self.last_sequence = Some(sequence);
                self.continuity_failure = Some(ButtonSequenceRefusal::InputGap);
                return self.reject(ButtonSequenceRefusal::InputGap);
            }
        }
        self.last_sequence = Some(sequence);
        let Some(index) = self
            .buttons
            .iter()
            .position(|value| *value == Some(event.button_identity().as_str()))
        else {
            return self.reject(ButtonSequenceRefusal::UnexpectedButton);
        };
        let bit = 1 << index;
        let pressed = match event.phase() {
            InputButtonPhase::Pressed => true,
            InputButtonPhase::Released => false,
        };
        if pressed && self.down & bit != 0 {
            return ButtonSequenceOutcome::Pending {
                matched: self.matched,
            };
        }
        if !pressed && self.down & bit == 0 {
            return self.reject(ButtonSequenceRefusal::UnmatchedRelease);
        }
        self.down ^= bit;
        if self.draining {
            self.draining = self.down != 0;
            return ButtonSequenceOutcome::Pending { matched: 0 };
        }
        let expected = &self.pattern[self.matched];
        if expected.button_identity != event.button_identity() || expected.pressed != pressed {
            return self.reject(ButtonSequenceRefusal::Mismatch);
        }
        self.matched += 1;
        if self.matched == self.pattern.len() {
            self.recognized = true;
            ButtonSequenceOutcome::Recognized
        } else {
            ButtonSequenceOutcome::Pending {
                matched: self.matched,
            }
        }
    }
}
