//! Typed measurement samples and deterministic count-bounded window state.

use alloc::vec::Vec;
use conduit_core::{
    Quantity, QuantityUnit, TemporalInstant, TemporalRelation, TemporalRelationError,
};

pub const MEASUREMENT_SAMPLE_INFO_ID: &str = "data/measurement-sample@1";
pub const MEASUREMENT_WINDOW_INFO_ID: &str = "data/measurement-window@1";
pub const MAXIMUM_MEASUREMENT_WINDOW_SAMPLES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementSample {
    pub value: Quantity,
    pub observed_at: TemporalInstant,
    pub uncertainty: Option<Quantity>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MeasurementRange {
    pub minimum: Quantity,
    pub maximum: Quantity,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FullWindowPolicy {
    Reject,
    DropOldest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementWindowProfile {
    pub capacity: usize,
    pub unit: QuantityUnit,
    pub range: MeasurementRange,
    pub clock_basis: alloc::string::String,
    pub full_policy: FullWindowPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedMeasurementWindow {
    profile: MeasurementWindowProfile,
    samples: Vec<MeasurementSample>,
    discarded_samples: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeasurementWindowRefusal {
    CapacityOutOfBounds,
    InvalidClockProfile,
    InvalidRange,
    InvalidTimestamp,
    UnitMismatch,
    UncertaintyUnitMismatch,
    NegativeUncertainty,
    ClockMismatch,
    TimestampRegression,
    OutOfRange,
    Full,
    DiscardCountOverflow,
}

impl MeasurementWindowProfile {
    pub fn validate(&self) -> Result<(), MeasurementWindowRefusal> {
        if self.capacity == 0 || self.capacity > MAXIMUM_MEASUREMENT_WINDOW_SAMPLES {
            return Err(MeasurementWindowRefusal::CapacityOutOfBounds);
        }
        if self.clock_basis.is_empty() {
            return Err(MeasurementWindowRefusal::InvalidClockProfile);
        }
        if self.range.minimum.unit() != self.unit
            || self.range.maximum.unit() != self.unit
            || self.range.minimum.value() > self.range.maximum.value()
        {
            return Err(MeasurementWindowRefusal::InvalidRange);
        }
        Ok(())
    }
}

impl BoundedMeasurementWindow {
    /// Creates all backing storage before samples are admitted.
    pub fn new(profile: MeasurementWindowProfile) -> Result<Self, MeasurementWindowRefusal> {
        profile.validate()?;
        let capacity = profile.capacity;
        Ok(Self {
            profile,
            samples: Vec::with_capacity(capacity),
            discarded_samples: 0,
        })
    }

    pub fn push(&mut self, sample: MeasurementSample) -> Result<(), MeasurementWindowRefusal> {
        self.validate_sample(&sample)?;
        if self.samples.len() == self.profile.capacity {
            match self.profile.full_policy {
                FullWindowPolicy::Reject => return Err(MeasurementWindowRefusal::Full),
                FullWindowPolicy::DropOldest => {
                    self.discarded_samples = self
                        .discarded_samples
                        .checked_add(1)
                        .ok_or(MeasurementWindowRefusal::DiscardCountOverflow)?;
                    self.samples.remove(0);
                }
            }
        }
        self.samples.push(sample);
        Ok(())
    }

    pub fn samples(&self) -> &[MeasurementSample] {
        &self.samples
    }

    pub const fn profile(&self) -> &MeasurementWindowProfile {
        &self.profile
    }

    pub const fn discarded_samples(&self) -> u64 {
        self.discarded_samples
    }

    fn validate_sample(&self, sample: &MeasurementSample) -> Result<(), MeasurementWindowRefusal> {
        sample
            .observed_at
            .validate()
            .map_err(|_| MeasurementWindowRefusal::InvalidTimestamp)?;
        if sample.value.unit() != self.profile.unit {
            return Err(MeasurementWindowRefusal::UnitMismatch);
        }
        if sample.observed_at.clock_basis != self.profile.clock_basis {
            return Err(MeasurementWindowRefusal::ClockMismatch);
        }
        if let Some(uncertainty) = sample.uncertainty {
            if uncertainty.unit() != self.profile.unit {
                return Err(MeasurementWindowRefusal::UncertaintyUnitMismatch);
            }
            if uncertainty.value() < 0 {
                return Err(MeasurementWindowRefusal::NegativeUncertainty);
            }
        }
        if sample.value.value() < self.profile.range.minimum.value()
            || sample.value.value() > self.profile.range.maximum.value()
        {
            return Err(MeasurementWindowRefusal::OutOfRange);
        }
        if let Some(previous) = self.samples.last() {
            match sample.observed_at.relation_to(&previous.observed_at) {
                Ok(TemporalRelation::Future { .. }) => {}
                Ok(_) => return Err(MeasurementWindowRefusal::TimestampRegression),
                Err(TemporalRelationError::Incomparable) => {
                    return Err(MeasurementWindowRefusal::ClockMismatch);
                }
                Err(
                    TemporalRelationError::InvalidInstant | TemporalRelationError::IntervalOverflow,
                ) => {
                    return Err(MeasurementWindowRefusal::InvalidTimestamp);
                }
            }
        }
        Ok(())
    }
}
