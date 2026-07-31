//! Hosted entry point for the portable fixed-storage pool controller.

use std::fmt;

use conduit_core::{
    EXECUTION_PLAN_SCHEMA_VERSION, ExecutionPlan, ImplementationError, ImplementationMachine,
    PlanInstancePool, PoolController, PoolError, PoolFailureDisposition, PoolGeneration,
    SemanticHash, StepObservation, StepOutcome, StepOutcomeKind, StepUsage,
};

/// Hosted runtime with compile-time fixed storage. Host profiles choose these
/// bounds explicitly; plans larger than the selected profile fail before any
/// instance is admitted.
pub type HostedPoolRuntime<'a, const SLOTS: usize, const EVIDENCE: usize> =
    PoolController<'a, SLOTS, EVIDENCE>;

/// Atomic result of one host-neutral implementation step observed through a
/// pool slot. Implementation evidence and pool supervision remain distinct.
#[derive(Clone, Copy, Debug)]
pub struct HostedPoolStepObservation<'a> {
    pub implementation: StepObservation,
    pub pool_failure: Option<PoolFailureDisposition<'a>>,
}

/// Validate a concrete #56 implementation step and then commit the matching
/// pool lifecycle observation. The implementation machine is copied first, so
/// evidence exhaustion or an illegal pool transition cannot partially advance
/// its lifecycle.
pub fn observe_pool_step<'pool, const SLOTS: usize, const EVIDENCE: usize>(
    runtime: &mut HostedPoolRuntime<'pool, SLOTS, EVIDENCE>,
    slot: u16,
    machine: &mut ImplementationMachine,
    outcome: StepOutcome<'_>,
    usage: StepUsage,
    failure_cause: SemanticHash,
    now_tick: u64,
) -> Result<HostedPoolStepObservation<'pool>, HostedPoolStepError> {
    let mut candidate = *machine;
    let implementation = match candidate.observe_step(outcome, usage) {
        Ok(observation) => observation,
        Err(error) => {
            runtime
                .contain_foreign(slot, Some(failure_cause), now_tick)
                .map_err(HostedPoolStepError::Pool)?;
            return Err(HostedPoolStepError::Implementation(error));
        }
    };
    let pool_failure = match implementation.outcome() {
        StepOutcomeKind::Progress => {
            runtime
                .progress(slot, now_tick)
                .map_err(HostedPoolStepError::Pool)?;
            None
        }
        StepOutcomeKind::Completed => {
            runtime
                .complete(slot, now_tick)
                .map_err(HostedPoolStepError::Pool)?;
            None
        }
        StepOutcomeKind::Failed => Some(
            runtime
                .fail(slot, failure_cause, now_tick)
                .map_err(HostedPoolStepError::Pool)?,
        ),
        StepOutcomeKind::Pending | StepOutcomeKind::Yielded => None,
    };
    *machine = candidate;
    Ok(HostedPoolStepObservation {
        implementation,
        pool_failure,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedPoolStepError {
    Implementation(ImplementationError),
    Pool(PoolError),
}

impl HostedPoolStepError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Implementation(error) => error.code(),
            Self::Pool(error) => error.code(),
        }
    }
}

impl fmt::Display for HostedPoolStepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Implementation(error) => error.fmt(formatter),
            Self::Pool(error) => error.fmt(formatter),
        }
    }
}

/// Instantiate one exact plan pool without reinterpreting source policy.
pub fn instantiate_pool<'a, const SLOTS: usize, const EVIDENCE: usize>(
    plan: &'a ExecutionPlan<'a>,
    pool_index: usize,
    epoch: u64,
    generation: u32,
) -> Result<HostedPoolRuntime<'a, SLOTS, EVIDENCE>, HostedPoolError> {
    let pool = plan
        .instance_pools
        .get(pool_index)
        .copied()
        .ok_or(HostedPoolError::UnknownPool)?;
    instantiate_plan_pool(plan.schema_version, plan.identity, pool, epoch, generation)
}

/// Instantiate an already selected plan pool.
pub fn instantiate_plan_pool<'a, const SLOTS: usize, const EVIDENCE: usize>(
    schema_version: u32,
    plan_identity: SemanticHash,
    pool: PlanInstancePool<'a>,
    epoch: u64,
    generation: u32,
) -> Result<HostedPoolRuntime<'a, SLOTS, EVIDENCE>, HostedPoolError> {
    if schema_version != EXECUTION_PLAN_SCHEMA_VERSION {
        return Err(HostedPoolError::UnsupportedPlan);
    }
    let runtime = pool
        .runtime
        .ok_or(HostedPoolError::MissingRuntimeContract)?;
    runtime
        .generation_reservation
        .validate()
        .map_err(HostedPoolError::Contract)?;
    PoolController::new(
        runtime.contract,
        PoolGeneration {
            plan: plan_identity,
            epoch,
            generation,
            template_hash: pool.template_hash,
        },
    )
    .map_err(HostedPoolError::Contract)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedPoolError {
    UnknownPool,
    UnsupportedPlan,
    MissingRuntimeContract,
    Contract(PoolError),
}

impl HostedPoolError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnknownPool => "CND-POOL-HOST-001",
            Self::UnsupportedPlan | Self::MissingRuntimeContract => "CND-POOL-HOST-002",
            Self::Contract(error) => error.code(),
        }
    }
}

impl fmt::Display for HostedPoolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPool => formatter.write_str("execution plan has no such pool"),
            Self::UnsupportedPlan => formatter.write_str("execution plan schema is unsupported"),
            Self::MissingRuntimeContract => {
                formatter.write_str("pool has no executable runtime contract")
            }
            Self::Contract(error) => error.fmt(formatter),
        }
    }
}
