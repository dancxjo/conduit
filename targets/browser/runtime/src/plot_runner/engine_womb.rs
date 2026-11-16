//! The existing kernel's exact local Womb input/output Fore, never a runner.
use super::*;
use conduit_body_make::womb_runtime::*;
use conduit_core::{PortDirection, PortTemporal};
use conduit_plan_lowering::lowering::LoweredForePort;

pub(super) struct WombFore {
    input: LoweredForePort,
    output: LoweredForePort,
    #[cfg(feature = "creche-surface")]
    plan: conduit_core::PlanId,
    #[cfg(feature = "creche-surface")]
    placement: conduit_core::PlacementId,
    request: WombRequest,
}

pub(super) fn prepare(
    partitions: &[(&PlanFragment, &LoweredPlanFragment)],
    body_plan: Option<&conduit_body::BodyPlan>,
) -> Result<Option<WombFore>, String> {
    let mut result = None;
    for (fragment, lowered) in partitions {
        for gear in &fragment.placements {
            if gear.kind_id.as_str() != WOMB_KIND {
                continue;
            }
            if result.is_some() {
                return Err("one browser Womb Fore is admitted per Body Play".into());
            }
            let body_plan =
                body_plan.ok_or("Womb cannot execute outside the actual Body runner")?;
            let request = prepared_request(body_plan, fragment, gear)?;
            let input = port(
                lowered,
                PortDirection::Input,
                WOMB_REQUEST_KIND,
                WOMB_REQUEST_BYTES,
            )?;
            let output = port(
                lowered,
                PortDirection::Output,
                WOMB_OUTCOME_KIND,
                WOMB_OUTCOME_BYTES,
            )?;
            result = Some(WombFore {
                input,
                output,
                #[cfg(feature = "creche-surface")]
                plan: body_plan.plan_id.clone(),
                #[cfg(feature = "creche-surface")]
                placement: gear.placement_id.clone(),
                request,
            });
        }
    }
    Ok(result)
}

fn port(
    lowered: &LoweredPlanFragment,
    direction: PortDirection,
    kind: &str,
    bytes: usize,
) -> Result<LoweredForePort, String> {
    let ports = lowered
        .fore_ports
        .iter()
        .filter(|port| port.direction == direction)
        .collect::<Vec<_>>();
    let [port] = ports.as_slice() else {
        return Err("Womb requires one exact Fore in each direction".into());
    };
    if port.value_kind.as_str() != kind
        || port.temporal != PortTemporal::Value
        || port.item_capacity != 1
        || port.byte_capacity != bytes as u32
        || port.selected_line.is_some()
    {
        return Err("Womb Fore differs from its admitted local value contract".into());
    }
    Ok((*port).clone())
}

pub(super) fn inject(scheduler: &mut TourScheduler) -> Result<(), String> {
    let Some(fore) = &scheduler.womb_fore else {
        return Ok(());
    };
    let input = fore.input.clone();
    let request = fore.request.0;
    input.validate_value(&request).map_err(debug_error)?;
    match scheduler
        .admit_remote_input(input.endpoint, input.cord, 0, &request)
        .map_err(debug_error)?
    {
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. } => {}
        conduit_kernel::scheduler::RemoteIngressOutcome::Full { .. } => {
            return Err("admitted Womb input Fore is unexpectedly full".into())
        }
    }
    scheduler
        .close_remote_input(input.endpoint, input.cord)
        .map_err(debug_error)
}

pub(super) fn capture(scheduler: &mut TourScheduler) -> Result<(), String> {
    let Some(fore) = &scheduler.womb_fore else {
        return Ok(());
    };
    let endpoint = fore.output.endpoint;
    let cord = fore.output.cord;
    let Some(offered) = scheduler
        .kernel
        .remote_egress_offer(endpoint, cord)
        .map_err(debug_error)?
    else {
        return Ok(());
    };
    let bytes = scheduler
        .kernel
        .host_value(offered.value)
        .map_err(debug_error)?;
    let outcome = WombOutcome::decode(bytes).ok_or("malformed Womb outcome")?;
    if outcome.request != fore.request {
        return Err("Womb emitted another provision's outcome".into());
    }
    #[cfg(feature = "creche-surface")]
    crate::workspace_runtime::womb_provision::record_kernel_outcome(
        &fore.plan,
        &fore.placement,
        outcome,
    )?;
    scheduler
        .kernel
        .remote_egress_accept(endpoint, cord, offered.sequence)
        .map_err(debug_error)?;
    scheduler
        .kernel
        .remote_egress_delivered(endpoint, cord, offered.sequence)
        .map_err(debug_error)
}

fn prepared_request(
    _plan: &conduit_body::BodyPlan,
    _fragment: &PlanFragment,
    _gear: &conduit_core::PlannedGear,
) -> Result<WombRequest, String> {
    #[cfg(feature = "creche-surface")]
    {
        crate::workspace_runtime::womb_provision::prepared_request(_plan, _fragment, _gear)
    }
    #[cfg(not(feature = "creche-surface"))]
    {
        Err("Womb has no installed private issuer owner".into())
    }
}

#[cfg(feature = "creche-surface")]
pub(super) fn validate_completion(
    scheduler: &TourScheduler,
    pending: &PendingHostEffect,
    output: &[u8],
) -> Result<u32, String> {
    let input = scheduler
        .host_value(pending.request.input.value)
        .map_err(debug_error)?;
    let request = WombRequest(
        input
            .try_into()
            .map_err(|_| "Womb request must have 32 bytes")?,
    );
    crate::workspace_runtime::womb_provision::validate_completion(request, output)?;
    Ok(WOMB_OUTCOME_BYTES as u32)
}

#[cfg(not(feature = "creche-surface"))]
pub(super) fn validate_completion(
    _: &TourScheduler,
    _: &PendingHostEffect,
    _: &[u8],
) -> Result<u32, String> {
    Err("Womb has no private issuer owner".into())
}
