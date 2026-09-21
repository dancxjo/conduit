//! Finite generic browser execution envelope for inline Forms.

#[path = "engine_attempt.rs"]
mod attempt;
#[path = "engine_completions.rs"]
mod completions;
pub(super) use completions::{complete_host_effect, complete_host_effect_with_output};
#[path = "engine_preparation.rs"]
pub(super) mod preparation;
#[path = "resource_effect.rs"]
pub(super) mod resource_effect;
#[path = "engine_transforms.rs"]
pub(super) mod transforms;
use preparation::{prepare_scheduler, validate_envelope};

#[cfg(test)]
#[path = "quantity_tests.rs"]
mod quantity_tests;

use crate::installed_browser::{
    factory, BrowserManifestation, BrowserOperation, BROWSER_HOST_CALLS_PER_GEAR,
    BROWSER_HOST_CALL_BINDINGS, BROWSER_PENDING_REQUESTS, BROWSER_PORTS_PER_GEAR,
    BROWSER_QUEUE_SLOTS, BROWSER_ROUTE_SLOTS, BROWSER_ROUTE_TARGETS, BROWSER_SIGN_ITEMS,
    BROWSER_TOTAL_VALUE_BYTES, BROWSER_VALUE_ITEMS, MAXIMUM_BROWSER_CORDS,
    MAXIMUM_BROWSER_FORM_CORDS, MAXIMUM_BROWSER_FORM_GEARS, MAXIMUM_BROWSER_GEARS,
    MAXIMUM_BROWSER_VALUE_BYTES,
};
use conduit_core::PlanFragment;
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostCallRequest, NodeSpec, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, CordEndpoint, CordId, FixedHostCallBindings, FixedRoutes, HostCallDisposition,
    HostCallOutcome, HostedSignLog, HostedValueStore, NodeId, PortId,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};

type BrowserKernel = FixedScheduler<
    OperationDriver<BrowserOperation, BROWSER_PORTS_PER_GEAR>,
    HostedValueStore,
    HostedSignLog,
    MAXIMUM_BROWSER_GEARS,
    MAXIMUM_BROWSER_CORDS,
    BROWSER_PORTS_PER_GEAR,
    BROWSER_QUEUE_SLOTS,
    BROWSER_ROUTE_SLOTS,
    BROWSER_ROUTE_TARGETS,
    BROWSER_HOST_CALL_BINDINGS,
    BROWSER_PENDING_REQUESTS,
>;

/// Host-prepared state accompanies, but never replaces, the production kernel.
pub(super) struct TourScheduler {
    pub(super) failure: Option<conduit_kernel::Failure>,
    kernel: BrowserKernel,
    snapshots: Vec<Option<Box<resource_effect::SnapshotState>>>,
    selectors: Vec<Option<crate::installed_browser::pointer_selector::PreparedSelector>>,
    mappings: Vec<Option<conduit_semantic_catalog::QuantityMapping>>,
    attempts: Vec<Option<conduit_semantic_catalog::BoundedButtonAttemptCodec>>,
    comparisons: Vec<Option<conduit_semantic_catalog::BoundedPatternComparisonCodec>>,
    timing: Vec<Option<crate::installed_browser::timing::PreparedTiming>>,
    keymaps: Vec<Option<crate::installed_browser::keymap::PreparedKeymap>>,
    text_states: Vec<Option<Box<crate::installed_browser::text_state::PreparedTextState>>>,
    deliveries: Vec<Option<conduit_net::BoundedRecordDeliveryStatusCodec>>,
    histories: Vec<Option<crate::installed_browser::historical::PreparedHistory>>,
    replay_sources: Vec<Option<Box<crate::installed_browser::replay_source::PreparedReplaySource>>>,
    replay_controls:
        Vec<Option<Box<crate::installed_browser::replay_control::PreparedReplayControl>>>,
    template_stores:
        Vec<Option<Box<crate::installed_browser::template_storage::PreparedTemplateStore>>>,
    structured_selectors:
        Vec<Option<crate::installed_browser::structured_selector::PreparedSelector>>,
    measurement_windows:
        Vec<Option<Box<crate::installed_browser::measurement_window::PreparedWindow>>>,
    measurement_hysteresis:
        Vec<Option<Box<crate::installed_browser::measurement_hysteresis::PreparedHysteresis>>>,
    garden_steps: Vec<Option<Box<crate::installed_browser::garden_step::PreparedGardenStep>>>,
    stroke_captures:
        Vec<Option<Box<crate::installed_browser::stroke_capture::PreparedStrokeCapture>>>,
    applications: Vec<Option<Box<super::application_state::PreparedApplication>>>,
}

impl core::ops::Deref for TourScheduler {
    type Target = BrowserKernel;

    fn deref(&self) -> &Self::Target {
        &self.kernel
    }
}

impl core::ops::DerefMut for TourScheduler {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.kernel
    }
}

pub(super) struct PendingHostEffect {
    pub request: HostCallRequest,
    pub effect: BrowserHostEffect,
}

pub(super) enum BrowserHostEffect {
    AudioCue,
    AudioCapture,
    PcmPlayback { frame: Vec<u8> },
    PitchTone { hertz: u32 },
    ClockObservation,
    Timer { duration_millis: u64 },
    Snapshot { publish: bool },
    KeyEvent,
    PointerEvent,
    ButtonTransition,
    ApplicationEvent,
    Manifestation(BrowserManifestation),
}

pub(super) enum DriveStatus {
    Effect(PendingHostEffect),
    Quiescent,
    SemanticCompleted,
    Waiting { pending_effects: usize },
}

pub(super) fn prepare(
    fragment: &PlanFragment,
) -> Result<(TourScheduler, PendingHostEffect), String> {
    let lowered = lower_plan_fragment(fragment)
        .map_err(|error| format!("lower executable-tour Plan: {error:?}"))?;
    validate_envelope(fragment, &lowered, false)?;
    let mut scheduler = prepare_scheduler(fragment, &lowered)?;
    let pending = match drive(&mut scheduler, fragment)? {
        DriveStatus::Effect(pending) => pending,
        DriveStatus::Quiescent => {
            return Err("Tour Play became quiescent without a planned host effect".into())
        }
        DriveStatus::SemanticCompleted => {
            return Err("Tour Play semantically completed without a planned host effect".into())
        }
        DriveStatus::Waiting { .. } => {
            return Err("initial browser effect is already pending".into())
        }
    };
    Ok((scheduler, pending))
}

pub(super) fn prepare_remote_fragment(
    fragment: &PlanFragment,
) -> Result<(TourScheduler, LoweredPlanFragment), String> {
    let lowered = lower_plan_fragment(fragment)
        .map_err(|error| format!("lower multi-host executable-tour Plan: {error:?}"))?;
    validate_envelope(fragment, &lowered, true)?;
    let scheduler = prepare_scheduler(fragment, &lowered)?;
    Ok((scheduler, lowered))
}

pub(super) fn drive(
    scheduler: &mut TourScheduler,
    fragment: &PlanFragment,
) -> Result<DriveStatus, String> {
    drive_with_placement(scheduler, fragment.completion_policy, |node| {
        fragment.placements.get(usize::from(node.0))
    })
}

pub(super) fn drive_with_placement<'a>(
    scheduler: &mut TourScheduler,
    completion_policy: conduit_core::PlanCompletionPolicy,
    placement_for: impl Fn(NodeId) -> Option<&'a conduit_core::PlannedGear>,
) -> Result<DriveStatus, String> {
    drive_with_boundary(scheduler, completion_policy, placement_for, false, &[])
}

/// The same installed effects, with an external Cord allowed to await traffic.
/// Waiting is not completion; the remote owner must inspect its exact endpoint.
pub(super) fn drive_remote(
    scheduler: &mut TourScheduler,
    fragment: &PlanFragment,
    egress: &[(conduit_kernel::RemoteEndpointId, CordId)],
) -> Result<DriveStatus, String> {
    drive_with_boundary(
        scheduler,
        fragment.completion_policy,
        |node| fragment.placements.get(usize::from(node.0)),
        true,
        egress,
    )
}

fn drive_with_boundary<'a>(
    scheduler: &mut TourScheduler,
    completion_policy: conduit_core::PlanCompletionPolicy,
    placement_for: impl Fn(NodeId) -> Option<&'a conduit_core::PlannedGear>,
    allow_remote_wait: bool,
    remote_egress: &[(conduit_kernel::RemoteEndpointId, CordId)],
) -> Result<DriveStatus, String> {
    loop {
        for &(endpoint, cord) in remote_egress {
            if scheduler
                .remote_egress_offer(endpoint, cord)
                .map_err(debug_error)?
                .is_some()
            {
                return Ok(DriveStatus::Waiting {
                    pending_effects: scheduler.pending_host_call_count(),
                });
            }
        }
        if scheduler.has_ready_work() {
            match scheduler.step().map_err(|error| {
                if let conduit_kernel::scheduler::SchedulerError::BackFailed(detail) = &error {
                    scheduler.failure = Some(*detail);
                }
                debug_error(error)
            })? {
                SchedulerStatus::Progress { .. } => continue,
                SchedulerStatus::Cancelled => return Err("Tour Play was cancelled".into()),
                SchedulerStatus::Drained | SchedulerStatus::Idle => {}
            }
        }
        if let Some(request) = scheduler.next_host_request() {
            let placement = placement_for(request.node)
                .ok_or_else(|| "browser request has no planned placement".to_string())?;
            let operation = placement
                .host_calls
                .get(usize::from(request.operation.0))
                .ok_or_else(|| "browser request has no planned Host Call".to_string())?;
            if resource_effect::matches(operation.contract_id.as_str()) {
                if let Some(pending) = resource_effect::begin(scheduler, placement, request)? {
                    return Ok(DriveStatus::Effect(pending));
                }
                continue;
            }
            if transforms::complete_transform(scheduler, placement, operation, request)? {
                continue;
            }
            let input = scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?
                .to_vec();
            if operation.contract_id.as_str()
                == crate::installed_browser::audio_io::CAPTURE_OPERATION
            {
                return Ok(DriveStatus::Effect(PendingHostEffect {
                    request,
                    effect: BrowserHostEffect::AudioCapture,
                }));
            }
            if operation.contract_id.as_str() == crate::installed_browser::audio_io::PLAY_OPERATION
            {
                conduit_audio::PcmFrameHeader::decode_frame(&input)
                    .map_err(|error| format!("browser PCM playback frame: {error:?}"))?;
                return Ok(DriveStatus::Effect(PendingHostEffect {
                    request,
                    effect: BrowserHostEffect::PcmPlayback { frame: input },
                }));
            }
            if operation.contract_id.as_str()
                == crate::installed_browser::button_attempt::TIMED_BUTTON_ATTEMPT_OBSERVE_HOST_CALL
            {
                return Ok(DriveStatus::Effect(PendingHostEffect {
                    request,
                    effect: BrowserHostEffect::ClockObservation,
                }));
            }
            if operation.contract_id.as_str() == conduit_core::WAIT_HOST_CALL_CONTRACT
                || operation.contract_id.as_str()
                    == conduit_core::MONOTONIC_TIMER_HOST_CALL_CONTRACT
            {
                let duration_millis = decode_timer_duration(operation, &input)?;
                return Ok(DriveStatus::Effect(PendingHostEffect {
                    request,
                    effect: BrowserHostEffect::Timer { duration_millis },
                }));
            }
            if operation.contract_id.as_str() == crate::installed_browser::startup_chime::HOST_CALL
            {
                let pulse = conduit_core::InfoBool::decode(&input)
                    .map_err(|error| format!("audio cue pulse: {error:?}"))?;
                if pulse.get() {
                    return Ok(DriveStatus::Effect(PendingHostEffect {
                        request,
                        effect: BrowserHostEffect::AudioCue,
                    }));
                }
                scheduler
                    .complete_host_call(
                        request.node,
                        request.request,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
                            output: None,
                            failure: None,
                        },
                    )
                    .map_err(debug_error)?;
                continue;
            }
            if operation.contract_id.as_str() == crate::installed_browser::pitch_tone::HOST_CALL {
                let quantity = conduit_core::Quantity::decode(&input)
                    .map_err(|error| format!("pitch tone quantity: {error:?}"))?
                    .convert(conduit_core::QuantityUnit::Hertz)
                    .map_err(|error| format!("pitch tone frequency: {error:?}"))?;
                let hertz = u32::try_from(quantity.value())
                    .map_err(|_| "pitch tone frequency must be positive".to_string())?;
                if !(20..=20_000).contains(&hertz) {
                    return Err("pitch tone frequency must be between 20 Hz and 20000 Hz".into());
                }
                return Ok(DriveStatus::Effect(PendingHostEffect {
                    request,
                    effect: BrowserHostEffect::PitchTone { hertz },
                }));
            }
            if operation.contract_id.as_str() == crate::installed_browser::KEY_EVENT_OPERATION {
                return Ok(DriveStatus::Effect(PendingHostEffect {
                    request,
                    effect: BrowserHostEffect::KeyEvent,
                }));
            }
            if operation.contract_id.as_str() == crate::installed_browser::POINTER_EVENT_OPERATION {
                return Ok(DriveStatus::Effect(PendingHostEffect {
                    request,
                    effect: BrowserHostEffect::PointerEvent,
                }));
            }
            if operation.contract_id.as_str() == crate::installed_browser::BUTTON_EVENT_OPERATION {
                return Ok(DriveStatus::Effect(PendingHostEffect {
                    request,
                    effect: BrowserHostEffect::ButtonTransition,
                }));
            }
            if operation.contract_id.as_str()
                == crate::installed_browser::APPLICATION_EVENT_OPERATION
            {
                return Ok(DriveStatus::Effect(PendingHostEffect {
                    request,
                    effect: BrowserHostEffect::ApplicationEvent,
                }));
            }
            let installation = factory(&placement.implementation_id)
                .ok_or_else(|| "browser request implementation is not installed".to_string())?;
            let perform = installation.perform.ok_or_else(|| {
                "local browser implementation requested an unknown Host Call".to_string()
            })?;
            let result = perform(placement, &input)?;
            match (result.output, result.manifestation) {
                (Some(output), None) => {
                    let output = scheduler.store_host_value(&output).map_err(debug_error)?;
                    scheduler
                        .complete_host_call(
                            request.node,
                            request.request,
                            HostCallOutcome {
                                disposition: HostCallDisposition::Completed,
                                output: Some(
                                    BoundedValueRef::new(output, operation.maximum_output_bytes)
                                        .map_err(|_| {
                                            "browser Host output exceeded its planned bound"
                                        })?,
                                ),
                                failure: None,
                            },
                        )
                        .map_err(debug_error)?;
                }
                (None, Some(manifestation)) => {
                    return Ok(DriveStatus::Effect(PendingHostEffect {
                        request,
                        effect: BrowserHostEffect::Manifestation(manifestation),
                    }));
                }
                _ => return Err("browser Host result has an invalid output shape".into()),
            }
            continue;
        }
        let status = scheduler.step().map_err(|error| {
            if let conduit_kernel::scheduler::SchedulerError::BackFailed(detail) = &error {
                scheduler.failure = Some(*detail);
            }
            debug_error(error)
        })?;
        match status {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Drained => {
                return Ok(match completion_policy {
                    conduit_core::PlanCompletionPolicy::Live => DriveStatus::Quiescent,
                    conduit_core::PlanCompletionPolicy::SemanticCompletion => {
                        DriveStatus::SemanticCompleted
                    }
                })
            }
            SchedulerStatus::Idle if scheduler.pending_host_call_count() > 0 => {
                return Ok(DriveStatus::Waiting {
                    pending_effects: scheduler.pending_host_call_count(),
                });
            }
            SchedulerStatus::Idle if allow_remote_wait => return Ok(DriveStatus::Quiescent),
            SchedulerStatus::Idle => return Err("Tour Play became idle".into()),
            SchedulerStatus::Cancelled => return Err("Tour Play was cancelled".into()),
        }
    }
}

#[cfg(test)]
#[path = "body_partition_tests.rs"]
mod body_partition_tests;

fn decode_timer_duration(
    operation: &conduit_core::HostCallRequirement,
    input: &[u8],
) -> Result<u64, String> {
    let expected_target =
        if operation.contract_id.as_str() == conduit_core::MONOTONIC_TIMER_HOST_CALL_CONTRACT {
            Some(conduit_semantic_catalog::TIMED_BUTTON_ATTEMPT_KIND.into())
        } else {
            None
        };
    if operation.target_kind != expected_target
        || operation.maximum_in_flight != 1
        || operation.maximum_input_bytes != conduit_time::TICK_ENCODED_LEN
        || operation.maximum_output_bytes != 0
    {
        return Err("planned browser timer operation has the wrong exact contract".into());
    }
    let duration_millis = conduit_core::decode_monotonic_duration(input)
        .map_err(|error| format!("decode browser timer duration: {error:?}"))?;
    if duration_millis > crate::installed_browser::BROWSER_TIMER_MAXIMUM_MILLIS {
        return Err("browser timer duration exceeds its admitted implementation bound".into());
    }
    Ok(duration_millis)
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
#[path = "json_tests.rs"]
mod json_tests;

#[cfg(test)]
#[path = "timing_kernel_tests.rs"]
mod timing_kernel_tests;

#[cfg(test)]
#[path = "history_kernel_tests.rs"]
mod history_kernel_tests;

#[cfg(test)]
#[path = "measurement_plot_kernel_tests.rs"]
mod measurement_plot_kernel_tests;

#[cfg(test)]
#[path = "measurement_window_kernel_tests.rs"]
mod measurement_window_kernel_tests;

#[cfg(test)]
#[path = "stroke_capture_kernel_tests.rs"]
mod stroke_capture_kernel_tests;

#[cfg(test)]
#[path = "garden_step_kernel_tests.rs"]
mod garden_step_kernel_tests;
#[cfg(test)]
#[path = "measurement_hysteresis_kernel_tests.rs"]
mod measurement_hysteresis_kernel_tests;

#[cfg(test)]
#[path = "little_seismograph_kernel_tests.rs"]
mod little_seismograph_kernel_tests;

#[cfg(test)]
#[path = "concurrent_effect_tests.rs"]
mod concurrent_effect_tests;
#[cfg(test)]
#[path = "resource_effect_tests.rs"]
mod resource_effect_tests;
