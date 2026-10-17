//! Demand-specific forecast custody to the existing sealed pool handoff.
//! All prediction arithmetic and provider-state validation happen outside Play.
use super::{
    CostCalibrationRevision, CostDemand, CostEvidenceRefusal, CostExecutionObservation,
    CostForecastStages, CostSourceAttribution, CostedForecast,
};
use alloc::vec::Vec;
use conduit_core::{
    ArtifactId, CostExecutionCondition, ExactNumericWork, ExactRational, HostAdvertisement,
    MonotonicInstant, Plan, PlanId, PoolCostCalibrationRevision, PoolCostPreparation,
    PoolCostPreparationRefusal, PoolOperationId, PreparedPoolCost, QuantityRole, RationalQuantity,
    SharedPoolId, SignId, Unit,
};

pub struct PoolForecastPreparation<'a, 'current> {
    pub plan: &'current Plan,
    pub fragment_index: usize,
    pub pool: &'current SharedPoolId,
    pub operation: &'current PoolOperationId,
    /// The admitted operation owner's identity, not a user workload-class label.
    pub demand_identity: [u8; 32],
    pub realization: u16,
    pub request: CostDemand<'a>,
    pub forecast: &'a CostedForecast<'a>,
    pub current_host: &'current HostAdvertisement,
    pub current_calibration: CostCalibrationRevision<'current>,
    pub now: &'current MonotonicInstant,
    pub current_execution: Option<&'current CostExecutionObservation<'current>>,
    /// Explicit error budget in the common seconds coordinate.
    pub tolerance: RationalQuantity,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolForecastPreparationRefusal {
    Forecast(CostEvidenceRefusal),
    Preparation(PoolCostPreparationRefusal),
}
impl From<CostEvidenceRefusal> for PoolForecastPreparationRefusal {
    fn from(value: CostEvidenceRefusal) -> Self {
        Self::Forecast(value)
    }
}
impl From<PoolCostPreparationRefusal> for PoolForecastPreparationRefusal {
    fn from(value: PoolCostPreparationRefusal) -> Self {
        Self::Preparation(value)
    }
}
/// Exact operation owner that admitted this forecast's demand and realization.
/// This is correlation for retained inspection, not authority to schedule work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolForecastBinding {
    pub plan: PlanId,
    pub pool: SharedPoolId,
    pub operation: PoolOperationId,
    pub demand: [u8; 32],
    pub realization: u16,
}

/// Owned explanation retains stages and sources without canonical input bytes.
#[derive(Debug)]
pub struct PoolForecastPreparationAccount {
    pub binding: PoolForecastBinding,
    pub source: CostSourceAttribution,
    pub calibration: Option<CostSourceAttribution>,
    pub execution_assumption: Option<(CostSourceAttribution, CostExecutionCondition)>,
    pub current_execution: Option<(CostSourceAttribution, CostExecutionCondition)>,
    pub stages: CostForecastStages,
    pub lower_seconds: ExactRational,
    pub upper_seconds: ExactRational,
    pub input_count: usize,
    pub input_bytes: usize,
    /// Metric identity, revision, admitted maximum, actual owner-derived amount.
    pub work: Option<(ArtifactId, u64, u64, u64)>,
}
pub struct PreparedPoolForecast<'a> {
    pub cost: PreparedPoolCost<'a>,
    pub account: PoolForecastPreparationAccount,
}

pub fn prepare_pool_cost_forecast<'a>(
    input: PoolForecastPreparation<'a, '_>,
    work: &mut ExactNumericWork,
) -> Result<PreparedPoolForecast<'a>, PoolForecastPreparationRefusal> {
    let forecast = input.forecast;
    // Neither an opaque key nor a request's preferred class can replace exact
    // checked meaning, owner codec/metric revision, work extent and inputs.
    forecast.demand().validate_match(input.request)?;
    forecast.validate_for_execution(
        input.current_host,
        input.current_calibration,
        input.now,
        input.current_execution,
    )?;
    let pool = input
        .plan
        .fragments
        .get(input.fragment_index)
        .and_then(|fragment| {
            fragment
                .shared_pools
                .iter()
                .find(|pool| &pool.pool_id == input.pool)
        })
        .ok_or(PoolCostPreparationRefusal::PoolMissing)?;
    if input.request.class().gear().checked_front() != pool.member_front {
        return Err(PoolCostPreparationRefusal::IncompatibleFront.into());
    }
    let source = forecast.source();
    let mut observed = source.window().observed().clone();
    let mut expires = source.window().expires().clone();
    let mut supporting_signs = Vec::<SignId>::with_capacity(2);
    if let Some(calibration) = forecast.calibration() {
        let extra = calibration.source();
        // Each source was validated against the same independent local sample
        // above. Intersection never converts provider wall time or Host origins.
        if extra.window().observed().clock() != observed.clock() {
            return Err(CostEvidenceRefusal::ClockQualityChanged.into());
        }
        if extra.window().observed().ticks() > observed.ticks() {
            observed = extra.window().observed().clone();
        }
        if extra.window().expires().ticks() < expires.ticks() {
            expires = extra.window().expires().clone();
        }
        if extra.sign() != source.sign() {
            supporting_signs.push(extra.sign().clone());
        }
    }
    if let Some(historical) = forecast.execution_assumption() {
        let sign = historical.source().sign();
        if sign != source.sign() && !supporting_signs.contains(sign) {
            supporting_signs.push(sign.clone());
        }
        // An expired historical condition sample remains explanatory evidence;
        // it never supplies freshness in place of current_execution.
    }
    let revision = PoolCostCalibrationRevision::new(
        source.calibration().identity().clone(),
        source.calibration().revision(),
    )?;
    let duration = RationalQuantity::new(
        forecast.upper_seconds(),
        Unit::resolve("s").expect("reviewed builtin Time Unit"),
        QuantityRole::Linear,
    )
    .map_err(CostEvidenceRefusal::from)?;
    let cost = PreparedPoolCost::prepare(
        PoolCostPreparation {
            plan: input.plan,
            fragment_index: input.fragment_index,
            pool: input.pool,
            operation: input.operation,
            demand: input.demand_identity,
            realization: input.realization,
            host: source.host(),
            offer: source.offer(),
            resource: source.resource(),
            calibration: &revision,
            expected_execution: forecast.expected_execution_condition(),
            sign: source.sign(),
            supporting_signs: &supporting_signs,
            observed: &observed,
            expires: &expires,
            evaluated_at: input.now,
            duration,
            tolerance: input.tolerance,
        },
        work,
    )?;
    Ok(PreparedPoolForecast {
        cost,
        account: PoolForecastPreparationAccount {
            binding: PoolForecastBinding {
                plan: input.plan.plan_id.clone(),
                pool: input.pool.clone(),
                operation: input.operation.clone(),
                demand: input.demand_identity,
                realization: input.realization,
            },
            source: CostSourceAttribution::from_source(source),
            calibration: forecast
                .calibration()
                .map(|c| CostSourceAttribution::from_source(c.source())),
            execution_assumption: forecast.execution_assumption().map(|e| {
                (
                    CostSourceAttribution::from_source(e.source()),
                    e.condition(),
                )
            }),
            current_execution: input
                .current_execution
                .filter(|_| forecast.expected_execution_condition().is_some())
                .map(|e| {
                    (
                        CostSourceAttribution::from_source(e.source()),
                        e.condition(),
                    )
                }),
            stages: forecast.forecast().stages(),
            lower_seconds: forecast.lower_seconds(),
            upper_seconds: forecast.upper_seconds(),
            input_count: input.request.inputs().len(),
            input_bytes: input.request.input_bytes(),
            work: input.request.work_estimate().map(|w| {
                (
                    w.metric_identity().clone(),
                    w.metric_revision(),
                    w.maximum_units(),
                    w.units(),
                )
            }),
        },
    })
}
