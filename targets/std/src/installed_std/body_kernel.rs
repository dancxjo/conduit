//! Local multi-partition composition of the existing kernel and Host effects.
use super::{
    fore_sign_storage, kernel_preparation::KernelTables, preparation, simple_face_host,
    InstalledBack, InstalledScheduler, MAX_CORDS, MAX_NODES, MAX_QUEUE_SLOTS, PENDING_REQUESTS,
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

mod clock_observation;
mod fore_route;
mod parsec_acknowledged_map;
mod parsec_sources;
mod parsec_terminal_owner;
pub(crate) use parsec_terminal_owner::QualifiedParsecInstruction;
mod each_route;
mod scan_route;
use crate::body_execution::ObservedKernelEvent;
use clock_observation::KernelClockObservations;
use fore_route::BodyForeRoute;

pub(crate) struct BodyKernel<'a> {
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
    input_keymaps: [Option<conduit_human::PreparedKeymapChoices>; MAX_NODES],
    requests: Vec<HostCallRequest>,
    clock_observations: KernelClockObservations,
    supported_pure_activation: bool,
    supported_parsec_input_scan: bool,
    parsec_map: Option<parsec_acknowledged_map::ParsecAcknowledgedMap>,
    parsec_terminal: Option<parsec_terminal_owner::ParsecTerminalOwner>,
    todo_checkpoint: Option<crate::todo_checkpoint_call::TodoCheckpointHost>,
    todo_checkpoint_read: Option<crate::todo_checkpoint_read_call::TodoCheckpointReadHost>,
    creche: Option<creche_route::CrecheRoute>,
}

pub(crate) struct BodyKernelResult {
    pub terminal: TerminalDisposition,
    pub failure: Option<String>,
    pub kernel_failure: Option<Failure>,
    pub cleanup_failure: Option<String>,
    pub partitions: Vec<KernelIdentityMap>,
    pub requests: Vec<HostCallRequest>,
    pub events: Vec<KernelEvent>,
    pub scan_child_signs:
        Result<Vec<conduit_composite::ScanChildSignReceipt>, conduit_composite::BoundedScanError>,
    pub scan_cancellation_failed: bool,
    pub scan_output_completion_failed: bool,
    pub each_child_signs: Result<
        Vec<conduit_composite::ScanChildSignReceipt>,
        conduit_composite::BoundedActivationError,
    >,
    pub each_cancellation_failed: bool,
    pub each_output_completion_failed: bool,
    pub fore_deliveries: Vec<ExternalForeDelivery>,
    pub fore_pending_output: Option<crate::body_execution::BodyForePendingOutput>,
    pub clock_observations: Vec<ObservedKernelEvent>,
    pub clock_quality: Option<BodyTimeQuality>,
    pub clock_execution_bounds: Option<(
        conduit_core::MonotonicDuration,
        conduit_core::MonotonicDuration,
    )>,
}

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

mod admission;
mod creche_route;
mod execution;
mod host_dispatch;
mod prepare;

fn bind_body_activations(
    partitions: &[BodyPlotPlan],
    fragments: &[&PlanFragment],
) -> Result<Vec<LoweredFragmentActivations>, String> {
    if partitions.len() != fragments.len() {
        return Err("Body activation binding partition count differs".into());
    }
    partitions
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
        .collect()
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
