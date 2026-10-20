//! Typed, bounded preparation of resource-cost evidence outside Play.
//! These primitives do not themselves select a realization or certify a forecast.
mod attribution;
mod calibration;
mod demand;
mod duration;
mod execution_observation;
mod forecast;
mod freshness;
mod global_planning;
mod input_codec;
mod measurement;
mod planning;
mod pool_preparation;
mod replacement;
mod replan_policy;
mod residence;
mod selection;
mod selection_account;
mod service_model;
mod source;
mod structured_input;
mod transfer;
mod work_class;
pub use attribution::CostSourceAttribution;
pub use calibration::*;
pub use demand::{
    CostDemand, CostInput, CostedForecast, MAXIMUM_COST_INPUTS, MAXIMUM_COST_INPUT_BYTES,
};
pub use duration::CostDuration;
pub use execution_observation::CostExecutionObservation;
pub use forecast::{CostDurationRange, CostForecastStages, FulfillmentCostForecast};
pub use freshness::CostObservationWindow;
pub use global_planning::*;
pub use input_codec::CostInputCodec;
pub use measurement::CostMeasuredDuration;
pub use planning::*;
pub use pool_preparation::*;
pub use replacement::*;
pub use replan_policy::*;
pub use residence::*;
pub use selection::*;
pub use selection_account::*;
pub use service_model::*;
pub use source::{
    CostCalibrationRevision, CostEvidenceSource, CostScopeChange, MAXIMUM_COST_ID_BYTES,
};
pub use structured_input::CostStructuredInputSchema;
pub use transfer::*;
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
    InvalidWorkInput,
    InputCodecChanged,
    Structured(conduit_core::StructuredInfoRefusal),
    WorkExtentChanged,
    WorkMetricChanged,
    MissingWorkEstimate,
    WorkLimit,
    InputLimit,
    AmbiguousEvidence,
    UnsupportedIntent,
    InvalidCalibrationPolicy,
    CalibrationUpdateLimit,
    CalibrationUpdateTooSoon,
    DuplicateCompletion,
    ExecutionConditionChanged,
    MissingExecutionObservation,
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
