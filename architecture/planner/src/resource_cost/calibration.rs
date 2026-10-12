//! Bounded completed-work calibration outside Play. Clock uncertainty remains
//! an interval; estimates and measured clock coordinates are distinct facts.
use super::{
    CostDuration, CostDurationRange, CostEvidenceRefusal as Refusal, CostEvidenceSource,
    CostMeasuredDuration, CostedForecast,
};
use alloc::vec::Vec;
use conduit_core::{
    ExactNumericWork, ExactRational, MonotonicDuration, MonotonicInstant, QuantityRole,
    RationalQuantity, SignId, Unit,
};

pub const MAXIMUM_COST_CALIBRATION_HISTORY: usize = 16;
pub const MAXIMUM_COST_CALIBRATION_UPDATES: usize = 64;
pub use conduit_core::CostExecutionCondition;
pub struct CostCalibrationRecord<'a> {
    forecast: &'a CostedForecast<'a>,
    completion: &'a CostEvidenceSource<'a>,
    measurement: CostMeasuredDuration,
    error_lower: RationalQuantity,
    error_upper: RationalQuantity,
    condition: CostExecutionCondition,
}
impl<'a> CostCalibrationRecord<'a> {
    pub fn admit(
        forecast: &'a CostedForecast<'a>,
        completion: &'a CostEvidenceSource<'a>,
        dispatched: &MonotonicInstant,
        condition: CostExecutionCondition,
        work: &mut ExactNumericWork,
    ) -> Result<Self, Refusal> {
        if matches!(
            condition,
            CostExecutionCondition::Warm {
                residency_revision: 0
            }
        ) {
            return Err(Refusal::InvalidIdentity);
        }
        if completion.sign() == forecast.source().sign() {
            return Err(Refusal::DuplicateCompletion);
        }
        matching_realization(forecast.source(), completion)?;
        forecast.validate_at(completion.host(), completion.calibration(), dispatched)?;
        let completed = completion.window().observed();
        let measurement = CostMeasuredDuration::measure(dispatched, completed, work)?;
        let measured = measurement.range();
        let lower = measured.lower();
        let upper = measured.upper();
        let error_lower = time_quantity(
            lower
                .seconds()
                .checked_subtract(forecast.upper_seconds(), work)?,
        )?;
        let error_upper = time_quantity(
            upper
                .seconds()
                .checked_subtract(forecast.lower_seconds(), work)?,
        )?;
        Ok(Self {
            forecast,
            completion,
            measurement,
            error_lower,
            error_upper,
            condition,
        })
    }
    pub fn forecast(&self) -> &'a CostedForecast<'a> {
        self.forecast
    }
    pub fn completion(&self) -> &'a CostEvidenceSource<'a> {
        self.completion
    }
    pub const fn elapsed(&self) -> MonotonicDuration {
        self.measurement.elapsed()
    }
    pub const fn measured(&self) -> CostDurationRange {
        self.measurement.range()
    }
    pub fn measurement(&self) -> &CostMeasuredDuration {
        &self.measurement
    }
    /// Signed Time residual interval: measured minus predicted. A negative
    /// error is retained, not clamped to a nonnegative cost or rounded away.
    pub const fn error_bounds(&self) -> (RationalQuantity, RationalQuantity) {
        (self.error_lower, self.error_upper)
    }
    pub const fn condition(&self) -> CostExecutionCondition {
        self.condition
    }
}
fn time_quantity(value: ExactRational) -> Result<RationalQuantity, Refusal> {
    Ok(RationalQuantity::new(
        value,
        Unit::resolve("s").expect("reviewed Time Unit"),
        QuantityRole::Linear,
    )?)
}
fn seconds(value: ExactRational, work: &mut ExactNumericWork) -> Result<CostDuration, Refusal> {
    CostDuration::admit(time_quantity(value)?, work)
}
fn matching_realization(
    left: &CostEvidenceSource<'_>,
    right: &CostEvidenceSource<'_>,
) -> Result<(), Refusal> {
    if left.offer() != right.offer() {
        return Err(Refusal::Scope(super::CostScopeChange::Offer));
    }
    if left.resource() != right.resource() {
        return Err(Refusal::Scope(super::CostScopeChange::Resource));
    }
    Ok(())
}

#[derive(Debug, Copy, Clone)]
pub struct CostCalibrationPolicy {
    pub history_capacity: usize,
    pub maximum_updates: usize,
    pub minimum_update_ticks: u64,
}
/// Frozen model output retains the latest measurement's custody and demand.
/// Its interval estimates the mean; it is not a future-outcome confidence bound.
pub struct CostCalibrationEstimate<'a> {
    record: &'a CostCalibrationRecord<'a>,
    duration: CostDurationRange,
    updates: usize,
}
impl<'a> CostCalibrationEstimate<'a> {
    /// Retained measurement and signed residuals for historical explanation.
    /// This does not change the estimate or admit a new observation.
    pub fn completion_record(&self) -> &'a CostCalibrationRecord<'a> {
        self.record
    }

    pub fn source(&self) -> &'a CostEvidenceSource<'a> {
        self.record.completion
    }
    pub const fn duration(&self) -> CostDurationRange {
        self.duration
    }
    pub const fn updates(&self) -> usize {
        self.updates
    }
    pub const fn condition(&self) -> CostExecutionCondition {
        self.record.condition
    }
    pub fn validate_use(
        &self,
        source: &CostEvidenceSource<'_>,
        demand: super::CostDemand<'_>,
        condition: CostExecutionCondition,
        now: &MonotonicInstant,
    ) -> Result<(), Refusal> {
        self.record.forecast.demand().validate_match(demand)?;
        matching_realization(self.record.completion, source)?;
        if self.record.condition != condition {
            return Err(Refusal::ExecutionConditionChanged);
        }
        self.record
            .completion
            .validate_at(source.host(), source.calibration(), now)
    }
}
impl core::fmt::Debug for CostCalibrationEstimate<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CostCalibrationEstimate")
            .field("sign", &self.source().sign())
            .field("updates", &self.updates)
            .field("condition", &self.condition())
            .finish()
    }
}
/// Exact EWMA interval (weight 1/4) and bounded provenance history. This never
/// runs in FixedScheduler::step, changes eligibility, or certifies a deadline.
pub struct BoundedCostCalibration<'a> {
    policy: CostCalibrationPolicy,
    history: Vec<&'a CostCalibrationRecord<'a>>,
    cursor: usize,
    latest: Option<&'a CostCalibrationRecord<'a>>,
    seen: [Option<&'a SignId>; MAXIMUM_COST_CALIBRATION_UPDATES],
    updates: usize,
    estimate: Option<CostDurationRange>,
}
impl<'a> BoundedCostCalibration<'a> {
    pub fn snapshot_at(
        &self,
        host: &conduit_core::HostAdvertisement,
        revision: super::CostCalibrationRevision<'_>,
        now: &MonotonicInstant,
    ) -> Result<Option<CostCalibrationEstimate<'a>>, Refusal> {
        Ok(self
            .estimate_at(host, revision, now)?
            .map(|duration| CostCalibrationEstimate {
                record: self.latest.expect("estimate has a completed source"),
                duration,
                updates: self.updates,
            }))
    }
    pub fn admit(policy: CostCalibrationPolicy) -> Result<Self, Refusal> {
        if policy.history_capacity == 0
            || policy.history_capacity > MAXIMUM_COST_CALIBRATION_HISTORY
            || policy.maximum_updates == 0
            || policy.maximum_updates > MAXIMUM_COST_CALIBRATION_UPDATES
            || policy.minimum_update_ticks == 0
        {
            return Err(Refusal::InvalidCalibrationPolicy);
        }
        Ok(Self {
            policy,
            history: Vec::with_capacity(policy.history_capacity),
            cursor: 0,
            latest: None,
            seen: [None; MAXIMUM_COST_CALIBRATION_UPDATES],
            updates: 0,
            estimate: None,
        })
    }
    pub fn updates(&self) -> usize {
        self.updates
    }
    /// Oldest to newest among the retained bounded records, without allocation.
    pub fn history(&self) -> impl Iterator<Item = &'a CostCalibrationRecord<'a>> + '_ {
        let start = if self.history.len() == self.policy.history_capacity {
            self.cursor
        } else {
            0
        };
        (0..self.history.len())
            .map(move |offset| self.history[(start + offset) % self.history.len()])
    }
    pub fn update(
        &mut self,
        record: &'a CostCalibrationRecord<'a>,
        work: &mut ExactNumericWork,
    ) -> Result<(), Refusal> {
        if self.updates == self.policy.maximum_updates {
            return Err(Refusal::CalibrationUpdateLimit);
        }
        if self.seen[..self.updates].contains(&Some(record.completion.sign())) {
            return Err(Refusal::DuplicateCompletion);
        }
        if let Some(previous) = self.latest {
            matching_realization(previous.completion, record.completion)?;
            previous
                .forecast
                .demand()
                .validate_match(record.forecast.demand())?;
            if previous.condition != record.condition {
                return Err(Refusal::ExecutionConditionChanged);
            }
            // Compare historical scope at its admitted observation instant.
            // Historical records are calibration data, not fresh hard facts.
            previous.completion.validate_at(
                record.completion.host(),
                record.completion.calibration(),
                previous.completion.window().observed(),
            )?;
            let now = record.completion.window().observed();
            let earlier = previous.completion.window().observed();
            let delta = now.elapsed_since(earlier).map_err(Refusal::InvalidClock)?;
            if now.clock() != earlier.clock() {
                return Err(Refusal::ClockQualityChanged);
            }
            if delta.ticks() < self.policy.minimum_update_ticks {
                return Err(Refusal::CalibrationUpdateTooSoon);
            }
        }
        let estimate = if let Some(previous) = self.estimate {
            let blend = |old: CostDuration,
                         new: CostDuration,
                         work: &mut ExactNumericWork|
             -> Result<CostDuration, Refusal> {
                let delta = new.seconds().checked_subtract(old.seconds(), work)?;
                let correction = delta.checked_divide(ExactRational::from_i128(4), work)?;
                seconds(old.seconds().checked_add(correction, work)?, work)
            };
            let lower = blend(previous.lower(), record.measured().lower(), work)?;
            let upper = blend(previous.upper(), record.measured().upper(), work)?;
            CostDurationRange::admit(lower, upper, work)?
        } else {
            record.measured()
        };
        // Every fallible calculation precedes mutation. Budget/scope refusal
        // leaves counters, history, deduplication and the estimate untouched.
        if self.history.len() < self.policy.history_capacity {
            self.history.push(record);
        } else {
            self.history[self.cursor] = record;
        }
        self.cursor = (self.cursor + 1) % self.policy.history_capacity;
        self.seen[self.updates] = Some(record.completion.sign());
        self.updates += 1;
        self.latest = Some(record);
        self.estimate = Some(estimate);
        Ok(())
    }
    /// The latest completed producer observation fences current use of the
    /// derived estimate. This output is a total-duration estimate, not a rate
    /// and not a replacement for the explicit forecast stage accounting.
    pub fn estimate_at(
        &self,
        host: &conduit_core::HostAdvertisement,
        revision: super::CostCalibrationRevision<'_>,
        now: &MonotonicInstant,
    ) -> Result<Option<CostDurationRange>, Refusal> {
        if let Some(latest) = self.latest {
            latest.completion.validate_at(host, revision, now)?;
        }
        Ok(self.estimate)
    }
}
