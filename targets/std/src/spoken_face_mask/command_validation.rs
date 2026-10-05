//! Read-only validation shared by a speech interruption preflight and command execution.

use conduit_presentation::{
    FaceInteraction, FaceInteractionArgument, FaceInteractionRefusal, FaceUtteranceProvenance,
    MaskShow, Presentation, PresentationActionAvailability, UTF8_TEXT_VALUE_KIND,
};

use super::{ReaderCommand, SpokenFaceRefusal, SpokenFaceSession};

impl SpokenFaceSession {
    /// Decide whether a queued command may interrupt selected playback without
    /// changing the cursor, draft, or in-flight speech. The caller still runs
    /// the command after the old Play has a terminal outcome.
    pub fn accepts_interruption(
        &self,
        current_face: &Presentation,
        current_show: &MaskShow,
        command: &ReaderCommand,
        sequence: u64,
    ) -> Result<(), SpokenFaceRefusal> {
        if *command == ReaderCommand::Stop {
            return Ok(());
        }
        self.check_current(current_face, current_show)?;
        match command {
            ReaderCommand::FocusSubject(identity) => {
                if !self.cursor.plan().clauses.iter().any(|clause| {
                    matches!(&clause.provenance, FaceUtteranceProvenance::Subject(subject) if subject.identity() == identity)
                }) {
                    return Err(SpokenFaceRefusal::UnknownSubject);
                }
            }
            ReaderCommand::FocusAction(identity) => {
                if !self.cursor.plan().clauses.iter().any(|clause| {
                    matches!(&clause.provenance, FaceUtteranceProvenance::Action(action) if action.identity() == identity)
                }) {
                    return Err(SpokenFaceRefusal::UnknownAction);
                }
            }
            ReaderCommand::Edit { argument, value } => {
                self.validated_edit(argument, value)?;
            }
            ReaderCommand::Activate => {
                self.validated_activation(sequence)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn validated_edit(
        &self,
        argument: &str,
        value: &[u8],
    ) -> Result<(String, String), SpokenFaceRefusal> {
        let action = self
            .focused_action()
            .ok_or(SpokenFaceRefusal::NoActionInFocus)?;
        let declaration = action
            .arguments
            .iter()
            .find(|item| item.name == argument)
            .ok_or(SpokenFaceRefusal::UnknownArgument)?;
        if declaration.contract.value_kind.as_str() != UTF8_TEXT_VALUE_KIND
            && declaration.contract.value_kind.as_str() != "value/bool"
        {
            return Err(SpokenFaceRefusal::UnsupportedValueKind);
        }
        declaration
            .contract
            .validate(value)
            .map_err(|_| SpokenFaceRefusal::InvalidValue)?;
        Ok((action.identity.clone(), declaration.value_name.clone()))
    }

    pub(super) fn validated_activation(
        &self,
        sequence: u64,
    ) -> Result<(FaceInteraction, String), SpokenFaceRefusal> {
        let action = self
            .focused_action()
            .ok_or(SpokenFaceRefusal::NoActionInFocus)?;
        if !action.availability.is_available() {
            return Err(SpokenFaceRefusal::Interaction(match action.availability {
                PresentationActionAvailability::Refused { .. } => {
                    FaceInteractionRefusal::RefusedAction
                }
                _ => FaceInteractionRefusal::UnavailableAction,
            }));
        }
        let arguments = action
            .arguments
            .iter()
            .map(|declaration| {
                self.drafts
                    .get(&(action.identity.clone(), declaration.name.clone()))
                    .map(|value| FaceInteractionArgument {
                        name: declaration.name.clone(),
                        value_kind: declaration.contract.value_kind.as_str().into(),
                        value: value.clone(),
                    })
                    .ok_or(SpokenFaceRefusal::Interaction(
                        FaceInteractionRefusal::MissingArgument,
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let accepted = FaceInteraction::new(
            &self.face,
            &self.show,
            &action.identity,
            &action.target,
            arguments,
            sequence,
        )
        .map_err(SpokenFaceRefusal::Interaction)?;
        Ok((accepted, action.name.clone()))
    }
}
