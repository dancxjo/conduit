//! Typed, bounded preparation of resource-cost evidence outside Play.
//! These primitives do not themselves select a realization or certify a forecast.
mod duration;
mod forecast;
mod freshness;
mod source;
mod work_class;
pub use duration::CostDuration;
pub use forecast::{CostDurationRange, CostForecastStages, FulfillmentCostForecast};
pub use freshness::CostObservationWindow;
pub use source::{
    CostCalibrationRevision, CostEvidenceSource, CostScopeChange, MAXIMUM_COST_ID_BYTES,
};
pub use work_class::CostWorkClass;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostEvidenceRefusal {
    Numeric(conduit_core::ExactNumericRefusal),
    Physical(conduit_core::RationalQuantityRefusal),
    NegativeDuration,
    InvertedDurationRange,
    MissingForecastStage,
    WorkClassChanged,
    IncompatibleRealization,
    InvalidIdentity,
    UnadvertisedSubject,
    Scope(CostScopeChange),
    InvalidClock(conduit_core::MonotonicTimeRefusal),
    ClockQualityChanged,
    InvalidWindow,
    FutureObservation,
    Stale,
}
impl From<conduit_core::ExactNumericRefusal> for CostEvidenceRefusal {
    fn from(value: conduit_core::ExactNumericRefusal) -> Self {
        Self::Numeric(value)
    }
}
impl From<conduit_core::RationalQuantityRefusal> for CostEvidenceRefusal {
    fn from(value: conduit_core::RationalQuantityRefusal) -> Self {
        match value {
            conduit_core::RationalQuantityRefusal::Numeric(error) => Self::Numeric(error),
            other => Self::Physical(other),
        }
    }
}
