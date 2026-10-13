//! Private source-derived Face and concrete acknowledged terminal Show ownership.
use super::parsec_acknowledged_map::ParsecAcknowledgedMap;
use crate::{
    terminal_face_mask::{TerminalFaceMask, TerminalMaskExecution},
    terminal_mask_execution::HostedTerminalMaskExecution,
};
use conduit_body::{BodyPlan, BodyPlayIdentity};
use conduit_core::HostAdvertisement;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_presentation::{
    ActionInputBindingProposalInfo, GeneratedActionAffordance, MaskShow, ParsecMapState,
    PreparedParsecInputPolicy, Presentation, PARSEC_MAP_FRAME_BYTES,
};
use std::io::Write;

#[path = "parsec_terminal_face.rs"]
mod face;
#[path = "parsec_terminal_input.rs"]
mod input;
#[path = "parsec_terminal_prompt.rs"]
mod prompt;

pub(super) struct ParsecTerminalOwner {
    plan: BodyPlan,
    host: HostAdvertisement,
    policy: PreparedParsecInputPolicy,
    execution: HostedTerminalMaskExecution,
    terminal: Option<TerminalFaceMask>,
    face: Option<Presentation>,
    state: Vec<u8>,
    shown_play: Option<BodyPlayIdentity>,
    columns: usize,
    rows: usize,
    retired: bool,
    maximum_shows: u16,
    rendered_shows: u16,
    scoped_play: Option<BodyPlayIdentity>,
    input_inflight: Option<u64>,
    refresh_after_input: bool,
    prompt: Option<prompt::CurrentPrompt>,
    prompt_visible: bool,
}
impl ParsecTerminalOwner {
    pub(super) fn prepare(
        plan: &BodyPlan,
        host: &HostAdvertisement,
        columns: usize,
        rows: usize,
    ) -> Result<Self, String> {
        if plan.plots.len() != 1 {
            return Err("Parsec terminal owner requires one exact parent".into());
        }
        crate::BodyLiveForeQueue::validate_parsec_input_plan(&plan.plots[0].plan, host)?;
        if !(40..=240).contains(&columns) || !(8..=100).contains(&rows) {
            return Err("Parsec terminal extent is outside admitted bounds".into());
        }
        let conduit_core::PlannedActivationEntry::Scan(scan) = &plan.plots[0].plan.activations[0]
        else {
            return Err("Parsec terminal owner has no exact selected scan".into());
        };
        let maximum_shows = scan
            .limits
            .maximum_items
            .checked_add(1)
            .ok_or("Parsec selected Show count overflows")?;
        let source = crate::flow_activation::ParsecInputPolicySourcePlan::prepare(host)?;
        Ok(Self {
            plan: plan.clone(),
            host: host.clone(),
            policy: source.evaluator()?,
            execution: HostedTerminalMaskExecution::new_parsec_attached(host)
                .map_err(|e| e.to_string())?,
            terminal: None,
            face: None,
            state: Vec::with_capacity(PARSEC_MAP_FRAME_BYTES),
            shown_play: None,
            columns,
            rows,
            retired: false,
            maximum_shows,
            rendered_shows: 0,
            scoped_play: None,
            input_inflight: None,
            refresh_after_input: false,
            prompt: None,
            prompt_visible: false,
        })
    }

    /// Projection and terminal effect only, not input admission. Foreground
    /// Face/renderer preparation allocates outside the kernel Step. Reuse this
    /// concrete Mask execution across map changes so its Show sequence advances.
    pub(super) fn ensure_present<W: Write + ?Sized>(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        output: &mut W,
    ) -> Result<bool, String> {
        let result = self.ensure_present_inner(map, host, output);
        if result.is_err() {
            self.retire();
        }
        result
    }

    fn ensure_present_inner<W: Write + ?Sized>(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        output: &mut W,
    ) -> Result<bool, String> {
        self.check_host(host)?;
        let play = map.current_play()?;
        if !play.validate_for(&self.plan)
            || self.shown_play.as_ref().is_some_and(|shown| shown != play)
        {
            return Err("Parsec terminal cannot replace its issued Body Play".into());
        }
        if self
            .input_inflight
            .is_some_and(|count| map.acknowledged_items() < count)
        {
            return Ok(false);
        }
        self.input_inflight = None;
        if !self.refresh_after_input
            && self.shown_play.as_ref() == Some(play)
            && map.current_state()? == self.state
        {
            self.current_face_show(map, host)?;
            return Ok(false);
        }
        self.present(map, host, output)?;
        Ok(true)
    }

    /// Allocating Face/renderer preparation sits outside the kernel Step. The
    /// candidate context is unavailable and cannot become a current binding.
    pub(super) fn present<W: Write + ?Sized>(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        output: &mut W,
    ) -> Result<(), String> {
        let result = self.present_inner(map, host, output);
        if result.is_err() {
            self.retire();
        }
        result
    }
    fn present_inner<W: Write + ?Sized>(
        &mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
        output: &mut W,
    ) -> Result<(), String> {
        self.check_host(host)?;
        let play = map.current_play()?;
        if !play.validate_for(&self.plan)
            || self.shown_play.as_ref().is_some_and(|shown| shown != play)
        {
            return Err("Parsec terminal has a different current Body Play".into());
        }
        if self.rendered_shows >= self.maximum_shows {
            return Err("Parsec terminal exceeds selected scan Show bound".into());
        }
        if let Some(scoped) = &self.scoped_play {
            if scoped != play {
                return Err("Parsec terminal has a different reserved Body Play scope".into());
            }
        } else {
            self.execution
                .bind_parsec_body_scope(play, self.maximum_shows)
                .map_err(|e| e.to_string())?;
            self.scoped_play = Some(play.clone());
        }
        let state = map.current_state()?;
        let native =
            ParsecMapState::decode(state).map_err(|e| format!("Parsec current map: {e:?}"))?;
        // These valid placeholders permit source-owned action projection before
        // an actual Face/Show exists. No caller can receive this candidate.
        let context = ActionInputBindingProposalInfo::new(
            false,
            host.boot_id.as_str().into(),
            "parsec/candidate-control".into(),
            "Candidate".into(),
            "parsec/candidate-face".into(),
            host.host_id.as_str().into(),
            self.plan.plots[0].plan.plan_id.as_str().into(),
            *native.mapping_revision(),
            GeneratedActionAffordance::new(
                "parsec/candidate-action".into(),
                *native.mapping_revision(),
            )
            .map_err(|e| format!("Parsec candidate action: {e:?}"))?,
            "parsec/candidate-show".into(),
            1,
        )
        .map_err(|e| format!("Parsec candidate context: {e:?}"))?
        .encode()
        .map_err(|e| format!("Parsec candidate encoding: {e:?}"))?;
        let proposal = self
            .policy
            .evaluate(state, &context)
            .map_err(|e| format!("Parsec source policy: {e:?}"))?;
        let face = face::project(&self.plan, play, &proposal)?;
        self.withdraw_prompt(output)?;
        // The old ordinary Mask Play is retired before a new Face can be shown.
        self.execution.cancel().map_err(|e| e.to_string())?;
        self.terminal = None;
        self.face = None;
        self.state.clear();
        let mut terminal = TerminalFaceMask::prepare(face.clone(), self.columns, self.rows)
            .map_err(|e| e.to_string())?;
        terminal
            .focus_prepared_action(&face.actions[0].identity)
            .map_err(|e| format!("Parsec prepared action focus: {e:?}"))?;
        // This concrete execution, not an arbitrary caller's trait implementation,
        // performs receipt -> actual Host Call completion -> actual Show Fore.
        terminal
            .present(&mut self.execution, output)
            .map_err(|e| e.to_string())?;
        if terminal.show().is_none() {
            return Err("Parsec terminal produced no acknowledged Show".into());
        }
        self.state.extend_from_slice(state);
        self.rendered_shows += 1;
        self.refresh_after_input = false;
        self.input_inflight = None;
        self.shown_play = Some(play.clone());
        self.face = Some(face);
        self.terminal = Some(terminal);
        Ok(())
    }

    pub(super) fn current_face_show<'a>(
        &'a mut self,
        map: &ParsecAcknowledgedMap,
        host: &HostAdvertisement,
    ) -> Result<(&'a Presentation, &'a MaskShow), String> {
        if self.input_inflight.is_some() || self.refresh_after_input {
            return Err("Parsec consumed Show awaits source acknowledgement".into());
        }
        let valid = self.check_host(host).and_then(|()| {
            let play = map.current_play()?;
            if !play.validate_for(&self.plan)
                || self.shown_play.as_ref() != Some(play)
                || map.current_state()? != self.state
            {
                return Err("Parsec terminal Face differs from current acknowledged map".into());
            }
            if self.face.is_none()
                || self
                    .terminal
                    .as_ref()
                    .and_then(TerminalFaceMask::show)
                    .is_none()
            {
                return Err("Parsec terminal Show has not been acknowledged".into());
            }
            Ok(())
        });
        if let Err(error) = valid {
            self.retire();
            return Err(error);
        }
        Ok((
            self.face.as_ref().ok_or("Parsec Face is unavailable")?,
            self.terminal
                .as_ref()
                .and_then(TerminalFaceMask::show)
                .ok_or("Parsec Show is unavailable")?,
        ))
    }
    fn check_host(&mut self, host: &HostAdvertisement) -> Result<(), String> {
        if self.retired || host != &self.host {
            return Err("Parsec terminal owner has been retired or Host changed".into());
        }
        self.execution
            .validate_current_host(host)
            .map_err(|e| e.to_string())
    }
    pub(super) fn retire(&mut self) {
        self.prompt = None;
        self.retired = true;
        self.input_inflight = None;
        self.refresh_after_input = false;
        let _ = self.execution.cancel();
        self.terminal = None;
        self.face = None;
        self.shown_play = None;
        self.state.fill(0);
        self.state.clear();
    }
}
impl Drop for ParsecTerminalOwner {
    fn drop(&mut self) {
        self.retire();
    }
}

#[cfg(test)]
#[path = "parsec_terminal_owner_tests.rs"]
mod tests;
