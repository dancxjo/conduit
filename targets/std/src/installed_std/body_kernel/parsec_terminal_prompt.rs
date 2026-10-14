//! Private input instructions correlated to actual accepted requests and Show.
//! Sidecar text is an output effect, not a new Show or physical input receipt.
use super::*;
use crate::installed_std::{
    keyboard_input_binding::InputRealization, keyboard_input_host::KeyboardInputHost,
};

#[derive(PartialEq, Eq)]
pub(super) struct CurrentPrompt {
    binding: ActionInputBindingProposalInfo,
    producer_plan: conduit_core::PlanId,
    producer_plot: conduit_core::PlotIdentity,
    text: String,
    epoch: u64,
    realization: InputRealization,
}

impl ParsecTerminalOwner {
    pub(in crate::installed_std::body_kernel) fn sync_prompt<W: Write + ?Sized>(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        keys: &KeyboardInputHost<'_>,
        output: &mut W,
    ) -> Result<bool, String> {
        let candidate = self.current_prompt(map, host, keys);
        let candidate = match candidate {
            Ok(candidate) => candidate,
            Err(error) => {
                let withdrawal = self.withdraw_prompt(output);
                self.retire();
                withdrawal?;
                return Err(error);
            }
        };
        let Some(candidate) = candidate else {
            return self.withdraw_prompt(output);
        };
        if self.prompt.as_ref() == Some(&candidate) {
            return Ok(false);
        }
        self.withdraw_prompt(output)?;
        // Bounded text is written and flushed before it can enable an epoch.
        let qualified = self
            .qualify_instruction(map, host, keys)?
            .ok_or("Parsec instruction lost accepted input scope")?;
        let qualified = self.revalidate_instruction(qualified, map, host, keys)?;
        self.prompt_visible = true;
        let effect =
            match self
                .execution
                .execute_instruction(qualified, host, output, &candidate.text)
            {
                Ok(effect) => effect,
                Err(error) => {
                    self.prompt = None;
                    // A failed write/flush can have exposed a prefix. Attempt visible
                    // withdrawal while refusing all input availability regardless.
                    let _ = self.withdraw_prompt(output);
                    self.retire();
                    return Err(format!("Parsec input prompt effect: {error}"));
                }
            };
        self.instruction_effect = Some(effect);
        self.prompt = Some(candidate);
        Ok(true)
    }

    pub(in crate::installed_std::body_kernel) fn withdraw_prompt<W: Write + ?Sized>(
        &mut self,
        output: &mut W,
    ) -> Result<bool, String> {
        self.prompt = None;
        self.instruction_effect = None;
        if !core::mem::take(&mut self.prompt_visible) {
            return Ok(false);
        }
        // The active prompt owns this current line (no trailing newline).
        if let Err(error) = output
            .write_all(b"\r\x1b[2KInput unavailable.\r\n")
            .and_then(|()| output.flush())
        {
            self.retire();
            return Err(format!("Parsec input prompt withdrawal: {error}"));
        }
        Ok(true)
    }

    pub(in crate::installed_std::body_kernel) fn prompt_input_view_epoch(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        keys: &KeyboardInputHost<'_>,
    ) -> Result<Option<u64>, String> {
        let current = self.current_prompt(map, host, keys)?;
        Ok(self
            .prompt
            .as_ref()
            .zip(current.as_ref())
            .and_then(|(shown, current)| (shown == current).then_some(shown.epoch)))
    }

    pub(in crate::installed_std::body_kernel) fn authorize_prompted_event(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        keys: &KeyboardInputHost<'_>,
        event: &crate::installed_std::keyboard_input_completion::CompletedInputEvent,
    ) -> Result<bool, String> {
        if keys
            .require_completed_event_scope(event, &self.plan, map.current_play()?, host)
            .is_err()
        {
            return Ok(false);
        }
        let current = self.current_prompt(map, host, keys)?;
        Ok(self
            .prompt
            .as_ref()
            .zip(current.as_ref())
            .is_some_and(|(shown, current)| {
                shown == current
                    && event.input_view_epoch == Some(shown.epoch)
                    && event.realization == shown.realization
            }))
    }

    /// Read-only snapshot only while the actual flushed prompt still matches
    /// current accepted scope, source map and exact Face/Show. No setter or replay.
    pub(in crate::installed_std::body_kernel) fn instruction_observation(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        keys: &KeyboardInputHost<'_>,
    ) -> Result<Option<crate::body_execution::ParsecInputInstructionObservation>, String> {
        use sha2::{Digest, Sha256};
        let current = self.current_prompt(map, host, keys)?;
        let Some((shown, current)) = self.prompt.as_ref().zip(current.as_ref()) else {
            return Ok(None);
        };
        if !self.prompt_visible || shown != current {
            return Ok(None);
        }
        let binding = shown.binding.clone();
        let producer_plan_id = shown.producer_plan.clone();
        let producer_plot = shown.producer_plot.clone();
        let prompt_text = shown.text.clone();
        let input_view_epoch = shown.epoch;
        let prompt_text_sha256 = Sha256::digest(prompt_text.as_bytes()).into();
        let (face, show) = self.current_face_show(map, host)?;
        Ok(Some(crate::body_execution::ParsecInputInstructionObservation {
            proof_class: "installed software input scope + checked policy + original internal Source Fore + supplementary terminal write/flush and effect ACK; no physical or other-Mask receipt",
            face: face.clone(), show: show.clone(), body_play: map.current_play()?.clone(),
            host: conduit_core::PreparationHostIdentity { host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(), offer_generation: host.offer_generation },
            binding, producer_plan_id, producer_plot, prompt_text, prompt_text_sha256,
            input_view_epoch,
            effect: self.instruction_effect.clone().ok_or("actual instruction effect observation is absent")?,
        }))
    }

    fn current_prompt(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        keys: &KeyboardInputHost<'_>,
    ) -> Result<Option<CurrentPrompt>, String> {
        if self.retired
            || self.input_inflight.is_some()
            || self.refresh_after_input
            || self.face.is_none()
        {
            return Ok(None);
        }
        let (face, show) = self.current_face_show(map, host)?;
        let face = face.clone();
        let show = show.clone();
        let state = ParsecMapState::decode(map.current_state()?)
            .map_err(|e| format!("Parsec prompt map: {e:?}"))?;
        let action = face
            .actions
            .first()
            .ok_or("Parsec prompt has no source action")?;
        let context = |available| {
            ActionInputBindingProposalInfo::new(
                available,
                host.boot_id.as_str().into(),
                "parsec/prompt-candidate".into(),
                "Candidate".into(),
                face.identity.as_str().into(),
                host.host_id.as_str().into(),
                self.plan.plots[0].plan.plan_id.as_str().into(),
                *state.mapping_revision(),
                GeneratedActionAffordance::new(action.identity.clone(), face.revision)
                    .map_err(|e| format!("Parsec prompt action: {e:?}"))?,
                show.show_id.as_str().into(),
                1,
            )
            .map_err(|e| format!("Parsec prompt context: {e:?}"))?
            .encode()
            .map_err(|e| format!("Parsec prompt encoding: {e:?}"))
        };
        let unavailable = context(false)?;
        let proposed = self
            .policy
            .evaluate(map.current_state()?, &unavailable)
            .map_err(|e| format!("Parsec proposed prompt policy: {e:?}"))?;
        let proposed = ActionInputBindingProposalInfo::decode(proposed.proposal_bytes())
            .map_err(|e| format!("Parsec proposed prompt: {e:?}"))?;
        let (realization, label) = match proposed.control_identity().as_str() {
            "control/fire" => (InputRealization::SpaceButton, "Space"),
            "keyboard/enter" => (InputRealization::RawKeyboard, "Enter"),
            _ => return Err("Parsec source policy proposes unsupported input realization".into()),
        };
        // This derives availability from actual request admission, never the
        // source proposal's available field or the presence of an adapter.
        let Ok(input_plan) =
            keys.require_current_input_scope(realization, &self.plan, map.current_play()?, host)
        else {
            return Ok(None);
        };
        if input_plan != &self.plan.plots[0].plan.plan_id {
            return Err("Parsec prompt origin belongs to a different input Plan".into());
        }
        let available = context(true)?;
        let produced = self
            .policy
            .evaluate(map.current_state()?, &available)
            .map_err(|e| format!("Parsec current prompt policy: {e:?}"))?;
        let binding = ActionInputBindingProposalInfo::decode(produced.proposal_bytes())
            .map_err(|e| format!("Parsec current prompt: {e:?}"))?;
        if !*binding.available()
            || binding.control_identity() != proposed.control_identity()
            || binding.control_label() != label
            || binding.face_identity() != face.identity.as_str()
            || binding.show_identity() != show.show_id.as_str()
            || binding.host_id() != host.host_id.as_str()
            || binding.boot_id() != host.boot_id.as_str()
            || binding.input_plan_id() != input_plan.as_str()
            || *binding.mapping_revision() != *state.mapping_revision()
            || *binding.offered_action().source_face_revision() != face.revision
            || binding.offered_action().action_identity() != &action.identity
        {
            return Err("Parsec prompt differs from source action/current input/Face/Show".into());
        }
        face.resolve_action(face.revision, binding.offered_action().action_identity())
            .map_err(|e| format!("Parsec prompt action correlation: {e:?}"))?;
        let binding_bytes = binding
            .clone()
            .encode()
            .map_err(|e| format!("Parsec instruction binding: {e:?}"))?;
        let instruction = self
            .instruction
            .evaluate(
                map.current_state()?,
                &binding_bytes,
                conduit_face::ParsecInputInstructionLook::Linear,
                false,
            )
            .map_err(|e| format!("Parsec instruction wording: {e:?}"))?;
        let text = format!("\r\x1b[2K{}", instruction.wording());
        if text.len() > 256 {
            return Err("Parsec prompt exceeds its bounded terminal line".into());
        }
        Ok(Some(CurrentPrompt {
            binding,
            producer_plan: produced.producer_plan_id().clone(),
            producer_plot: produced.producer_plot().clone(),
            text,
            epoch: show.show.play_sequence,
            realization,
        }))
    }
}

/// Runtime custody minted only by the private owner. It cannot be cloned,
/// deserialized or constructed from a documentary instruction observation.
pub(crate) struct QualifiedParsecInstruction {
    binding: Vec<u8>,
    map: Vec<u8>,
    face: Face,
    show: MaskShow,
    parent_plan: conduit_body::BodyPlan,
    parent_play: conduit_body::BodyPlayIdentity,
    host: HostAdvertisement,
}

impl QualifiedParsecInstruction {
    pub(crate) fn binding_bytes(&self) -> &[u8] {
        &self.binding
    }
    pub(crate) fn map_bytes(&self) -> &[u8] {
        &self.map
    }

    pub(crate) fn require_source_scope(
        &self,
        parent_plan: &conduit_body::BodyPlan,
        parent_play: &conduit_body::BodyPlayIdentity,
        mask: &conduit_face::PlannedMaskPlot,
        mask_play: &conduit_core::ActivePlayIdentity,
        host: &HostAdvertisement,
    ) -> Result<(), String> {
        let expected = conduit_core::bind_active_play(
            &mask.plan.plan_id,
            &host.host_id,
            &host.boot_id,
            self.show.show.play_sequence,
        );
        if parent_plan != &self.parent_plan
            || parent_play != &self.parent_play
            || !parent_play.validate_for(parent_plan)
            || host != &self.host
            || mask != &self.show.planned_mask
            || mask_play != &expected
            || self.show.show.active_play_id != expected.active_play_id
        {
            return Err("Parsec instruction belongs to a different Body/Mask/Host scope".into());
        }
        self.show
            .validate(&self.face)
            .map_err(|e| format!("Parsec instruction Show: {e:?}"))
    }
}

impl ParsecTerminalOwner {
    pub(in crate::installed_std::body_kernel) fn qualify_instruction(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        keys: &KeyboardInputHost<'_>,
    ) -> Result<Option<QualifiedParsecInstruction>, String> {
        let Some(current) = self.current_prompt(map, host, keys)? else {
            return Ok(None);
        };
        let binding = current
            .binding
            .encode()
            .map_err(|e| format!("Parsec qualified binding: {e:?}"))?;
        let (face, show) = self.current_face_show(map, host)?;
        let face = face.clone();
        let show = show.clone();
        Ok(Some(QualifiedParsecInstruction {
            binding,
            map: map.current_state()?.to_vec(),
            face,
            show,
            parent_plan: self.plan.clone(),
            parent_play: map.current_play()?.clone(),
            host: host.clone(),
        }))
    }

    /// Reconsume current owner truth immediately before Source completion.
    /// A retained witness cannot authorize after remap, Show consumption or loss.
    pub(in crate::installed_std::body_kernel) fn revalidate_instruction(
        &mut self,
        witness: QualifiedParsecInstruction,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        keys: &KeyboardInputHost<'_>,
    ) -> Result<QualifiedParsecInstruction, String> {
        let current = self
            .qualify_instruction(map, host, keys)?
            .ok_or("Parsec instruction input scope is no longer available")?;
        if current.binding != witness.binding
            || current.map != witness.map
            || current.face != witness.face
            || current.show != witness.show
            || current.parent_plan != witness.parent_plan
            || current.parent_play != witness.parent_play
            || current.host != witness.host
        {
            return Err("Parsec instruction witness is stale".into());
        }
        Ok(current)
    }
}

#[cfg(test)]
#[path = "parsec_terminal_prompt_tests.rs"]
pub(super) mod tests;
