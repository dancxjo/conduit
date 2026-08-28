//! Exact monotonic clock truth and explicit wall-clock correlation.

use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::{BootId, HostId, TemporalInstant, TemporalRelationError, TemporalScale};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonotonicDuration {
    ticks: u64,
    scale: TemporalScale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonotonicClockIdentity {
    host_id: HostId,
    boot_id: BootId,
    basis_id: String,
    scale: TemporalScale,
    resolution_ticks: u64,
    uncertainty_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonotonicInstant {
    ticks: u64,
    clock: MonotonicClockIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonotonicDeadline {
    instant: MonotonicInstant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockCorrelation {
    identity: String,
    monotonic: MonotonicInstant,
    wall: TemporalInstant,
    wall_uncertainty_ticks: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MonotonicTimeRefusal {
    EmptyIdentity,
    IdentityTooLarge,
    InvalidResolution,
    InvalidWallInstant,
    DifferentClock,
    DifferentScale,
    Regressed,
    Overflow,
}

impl MonotonicDuration {
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

impl MonotonicClockIdentity {
    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        basis_id: String,
        scale: TemporalScale,
        resolution_ticks: u64,
        uncertainty_ticks: u64,
    ) -> Result<Self, MonotonicTimeRefusal> {
        let value = Self {
            host_id,
            boot_id,
            basis_id,
            scale,
            resolution_ticks,
            uncertainty_ticks,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MonotonicTimeRefusal> {
        validate_identity(self.host_id.as_str())?;
        validate_identity(self.boot_id.as_str())?;
        validate_identity(&self.basis_id)?;
        if self.resolution_ticks == 0 {
            return Err(MonotonicTimeRefusal::InvalidResolution);
        }
        Ok(())
    }

    pub fn host_id(&self) -> &HostId {
        &self.host_id
    }

    pub fn boot_id(&self) -> &BootId {
        &self.boot_id
    }

    pub fn basis_id(&self) -> &str {
        &self.basis_id
    }

    pub const fn scale(&self) -> TemporalScale {
        self.scale
    }

    pub const fn resolution_ticks(&self) -> u64 {
        self.resolution_ticks
    }

    pub const fn uncertainty_ticks(&self) -> u64 {
        self.uncertainty_ticks
    }
}

impl MonotonicInstant {
    pub fn new(ticks: u64, clock: MonotonicClockIdentity) -> Result<Self, MonotonicTimeRefusal> {
        clock.validate()?;
        Ok(Self { ticks, clock })
    }

    pub fn validate(&self) -> Result<(), MonotonicTimeRefusal> {
        self.clock.validate()
    }

    pub const fn ticks(&self) -> u64 {
        self.ticks
    }

    pub const fn clock(&self) -> &MonotonicClockIdentity {
        &self.clock
    }

    pub fn elapsed_since(&self, earlier: &Self) -> Result<MonotonicDuration, MonotonicTimeRefusal> {
        self.validate()?;
        earlier.validate()?;
        ensure_same_clock(&self.clock, &earlier.clock)?;
        let ticks = self
            .ticks
            .checked_sub(earlier.ticks)
            .ok_or(MonotonicTimeRefusal::Regressed)?;
        Ok(MonotonicDuration::new(ticks, self.clock.scale))
    }

    pub fn deadline_after(
        &self,
        duration: MonotonicDuration,
    ) -> Result<MonotonicDeadline, MonotonicTimeRefusal> {
        self.validate()?;
        if duration.scale != self.clock.scale {
            return Err(MonotonicTimeRefusal::DifferentScale);
        }
        let ticks = self
            .ticks
            .checked_add(duration.ticks)
            .ok_or(MonotonicTimeRefusal::Overflow)?;
        Ok(MonotonicDeadline {
            instant: Self {
                ticks,
                clock: self.clock.clone(),
            },
        })
    }
}

impl MonotonicDeadline {
    pub fn validate(&self) -> Result<(), MonotonicTimeRefusal> {
        self.instant.validate()
    }

    pub const fn instant(&self) -> &MonotonicInstant {
        &self.instant
    }

    pub fn remaining_at(
        &self,
        now: &MonotonicInstant,
    ) -> Result<Option<MonotonicDuration>, MonotonicTimeRefusal> {
        self.validate()?;
        now.validate()?;
        ensure_same_clock(self.instant.clock(), now.clock())?;
        if now.ticks >= self.instant.ticks {
            Ok(None)
        } else {
            Ok(Some(MonotonicDuration::new(
                self.instant.ticks - now.ticks,
                self.instant.clock.scale,
            )))
        }
    }
}

impl ClockCorrelation {
    pub fn new(
        identity: String,
        monotonic: MonotonicInstant,
        wall: TemporalInstant,
        wall_uncertainty_ticks: u64,
    ) -> Result<Self, MonotonicTimeRefusal> {
        let value = Self {
            identity,
            monotonic,
            wall,
            wall_uncertainty_ticks,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MonotonicTimeRefusal> {
        validate_identity(&self.identity)?;
        self.monotonic.validate()?;
        self.wall
            .validate()
            .map_err(|_: TemporalRelationError| MonotonicTimeRefusal::InvalidWallInstant)
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub const fn monotonic(&self) -> &MonotonicInstant {
        &self.monotonic
    }

    pub const fn wall(&self) -> &TemporalInstant {
        &self.wall
    }

    /// Additional correlation uncertainty expressed in the wall instant's scale.
    pub const fn wall_uncertainty_ticks(&self) -> u64 {
        self.wall_uncertainty_ticks
    }
}

fn ensure_same_clock(
    left: &MonotonicClockIdentity,
    right: &MonotonicClockIdentity,
) -> Result<(), MonotonicTimeRefusal> {
    if left.host_id != right.host_id
        || left.boot_id != right.boot_id
        || left.basis_id != right.basis_id
    {
        Err(MonotonicTimeRefusal::DifferentClock)
    } else if left.scale != right.scale {
        Err(MonotonicTimeRefusal::DifferentScale)
    } else {
        Ok(())
    }
}

fn validate_identity(value: &str) -> Result<(), MonotonicTimeRefusal> {
    if value.is_empty() {
        Err(MonotonicTimeRefusal::EmptyIdentity)
    } else if value.len() > crate::MAXIMUM_TEMPORAL_IDENTITY_BYTES {
        Err(MonotonicTimeRefusal::IdentityTooLarge)
    } else {
        Ok(())
    }
}
