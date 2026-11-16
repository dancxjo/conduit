//! Optional ordered-button shortcut to the ordinary focused terminal action.
//! Observations come from the caller's selected input source; this adapter does
//! not establish physical input availability or authorize an owner operation.
use super::{
    FaceInteraction, MaskShow, TerminalControl, TerminalError, TerminalFaceMask, TerminalInput,
    TerminalInputOutcome,
};
use conduit_human::{
    ButtonSequence, ButtonSequenceOutcome, ButtonSequenceRefusal, ButtonSequenceStep,
    InputButtonTransition,
};

#[derive(Debug, PartialEq, Eq)]
pub enum TerminalButtonSequenceRefusal {
    Pattern(ButtonSequenceRefusal),
    Encounter(TerminalError),
    Cancelled,
    AlreadyRecognized,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalButtonSequenceOutcome {
    pub recognition: ButtonSequenceOutcome,
    pub interaction: Option<FaceInteraction>,
}

#[derive(Clone, Copy)]
enum EncounterEnd {
    Cancelled,
    Recognized,
}

pub struct TerminalButtonSequence<'a> {
    recognizer: ButtonSequence<'a>,
    // A stale fence, never authority: the Mask checks its current encounter on
    // every sample, and the execution/owner still validate the returned action.
    expected: MaskShow,
    focus: TerminalControl,
    ended: Option<EncounterEnd>,
}

impl<'a> TerminalButtonSequence<'a> {
    pub fn prepare(
        mask: &TerminalFaceMask,
        expected: &MaskShow,
        pattern: &'a [ButtonSequenceStep<'a>],
    ) -> Result<Self, TerminalButtonSequenceRefusal> {
        mask.check_show(expected)
            .map_err(TerminalButtonSequenceRefusal::Encounter)?;
        if !mask.interaction_admitted {
            return Err(TerminalButtonSequenceRefusal::Encounter(
                TerminalError::UnsupportedInput,
            ));
        }
        let focus = mask.focus.ok_or(TerminalButtonSequenceRefusal::Encounter(
            TerminalError::UnknownControl,
        ))?;
        let recognizer =
            ButtonSequence::prepare(pattern).map_err(TerminalButtonSequenceRefusal::Pattern)?;
        Ok(Self {
            recognizer,
            expected: expected.clone(),
            focus,
            ended: None,
        })
    }

    /// Loss/cancellation retires this encounter. Prepare a fresh shortcut only
    /// after the ordinary input-source and current Show checks succeed again.
    pub fn cancel(&mut self) -> ButtonSequenceOutcome {
        self.ended.get_or_insert(EncounterEnd::Cancelled);
        self.recognizer.cancel()
    }

    pub fn observe(
        &mut self,
        mask: &mut TerminalFaceMask,
        event: &InputButtonTransition,
        interaction_sequence: u64,
    ) -> Result<TerminalButtonSequenceOutcome, TerminalButtonSequenceRefusal> {
        if let Some(end) = self.ended {
            return Err(match end {
                EncounterEnd::Cancelled => TerminalButtonSequenceRefusal::Cancelled,
                EncounterEnd::Recognized => TerminalButtonSequenceRefusal::AlreadyRecognized,
            });
        }
        let encounter = if mask.focus != Some(self.focus) {
            Err(TerminalError::StaleShow)
        } else {
            mask.check_show(&self.expected)
        };
        if let Err(error) = encounter {
            self.cancel();
            return Err(TerminalButtonSequenceRefusal::Encounter(error));
        }
        let recognition = self.recognizer.observe(event);
        let interaction = if recognition == ButtonSequenceOutcome::Recognized {
            self.ended = Some(EncounterEnd::Recognized);
            match mask
                .input(TerminalInput::Apply, &self.expected, interaction_sequence)
                .map_err(TerminalButtonSequenceRefusal::Encounter)?
            {
                TerminalInputOutcome::Interaction(interaction) => Some(interaction),
                _ => {
                    return Err(TerminalButtonSequenceRefusal::Encounter(
                        TerminalError::UnsupportedInput,
                    ))
                }
            }
        } else {
            None
        };
        Ok(TerminalButtonSequenceOutcome {
            recognition,
            interaction,
        })
    }
}

#[cfg(test)]
mod tests;
