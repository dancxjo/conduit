//! Bounded projections from one Boot-scoped monotonic clock into Body time.

use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::{
    MonotonicClockIdentity, MonotonicInstant, TemporalScale, MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyTimeRefusal {
    InvalidIdentity,
    InvalidRate,
    InvalidHorizon,
    DifferentClock,
    DifferentBody,
    OldGeneration,
    Regressed,
    ConflictingEvidence,
    InvalidExchange,
    InvalidEstimate,
    InsufficientQuality,
    UnadmittedPeer,
    ExcessiveRoundTrip,
    Stale,
    Overflow,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ClockProvenance {
    Peer {
        host_id: crate::HostId,
        boot_id: crate::BootId,
        admission_reference: String,
        policy_id: String,
        membership_revision: u64,
        observation_sequence: u64,
    },
    External {
        provider_id: String,
        admission_reference: String,
        policy_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyTimeEstimate {
    pub body_basis: String,
    pub generation: u64,
    pub center_ticks: u64,
    pub earliest_ticks: u64,
    pub latest_ticks: u64,
    pub scale: TemporalScale,
    pub local_sample: MonotonicInstant,
    pub provenance: ClockProvenance,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyTimeRelation {
    Before,
    After,
    Indeterminate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyClockCorrelation {
    body_basis: String,
    body_scale: TemporalScale,
    generation: u64,
    local_anchor: MonotonicInstant,
    body_anchor_ticks: u64,
    rate_parts_per_million: i32,
    rate_error_parts_per_million: u32,
    anchor_uncertainty_ticks: u64,
    maximum_age_ticks: u64,
    provenance: ClockProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyTimeTracker {
    correlation: BodyClockCorrelation,
    last: BodyTimeEstimate,
}

impl BodyClockCorrelation {
    pub fn new(
        body_basis: String,
        body_scale: TemporalScale,
        generation: u64,
        local_anchor: MonotonicInstant,
        body_anchor_ticks: u64,
        rate_parts_per_million: i32,
        rate_error_parts_per_million: u32,
        anchor_uncertainty_ticks: u64,
        maximum_age_ticks: u64,
        provenance: ClockProvenance,
    ) -> Result<Self, BodyTimeRefusal> {
        let correlation = Self {
            body_basis,
            body_scale,
            generation,
            local_anchor,
            body_anchor_ticks,
            rate_parts_per_million,
            rate_error_parts_per_million,
            anchor_uncertainty_ticks,
            maximum_age_ticks,
            provenance,
        };
        correlation.validate()?;
        Ok(correlation)
    }

    pub fn validate(&self) -> Result<(), BodyTimeRefusal> {
        for identity in [&self.body_basis] {
            if identity.is_empty() || identity.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES {
                return Err(BodyTimeRefusal::InvalidIdentity);
            }
        }
        self.provenance.validate()?;
        self.local_anchor
            .validate()
            .map_err(|_| BodyTimeRefusal::DifferentClock)?;
        if self.rate_parts_per_million <= -1_000_000
            || self.rate_parts_per_million > 1_000_000
            || self.rate_error_parts_per_million > 1_000_000
        {
            return Err(BodyTimeRefusal::InvalidRate);
        }
        if self.maximum_age_ticks == 0 {
            return Err(BodyTimeRefusal::InvalidHorizon);
        }
        Ok(())
    }

    pub fn local_clock(&self) -> &MonotonicClockIdentity {
        self.local_anchor.clock()
    }

    pub fn body_basis(&self) -> &str {
        &self.body_basis
    }

    pub const fn body_scale(&self) -> TemporalScale {
        self.body_scale
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn local_anchor(&self) -> &MonotonicInstant {
        &self.local_anchor
    }

    pub fn project(&self, sample: &MonotonicInstant) -> Result<BodyTimeEstimate, BodyTimeRefusal> {
        self.project_with_horizon(sample, true)
    }

    fn project_with_horizon(
        &self,
        sample: &MonotonicInstant,
        enforce_horizon: bool,
    ) -> Result<BodyTimeEstimate, BodyTimeRefusal> {
        self.validate()?;
        sample
            .validate()
            .map_err(|_| BodyTimeRefusal::DifferentClock)?;
        if sample.clock() != self.local_anchor.clock() {
            return Err(BodyTimeRefusal::DifferentClock);
        }
        let age = sample
            .ticks()
            .checked_sub(self.local_anchor.ticks())
            .ok_or(BodyTimeRefusal::Stale)?;
        if enforce_horizon && age > self.maximum_age_ticks {
            return Err(BodyTimeRefusal::Stale);
        }
        let local_quantum = u128::from(nanoseconds_per_tick(sample.clock().scale()));
        let body_quantum = u128::from(nanoseconds_per_tick(self.body_scale));
        let denominator = body_quantum * 1_000_000;
        let rate = u128::try_from(1_000_000_i64 + i64::from(self.rate_parts_per_million))
            .map_err(|_| BodyTimeRefusal::InvalidRate)?;
        let scaled = u128::from(age) * local_quantum * rate;
        let delta = scaled / denominator;
        let center = i128::from(self.body_anchor_ticks)
            .checked_add(i128::try_from(delta).map_err(|_| BodyTimeRefusal::Overflow)?)
            .ok_or(BodyTimeRefusal::Overflow)?;
        let center_ticks = u64::try_from(center).map_err(|_| BodyTimeRefusal::Overflow)?;
        let drift =
            (u128::from(age) * local_quantum * u128::from(self.rate_error_parts_per_million))
                .div_ceil(denominator);
        let local_error = u128::from(sample.clock().uncertainty_ticks())
            + u128::from(sample.clock().resolution_ticks());
        let uncertainty = u128::from(self.anchor_uncertainty_ticks)
            + (local_error * local_quantum).div_ceil(body_quantum)
            + 1
            + drift;
        let uncertainty = u64::try_from(uncertainty).map_err(|_| BodyTimeRefusal::Overflow)?;
        let earliest_ticks = center_ticks
            .checked_sub(uncertainty)
            .ok_or(BodyTimeRefusal::Overflow)?;
        let latest_ticks = center_ticks
            .checked_add(uncertainty)
            .ok_or(BodyTimeRefusal::Overflow)?;
        Ok(BodyTimeEstimate {
            body_basis: self.body_basis.clone(),
            generation: self.generation,
            center_ticks,
            earliest_ticks,
            latest_ticks,
            scale: self.body_scale,
            local_sample: sample.clone(),
            provenance: self.provenance.clone(),
        })
    }
}

impl BodyTimeTracker {
    pub fn new(
        correlation: BodyClockCorrelation,
        first_sample: &MonotonicInstant,
    ) -> Result<Self, BodyTimeRefusal> {
        let last = correlation.project(first_sample)?;
        Ok(Self { correlation, last })
    }

    pub const fn last(&self) -> &BodyTimeEstimate {
        &self.last
    }

    pub const fn correlation(&self) -> &BodyClockCorrelation {
        &self.correlation
    }

    pub fn observe(
        &mut self,
        sample: &MonotonicInstant,
    ) -> Result<&BodyTimeEstimate, BodyTimeRefusal> {
        if sample.clock() != self.last.local_sample.clock() {
            return Err(BodyTimeRefusal::DifferentClock);
        }
        if sample.ticks() < self.last.local_sample.ticks() {
            return Err(BodyTimeRefusal::Regressed);
        }
        let next = self.correlation.project(sample)?;
        if next.center_ticks < self.last.center_ticks {
            return Err(BodyTimeRefusal::Regressed);
        }
        self.last = next;
        Ok(&self.last)
    }

    pub fn reconcile(
        &mut self,
        candidate: BodyClockCorrelation,
        at_sample: &MonotonicInstant,
    ) -> Result<&BodyTimeEstimate, BodyTimeRefusal> {
        if candidate.body_basis != self.correlation.body_basis
            || candidate.body_scale != self.correlation.body_scale
        {
            return Err(BodyTimeRefusal::DifferentBody);
        }
        if candidate.generation <= self.correlation.generation {
            return Err(BodyTimeRefusal::OldGeneration);
        }
        if candidate.local_anchor.clock() != self.correlation.local_anchor.clock()
            || at_sample.clock() != self.last.local_sample.clock()
        {
            return Err(BodyTimeRefusal::DifferentClock);
        }
        if at_sample.ticks() < self.last.local_sample.ticks() {
            return Err(BodyTimeRefusal::Regressed);
        }
        let next = candidate.project(at_sample)?;
        if next.center_ticks < self.last.center_ticks {
            return Err(BodyTimeRefusal::Regressed);
        }
        let current = self.correlation.project_with_horizon(at_sample, false)?;
        if current.latest_ticks < next.earliest_ticks || next.latest_ticks < current.earliest_ticks
        {
            return Err(BodyTimeRefusal::ConflictingEvidence);
        }
        self.correlation = candidate;
        self.last = next;
        Ok(&self.last)
    }
}

const fn nanoseconds_per_tick(scale: TemporalScale) -> u64 {
    match scale {
        TemporalScale::Seconds => 1_000_000_000,
        TemporalScale::Milliseconds => 1_000_000,
        TemporalScale::Microseconds => 1_000,
        TemporalScale::Nanoseconds => 1,
    }
}

impl BodyTimeEstimate {
    pub fn validate(&self) -> Result<(), BodyTimeRefusal> {
        if self.body_basis.is_empty()
            || self.body_basis.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES
            || self.provenance.validate().is_err()
            || self.earliest_ticks > self.center_ticks
            || self.center_ticks > self.latest_ticks
            || self.local_sample.validate().is_err()
        {
            return Err(BodyTimeRefusal::InvalidEstimate);
        }
        Ok(())
    }

    pub fn physical_relation(&self, other: &Self) -> Result<BodyTimeRelation, BodyTimeRefusal> {
        self.validate()?;
        other.validate()?;
        if self.body_basis != other.body_basis || self.scale != other.scale {
            return Err(BodyTimeRefusal::Unsupported);
        }
        Ok(if self.latest_ticks < other.earliest_ticks {
            BodyTimeRelation::Before
        } else if other.latest_ticks < self.earliest_ticks {
            BodyTimeRelation::After
        } else {
            BodyTimeRelation::Indeterminate
        })
    }
}

impl ClockProvenance {
    pub fn validate(&self) -> Result<(), BodyTimeRefusal> {
        let identities: &[&str] = match self {
            Self::Peer {
                host_id,
                boot_id,
                admission_reference,
                policy_id,
                ..
            } => &[
                host_id.as_str(),
                boot_id.as_str(),
                admission_reference,
                policy_id,
            ],
            Self::External {
                provider_id,
                admission_reference,
                policy_id,
            } => &[provider_id, admission_reference, policy_id],
        };
        if identities
            .iter()
            .any(|value| value.is_empty() || value.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES)
        {
            Err(BodyTimeRefusal::InvalidIdentity)
        } else {
            Ok(())
        }
    }
}
