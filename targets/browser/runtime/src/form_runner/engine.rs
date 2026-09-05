//! Finite generic browser execution envelope for inline Forms.

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

pub(super) type TourScheduler = FixedScheduler<
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

pub(super) struct PendingHostEffect {
    pub request: HostOperationRequest,
    pub effect: BrowserHostEffect,
}

pub(super) enum BrowserHostEffect {
    Timer { duration_millis: u64 },
    KeyEvent,
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

fn validate_envelope(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    allow_one_remote_endpoint: bool,
) -> Result<(), String> {
    let route_targets = lowered
        .routes
        .iter()
        .map(|route| route.targets.len())
        .sum::<usize>();
    if lowered.nodes.is_empty()
        || lowered.nodes.len() > MAXIMUM_BROWSER_GEARS
        || lowered.cords.is_empty()
        || lowered.cords.len() > MAXIMUM_BROWSER_CORDS
        || lowered.cord_value_slots as usize > BROWSER_QUEUE_SLOTS
        || lowered.routes.len() > BROWSER_ROUTE_SLOTS
        || route_targets > BROWSER_ROUTE_TARGETS
        || if allow_one_remote_endpoint {
            lowered.remote_endpoints.len() != 1
        } else {
            !lowered.remote_endpoints.is_empty()
        }
        || lowered.host_operations.len() > BROWSER_HOST_OPERATION_BINDINGS
        || fragment
            .placements
            .iter()
            .any(|placement| factory(&placement.implementation_id).is_none())
    {
        return Err("Form exceeds the installed finite browser execution envelope".into());
    }
    Ok(())
}

fn prepare_scheduler(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<TourScheduler, String> {
    let active_nodes = lowered.nodes.len();
    let active_cords = lowered.cords.len();
    let mut values = HostedValueStore::new(
        BROWSER_VALUE_ITEMS,
        MAXIMUM_BROWSER_VALUE_BYTES as u32,
        BROWSER_TOTAL_VALUE_BYTES,
    )
    .map_err(|error| format!("browser value store: {error:?}"))?;
    let mut operations = Vec::with_capacity(MAXIMUM_BROWSER_GEARS);
    for node in &lowered.nodes {
        let placement = fragment
            .placements
            .get(usize::from(node.node.0))
            .ok_or_else(|| "lowered browser node has no planned placement".to_string())?;
        let installation = factory(&placement.implementation_id)
            .ok_or_else(|| "planned browser implementation is not installed".to_string())?;
        operations.push((installation.prepare)(placement, &mut values)?);
    }
    while operations.len() < MAXIMUM_BROWSER_GEARS {
        operations.push(BrowserOperation::inactive());
    }
    let drivers = operations
        .into_iter()
        .map(|operation| OperationDriver::new(operation).map_err(debug_error))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "browser operation table exceeded its admitted bound")?;

    let inactive_node = NodeSpec {
        input_cords: [None; BROWSER_PORTS_PER_GEAR],
        maximum_step_work: 1,
    };
    let mut nodes = [inactive_node; MAXIMUM_BROWSER_GEARS];
    nodes[..active_nodes].copy_from_slice(&lowered.node_specs);
    let inactive_cord = CordSpec {
        cord: CordId(u16::MAX),
        source: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        sink: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        slot_start: u16::MAX,
        item_capacity: 0,
        byte_capacity: 0,
    };
    let mut cords = [inactive_cord; MAXIMUM_BROWSER_CORDS];
    for (destination, lowered_cord) in cords.iter_mut().zip(&lowered.cords) {
        *destination = lowered_cord.spec;
    }
    let mut routes = FixedRoutes::<BROWSER_ROUTE_SLOTS, BROWSER_ROUTE_TARGETS>::new(
        BROWSER_PORTS_PER_GEAR as u16,
    );
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(debug_error)?;
    }
    routes.seal().map_err(debug_error)?;
    let mut bindings = FixedHostOperationBindings::<BROWSER_HOST_OPERATION_BINDINGS>::new(
        BROWSER_HOST_OPERATIONS_PER_GEAR,
    );
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(debug_error)?;
    }
    bindings.seal().map_err(debug_error)?;
    let sign_bytes = u32::from(BROWSER_SIGN_ITEMS)
        .checked_mul(
            u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>())
                .map_err(|_| "browser Sign size overflow")?,
        )
        .ok_or("browser Sign budget overflow")?;
    let remote_sign_bytes = conduit_kernel::remote_sign_storage_bytes(BROWSER_SIGN_ITEMS)
        .ok_or("browser remote Sign budget overflow")?;
    let signs = HostedSignLog::new_with_remote_storage(
        BROWSER_SIGN_ITEMS,
        sign_bytes,
        BROWSER_SIGN_ITEMS,
        remote_sign_bytes,
    )
    .map_err(debug_error)?;
    TourScheduler::new_with_active_counts_and_host_operations(
        active_nodes,
        active_cords,
        nodes,
        cords,
        routes,
        bindings,
        drivers,
        values,
        signs,
    )
    .map_err(debug_error)
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
