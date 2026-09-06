//! Finite generic browser execution envelope for inline Forms.

#[path = "engine_preparation.rs"]
mod preparation;
#[path = "engine_transforms.rs"]
mod transforms;
use preparation::{prepare_scheduler, validate_envelope};

#[cfg(test)]
#[path = "quantity_tests.rs"]
mod quantity_tests;

use crate::installed_browser::{
    factory, BrowserManifestation, BrowserOperation, BROWSER_HOST_OPERATIONS_PER_GEAR,
    BROWSER_HOST_OPERATION_BINDINGS, BROWSER_PENDING_REQUESTS, BROWSER_PORTS_PER_GEAR,
    BROWSER_QUEUE_SLOTS, BROWSER_ROUTE_SLOTS, BROWSER_ROUTE_TARGETS, BROWSER_SIGN_ITEMS,
    BROWSER_TOTAL_VALUE_BYTES, BROWSER_VALUE_ITEMS, MAXIMUM_BROWSER_CORDS, MAXIMUM_BROWSER_GEARS,
    MAXIMUM_BROWSER_VALUE_BYTES,
};
use conduit_core::PlanFragment;
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, NodeSpec, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, CordEndpoint, CordId, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationOutcome, HostedSignLog, HostedValueStore, NodeId,
    PortId,
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
    BROWSER_HOST_OPERATION_BINDINGS,
    BROWSER_PENDING_REQUESTS,
>;

/// Host-prepared state accompanies, but never replaces, the production kernel.
pub(super) struct TourScheduler {
    kernel: BrowserKernel,
    selectors: [Option<crate::installed_browser::pointer_selector::PreparedSelector>;
        MAXIMUM_BROWSER_GEARS],
    mappings: [Option<conduit_semantic_catalog::QuantityMapping>; MAXIMUM_BROWSER_GEARS],
    timing: [Option<crate::installed_browser::timing::PreparedTiming>; MAXIMUM_BROWSER_GEARS],
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
    pub request: HostOperationRequest,
    pub effect: BrowserHostEffect,
}

pub(super) enum BrowserHostEffect {
    Timer { duration_millis: u64 },
    KeyEvent,
    PointerEvent,
    ButtonTransition,
    Manifestation(BrowserManifestation),
}

pub(super) enum DriveStatus {
    Effect(PendingHostEffect),
    Complete,
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
        DriveStatus::Complete => {
            return Err("Tour Play completed without a planned Host effect".into())
        }
    };
    Ok((scheduler, pending))
}

pub(super) fn prepare_remote_fragment(
    fragment: &PlanFragment,
) -> Result<(TourScheduler, LoweredPlanFragment), String> {
    let lowered = lower_plan_fragment(fragment)
        .map_err(|error| format!("lower multi-Host executable-tour Plan: {error:?}"))?;
    validate_envelope(fragment, &lowered, true)?;
    let scheduler = prepare_scheduler(fragment, &lowered)?;
    Ok((scheduler, lowered))
}

pub(super) fn complete_host_effect(
    scheduler: &mut TourScheduler,
    pending: &PendingHostEffect,
) -> Result<(), String> {
    scheduler
        .complete_host_operation(
            pending.request.node,
            pending.request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .map_err(debug_error)
}

pub(super) fn complete_host_effect_with_output(
    scheduler: &mut TourScheduler,
    pending: &PendingHostEffect,
    output: &[u8],
) -> Result<(), String> {
    let maximum_output_bytes = match &pending.effect {
        BrowserHostEffect::PointerEvent => {
            let value = conduit_core::StructuredInfoValue::from_canonical_bytes(output)
                .map_err(|error| format!("decode pointer input: {error:?}"))?;
            if value.value_type() != &conduit_semantic_catalog::pointer_event_type() {
                return Err("pointer input has the wrong exact type".into());
            }
            MAXIMUM_BROWSER_VALUE_BYTES as u32
        }
        BrowserHostEffect::KeyEvent => {
            conduit_human::KeyEvent::decode(output)
                .map(|_| ())
                .map_err(|error| format!("decode browser key event: {error:?}"))?;
            conduit_human::KEY_EVENT_ENCODED_LEN as u32
        }
        BrowserHostEffect::ButtonTransition => {
            conduit_semantic_catalog::map_button_transition_to_indicator(output)
                .map_err(|error| format!("decode browser button transition: {error:?}"))?;
            conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES
        }
        _ => return Err("browser Host effect does not accept completion output".into()),
    };
    let value = scheduler.store_host_value(output).map_err(debug_error)?;
    scheduler
        .complete_host_operation(
            pending.request.node,
            pending.request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(value, maximum_output_bytes)
                        .map_err(|_| "browser input exceeded its planned bound")?,
                ),
                failure: None,
            },
        )
        .map_err(debug_error)
}

pub(super) fn drive(
    scheduler: &mut TourScheduler,
    fragment: &PlanFragment,
) -> Result<DriveStatus, String> {
    loop {
        if let Some(request) = scheduler.next_host_request() {
            let placement = fragment
                .placements
                .get(usize::from(request.node.0))
                .ok_or_else(|| "browser request has no planned placement".to_string())?;
            let operation = placement
                .host_operations
                .get(usize::from(request.operation.0))
                .ok_or_else(|| "browser request has no planned Host operation".to_string())?;
            if transforms::complete_transform(scheduler, placement, operation, request)? {
                continue;
            }
            let input = scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?
                .to_vec();
            if operation.contract_id.as_str() == conduit_core::WAIT_HOST_OPERATION_CONTRACT {
                let duration_millis = decode_timer_duration(operation, &input)?;
                return Ok(DriveStatus::Effect(PendingHostEffect {
                    request,
                    effect: BrowserHostEffect::Timer { duration_millis },
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
            let installation = factory(&placement.implementation_id)
                .ok_or_else(|| "browser request implementation is not installed".to_string())?;
            let perform = installation.perform.ok_or_else(|| {
                "local browser implementation requested an unknown Host operation".to_string()
            })?;
            let result = perform(placement, &input)?;
            match (result.output, result.manifestation) {
                (Some(output), None) => {
                    let output = scheduler.store_host_value(&output).map_err(debug_error)?;
                    scheduler
                        .complete_host_operation(
                            request.node,
                            request.request,
                            HostOperationOutcome {
                                disposition: HostOperationDisposition::Completed,
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
        match scheduler.step().map_err(debug_error)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => return Ok(DriveStatus::Complete),
            SchedulerStatus::Idle => return Err("Tour Play became idle".into()),
            SchedulerStatus::Cancelled => return Err("Tour Play was cancelled".into()),
        }
    }
}

fn decode_timer_duration(
    operation: &conduit_core::HostOperationRequirement,
    input: &[u8],
) -> Result<u64, String> {
    if operation.target_kind.is_some()
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
