//! Immutable native payload custody during preparation-side pool waiting.
//! This owns input bytes, not selection policy or resource reservations.
use conduit_core::{Plan, ValuePayload};
use conduit_plan_lowering::shared_pool_runtime::QueuedPoolOperation;
use conduit_planner::resource_cost::{
    prepare_pool_cost_forecast, CostDemand, PoolForecastPreparation,
    PoolForecastPreparationRefusal, PreparedPoolForecast,
};

/// Checked meaning remains pinned by immutable caller-owned source/demand.
/// A separate owned payload is retained for delivery; access never exposes a
/// mutable byte buffer or permits replacing the request's workload class.
pub struct NativePoolRequestCustody<'a> {
    operation: QueuedPoolOperation,
    demand: CostDemand<'a>,
    payload: ValuePayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePoolCustodyRefusal {
    InvalidPlan,
    Correlation,
    FrontChanged,
    InputShape,
    ExtentChanged,
}

impl<'a> NativePoolRequestCustody<'a> {
    /// Native model members have one exact input. Validation and copying occur
    /// before queuing, outside Play. No provider is selected or occupied here.
    pub fn prepare(
        plan: &Plan,
        operation: QueuedPoolOperation,
        demand: CostDemand<'a>,
    ) -> Result<Self, NativePoolCustodyRefusal> {
        use NativePoolCustodyRefusal as R;
        if !conduit_core::verify_plan(plan) {
            return Err(R::InvalidPlan);
        }
        if operation.plan != plan.plan_id {
            return Err(R::Correlation);
        }
        let pool = plan
            .fragments
            .first()
            .and_then(|fragment| {
                fragment
                    .shared_pools
                    .iter()
                    .find(|pool| pool.pool_id == operation.pool)
            })
            .ok_or(R::Correlation)?;
        let gear = demand.class().gear();
        if gear.checked_front() != pool.member_front {
            return Err(R::FrontChanged);
        }
        if demand.inputs().len() != 1
            || gear.inputs.len() != 1
            || demand.inputs()[0].port != &gear.inputs[0].port_id
        {
            return Err(R::InputShape);
        }
        if usize::try_from(operation.admitted_bytes).ok() != Some(demand.input_bytes()) {
            return Err(R::ExtentChanged);
        }
        let payload = demand.inputs()[0].value.clone();
        Ok(Self {
            operation,
            demand,
            payload,
        })
    }

    pub fn operation(&self) -> &QueuedPoolOperation {
        &self.operation
    }
    pub fn demand(&self) -> CostDemand<'a> {
        self.demand
    }
    pub fn payload(&self) -> &ValuePayload {
        &self.payload
    }

    /// Bind forecasts to this exact queued request before handing cost facts to
    /// the common pool selector. An opaque correlation key cannot substitute
    /// different checked meaning, inputs, codecs or workload metrics.
    pub fn prepare_forecast(
        &self,
        input: PoolForecastPreparation<'a, '_>,
        work: &mut conduit_core::ExactNumericWork,
    ) -> Result<PreparedPoolForecast<'a>, PoolForecastPreparationRefusal> {
        if input.plan.plan_id != self.operation.plan
            || input.pool != &self.operation.pool
            || input.operation != &self.operation.operation
            || input.demand_identity != self.operation.demand
        {
            return Err(
                conduit_planner::resource_cost::CostEvidenceRefusal::WorkClassChanged.into(),
            );
        }
        self.demand.validate_match(input.request)?;
        prepare_pool_cost_forecast(input, work)
    }
}

impl core::fmt::Debug for NativePoolRequestCustody<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NativePoolRequestCustody")
            .field("plan", &self.operation.plan)
            .field("pool", &self.operation.pool)
            .field("operation", &self.operation.operation)
            .field("input_bytes", &self.payload.encoded.len())
            .finish()
    }
}
