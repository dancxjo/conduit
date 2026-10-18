//! Identified elapsed observations projected to typed Time during preparation.
//! Endpoint uncertainty is measurement evidence, not a future cost guarantee.
use super::{CostDuration, CostDurationRange, CostEvidenceRefusal as Refusal};
use conduit_core::{
    ExactNumericWork, ExactRational, MonotonicDuration, MonotonicInstant, MonotonicTimeRefusal,
    QuantityRole, RationalQuantity, TemporalScale, Unit,
};

/// Retains the actual clock coordinates alongside their nonnegative Time range.
/// Callers must separately admit the producer, measurement scope, request and
/// realization. Matching clock names alone do not authenticate observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostMeasuredDuration {
    started: MonotonicInstant,
    completed: MonotonicInstant,
    elapsed: MonotonicDuration,
    range: CostDurationRange,
}
impl CostMeasuredDuration {
    /// Both endpoints must use the identical identified clock and quality.
    /// This does not correlate foreign clocks or add overlapping intervals.
    /// Numeric work and owned endpoint storage are prepared outside Play.
    pub fn measure(
        started: &MonotonicInstant,
        completed: &MonotonicInstant,
        work: &mut ExactNumericWork,
    ) -> Result<Self, Refusal> {
        let elapsed = completed
            .elapsed_since(started)
            .map_err(Refusal::InvalidClock)?;
        if completed.clock() != started.clock() {
            return Err(Refusal::ClockQualityChanged);
        }
        // Each endpoint contributes resolution and uncertainty. Zero elapsed
        // ticks therefore preserve a positive upper bound, never infinite rate.
        let error = completed
            .clock()
            .resolution_ticks()
            .checked_add(completed.clock().uncertainty_ticks())
            .and_then(|value| value.checked_mul(2))
            .ok_or(Refusal::InvalidClock(MonotonicTimeRefusal::Overflow))?;
        let upper = elapsed
            .ticks()
            .checked_add(error)
            .ok_or(Refusal::InvalidClock(MonotonicTimeRefusal::Overflow))?;
        let denominator = match elapsed.scale() {
            TemporalScale::Seconds => 1,
            TemporalScale::Milliseconds => 1_000,
            TemporalScale::Microseconds => 1_000_000,
            TemporalScale::Nanoseconds => 1_000_000_000,
        };
        let mut seconds = |ticks: u64| -> Result<CostDuration, Refusal> {
            let coordinate = ExactRational::ratio(ticks.into(), denominator, work)?;
            CostDuration::admit(
                RationalQuantity::new(
                    coordinate,
                    Unit::resolve("s").expect("reviewed Time Unit"),
                    QuantityRole::Linear,
                )?,
                work,
            )
        };
        let lower = seconds(elapsed.ticks().saturating_sub(error))?;
        let upper = seconds(upper)?;
        let range = CostDurationRange::admit(lower, upper, work)?;
        Ok(Self {
            started: started.clone(),
            completed: completed.clone(),
            elapsed,
            range,
        })
    }
    pub fn started(&self) -> &MonotonicInstant {
        &self.started
    }
    pub fn completed(&self) -> &MonotonicInstant {
        &self.completed
    }
    pub const fn elapsed(&self) -> MonotonicDuration {
        self.elapsed
    }
    pub const fn range(&self) -> CostDurationRange {
        self.range
    }
}
