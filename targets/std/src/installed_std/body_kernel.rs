//! Local multi-partition composition of the existing kernel and Host effects.
use super::{
    fore_sign_storage, kernel_preparation::KernelTables, simple_face_host, InstalledBack,
    InstalledScheduler, MAX_CORDS, MAX_NODES, MAX_QUEUE_SLOTS, PENDING_REQUESTS,
};
use crate::{
    body_execution::BodyForeOutputAdapter, hosted_keyboard::HostedKeyboardAdapter,
    BodyLiveForeQueue, ExternalForeDelivery, ExternalForeInput, RunControl, TimerAdapter,
};
use conduit_body::BodyPlotPlan;
use conduit_core::{
    BodyClockCorrelation, BodyTimeQuality, BodyTimeRefusal, BodyTimeRequirement,
    CancellationReason, FailureReason, PlanFragment, TerminalDisposition,
};
use conduit_kernel::{
    scheduler::{HostCallRequest, SchedulerStatus},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallOutcome, HostedValueStore,
    KernelEvent,
};
use conduit_plan_lowering::{
    activation_fragment::{lower_fragment_activations, LoweredFragmentActivations},
    fragment_set::{lower_local_fragment_set_from_plans, FragmentSetBounds},
    lowering::{KernelIdentityMap, LoweredHostCall, FIXED_KERNEL_STORAGE_PROFILE},
};
use std::io::Write;

mod creche_route;
use creche_route::CrecheRoute;
mod clock_observation;
mod data_text;
mod fore_route;
mod parsec_acknowledged_map;
mod parsec_sources;
mod parsec_terminal_owner;
mod scalar_math;
pub(crate) use parsec_terminal_owner::QualifiedParsecInstruction;
mod each_route;
mod scan_route;
use crate::body_execution::ObservedKernelEvent;
use clock_observation::KernelClockObservations;
use fore_route::BodyForeRoute;

mod diagnostic;
mod execution;
pub(crate) use execution::state::BodyExecutionState;
mod face_contributions;
pub(crate) mod funded;
mod host_calls;
#[path = "body_kernel/preparation.rs"]
mod materialization;
pub(crate) mod memory;
pub(crate) mod native_source;
mod startup;

pub(crate) struct BodyKernel<'a> {
    startup_outputs: Vec<(
        conduit_kernel::NodeId,
        crate::hosted_audio::startup_chime::PreparedChimeOutput,
    )>,
    startup_receipts: Vec<crate::body_execution::StartupChimeCallReceipt>,
    startup_gates: Vec<super::startup_source_back::StartupGate>,
    terminal_events: Option<Vec<KernelEvent>>,
    error_diagnostic: Option<diagnostic::Diagnostic>,
    cleanup_diagnostic: Option<diagnostic::Diagnostic>,
    creche: Option<CrecheRoute>,
    native_source: Option<native_source::NativeSourceRoute<'a>>,
    face_contribution_routes: Vec<super::face_contribution_route::InstalledFaceRoute>,
    scheduler: Box<InstalledScheduler>,
    fore: BodyForeRoute<'a>,
    activations: Vec<LoweredFragmentActivations>,
    partitions: Vec<KernelIdentityMap>,
    operations: Vec<LoweredHostCall>,
    face_labels: Vec<Option<String>>,
    input_origin: Option<super::keyboard_input_binding::KeyboardRequestAdmission>,
    typed_record_hosts: Vec<Option<super::typed_record_back::TypedRecordHost>>,
    image_text_hosts: Vec<Option<super::image_text_back::ImageTextHost>>,
    image_text_record_hosts: Vec<Option<super::image_text_record_back::ImageTextRecordHost>>,
    text_state_hosts: Vec<Option<super::text_state_back::TextStateHost>>,
    data_text_hosts: Vec<super::data_text_host::DataTextGenerationHosts<'a>>,
    input_keymaps: [Option<conduit_human::PreparedKeymapChoices>; MAX_NODES],
    requests: Vec<HostCallRequest>,
    clock_observations: KernelClockObservations,
    supported_pure_activation: bool,
    supported_parsec_input_scan: bool,
    parsec_map: Option<parsec_acknowledged_map::ParsecAcknowledgedMap>,
    parsec_terminal: Option<parsec_terminal_owner::ParsecTerminalOwner>,
    todo_checkpoint: Option<crate::todo_checkpoint_call::TodoCheckpointHost>,
    todo_checkpoint_read: Option<crate::todo_checkpoint_read_call::TodoCheckpointReadHost>,
}

mod result;
pub(crate) use result::BodyKernelResult;

fn keyboard(contract: &conduit_core::HostCallContractId) -> bool {
    contract.as_str() == conduit_std_offers::NEXT_KEY_EVENT_HOST_CALL_CONTRACT
}
fn button(contract: &conduit_core::HostCallContractId) -> bool {
    contract.as_str() == conduit_std_offers::button::NEXT_TRANSITION_HOST_CALL
}
fn text_state(contract: &conduit_core::HostCallContractId) -> bool {
    contract.as_str() == conduit_std_offers::TEXT_STATE_HOST_CALL
}
fn input_semantic(contract: &conduit_core::HostCallContractId) -> bool {
    matches!(
        contract.as_str(),
        conduit_std_offers::KEYMAP_HOST_CALL
            | conduit_std_offers::KEYMAP_SELECT_HOST_CALL
            | conduit_std_offers::CHORDS_HOST_CALL
    )
}
fn todo_checkpoint(operation: &LoweredHostCall) -> bool {
    operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_PUBLISH_CALL
        && operation
            .target_kind
            .as_ref()
            .is_some_and(|kind| kind.as_str() == conduit_todo_plot::TODO_CHECKPOINT_KIND)
        && operation.binding.maximum_input_bytes == 4096
        && operation.binding.maximum_output_bytes == 4096
}
fn todo_checkpoint_read(operation: &LoweredHostCall) -> bool {
    operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_READ_CALL
        && operation
            .target_kind
            .as_ref()
            .is_some_and(|kind| kind.as_str() == conduit_todo_plot::TODO_CHECKPOINT_READ_KIND)
        && operation.binding.maximum_input_bytes == 0
        && operation.binding.maximum_output_bytes == conduit_todo_plot::STATE_MAX_BYTES as u32
}
fn timer(contract: &conduit_core::HostCallContractId) -> bool {
    contract.as_str() == conduit_core::WAIT_HOST_CALL_CONTRACT
}
fn typed_record_codec(contract: &conduit_core::HostCallContractId) -> bool {
    [
        conduit_std_offers::TYPED_RECORD_FRAME_HOST_CALL,
        conduit_std_offers::TYPED_RECORD_DEFRAME_HOST_CALL,
        conduit_std_offers::TEXT_TO_TYPED_RECORD_HOST_CALL,
        conduit_std_offers::TYPED_RECORD_TO_TEXT_HOST_CALL,
    ]
    .contains(&contract.as_str())
}
fn image_text(contract: &conduit_core::HostCallContractId) -> bool {
    matches!(
        contract.as_str(),
        conduit_std_offers::IMAGE_TEXT_IMAGE_OPERATION
            | conduit_std_offers::IMAGE_TEXT_CAPTION_OPERATION
            | conduit_std_offers::IMAGE_TEXT_RECORD_OPERATION
    )
}
fn face(operation: &LoweredHostCall) -> bool {
    operation.target_kind.as_ref().is_some_and(|target| {
        matches!(
            target.as_str(),
            "face/stdout-text"
                | conduit_std_offers::TICK_FACE_TARGET
                | conduit_std_offers::COUNT_FACE_TARGET
                | conduit_std_offers::BOOL_FACE_TARGET
        ) && operation.contract_id
            == conduit_core::present_host_call_requirement(
                target.clone(),
                operation.binding.maximum_input_bytes,
            )
            .contract_id
    })
}

impl<'a> BodyKernel<'a> {
    #[cfg(test)]
    pub(crate) fn prepare(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        fore_inputs: &'a [ExternalForeInput],
        sequential_fore: bool,
        has_fore_output: bool,
    ) -> Result<Self, String> {
        Self::prepare_with_live(
            partitions,
            has_keyboard,
            parent_play,
            fore_inputs,
            sequential_fore,
            has_fore_output,
            None,
            None,
            #[cfg(target_os = "linux")]
            None,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_on_host(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        fore_inputs: &'a [ExternalForeInput],
        sequential_fore: bool,
        has_fore_output: bool,
        host: &conduit_core::HostAdvertisement,
        #[cfg(target_os = "linux")] selected_file: Option<
            &'a crate::text_file::SelectedTextFileResidence,
        >,
    ) -> Result<Self, String> {
        let sources = parsec_sources::prepare(partitions, host)?;
        Self::prepare_with_live(
            partitions,
            has_keyboard,
            parent_play,
            fore_inputs,
            sequential_fore,
            has_fore_output,
            None,
            sources.as_ref(),
            #[cfg(target_os = "linux")]
            selected_file,
            None,
            None,
        )
    }

    pub(crate) fn prepare_live_on_host(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        queue: &BodyLiveForeQueue,
        control: &RunControl,
        host: &conduit_core::HostAdvertisement,
        #[cfg(target_os = "linux")] selected_file: Option<
            &'a crate::text_file::SelectedTextFileResidence,
        >,
    ) -> Result<Self, String> {
        if partitions.len() != 1 || !queue.matches_plan(&partitions[0].plan, control) {
            return Err("live Fore queue differs from the exact Body Plan or stop control".into());
        }
        if has_keyboard {
            BodyLiveForeQueue::validate_parsec_input_plan(&partitions[0].plan, host)?;
        }
        let sources = parsec_sources::prepare(partitions, host)?;
        Self::prepare_with_live(
            partitions,
            has_keyboard,
            parent_play,
            &[],
            true,
            true,
            Some(queue),
            sources.as_ref(),
            #[cfg(target_os = "linux")]
            selected_file,
            None,
            None,
        )
    }

    /// Prepare input origin descriptors without granting issued-Play authority.
    pub(crate) fn prepare_input_origin(
        &mut self,
        advertisement: &conduit_core::HostAdvertisement,
        plan: &conduit_body::BodyPlan,
        prospective: &conduit_body::BodyPlayIdentity,
    ) -> Result<(), String> {
        if !super::keyboard_input_binding::has_input_source(&self.operations) {
            // Ordinary non-input admission retains its own refusals and ordering.
            return Ok(());
        }
        if self.input_origin.is_some() {
            return Err("Body input origin was prepared twice".into());
        }
        self.input_origin = Some(
            super::keyboard_input_binding::KeyboardRequestAdmission::prepare_body(
                advertisement,
                plan,
                prospective,
                &self.partitions,
                &self.operations,
            )?,
        );
        if self.supported_parsec_input_scan {
            self.input_origin
                .as_mut()
                .ok_or("Parsec input origin is absent")?
                .enable_completed_event_custody()?;
            self.parsec_map = Some(parsec_acknowledged_map::ParsecAcknowledgedMap::prepare(
                plan,
                advertisement,
                prospective,
                self.partitions
                    .first()
                    .ok_or("Parsec map partition is absent")?,
            )?);
        }
        Ok(())
    }

    pub(crate) fn confirm_input_play(
        &mut self,
        advertisement: &conduit_core::HostAdvertisement,
        issued: &conduit_body::BodyPlayIdentity,
    ) -> Result<(), String> {
        match self.input_origin.as_mut() {
            Some(origin) => origin.confirm_body_play(advertisement, issued),
            None if !super::keyboard_input_binding::has_input_source(&self.operations) => Ok(()),
            None => Err("Body input origin was not prepared".into()),
        }?;
        if let Some(map) = self.parsec_map.as_mut() {
            map.confirm(advertisement, issued)?;
        }
        Ok(())
    }

    pub(crate) fn attach_parsec_terminal(
        &mut self,
        plan: &conduit_body::BodyPlan,
        host: &conduit_core::HostAdvertisement,
        columns: usize,
        rows: usize,
    ) -> Result<(), String> {
        if !self.supported_parsec_input_scan
            || self.parsec_map.is_none()
            || self.parsec_terminal.is_some()
        {
            return Err("terminal attachment requires the exact prepared Parsec input Body".into());
        }
        self.parsec_terminal = Some(parsec_terminal_owner::ParsecTerminalOwner::prepare(
            plan, host, columns, rows,
        )?);
        Ok(())
    }

    /// Bind at most one selected reader and publisher to their exact lowered
    /// placements before Play. No ambient storage fallback is available.
    pub(crate) fn attach_todo_checkpoint(
        &mut self,
        partitions: &[BodyPlotPlan],
        root: &std::path::Path,
        checkpoint: crate::todo_durable_resource::CheckpointIdentity,
    ) -> Result<(), String> {
        let mut selected = 0;
        for (partition, body_plot) in partitions.iter().enumerate() {
            let lowered = self
                .partitions
                .get(partition)
                .ok_or("Todo checkpoint partition missing")?;
            for placement in body_plot
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
            {
                match placement.implementation_id.as_str() {
                    conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION => {
                        if self.todo_checkpoint.is_some() {
                            return Err("duplicate Todo checkpoint publisher".into());
                        }
                        self.todo_checkpoint = Some(
                            crate::todo_checkpoint_call::TodoCheckpointHost::prepare(
                                root,
                                placement,
                                lowered,
                                checkpoint.clone(),
                            )
                            .map_err(|error| {
                                format!("prepare selected Todo checkpoint: {error:?}")
                            })?,
                        );
                        selected += 1;
                    }
                    conduit_std_offers::TODO_CHECKPOINT_READ_IMPLEMENTATION => {
                        if self.todo_checkpoint_read.is_some() {
                            return Err("duplicate Todo checkpoint reader".into());
                        }
                        self.todo_checkpoint_read = Some(
                            crate::todo_checkpoint_read_call::TodoCheckpointReadHost::prepare(
                                root,
                                placement,
                                lowered,
                                checkpoint.clone(),
                            )
                            .map_err(|error| {
                                format!("prepare selected Todo checkpoint read: {error:?}")
                            })?,
                        );
                        selected += 1;
                    }
                    _ => {}
                }
            }
        }
        if selected == 0 {
            return Err("Body has no selected Todo checkpoint placement".into());
        }
        Ok(())
    }

    /// Exact finite pure scans and the separately validated live Parsec input
    /// profile have installed Body routes. This admits execution shape only;
    /// Face action grants and current input-map authority remain separate.
    pub(crate) fn require_supported_execution(&self) -> Result<(), String> {
        if self.operations.iter().any(|operation| {
            operation.contract_id.as_str()
                == conduit_body_make::deployment_runtime_catalog::CRECHE_HOST_CALL
        }) && self.creche.is_none()
        {
            return Err("planned Creche has no scoped admitted encounter owner".into());
        }
        if self.operations.iter().any(|operation| {
            operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_PUBLISH_CALL
        }) && self.todo_checkpoint.is_none()
        {
            return Err("planned Todo checkpoint has no selected durable Host residence".into());
        }
        if self.operations.iter().any(|operation| {
            operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_READ_CALL
        }) && self.todo_checkpoint_read.is_none()
        {
            return Err(
                "planned Todo checkpoint read has no selected durable Host residence".into(),
            );
        }
        if self
            .activations
            .iter()
            .any(|bound| !bound.entries.is_empty())
            && !self.supported_pure_activation
            && !self.supported_parsec_input_scan
        {
            return Err("Body activation coordinator is not installed".into());
        }
        Ok(())
    }
}

fn bind_body_activations(
    partitions: &[BodyPlotPlan],
    fragments: &[&PlanFragment],
) -> Result<Vec<LoweredFragmentActivations>, String> {
    if partitions.len() != fragments.len() {
        return Err("Body activation binding partition count differs".into());
    }
    let mut admitted = Vec::with_capacity(partitions.len());
    for activation in partitions
        .iter()
        .zip(fragments)
        .map(|(partition, fragment)| {
            if partition.plan.fragments.len() != 1
                || partition.plan.fragments[0].fragment_id != fragment.fragment_id
                || partition.plan.fragments[0].plan_id != fragment.plan_id
            {
                return Err("Body activation binding fragment differs from exact Plan".into());
            }
            lower_fragment_activations(&partition.plan, &fragment.fragment_id)
                .map_err(|error| format!("Body activation binding: {error:?}"))
        })
    {
        admitted.push(activation?);
    }
    Ok(admitted)
}

#[cfg(test)]
#[path = "body_kernel/activation_binding_tests.rs"]
mod activation_binding_tests;

#[cfg(test)]
#[path = "body_kernel/live_fore_tests.rs"]
mod live_fore_tests;

#[cfg(test)]
#[path = "body_kernel/todo_core_replay_tests.rs"]
mod todo_core_replay_tests;

#[cfg(test)]
#[path = "body_kernel/thermostat_live_fore_tests.rs"]
mod thermostat_live_fore_tests;

#[cfg(test)]
#[path = "body_kernel/parsec_live_fore_tests.rs"]
mod parsec_live_fore_tests;

#[cfg(test)]
#[path = "body_kernel/parsec_instruction_source_tests.rs"]
mod parsec_instruction_source_tests;
