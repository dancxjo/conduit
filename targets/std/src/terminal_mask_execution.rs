//! Ordinary hosted terminal Mask execution through the shared planner and kernel.
//! One pending Play survives the actual terminal effect and typed return path.
use crate::terminal_face_mask::{TerminalEffectReceipt, TerminalError, TerminalMaskExecution};
use conduit_core::{HostAdvertisement, PortDirection};
use conduit_face::{
    Face, FaceInteraction, ManifestationLifecycle, MaskInteractionCorrelation, MaskShow,
    PlannedMaskPlot,
};
use conduit_kernel::scheduler::{
    HostCallRequest, RemoteIngressOutcome, RemoteTerminalDisposition, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallId, HostCallOutcome, RequestId,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, LoweredForePort, FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};

#[path = "terminal_mask_execution/backs.rs"]
mod backs;
#[path = "terminal_mask_execution/instruction.rs"]
mod instruction;
#[path = "terminal_mask_execution/kernel.rs"]
mod kernel;
#[path = "terminal_mask_execution/parsec_instruction_effect.rs"]
mod parsec_instruction_effect;
#[path = "terminal_mask_execution/planning.rs"]
pub(crate) mod planning;
#[path = "terminal_mask_execution/play.rs"]
mod play;
#[cfg(test)]
#[path = "terminal_mask_execution/tests.rs"]
mod tests;

// The canonical interaction kind admits at most 8192 * 8 queue bytes.
// Face/Show also enter that kind, so this profile cannot enlarge its 64KiB
// input queues merely because the renderer supports larger documents.
pub const MAX_TERMINAL_VALUE_BYTES: u32 = 64 * 1024;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const PRESENT_CALL: &str = "conduit.host/present@1";
const INTERACTION_CALL: &str = "conduit.host/face-interaction@1";
const TERMINAL_TARGET: &str = "face/base/std-terminal@1";
fn error(message: impl Into<String>) -> TerminalError {
    TerminalError::Execution(message.into())
}
fn debug_error(error: impl core::fmt::Debug) -> TerminalError {
    TerminalError::Execution(format!("{error:?}"))
}

/// Construct only after the caller explicitly attaches its terminal output and
/// input adapters. Identity and generation come from its actual current Host;
/// this attachment does not discover a terminal, create a Body, or grant remote
/// access. The caller must retire the adapter on terminal/Host/provider loss.
pub struct HostedTerminalMaskExecution {
    host: HostAdvertisement,
    planned: PlannedMaskPlot,
    read_only: bool,
    next_sequence: u64,
    scope_end: Option<u64>,
    body_scope: Option<(conduit_body::BodyPlan, conduit_body::BodyPlayIdentity)>,
    pending: Option<Play>,
    host_retired: bool,
}
struct Play {
    scheduler: Box<kernel::Scheduler>,
    face: Face,
    show: MaskShow,
    request: HostCallRequest,
    show_fore: LoweredForePort,
    interaction_fore: LoweredForePort,
    interaction_node: Option<conduit_kernel::NodeId>,
    awaiting_input: bool,
    read_only: bool,
    retired: bool,
    instruction_admission: Option<
        crate::installed_std::parsec_instruction_source_admission::ParsecInstructionSourceAdmission,
    >,
    instruction_node: Option<conduit_kernel::NodeId>,
    instruction_effect: Option<conduit_plan_lowering::lowering::LoweredHostCall>,
    instruction_request: Option<HostCallRequest>,
    acknowledged_show_value: Option<conduit_kernel::ValueRef>,
}
impl Drop for Play {
    fn drop(&mut self) {
        if let Some(admission) = &mut self.instruction_admission {
            admission.revoke();
        }
        if !self.retired {
            let _ = self.scheduler.cancel();
        }
    }
}

impl HostedTerminalMaskExecution {
    pub fn new(host: &HostAdvertisement) -> Result<Self, TerminalError> {
        Self::with_plan(host, planning::plan(host)?, false)
    }

    /// Execute only the terminal Back offered by this exact current Host.
    /// The caller must retain the attached I/O provider through the Show.
    pub fn new_attached(host: &HostAdvertisement) -> Result<Self, TerminalError> {
        Self::with_plan(host, planning::plan_attached(host)?, false)
    }

    pub(crate) fn new_parsec_attached(host: &HostAdvertisement) -> Result<Self, TerminalError> {
        Self::with_plan(host, planning::plan_attached_parsec(host)?, false)
    }

    /// Called only with the private map owner's confirmed parent Play.
    pub(crate) fn bind_parsec_body_scope(
        &mut self,
        plan: &conduit_body::BodyPlan,
        parent: &conduit_body::BodyPlayIdentity,
        maximum_shows: u16,
    ) -> Result<(), TerminalError> {
        if self.scope_end.is_some()
            || self.next_sequence != 0
            || self.pending.is_some()
            || self.host_retired
            || !parent.validate_for(plan)
            || maximum_shows == 0
            || maximum_shows > 64
        {
            return Err(error("terminal Body scope cannot be rebound"));
        }
        let base = parent
            .play_sequence
            .checked_mul(64)
            .ok_or_else(|| error("terminal Body sequence scope exhausted"))?;
        let end = base
            .checked_add(u64::from(maximum_shows))
            .ok_or_else(|| error("terminal Body sequence scope exhausted"))?;
        self.next_sequence = base;
        self.scope_end = Some(end);
        self.body_scope = Some((plan.clone(), parent.clone()));
        Ok(())
    }

    fn with_plan(
        host: &HostAdvertisement,
        planned: PlannedMaskPlot,
        read_only: bool,
    ) -> Result<Self, TerminalError> {
        Ok(Self {
            planned,
            host: host.clone(),
            read_only,
            next_sequence: 0,
            scope_end: None,
            body_scope: None,
            pending: None,
            host_retired: false,
        })
    }
    pub fn planned_mask(&self) -> &PlannedMaskPlot {
        &self.planned
    }
    pub fn has_pending_play(&self) -> bool {
        self.pending.is_some()
    }
    /// Supervisor check before each operation; a changed Host/Boot/generation or
    /// advertised offer set retires the current execution instead of rebinding it.
    pub fn validate_current_host(
        &mut self,
        current: &HostAdvertisement,
    ) -> Result<(), TerminalError> {
        if current != &self.host {
            self.pending.take();
            self.host_retired = true;
            return Err(error("stale terminal Host advertisement"));
        }
        Ok(())
    }
}

impl TerminalMaskExecution for HostedTerminalMaskExecution {
    fn begin_render(&mut self, face: &Face) -> Result<MaskShow, TerminalError> {
        if self.scope_end.is_some_and(|end| self.next_sequence >= end) {
            return Err(error("terminal Body Show bound exhausted"));
        }
        if self.host_retired {
            return Err(error("terminal Host was retired; prepare a fresh adapter"));
        }
        if self.pending.is_some() {
            return Err(error("terminal Mask already has a pending Play"));
        }
        face.validate().map_err(debug_error)?;
        let bytes = serde_json::to_vec(face).map_err(debug_error)?;
        if bytes.len() > MAX_TERMINAL_VALUE_BYTES as usize {
            return Err(TerminalError::DocumentPressure);
        }
        let fragment = &self.planned.plan.fragments[0];
        let sequence = self.next_sequence;
        self.next_sequence = sequence
            .checked_add(1)
            .ok_or_else(|| error("terminal Play sequence exhausted"))?;
        let active = conduit_core::bind_active_play(
            &self.planned.plan.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            sequence,
        );
        let show = MaskShow::prepared(
            &self.planned,
            face,
            active.clone(),
            face.subjects
                .first()
                .ok_or(TerminalError::InvalidFace)?
                .identity
                .clone(),
            "terminal/output".into(),
            format!("std/terminal/prepared/{sequence}").into(),
        )
        .map_err(debug_error)?;
        let lowered = lower_plan_fragment(fragment).map_err(debug_error)?;
        let fore = |name: &str, direction| {
            lowered
                .fore_ports
                .iter()
                .find(|p| p.front_port_id.as_str() == name && p.direction == direction)
                .cloned()
                .ok_or_else(|| error("missing terminal Fore"))
        };
        let face_fore = fore("face", PortDirection::Input)?;
        let show_fore = fore("show", PortDirection::Output)?;
        let interaction_fore = fore("interaction", PortDirection::Output)?;
        let call = |contract: &str| {
            lowered
                .host_calls
                .iter()
                .find(|c| c.contract_id.as_str() == contract)
                .ok_or_else(|| error("missing terminal Host Call"))
        };
        let renderer = call(PRESENT_CALL)?;
        let interaction = if self.read_only {
            None
        } else {
            Some(call(INTERACTION_CALL)?)
        };
        if renderer.target_kind.as_ref().map(|k| k.as_str()) != Some(TERMINAL_TARGET)
            || renderer.call != HostCallId(0)
            || interaction.as_ref().is_some_and(|interaction| {
                interaction.call != HostCallId(0)
                    || interaction.target_kind.as_ref().map(|k| k.as_str())
                        != Some(conduit_face::FACE_INTERACTION_VALUE_KIND)
            })
        {
            return Err(error("terminal Host Call binding mismatch"));
        }
        let instruction_node = lowered
            .host_calls
            .iter()
            .find(|call| {
                call.contract_id.as_str()
                    == crate::flow_activation::parsec_instruction_source_offer::CONTRACT
            })
            .map(|call| call.node);
        let instruction_effect = lowered
            .host_calls
            .iter()
            .find(|call| call.contract_id.as_str() == parsec_instruction_effect::CONTRACT)
            .cloned();
        if instruction_node.is_some() != instruction_effect.is_some() {
            return Err(error(
                "instruction Source and effect must be selected together",
            ));
        }
        if let Some(effect) = &instruction_effect {
            let placement = fragment
                .placements
                .iter()
                .find(|p| p.kind_id.as_str() == parsec_instruction_effect::KIND)
                .ok_or_else(|| error("instruction effect placement is absent"))?;
            if placement.implementation_id.as_str() != parsec_instruction_effect::IMPLEMENTATION
                || effect.binding.maximum_input_bytes != crate::flow_activation::parsec_instruction_source_offer::MAXIMUM_INSTRUCTION_BYTES
                || effect.binding.maximum_output_bytes != 0 {
                return Err(error("instruction effect differs from exact selected Back"));
            }
        }
        let mut instruction_admission = if instruction_node.is_some() {
            let (parent_plan, parent_play) = self
                .body_scope
                .as_ref()
                .ok_or_else(|| error("Parsec instruction has no issued parent scope"))?;
            Some(crate::installed_std::parsec_instruction_source_admission::ParsecInstructionSourceAdmission::prepare(
                &self.host, parent_plan, parent_play, &self.planned, &active,
                &lowered.identity, &lowered.host_calls).map_err(error)?)
        } else {
            None
        };
        let mut scheduler = Box::new(kernel::prepare(fragment, &lowered)?);
        if scheduler
            .admit_remote_input(face_fore.endpoint, face_fore.cord, 0, &bytes)
            .map_err(debug_error)?
            != (RemoteIngressOutcome::Accepted { sequence: 0 })
        {
            return Err(error("terminal Face Fore refused"));
        }
        scheduler
            .close_remote_input(face_fore.endpoint, face_fore.cord)
            .map_err(debug_error)?;
        let request = wait_request(&mut scheduler)?;
        if request.node != renderer.node
            || request.call != renderer.call
            || request.request != RequestId(0)
            || scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?
                != bytes
        {
            scheduler.cancel().map_err(debug_error)?;
            return Err(error("terminal renderer received another Face"));
        }
        if let Some(admission) = &mut instruction_admission {
            let (_, parent) = self
                .body_scope
                .as_ref()
                .ok_or_else(|| error("missing parent scope"))?;
            admission
                .confirm_issued(parent, &active, &self.host)
                .map_err(error)?;
        }
        self.pending = Some(Play {
            scheduler,
            face: face.clone(),
            show: show.clone(),
            request,
            show_fore,
            interaction_fore,
            interaction_node: interaction.map(|call| call.node),
            awaiting_input: false,
            read_only: self.read_only,
            retired: false,
            instruction_admission,
            instruction_node,
            instruction_effect,
            instruction_request: None,
            acknowledged_show_value: None,
        });
        Ok(show)
    }

    fn complete_render(
        &mut self,
        receipt: &TerminalEffectReceipt,
    ) -> Result<MaskShow, TerminalError> {
        let mut play = self
            .pending
            .take()
            .ok_or_else(|| error("no terminal renderer pending"))?;
        if play.awaiting_input
            || receipt.prepared_show() != &play.show
            || receipt.bytes_written() == 0
            || receipt.frame_sequence() == 0
        {
            return Err(TerminalError::StaleShow);
        }
        let available = play
            .show
            .transition(
                ManifestationLifecycle::Available,
                format!("std/terminal/available/{}", play.show.show.play_sequence).into(),
            )
            .map_err(debug_error)?;
        let bytes = serde_json::to_vec(&available).map_err(debug_error)?;
        play.complete_call(Some(&bytes))?;
        play.show = available;
        let mut observed = false;
        let mut pending = None;
        for _ in 0..64 {
            if let Some(request) = play.scheduler.next_host_request() {
                if Some(request.node) == play.instruction_node {
                    play.accept_instruction_request(request, &self.host)?;
                    continue;
                }
                if play.read_only
                    || pending.is_some()
                    || Some(request.node) != play.interaction_node
                    || request.call != HostCallId(0)
                    || request.request != RequestId(1)
                    || play
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(debug_error)?
                        != bytes
                {
                    return Err(error("terminal interaction Host Call mismatch"));
                }
                pending = Some(request);
            }
            if let Some(actual) = play.take_output(&play.show_fore.clone())? {
                if observed || actual != bytes {
                    return Err(error("terminal Show Fore mismatch"));
                }
                let actual_show: MaskShow = serde_json::from_slice(&actual).map_err(debug_error)?;
                actual_show.validate(&play.face).map_err(debug_error)?;
                play.show = actual_show;
                observed = true;
            }
            if observed && play.read_only {
                play.finish(None)?;
                return Ok(play.show.clone());
            }
            if observed && pending.is_some() {
                play.request = pending.take().expect("observed pending Host Call");
                play.awaiting_input = true;
                let show = play.show.clone();
                self.pending = Some(play);
                return Ok(show);
            }
            step(&mut play.scheduler)?;
        }
        Err(error("terminal Show Fore work bound exhausted"))
    }

    fn interact(
        &mut self,
        interaction: FaceInteraction,
    ) -> Result<MaskInteractionCorrelation, TerminalError> {
        let mut play = self
            .pending
            .take()
            .ok_or_else(|| error("no terminal interaction pending"))?;
        if !play.awaiting_input {
            return Err(TerminalError::UnacknowledgedShow);
        }
        interaction
            .validate_against(&play.face, &play.show)
            .map_err(TerminalError::Interaction)?;
        let expected = interaction.encode();
        if expected.len() > conduit_face::MAX_FACE_INTERACTION_BYTES {
            return Err(TerminalError::InputPressure);
        }
        play.complete_call(Some(&expected))?;
        let delivered = play
            .finish(Some(&expected))?
            .ok_or_else(|| error("terminal interaction Fore omitted output"))?;
        let actual = FaceInteraction::decode(&delivered).map_err(TerminalError::Interaction)?;
        actual
            .validate_against(&play.face, &play.show)
            .map_err(TerminalError::Interaction)?;
        play.show.correlate_interaction(actual).map_err(debug_error)
    }

    fn close_without_input(&mut self) -> Result<(), TerminalError> {
        let mut play = self
            .pending
            .take()
            .ok_or_else(|| error("no terminal interaction pending"))?;
        if !play.awaiting_input {
            return Err(TerminalError::UnacknowledgedShow);
        }
        play.complete_call(None)?;
        play.finish(None)?;
        Ok(())
    }
    fn cancel(&mut self) -> Result<(), TerminalError> {
        if let Some(mut play) = self.pending.take() {
            play.scheduler.cancel().map_err(debug_error)?;
            play.retired = true;
        }
        Ok(())
    }
}

fn step(scheduler: &mut kernel::Scheduler) -> Result<(), TerminalError> {
    if !matches!(
        scheduler.step().map_err(debug_error)?,
        SchedulerStatus::Progress { .. }
    ) {
        return Err(error("terminal Mask stopped before expected boundary"));
    }
    Ok(())
}
fn wait_request(scheduler: &mut kernel::Scheduler) -> Result<HostCallRequest, TerminalError> {
    for _ in 0..64 {
        if let Some(request) = scheduler.next_host_request() {
            return Ok(request);
        }
        step(scheduler)?;
    }
    Err(error("terminal Host Call work bound exhausted"))
}
