//! Bounded projections from one Boot-scoped monotonic clock into Body time.

use alloc::string::String;
use alloc::vec::Vec;
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
    Unavailable,
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
    pub correlation_age_ticks: u64,
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

pub const MAXIMUM_BODY_CLOCK_RATE_SAMPLES: usize = 4;
pub const MAXIMUM_BODY_CLOCK_REJECTIONS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyClockRejection {
    pub body_basis: String,
    pub generation: u64,
    pub local_sample: MonotonicInstant,
    pub provenance: ClockProvenance,
    pub reason: BodyTimeRefusal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyClockRateEstimator {
    accepted: Vec<BodyClockCorrelation>,
    rejections: Vec<BodyClockRejection>,
}

impl BodyClockRateEstimator {
    pub fn new(initial: BodyClockCorrelation) -> Result<Self, BodyTimeRefusal> {
        initial.project(initial.local_anchor())?;
        Ok(Self {
            accepted: alloc::vec![initial],
            rejections: Vec::new(),
        })
    }

    pub fn retained_samples(&self) -> usize {
        self.accepted.len()
    }

    pub fn latest(&self) -> &BodyClockCorrelation {
        self.accepted
            .last()
            .expect("a rate estimator retains its initial correlation")
    }

    pub fn rejections(&self) -> &[BodyClockRejection] {
        &self.rejections
    }

    pub fn refine(
        &mut self,
        candidate: &BodyClockCorrelation,
        maximum_absolute_rate_ppm: u32,
    ) -> Result<BodyClockCorrelation, BodyTimeRefusal> {
        let result = self.refine_candidate(candidate, maximum_absolute_rate_ppm);
        if let Err(reason) = &result {
            if self.rejections.len() == MAXIMUM_BODY_CLOCK_REJECTIONS {
                self.rejections.remove(0);
            }
            self.rejections.push(BodyClockRejection {
                body_basis: candidate.body_basis.clone(),
                generation: candidate.generation,
                local_sample: candidate.local_anchor.clone(),
                provenance: candidate.provenance.clone(),
                reason: *reason,
            });
        }
        result
    }

    fn refine_candidate(
        &self,
        candidate: &BodyClockCorrelation,
        maximum_absolute_rate_ppm: u32,
    ) -> Result<BodyClockCorrelation, BodyTimeRefusal> {
        if maximum_absolute_rate_ppm >= 1_000_000 {
            return Err(BodyTimeRefusal::InvalidRate);
        }
        let latest = self
            .accepted
            .last()
            .ok_or(BodyTimeRefusal::InvalidEstimate)?;
        if candidate.body_basis != latest.body_basis || candidate.body_scale != latest.body_scale {
            return Err(BodyTimeRefusal::DifferentBody);
        }
        if candidate.local_clock() != latest.local_clock() {
            return Err(BodyTimeRefusal::DifferentClock);
        }
        if candidate.generation <= latest.generation {
            return Err(BodyTimeRefusal::OldGeneration);
        }
        candidate.project(candidate.local_anchor())?;
        if candidate.local_anchor.ticks() <= latest.local_anchor.ticks() {
            return Err(BodyTimeRefusal::Regressed);
        }
        for accepted in &self.accepted {
            let prior = accepted.project_with_horizon(candidate.local_anchor(), false)?;
            let current = candidate.project(candidate.local_anchor())?;
            if prior.latest_ticks < current.earliest_ticks
                || current.latest_ticks < prior.earliest_ticks
            {
                return Err(BodyTimeRefusal::ConflictingEvidence);
            }
        }
        let first = &self.accepted[0];
        let first_estimate = first.project(first.local_anchor())?;
        let next_estimate = candidate.project(candidate.local_anchor())?;
        let local_delta = u128::from(candidate.local_anchor.ticks() - first.local_anchor.ticks())
            * u128::from(nanoseconds_per_tick(candidate.local_clock().scale()));
        let body_quantum = i128::from(nanoseconds_per_tick(candidate.body_scale));
        let minimum_delta = (i128::from(next_estimate.earliest_ticks)
            - i128::from(first_estimate.latest_ticks))
            * body_quantum;
        let maximum_delta = (i128::from(next_estimate.latest_ticks)
            - i128::from(first_estimate.earliest_ticks))
            * body_quantum;
        let denominator = i128::try_from(local_delta).map_err(|_| BodyTimeRefusal::Overflow)?;
        let lower = (minimum_delta * 1_000_000).div_euclid(denominator) - 1_000_000;
        let upper =
            (maximum_delta * 1_000_000 + denominator - 1).div_euclid(denominator) - 1_000_000;
        let limit = i128::from(maximum_absolute_rate_ppm);
        let candidate_rate = i128::from(candidate.rate_parts_per_million);
        let candidate_error = i128::from(candidate.rate_error_parts_per_million);
        let lower = lower.max(-limit).max(candidate_rate - candidate_error);
        let upper = upper.min(limit).min(candidate_rate + candidate_error);
        if lower > upper {
            return Err(BodyTimeRefusal::ConflictingEvidence);
        }
        let rate = (lower + upper).div_euclid(2);
        let error = (rate - lower).max(upper - rate);
        BodyClockCorrelation::new(
            candidate.body_basis.clone(),
            candidate.body_scale,
            candidate.generation,
            candidate.local_anchor.clone(),
            candidate.body_anchor_ticks,
            i32::try_from(rate).map_err(|_| BodyTimeRefusal::InvalidRate)?,
            u32::try_from(error).map_err(|_| BodyTimeRefusal::InvalidRate)?,
            candidate.anchor_uncertainty_ticks,
            candidate.maximum_age_ticks,
            candidate.provenance.clone(),
        )
    }

    pub fn record(&mut self, accepted: BodyClockCorrelation) -> Result<(), BodyTimeRefusal> {
        let latest = self
            .accepted
            .last()
            .ok_or(BodyTimeRefusal::InvalidEstimate)?;
        if accepted.body_basis != latest.body_basis || accepted.body_scale != latest.body_scale {
            return Err(BodyTimeRefusal::DifferentBody);
        }
        if accepted.local_clock() != latest.local_clock() {
            return Err(BodyTimeRefusal::DifferentClock);
        }
        if accepted.generation <= latest.generation {
            return Err(BodyTimeRefusal::OldGeneration);
        }
        accepted.project(accepted.local_anchor())?;
        if accepted.local_anchor.ticks() <= latest.local_anchor.ticks() {
            return Err(BodyTimeRefusal::Regressed);
        }
        let prior = latest.project_with_horizon(accepted.local_anchor(), false)?;
        let current = accepted.project(accepted.local_anchor())?;
        if prior.latest_ticks < current.earliest_ticks
            || current.latest_ticks < prior.earliest_ticks
        {
            return Err(BodyTimeRefusal::ConflictingEvidence);
        }
        if self.accepted.len() == MAXIMUM_BODY_CLOCK_RATE_SAMPLES {
            self.accepted.remove(0);
        }
        self.accepted.push(accepted);
        Ok(())
    }
}

impl BodyClockCorrelation {
    #[allow(clippy::too_many_arguments)]
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

    pub const fn rate_parts_per_million(&self) -> i32 {
        self.rate_parts_per_million
    }

    pub const fn rate_error_parts_per_million(&self) -> u32 {
        self.rate_error_parts_per_million
    }

    pub const fn anchor_uncertainty_ticks(&self) -> u64 {
        self.anchor_uncertainty_ticks
    }

    pub const fn maximum_age_ticks(&self) -> u64 {
        self.maximum_age_ticks
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
            correlation_age_ticks: age,
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
            || self.correlation_age_ticks > self.local_sample.ticks()
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
