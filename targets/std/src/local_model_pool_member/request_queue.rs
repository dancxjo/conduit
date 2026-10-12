//! Fixed native payload custody around the existing preparation-side pool.
//! Policy, cost ordering, freshness and atomic occupation remain delegated.
use super::request_custody::NativePoolRequestCustody;
use conduit_core::{
    ExactNumericWork, MonotonicInstant, PoolOperationId, PoolRealizationObservation,
    PreparedPoolCost, SignId,
};
use conduit_kernel::shared_pool::MemberIdentity;
use conduit_plan_lowering::shared_pool_runtime::{
    HostedSharedPool, PoolCostEvaluation, PoolWaitingError, PoolWaitingLimits, PoolWaitingPolicy,
    QueuedCostAdmission, QueuedPoolOperation, SharedPoolRuntimeError,
    MAXIMUM_POOL_WAITING_OPERATIONS,
};
use conduit_planner::resource_cost::{
    CostDemand, PoolForecastPreparation, PoolForecastPreparationAccount,
    PoolForecastPreparationRefusal,
};

const MAXIMUM_NATIVE_REQUEST_COSTS: usize = 32;

/// Costs can enter this wrapper only after binding to the retained request.
pub struct NativePoolPreparedCosts<'a> {
    operation: QueuedPoolOperation,
    demand: CostDemand<'a>,
    costs: Vec<PreparedPoolCost<'a>>,
    accounts: Vec<PoolForecastPreparationAccount>,
}
impl<'a> NativePoolPreparedCosts<'a> {
    pub fn prepare(
        request: &NativePoolRequestCustody<'a>,
        inputs: Vec<PoolForecastPreparation<'a, '_>>,
        work: &mut ExactNumericWork,
    ) -> Result<Self, PoolForecastPreparationRefusal> {
        if inputs.len() > MAXIMUM_NATIVE_REQUEST_COSTS {
            return Err(conduit_planner::resource_cost::CostEvidenceRefusal::InputLimit.into());
        }
        let mut costs = Vec::with_capacity(inputs.len());
        let mut accounts = Vec::with_capacity(inputs.len());
        for input in inputs {
            let prepared = request.prepare_forecast(input, work)?;
            costs.push(prepared.cost);
            accounts.push(prepared.account);
        }
        Ok(Self {
            operation: request.operation().clone(),
            demand: request.demand(),
            costs,
            accounts,
        })
    }
    pub fn accounts(&self) -> &[PoolForecastPreparationAccount] {
        &self.accounts
    }
}

#[derive(Debug)]
pub enum NativePoolQueueError {
    Full,
    Missing,
    Correlation,
    InputLimit,
    Demand(conduit_planner::resource_cost::CostEvidenceRefusal),
    Waiting(PoolWaitingError),
}
impl From<PoolWaitingError> for NativePoolQueueError {
    fn from(value: PoolWaitingError) -> Self {
        Self::Waiting(value)
    }
}

pub struct NativePoolDispatch<'a> {
    pub admission: QueuedCostAdmission,
    /// None on retained capacity refusal. Selected/permanent-refused requests
    /// return their exact owned bytes; callers must inspect admission first.
    pub request: Option<NativePoolRequestCustody<'a>>,
}

pub struct NativePoolRequestQueue<'a, const M: usize, const S: usize, const O: usize> {
    pool: HostedSharedPool<M, S, O>,
    entries: [Option<NativePoolRequestCustody<'a>>; MAXIMUM_POOL_WAITING_OPERATIONS],
}
impl<'a, const M: usize, const S: usize, const O: usize> NativePoolRequestQueue<'a, M, S, O> {
    pub fn prepare(
        mut pool: HostedSharedPool<M, S, O>,
        limits: PoolWaitingLimits,
        policy: PoolWaitingPolicy,
        now: MonotonicInstant,
    ) -> Result<Self, NativePoolQueueError> {
        pool.enable_waiting_with_policy(limits, policy, now)?;
        Ok(Self {
            pool,
            entries: core::array::from_fn(|_| None),
        })
    }
    pub fn waiting_population(&self) -> usize {
        self.pool.waiting_population()
    }
    pub const fn descriptor_storage_bytes() -> usize {
        core::mem::size_of::<Self>()
    }
    pub fn enqueue(
        &mut self,
        request: NativePoolRequestCustody<'a>,
        now: &MonotonicInstant,
    ) -> Result<(), NativePoolQueueError> {
        let metadata = request.operation();
        // Bound copied identity storage before cloning into the common queue.
        if [metadata.operation.as_str(), metadata.requester.as_str()]
            .iter()
            .any(|id| id.is_empty() || id.len() > 256)
        {
            return Err(NativePoolQueueError::InputLimit);
        }
        let slot = self
            .entries
            .iter()
            .position(Option::is_none)
            .ok_or(NativePoolQueueError::Full)?;
        self.pool.queue_operation(metadata.clone(), now)?;
        self.entries[slot] = Some(request);
        Ok(())
    }
    pub fn next_request(
        &self,
        now: &MonotonicInstant,
    ) -> Result<Option<&NativePoolRequestCustody<'a>>, NativePoolQueueError> {
        let Some(operation) = self.pool.next_waiting_operation(now)? else {
            return Ok(None);
        };
        self.entries
            .iter()
            .flatten()
            .find(|request| request.operation() == operation)
            .map(Some)
            .ok_or(NativePoolQueueError::Missing)
    }
    fn take_retired(&mut self, operation: &QueuedPoolOperation) -> NativePoolRequestCustody<'a> {
        let index = self
            .entries
            .iter()
            .position(|entry| {
                entry
                    .as_ref()
                    .is_some_and(|request| request.operation() == operation)
            })
            .expect("private common queue and native custody remain correlated");
        self.entries[index]
            .take()
            .expect("checked native custody slot")
    }
    pub fn cancel(
        &mut self,
        operation: &PoolOperationId,
    ) -> Result<NativePoolRequestCustody<'a>, NativePoolQueueError> {
        if !self
            .entries
            .iter()
            .flatten()
            .any(|request| &request.operation().operation == operation)
        {
            return Err(NativePoolQueueError::Missing);
        }
        let retired = self.pool.cancel_waiting_operation(operation)?;
        Ok(self.take_retired(&retired))
    }
    pub fn retire_expired(
        &mut self,
        now: &MonotonicInstant,
    ) -> Result<Option<NativePoolRequestCustody<'a>>, NativePoolQueueError> {
        let retired = self.pool.retire_expired_waiting(now)?;
        Ok(retired
            .as_ref()
            .map(|operation| self.take_retired(operation)))
    }
    pub fn dispatch(
        &mut self,
        now: &MonotonicInstant,
        current: &[PoolRealizationObservation],
        sign: SignId,
        costs: &NativePoolPreparedCosts<'a>,
        evaluations: &[PoolCostEvaluation<'_>],
    ) -> Result<Option<NativePoolDispatch<'a>>, NativePoolQueueError> {
        if current.len() > MAXIMUM_NATIVE_REQUEST_COSTS
            || evaluations.len() > MAXIMUM_NATIVE_REQUEST_COSTS
        {
            return Err(NativePoolQueueError::InputLimit);
        }
        let Some(request) = self.next_request(now)? else {
            return Ok(None);
        };
        if request.operation() != &costs.operation {
            return Err(NativePoolQueueError::Correlation);
        }
        request
            .demand()
            .validate_match(costs.demand)
            .map_err(NativePoolQueueError::Demand)?;
        let admission = self
            .pool
            .admit_next_waiting_with_costs(now, current, sign, &costs.costs, evaluations)?
            .expect("checked nonempty private common queue");
        let request = admission
            .retired
            .as_ref()
            .map(|operation| self.take_retired(operation));
        Ok(Some(NativePoolDispatch { admission, request }))
    }
    pub fn trigger(&mut self, member: MemberIdentity) -> Result<(), SharedPoolRuntimeError> {
        self.pool.trigger(member)
    }
    pub fn fail_preparation(
        &mut self,
        member: MemberIdentity,
    ) -> Result<(), SharedPoolRuntimeError> {
        self.pool.fail_preparation(member)
    }
    pub fn release(&mut self, member: MemberIdentity) -> Result<(), SharedPoolRuntimeError> {
        self.pool.release(member)
    }
}
