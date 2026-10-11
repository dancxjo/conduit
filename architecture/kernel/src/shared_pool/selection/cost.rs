//! Allocation-free comparison of already-prepared relative Time coordinates.
//! Rich evidence, typed projection and per-operation demand admission belong
//! above the kernel. No absolute ticks from different Hosts are compared here.
use super::{LoweredPoolRealization, PoolSelectionError};

pub const MAXIMUM_POOL_COST_REALIZATIONS: usize = 32;
pub const MAXIMUM_POOL_COST_FACTS: usize = 256;

/// Common nanosecond coordinate lowered from a checked nonnegative Time bound.
/// Zero is a measured/projected value; absence is represented separately.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoweredPoolDuration(u64);
impl LoweredPoolDuration {
    pub const fn from_nanoseconds(value: u64) -> Self {
        Self(value)
    }
    pub const fn nanoseconds(self) -> u64 {
        self.0
    }
}

/// Exact source identity and its Host-local evaluation window. The lowering
/// owner supplies clock tokens after validating their quality and scale, and
/// retains the richer source/condition/projection custody outside Play.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LoweredPoolCost {
    pub realization: LoweredPoolRealization,
    pub demand: [u8; 32],
    pub upper_completion: LoweredPoolDuration,
    pub source_clock: u16,
    pub evaluation_clock: u16,
    pub observed_ticks: u64,
    pub expires_ticks: u64,
    pub evaluation_ticks: u64,
    pub sign: u16,
}
#[derive(Debug, Copy, Clone)]
pub struct PoolCostSelection<'a> {
    pub demand: [u8; 32],
    pub costs: &'a [LoweredPoolCost],
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolCostAvailability {
    Missing,
    ScopeChanged,
    DemandChanged,
    ClockChanged,
    InvalidWindow,
    Future,
    Stale,
    Known {
        upper_completion: LoweredPoolDuration,
        sign: u16,
    },
}
impl PoolCostAvailability {
    pub(super) fn known(self) -> Option<LoweredPoolDuration> {
        match self {
            Self::Known {
                upper_completion, ..
            } => Some(upper_completion),
            _ => None,
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolCostAdmission {
    NotCurrent,
    Full,
    Eligible,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PoolCostCandidateDecision {
    pub realization: u16,
    pub admission: PoolCostAdmission,
    pub cost: PoolCostAvailability,
}
impl PoolCostCandidateDecision {
    pub const EMPTY: Self = Self {
        realization: u16::MAX,
        admission: PoolCostAdmission::NotCurrent,
        cost: PoolCostAvailability::Missing,
    };
}
impl PoolCostSelection<'_> {
    pub(super) fn validate(
        self,
        envelope: &[LoweredPoolRealization],
        requirements: usize,
        observations: usize,
    ) -> Result<(), PoolSelectionError> {
        if envelope.len() > MAXIMUM_POOL_COST_REALIZATIONS
            || self.costs.len() > MAXIMUM_POOL_COST_REALIZATIONS
            || requirements > MAXIMUM_POOL_COST_FACTS
            || observations > MAXIMUM_POOL_COST_FACTS
        {
            return Err(PoolSelectionError::CostInputLimit);
        }
        if self.costs.iter().enumerate().any(|(index, cost)| {
            self.costs[..index]
                .iter()
                .any(|prior| prior.realization.realization == cost.realization.realization)
        }) {
            return Err(PoolSelectionError::DuplicateCost);
        }
        Ok(())
    }
    pub(super) fn availability(self, candidate: &LoweredPoolRealization) -> PoolCostAvailability {
        use PoolCostAvailability as A;
        let Some(cost) = self
            .costs
            .iter()
            .find(|cost| cost.realization.realization == candidate.realization)
        else {
            return A::Missing;
        };
        if cost.realization != *candidate {
            return A::ScopeChanged;
        }
        if cost.demand != self.demand {
            return A::DemandChanged;
        }
        if cost.source_clock != cost.evaluation_clock {
            return A::ClockChanged;
        }
        if cost.expires_ticks <= cost.observed_ticks {
            return A::InvalidWindow;
        }
        if cost.evaluation_ticks < cost.observed_ticks {
            return A::Future;
        }
        if cost.evaluation_ticks >= cost.expires_ticks {
            return A::Stale;
        }
        A::Known {
            upper_completion: cost.upper_completion,
            sign: cost.sign,
        }
    }
    pub(super) fn better(
        self,
        candidate: &LoweredPoolRealization,
        current: &LoweredPoolRealization,
    ) -> Option<bool> {
        match (
            self.availability(candidate).known(),
            self.availability(current).known(),
        ) {
            (Some(left), Some(right)) if left != right => Some(left < right),
            (Some(_), None) => Some(true),
            (None, Some(_)) => Some(false),
            _ => None,
        }
    }
}
