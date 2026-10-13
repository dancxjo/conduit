//! Private input-to-action correlation after root verifies completed scheduler origin.
//! The checked policy owns action choice; raw Space is never a second Fire return.
use super::*;
use crate::installed_std::{
    keyboard_input_binding::InputRealization, keyboard_input_completion::CompletedInputEvent,
};
use crate::terminal_face_mask::{TerminalInput, TerminalInputOutcome};
use crate::{BodyLiveForeAdmission, BodyLiveForeQueue};
use conduit_human::{
    InputButtonPhase, InputButtonTransition, KeyEvent, KeyModifiers, KeyTransition,
};
use conduit_presentation::ParsecMapCommand;

impl ParsecTerminalOwner {
    pub(in crate::installed_std::body_kernel) fn body_plan(&self) -> &BodyPlan {
        &self.plan
    }

    // Mechanism-only fixture: production epochs require an actual flushed prompt.
    #[cfg(test)]
    pub(in crate::installed_std::body_kernel) fn input_view_epoch(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
    ) -> Result<Option<u64>, String> {
        if self.input_inflight.is_some()
            || self.refresh_after_input
            || self.retired
            || self.face.is_none()
        {
            return Ok(None);
        }
        Ok(Some(
            self.current_face_show(map, host)?.1.show.play_sequence,
        ))
    }

    pub(in crate::installed_std::body_kernel) fn admit_completed_input(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        event: &CompletedInputEvent,
        bytes: &[u8],
        queue: &BodyLiveForeQueue,
    ) -> Result<bool, String> {
        if self.retired
            || host != &self.host
            || self.input_inflight.is_some()
            || self.refresh_after_input
            || event.input_view_epoch.is_none()
            || self.face.is_none()
        {
            return Ok(false);
        }
        if map.current_play().ok() != self.shown_play.as_ref()
            || map.current_state().ok() != Some(self.state.as_slice())
        {
            return Ok(false);
        }
        let (face, show) = self.current_face_show(map, host)?;
        if event.input_view_epoch != Some(show.show.play_sequence) {
            return Ok(false);
        }
        let face = face.clone();
        let show = show.clone();
        let control = match event.realization {
            InputRealization::RawKeyboard => {
                let key = KeyEvent::decode(bytes)
                    .map_err(|error| format!("Parsec completed key: {error:?}"))?;
                if key.usage() != 0x28
                    || key.transition() != KeyTransition::Pressed
                    || key.modifiers_after() != KeyModifiers::NONE
                {
                    return Ok(false);
                }
                "keyboard/enter"
            }
            InputRealization::SpaceButton => {
                let value = conduit_core::StructuredInfoValue::from_canonical_bytes(bytes)
                    .map_err(|error| format!("Parsec completed button: {error:?}"))?;
                let button = InputButtonTransition::from_structured(value)
                    .map_err(|error| format!("Parsec completed button laws: {error:?}"))?;
                if button.button_identity() != "button/primary"
                    || *button.phase() != InputButtonPhase::Pressed
                {
                    return Ok(false);
                }
                "control/fire"
            }
        };
        let state = ParsecMapState::decode(map.current_state()?)
            .map_err(|e| format!("Parsec input state: {e:?}"))?;
        let action = face
            .actions
            .first()
            .ok_or("Parsec current Face has no source action")?;
        let context = ActionInputBindingProposalInfo::new(
            true,
            host.boot_id.as_str().into(),
            control.into(),
            "Observed control".into(),
            face.identity.as_str().into(),
            host.host_id.as_str().into(),
            self.plan.plots[0].plan.plan_id.as_str().into(),
            *state.mapping_revision(),
            GeneratedActionAffordance::new(action.identity.clone(), face.revision)
                .map_err(|e| format!("Parsec current action: {e:?}"))?,
            show.show_id.as_str().into(),
            1,
        )
        .map_err(|e| format!("Parsec input context: {e:?}"))?
        .encode()
        .map_err(|e| format!("Parsec input context bytes: {e:?}"))?;
        let proposal = self
            .policy
            .evaluate(map.current_state()?, &context)
            .map_err(|e| format!("Parsec current policy: {e:?}"))?;
        let binding = ActionInputBindingProposalInfo::decode(proposal.proposal_bytes())
            .map_err(|e| format!("Parsec policy output: {e:?}"))?;
        if binding.control_identity() != control {
            return Ok(false);
        }
        if !*binding.available()
            || binding.face_identity() != face.identity.as_str()
            || binding.show_identity() != show.show_id.as_str()
            || binding.host_id() != host.host_id.as_str()
            || binding.boot_id() != host.boot_id.as_str()
            || binding.input_plan_id() != self.plan.plots[0].plan.plan_id.as_str()
            || *binding.mapping_revision() != *state.mapping_revision()
            || binding.offered_action().action_identity() != &action.identity
            || *binding.offered_action().source_presentation_revision() != face.revision
        {
            return Err(
                "Parsec source policy differs from exact current Face/Show/input context".into(),
            );
        }
        let result = self
            .terminal
            .as_mut()
            .ok_or("Parsec terminal unavailable")?
            .input(TerminalInput::Apply, &show, event.physical_event)
            .map_err(|e| format!("Parsec terminal Apply: {e:?}"))?;
        let TerminalInputOutcome::Interaction(interaction) = result else {
            return Err("Parsec terminal Apply emitted no typed interaction".into());
        };
        if interaction.action_id != action.identity || interaction.target != action.target {
            return Err("Parsec terminal focused a different source action".into());
        }
        self.refresh_after_input = true;
        let returned = self
            .execution
            .interact(interaction)
            .map_err(|e| format!("Parsec checked Mask return: {e:?}"))?;
        if returned.show_id != show.show_id
            || returned.mask_plot != show.mask_plot
            || returned.presentation_id != face.identity
            || returned.presentation_revision != face.revision
            || returned.interaction.face_id != face.identity.as_str()
            || returned.interaction.face_revision != face.revision
            || returned.interaction.show_id != show.show_id.as_str()
            || returned.interaction.action_id != action.identity
            || returned.interaction.target != action.target
            || !returned.interaction.arguments.is_empty()
            || returned.interaction.sequence != event.physical_event
        {
            self.retire();
            return Err("Parsec typed Mask return differs from admitted exact action".into());
        }
        let command = ParsecMapCommand::new(
            returned.interaction.action_id,
            *state.mapping_revision(),
            *state.keyboard(),
        )
        .map_err(|e| format!("Parsec command: {e:?}"))?
        .encode()
        .map_err(|e| format!("Parsec command bytes: {e:?}"))?;
        match queue.submit(&command) {
            Ok(BodyLiveForeAdmission::Accepted { sequence }) => {
                self.input_inflight = Some(
                    sequence
                        .checked_add(1)
                        .ok_or("Parsec ACK sequence overflow")?,
                );
                Ok(true)
            }
            Ok(BodyLiveForeAdmission::Full) => {
                self.input_inflight = Some(u64::from(queue.status().queue_accepted));
                Ok(false)
            }
            Err(error) => {
                self.retire();
                Err(error)
            }
        }
    }
}

#[cfg(test)]
#[path = "parsec_terminal_input/tests.rs"]
mod tests;
