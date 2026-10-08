//! Admission of bounded Body time over an operation's local horizon.

use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::{
    BodyClockCorrelation, BodyTimeEstimate, BodyTimeRefusal, MonotonicDuration, MonotonicInstant,
    TemporalScale, MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyTimeTolerance {
    ticks: u64,
    scale: TemporalScale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

    pub fn body_basis(&self) -> &str {
        &self.body_basis
    }

    pub const fn tolerance(&self) -> BodyTimeTolerance {
        self.tolerance
    }

    pub const fn horizon(&self) -> MonotonicDuration {
        self.horizon
    }

    pub fn valid_basis(&self) -> bool {
        !self.body_basis.is_empty() && self.body_basis.len() <= MAXIMUM_TEMPORAL_IDENTITY_BYTES
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
        let local_horizon = match convert_duration(self.horizon, sample.clock().scale()) {
            Some(duration) => duration,
            None => {
                return BodyTimeQuality::Degrading {
                    now,
                    reason: BodyTimeRefusal::InvalidHorizon,
                }
            }
        };
        let future_local = match sample.deadline_after(local_horizon) {
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

fn convert_duration(
    duration: MonotonicDuration,
    scale: TemporalScale,
) -> Option<MonotonicDuration> {
    let source_nanos = nanos_per_tick(duration.scale());
    let target_nanos = nanos_per_tick(scale);
    let nanos = u128::from(duration.ticks()).checked_mul(source_nanos)?;
    let rounded_ticks = nanos.checked_add(target_nanos - 1)? / target_nanos;
    Some(MonotonicDuration::new(
        u64::try_from(rounded_ticks).ok()?,
        scale,
    ))
}

const fn nanos_per_tick(scale: TemporalScale) -> u128 {
    match scale {
        TemporalScale::Seconds => 1_000_000_000,
        TemporalScale::Milliseconds => 1_000_000,
        TemporalScale::Microseconds => 1_000,
        TemporalScale::Nanoseconds => 1,
    }
}

fn within_tolerance(estimate: &BodyTimeEstimate, ticks: u64) -> bool {
    estimate.center_ticks - estimate.earliest_ticks <= ticks
        && estimate.latest_ticks - estimate.center_ticks <= ticks
}
