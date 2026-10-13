//! Admission of bounded Body time over an operation's local horizon.

use alloc::string::String;

use crate::{
    BodyClockCorrelation, BodyTimeEstimate, BodyTimeRefusal, MonotonicDuration, MonotonicInstant,
    TemporalScale, MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BodyTimeTolerance {
    ticks: u64,
    scale: TemporalScale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyTimeRequirement {
    body_basis: String,
    tolerance: BodyTimeTolerance,
    horizon: MonotonicDuration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyTimeQuality {
    Ready {
        now: BodyTimeEstimate,
        horizon: BodyTimeEstimate,
    },
    Degrading {
        now: BodyTimeEstimate,
        reason: BodyTimeRefusal,
    },
    Unsupported {
        reason: BodyTimeRefusal,
    },
}

impl BodyTimeTolerance {
    pub const fn new(ticks: u64, scale: TemporalScale) -> Self {
        Self { ticks, scale }
    }

    pub const fn ticks(self) -> u64 {
        self.ticks
    }

    pub const fn scale(self) -> TemporalScale {
        self.scale
    }
}

impl BodyTimeRequirement {
    pub fn new(
        body_basis: String,
        tolerance: BodyTimeTolerance,
        horizon: MonotonicDuration,
    ) -> Result<Self, BodyTimeRefusal> {
        if body_basis.is_empty() || body_basis.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES {
            return Err(BodyTimeRefusal::DifferentBody);
        }
        Ok(Self {
            body_basis,
            tolerance,
            horizon,
        })
    }

    pub fn assess(
        &self,
        correlation: &BodyClockCorrelation,
        sample: &MonotonicInstant,
    ) -> BodyTimeQuality {
        if self.body_basis != correlation.body_basis()
            || self.tolerance.scale() != correlation.body_scale()
        {
            return BodyTimeQuality::Unsupported {
                reason: BodyTimeRefusal::DifferentBody,
            };
        }
        let now = match correlation.project(sample) {
            Ok(estimate) => estimate,
            Err(reason) => return BodyTimeQuality::Unsupported { reason },
        };
        if !within_tolerance(&now, self.tolerance.ticks()) {
            return BodyTimeQuality::Unsupported {
                reason: BodyTimeRefusal::InsufficientQuality,
            };
        }
        let future_local = match sample.deadline_after(self.horizon) {
            Ok(deadline) => deadline,
            Err(_) => {
                return BodyTimeQuality::Degrading {
                    now,
                    reason: BodyTimeRefusal::InvalidHorizon,
                }
            }
        };
        match correlation.project(future_local.instant()) {
            Ok(horizon) if within_tolerance(&horizon, self.tolerance.ticks()) => {
                BodyTimeQuality::Ready { now, horizon }
            }
            Ok(_) => BodyTimeQuality::Degrading {
                now,
                reason: BodyTimeRefusal::InsufficientQuality,
            },
            Err(reason) => BodyTimeQuality::Degrading { now, reason },
        }
    }
}

fn within_tolerance(estimate: &BodyTimeEstimate, ticks: u64) -> bool {
    estimate.center_ticks - estimate.earliest_ticks <= ticks
        && estimate.latest_ticks - estimate.center_ticks <= ticks
}
