//! Exact interval arithmetic for a declared fulfillment model outside Play.
//! Bounds here are model estimates, not certified deadlines or WCET. Source,
//! work-class matching and hard admission must be checked by the consumer.
use super::{CostDuration, CostEvidenceRefusal as Refusal};
use conduit_core::{ExactNumericWork, ExactRational};
use core::cmp::Ordering;

/// Nonnegative duration estimates expressed in the common Time coordinate.
/// Admission keeps the original physical quantities in the stage inputs.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CostDurationRange {
    lower: CostDuration,
    upper: CostDuration,
}
impl CostDurationRange {
    pub fn admit(
        lower: CostDuration,
        upper: CostDuration,
        work: &mut ExactNumericWork,
    ) -> Result<Self, Refusal> {
        if lower.compare(upper, work)? == Ordering::Greater {
            return Err(Refusal::InvertedDurationRange);
        }
        Ok(Self { lower, upper })
    }
    pub const fn lower(self) -> CostDuration {
        self.lower
    }
    pub const fn upper(self) -> CostDuration {
        self.upper
    }
}

/// Each stage must be supplied, including explicit evidence for a zero stage.
/// `service` is the estimate for the actual requested work, not a raw rate whose
/// units could mean tokens, pages or bytes depending on the implementation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CostForecastStages {
    pub setup: Option<CostDurationRange>,
    pub queue: Option<CostDurationRange>,
    pub service: Option<CostDurationRange>,
    pub transfer: Option<CostDurationRange>,
}

/// Sum of four sequential stages. Overlapping stages require a different
/// explicitly admitted model; this API does not silently subtract overlap.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FulfillmentCostForecast {
    stages: CostForecastStages,
    lower_seconds: ExactRational,
    upper_seconds: ExactRational,
}
impl FulfillmentCostForecast {
    pub fn sequential(
        stages: CostForecastStages,
        work: &mut ExactNumericWork,
    ) -> Result<Self, Refusal> {
        let ranges = [stages.setup, stages.queue, stages.service, stages.transfer];
        // Refuse unknown inputs before spending arithmetic work.
        if ranges.iter().any(Option::is_none) {
            return Err(Refusal::MissingForecastStage);
        }
        let mut lower = ExactRational::ZERO;
        let mut upper = ExactRational::ZERO;
        for range in ranges.into_iter().flatten() {
            lower = lower.checked_add(range.lower.seconds(), work)?;
            upper = upper.checked_add(range.upper.seconds(), work)?;
        }
        Ok(Self {
            stages,
            lower_seconds: lower,
            upper_seconds: upper,
        })
    }
    pub const fn stages(self) -> CostForecastStages {
        self.stages
    }
    pub const fn lower_seconds(self) -> ExactRational {
        self.lower_seconds
    }
    pub const fn upper_seconds(self) -> ExactRational {
        self.upper_seconds
    }
    /// A strict separation of model intervals. Overlapping intervals yield no
    /// preference; callers may apply their explicit policy and deterministic tie.
    /// This is not a claim that either model certifies the eventual execution.
    pub fn strictly_precedes(
        self,
        other: Self,
        work: &mut ExactNumericWork,
    ) -> Result<bool, Refusal> {
        Ok(self
            .upper_seconds
            .checked_compare(other.lower_seconds, work)?
            == Ordering::Less)
    }
}
