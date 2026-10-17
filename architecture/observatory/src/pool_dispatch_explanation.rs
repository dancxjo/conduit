//! Read-only join of owner-produced dispatch and forecast preparation receipts.
use crate::cost_explanation::{exact, stage_range, BoundedText, CostExplanationRefusal};
use alloc::string::String;
use conduit_core::{ExactNumericWork, Plan, PoolCostSourceReceipt, Unit};
use conduit_plan_lowering::shared_pool_runtime::{
    CostAwareSharedPoolAdmissionOutcome, SharedPoolAdmissionOutcome,
    MAXIMUM_POOL_WAITING_OPERATIONS,
};
use conduit_planner::resource_cost::{
    FulfillmentCostForecast, PoolForecastPreparationAccount, MAXIMUM_COST_CANDIDATES,
    MAXIMUM_COST_ID_BYTES, MAXIMUM_COST_INPUTS, MAXIMUM_COST_INPUT_BYTES,
};
use core::fmt::Write;

/// Inspect retained receipts only. The mutable argument is an arithmetic budget,
/// never a pool/controller. Missing execution measurements remain missing.
pub fn render_pool_dispatch(
    plan: &Plan,
    dispatch: &CostAwareSharedPoolAdmissionOutcome,
    forecasts: &[PoolForecastPreparationAccount],
    work: &mut ExactNumericWork,
) -> Result<String, CostExplanationRefusal> {
    use CostExplanationRefusal::{Extent, InconsistentSelection};
    if forecasts.len() > MAXIMUM_COST_CANDIDATES {
        return Err(Extent);
    }
    let account = &dispatch.account;
    account.validate(plan).map_err(|_| InconsistentSelection)?;
    let evidence = match &dispatch.admission {
        SharedPoolAdmissionOutcome::Selected { member, evidence } => {
            if evidence.selected_realization != Some(member.placement.realization) {
                return Err(InconsistentSelection);
            }
            evidence
        }
        SharedPoolAdmissionOutcome::Refused(evidence) => evidence,
    };
    if evidence != &account.evidence {
        return Err(InconsistentSelection);
    }
    let pool = plan.fragments[0]
        .shared_pools
        .iter()
        .find(|p| p.pool_id == evidence.pool_id)
        .ok_or(InconsistentSelection)?;
    let population = dispatch.population_before_selection;
    if population.occupied_members > pool.maximum_members
        || population.waiting_operations > MAXIMUM_POOL_WAITING_OPERATIONS
    {
        return Err(Extent);
    }
    for (index, forecast) in forecasts.iter().enumerate() {
        let binding = &forecast.binding;
        if binding.plan != evidence.plan_id
            || binding.pool != evidence.pool_id
            || binding.operation != evidence.operation_id
            || binding.demand != account.demand
            || forecasts[..index]
                .iter()
                .any(|f| f.binding.realization == binding.realization)
        {
            return Err(InconsistentSelection);
        }
        if forecast.input_count > MAXIMUM_COST_INPUTS
            || forecast.input_bytes > MAXIMUM_COST_INPUT_BYTES
            || forecast
                .work
                .as_ref()
                .is_some_and(|(id, revision, maximum, amount)| {
                    id.as_str().is_empty()
                        || id.as_str().len() > MAXIMUM_COST_ID_BYTES
                        || *revision == 0
                        || amount > maximum
                })
        {
            return Err(Extent);
        }
        if forecast.input_count != forecast.inputs.len()
            || forecast
                .inputs
                .iter()
                .try_fold(0usize, |sum, input| sum.checked_add(input.bytes))
                != Some(forecast.input_bytes)
            || forecast.inputs.iter().enumerate().any(|(index, input)| {
                input.port.as_str().is_empty()
                    || input.port.as_str().len() > MAXIMUM_COST_ID_BYTES
                    || input.kind.as_str().is_empty()
                    || input.kind.as_str().len() > MAXIMUM_COST_ID_BYTES
                    || forecast.inputs[..index]
                        .iter()
                        .any(|prior| prior.port == input.port)
            })
        {
            return Err(InconsistentSelection);
        }
        let source = account
            .sources
            .iter()
            .find(|s| s.realization == binding.realization)
            .ok_or(InconsistentSelection)?;
        if !matches_source(forecast, source) {
            return Err(InconsistentSelection);
        }
        let sum = FulfillmentCostForecast::sequential(forecast.stages, work).map_err(|e| {
            if matches!(
                e,
                conduit_planner::resource_cost::CostEvidenceRefusal::Numeric(_)
            ) {
                Extent
            } else {
                InconsistentSelection
            }
        })?;
        if sum.lower_seconds() != forecast.lower_seconds
            || sum.upper_seconds() != forecast.upper_seconds
            || source.duration.source().unit() != Unit::resolve("s").expect("builtin Time Unit")
            || source.duration.source().coordinate() != forecast.upper_seconds
        {
            return Err(InconsistentSelection);
        }
    }
    let mut out = BoundedText(crate::render_pool_cost_selection(plan, account)?);
    writeln!(out, "sample=before-selection occupied_members={} waiting_operations={} (owner snapshot; not predicted Time)", population.occupied_members, population.waiting_operations).map_err(|_| Extent)?;
    writeln!(out, "dispatch_receipt_execution_outcome=not-retained dispatch_receipt_measured_latency=not-retained warmup_prediction=not-separately-retained").map_err(|_| Extent)?;
    for candidate in &account.candidates {
        let Some(f) = forecasts
            .iter()
            .find(|f| f.binding.realization == candidate.realization)
        else {
            writeln!(
                out,
                "candidate={} demand_details=not-retained stage_predictions=not-retained",
                candidate.realization
            )
            .map_err(|_| Extent)?;
            continue;
        };
        writeln!(
            out,
            "candidate={} retained_input_count={} retained_input_bytes={} forecast_sign={}",
            candidate.realization,
            f.input_count,
            f.input_bytes,
            f.source.sign_id.as_str().escape_default()
        )
        .map_err(|_| Extent)?;
        if let Some((identity, revision, maximum, amount)) = &f.work {
            writeln!(
                out,
                "work_metric={} revision={} admitted_maximum={} owner_derived_amount={}",
                identity.as_str().escape_default(),
                revision,
                maximum,
                amount
            )
            .map_err(|_| Extent)?;
        } else {
            writeln!(out, "work_metric=not-retained").map_err(|_| Extent)?;
        }
        stage_range("setup_prediction", f.stages.setup, &mut out).map_err(|_| Extent)?;
        stage_range("queue_prediction", f.stages.queue, &mut out).map_err(|_| Extent)?;
        stage_range("service_prediction", f.stages.service, &mut out).map_err(|_| Extent)?;
        stage_range("transfer_prediction", f.stages.transfer, &mut out).map_err(|_| Extent)?;
        write!(out, "total_prediction_seconds=").map_err(|_| Extent)?;
        exact(f.lower_seconds, &mut out).map_err(|_| Extent)?;
        write!(out, "..").map_err(|_| Extent)?;
        exact(f.upper_seconds, &mut out).map_err(|_| Extent)?;
        writeln!(out).map_err(|_| Extent)?;
    }
    Ok(out.0)
}

fn matches_source(f: &PoolForecastPreparationAccount, s: &PoolCostSourceReceipt) -> bool {
    let a = &f.source;
    a.host_id == s.host
        && a.boot_id == s.boot
        && a.offer_generation == s.generation
        && a.host_profile == s.host_profile
        && a.capability_id == s.capability
        && a.kind_id == s.kind
        && a.kind_contract_revision == s.kind_contract
        && a.implementation_id == s.implementation
        && a.implementation_artifact == s.artifact
        && a.execution_profile_id == s.execution_profile
        && a.resource == s.resource
        && a.sign_id == s.sign
        && a.calibration_identity == *s.calibration.identity()
        && a.calibration_revision == s.calibration.revision()
        && a.window.observed().clock() == s.observed.clock()
        && a.window.observed().ticks() <= s.observed.ticks()
        && a.window.expires().ticks() >= s.expires.ticks()
}
