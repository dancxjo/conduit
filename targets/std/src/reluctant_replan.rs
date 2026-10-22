//! Explicit reluctant replacement at the ordinary installed execution boundary.
//! A cost recommendation is historical evidence, never execution authority.
use crate::{ExternalForeInput, ExternalForeOutputAdapter, StdHost, StdRunReport, TimerAdapter};
use conduit_core::{
    ConnectionTrack, ExactNumericWork, ObservationKind, Plan, PlannedGear, PortDirection,
    PortTemporal, TerminalDisposition,
};
use conduit_planner::resource_cost::{
    CostEvaluationContext, ReluctantReplanDecision, ReluctantReplanGate, ReluctantReplanPolicy,
    ReluctantReplanRefusal, ReplacementCostFacts, ReplanCostObservation, MAXIMUM_COST_INPUT_BYTES,
    MAXIMUM_REPLAN_OBSERVATIONS,
};
use std::{fmt, io::Write};

/// Maximum scanned observations and kernel Signs from the prior one-Value report.
pub const STD_REPLAN_PREVIOUS_EVIDENCE_CAPACITY: usize = 256;

/// All borrowed facts belong to the caller's explicit decision owner. Neither
/// an old report nor this request reconstructs authority or a live Play.
pub struct StdReluctantReplanRequest<'a> {
    pub previous_report: &'a StdRunReport,
    pub policy: ReluctantReplanPolicy,
    pub costs: &'a ReplacementCostFacts<'a>,
    pub observations: &'a [ReplanCostObservation<'a>],
    pub context: &'a CostEvaluationContext<'a>,
    pub inputs: &'a [ExternalForeInput],
}
#[derive(Debug)]
pub enum StdReluctantReplanRefusal {
    UnsupportedProfile,
    RequestChanged,
    PreviousOutcomeUnavailable,
    CurrentOfferChanged,
    Policy(ReluctantReplanRefusal),
    /// The ordinary route may refuse admission or fail after starting. This
    /// text does not infer that an attempted output was externally accepted.
    Execution(String),
}
#[derive(Debug)]
pub struct StdReluctantReplanError {
    pub reason: StdReluctantReplanRefusal,
    /// Retain the recommendation even if ordinary admission/execution fails.
    /// The gate remains latched; failure is not permission to retry effects.
    pub decision: Option<Box<ReluctantReplanDecision>>,
}
impl fmt::Display for StdReluctantReplanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.reason)
    }
}
impl std::error::Error for StdReluctantReplanError {}
fn before(reason: StdReluctantReplanRefusal) -> StdReluctantReplanError {
    StdReluctantReplanError {
        reason,
        decision: None,
    }
}
#[derive(Debug)]
pub struct StdReluctantReplanOutcome {
    pub decision: Box<ReluctantReplanDecision>,
    /// None means keep the current Plan: no kernel or adapter was invoked.
    pub replacement_report: Option<StdRunReport>,
}
impl StdHost {
    /// Consider one same-meaning pure Value replacement after the old Play
    /// completed. The caller retains the old Plan/report and owns gate lifetime.
    /// No old input or output is replayed; `inputs` are a separate new request.
    pub fn run_reluctant_replacement_to<W: Write, T: TimerAdapter>(
        &mut self,
        gate: &mut ReluctantReplanGate,
        request: StdReluctantReplanRequest<'_>,
        adapter: &mut dyn ExternalForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        work: &mut ExactNumericWork,
    ) -> Result<StdReluctantReplanOutcome, StdReluctantReplanError> {
        use StdReluctantReplanRefusal as R;
        if self.advertisement() != request.context.host {
            return Err(before(R::CurrentOfferChanged));
        }
        let old = request.costs.previous();
        let candidate = request.costs.candidate();
        profile(old)
            .and_then(|_| profile(candidate))
            .map_err(before)?;
        let (old_gear, new_gear) = request.costs.gears();
        if old.fragments[0].placements[0] != *old_gear
            || candidate.fragments[0].placements[0] != *new_gear
        {
            return Err(before(R::UnsupportedProfile));
        }
        // Physical realization can change; the exact external Fore contract
        // cannot change while pretending to fulfill the same submitted demand.
        for port in &old.fragments[0].fore_ports {
            let mut expected = port.clone();
            expected.placement_id = new_gear.placement_id.clone();
            if !candidate.fragments[0].fore_ports.contains(&expected) {
                return Err(before(R::RequestChanged));
            }
        }
        validate_previous(old, request.previous_report).map_err(before)?;
        validate_request(candidate, request.inputs, request.observations).map_err(before)?;
        let decision = Box::new(
            gate.evaluate(
                request.policy,
                request.costs,
                request.observations,
                request.context,
                work,
            )
            .map_err(|e| before(R::Policy(e)))?,
        );
        if !decision.recommends_replacement() {
            return Ok(StdReluctantReplanOutcome {
                decision,
                replacement_report: None,
            });
        }
        // This is the existing reservation/factory/Host Call/Fore route. It
        // checks the current offer again; the recommendation grants nothing.
        match self.run_external_plot_to(
            candidate.fragments[0].clone(),
            request.inputs,
            adapter,
            output,
            timer,
        ) {
            Ok(report) => Ok(StdReluctantReplanOutcome {
                decision,
                replacement_report: Some(report),
            }),
            Err(error) => Err(StdReluctantReplanError {
                reason: R::Execution(error),
                decision: Some(decision),
            }),
        }
    }
}
fn profile(plan: &Plan) -> Result<&PlannedGear, StdReluctantReplanRefusal> {
    use StdReluctantReplanRefusal::UnsupportedProfile as R;
    if plan.fragments.len() != 1 || !plan.realization_backs.is_empty() {
        return Err(R);
    }
    let fragment = &plan.fragments[0];
    if fragment.placements.len() != 1
        || fragment.fore_ports.len() != 2
        || !fragment.realization_backs.is_empty()
        || !fragment.connections.is_empty()
        || !fragment.execution_fusions.is_empty()
        || !fragment.execution_regions.is_empty()
        || !fragment.states.is_empty()
        || !fragment.shared_pools.is_empty()
    {
        return Err(R);
    }
    let gear = &fragment.placements[0];
    // These genuine installed profiles implement one checked pure expression.
    // Other effects/flows/resources need their own admission/lifecycle bridge.
    if !matches!(
        gear.implementation_id.as_str(),
        conduit_std_offers::PURE_EXPRESSION_STD_IMPLEMENTATION
            | conduit_std_offers::PURE_SCALAR_TAPE_STD_IMPLEMENTATION
            | conduit_std_offers::PURE_EXPRESSION_LOCAL_IMPLEMENTATION
    ) || gear.inputs.len() != 1
        || gear.outputs.len() != 1
        || gear.inputs[0].temporal != PortTemporal::Value
        || gear.outputs[0].temporal != PortTemporal::Value
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
        || fragment.fore_ports.iter().any(|p| {
            p.item_capacity != 1
                || p.byte_capacity == 0
                || p.byte_capacity as usize > MAXIMUM_COST_INPUT_BYTES
                || p.temporal != PortTemporal::Value
                || p.track != ConnectionTrack::Payload
                || p.selected_line.is_some()
                || p.placement_id != gear.placement_id
        })
        || fragment
            .fore_ports
            .iter()
            .filter(|p| p.direction == PortDirection::Input)
            .count()
            != 1
    {
        return Err(R);
    }
    Ok(gear)
}
fn validate_previous(plan: &Plan, report: &StdRunReport) -> Result<(), StdReluctantReplanRefusal> {
    use StdReluctantReplanRefusal::PreviousOutcomeUnavailable as R;
    let kernel = report.kernel.as_ref().ok_or(R)?;
    // This one-Value profile has a finite retained historical input budget.
    // Refuse a foreign oversized report before scanning its observations.
    if report.observations.len() > STD_REPLAN_PREVIOUS_EVIDENCE_CAPACITY
        || kernel.kernel_sign.len() > STD_REPLAN_PREVIOUS_EVIDENCE_CAPACITY
    {
        return Err(R);
    }
    let identity = &kernel.active_play;
    let fragment = &plan.fragments[0];
    if identity.plan_id != plan.plan_id
        || identity.host_id != fragment.host_id
        || identity.boot_id != fragment.boot_id
        || identity.active_play_id != kernel.active_play_id
        || kernel.identity.plan_id != plan.plan_id
        || kernel.identity.active_play_id != identity.active_play_id
        || usize::from(kernel.kernel_events) != kernel.kernel_sign.len()
    {
        return Err(R);
    }
    if kernel
        .kernel_sign
        .iter()
        .enumerate()
        .any(|(i, event)| event.sequence != i as u32)
        || kernel
            .kernel_sign
            .iter()
            .filter(|e| e.kind == conduit_kernel::KernelEventKind::BackCompleted)
            .count()
            != 1
        || kernel
            .kernel_sign
            .iter()
            .filter(|e| e.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered)
            .count()
            != 1
    {
        return Err(R);
    }
    let mut terminals = report.observations.iter().filter(|o| {
        o.plan_id.as_ref() == Some(&plan.plan_id)
            && o.active_play_id.as_ref() == Some(&identity.active_play_id)
            && o.host_id == identity.host_id
            && o.boot_id == identity.boot_id
            && matches!(o.kind, ObservationKind::PlanTerminal { .. })
    });
    if !terminals.next().is_some_and(|o| {
        matches!(
            o.kind,
            ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            }
        )
    }) || terminals.next().is_some()
    {
        return Err(R);
    }
    Ok(())
}
fn validate_request(
    plan: &Plan,
    inputs: &[ExternalForeInput],
    observations: &[ReplanCostObservation<'_>],
) -> Result<(), StdReluctantReplanRefusal> {
    use StdReluctantReplanRefusal::RequestChanged as R;
    if inputs.len() != 1
        || inputs[0].bytes.len() > MAXIMUM_COST_INPUT_BYTES
        || observations.is_empty()
        || observations.len() > MAXIMUM_REPLAN_OBSERVATIONS
    {
        return Err(R);
    }
    let front = plan.fragments[0]
        .fore_ports
        .iter()
        .find(|p| p.direction == PortDirection::Input)
        .ok_or(R)?;
    let input = &inputs[0];
    if input.front_port_id != front.front_port_id
        || input.track != front.track
        || input.bytes.len() > front.byte_capacity as usize
    {
        return Err(R);
    }
    for observation in observations {
        for forecast in [observation.previous, observation.candidate] {
            let demand = forecast.demand();
            if demand.inputs().len() != 1 {
                return Err(R);
            }
            let expected = demand.inputs()[0];
            if expected.port != &front.gear_port_id
                || expected.value.value_kind != front.value_kind
                || expected.value.encoded != input.bytes
            {
                return Err(R);
            }
        }
    }
    Ok(())
}
