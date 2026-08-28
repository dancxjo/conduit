//! Exact finite temporal primitives without ambient clocks or timezone databases.

use alloc::string::String;
use serde::{Deserialize, Serialize};

pub const MAXIMUM_TEMPORAL_IDENTITY_BYTES: usize = 256;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalScale {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalInstant {
    pub ticks: u64,
    pub scale: TemporalScale,
    pub clock_basis: String,
    pub resolution_ticks: u64,
    pub uncertainty_ticks: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalRelation {
    Past {
        minimum_ticks: u64,
        maximum_ticks: u64,
    },
    Present,
    Future {
        minimum_ticks: u64,
        maximum_ticks: u64,
    },
    Indeterminate,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TemporalRelationError {
    InvalidInstant,
    Incomparable,
    IntervalOverflow,
}

impl TemporalInstant {
    pub fn validate(&self) -> Result<(), TemporalRelationError> {
        if self.clock_basis.is_empty()
            || self.clock_basis.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES
            || self.resolution_ticks == 0
        {
            Err(TemporalRelationError::InvalidInstant)
        } else {
            Ok(())
        }
    }

    pub fn relation_to(
        &self,
        reference: &TemporalInstant,
    ) -> Result<TemporalRelation, TemporalRelationError> {
        self.validate()?;
        reference.validate()?;
        if self.clock_basis != reference.clock_basis || self.scale != reference.scale {
            return Err(TemporalRelationError::Incomparable);
        }
        let source = self.interval()?;
        let target = reference.interval()?;
        if source.1 < target.0 {
            return Ok(TemporalRelation::Past {
                minimum_ticks: target.0 - source.1,
                maximum_ticks: target.1 - source.0,
            });
        }
        if source.0 > target.1 {
            return Ok(TemporalRelation::Future {
                minimum_ticks: source.0 - target.1,
                maximum_ticks: source.1 - target.0,
            });
        }
        if self.ticks == reference.ticks
            && self.uncertainty_ticks == 0
            && reference.uncertainty_ticks == 0
        {
            Ok(TemporalRelation::Present)
        } else {
            Ok(TemporalRelation::Indeterminate)
        }
    }

    fn interval(&self) -> Result<(u64, u64), TemporalRelationError> {
        let lower = self
            .ticks
            .checked_sub(self.uncertainty_ticks)
            .ok_or(TemporalRelationError::IntervalOverflow)?;
        let upper = self
            .ticks
            .checked_add(self.uncertainty_ticks)
            .ok_or(TemporalRelationError::IntervalOverflow)?;
        Ok((lower, upper))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalDate {
    year: i32,
    month: u8,
    day: u8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalTime {
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalDateTime {
    pub date: LocalDate,
    pub time: LocalTime,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UtcOffsetSeconds(i32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedTimeZone {
    identity: String,
    rule_set: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CivilTimeBasis {
    Floating,
    Offset(UtcOffsetSeconds),
    Named(NamedTimeZone),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZonedResolution {
    Unique {
        local: LocalDateTime,
        zone: NamedTimeZone,
        instant: TemporalInstant,
    },
    Ambiguous {
        local: LocalDateTime,
        zone: NamedTimeZone,
        earlier: TemporalInstant,
        later: TemporalInstant,
    },
    Nonexistent {
        local: LocalDateTime,
        zone: NamedTimeZone,
        gap_before: TemporalInstant,
        gap_after: TemporalInstant,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CivilTimeRefusal {
    InvalidDate,
    InvalidTime,
    InvalidOffset,
    EmptyTimeZoneIdentity,
    EmptyRuleSetIdentity,
    IdentityTooLarge,
    InvalidResolutionInstant,
    IncomparableResolutionInstants,
    ReversedResolutionInstants,
}

impl LocalDate {
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, CivilTimeRefusal> {
        let value = Self { year, month, day };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(self) -> Result<(), CivilTimeRefusal> {
        let maximum = days_in_month(self.year, self.month).ok_or(CivilTimeRefusal::InvalidDate)?;
        (self.day != 0 && self.day <= maximum)
            .then_some(())
            .ok_or(CivilTimeRefusal::InvalidDate)
    }

    pub const fn year(self) -> i32 {
        self.year
    }

    pub const fn month(self) -> u8 {
        self.month
    }

    pub const fn day(self) -> u8 {
        self.day
    }
}

impl LocalTime {
    pub fn new(
        hour: u8,
        minute: u8,
        second: u8,
        nanosecond: u32,
    ) -> Result<Self, CivilTimeRefusal> {
        let value = Self {
            hour,
            minute,
            second,
            nanosecond,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(self) -> Result<(), CivilTimeRefusal> {
        (self.hour <= 23
            && self.minute <= 59
            && self.second <= 59
            && self.nanosecond <= 999_999_999)
            .then_some(())
            .ok_or(CivilTimeRefusal::InvalidTime)
    }

    pub const fn hour(self) -> u8 {
        self.hour
    }

    pub const fn minute(self) -> u8 {
        self.minute
    }

    pub const fn second(self) -> u8 {
        self.second
    }

    pub const fn nanosecond(self) -> u32 {
        self.nanosecond
    }
}

impl LocalDateTime {
    pub fn new(date: LocalDate, time: LocalTime) -> Self {
        Self { date, time }
    }

    pub fn validate(self) -> Result<(), CivilTimeRefusal> {
        self.date.validate()?;
        self.time.validate()
    }
}

impl UtcOffsetSeconds {
    pub fn new(seconds: i32) -> Result<Self, CivilTimeRefusal> {
        let value = Self(seconds);
        value.validate()?;
        Ok(value)
    }

    pub fn validate(self) -> Result<(), CivilTimeRefusal> {
        (-86_399..=86_399)
            .contains(&self.0)
            .then_some(())
            .ok_or(CivilTimeRefusal::InvalidOffset)
    }

    pub const fn seconds(self) -> i32 {
        self.0
    }
}

impl NamedTimeZone {
    pub fn new(identity: String, rule_set: String) -> Result<Self, CivilTimeRefusal> {
        validate_identity(&identity, CivilTimeRefusal::EmptyTimeZoneIdentity)?;
        validate_identity(&rule_set, CivilTimeRefusal::EmptyRuleSetIdentity)?;
        Ok(Self { identity, rule_set })
    }

    pub fn validate(&self) -> Result<(), CivilTimeRefusal> {
        validate_identity(&self.identity, CivilTimeRefusal::EmptyTimeZoneIdentity)?;
        validate_identity(&self.rule_set, CivilTimeRefusal::EmptyRuleSetIdentity)
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn rule_set(&self) -> &str {
        &self.rule_set
    }
}

impl CivilTimeBasis {
    pub fn validate(&self) -> Result<(), CivilTimeRefusal> {
        match self {
            Self::Floating => Ok(()),
            Self::Offset(offset) => offset.validate(),
            Self::Named(zone) => zone.validate(),
        }
    }
}

impl ZonedResolution {
    pub fn validate(&self) -> Result<(), CivilTimeRefusal> {
        let (local, zone, first, second) = match self {
            Self::Unique {
                local,
                zone,
                instant,
            } => (local, zone, instant, None),
            Self::Ambiguous {
                local,
                zone,
                earlier,
                later,
                ..
            } => (local, zone, earlier, Some(later)),
            Self::Nonexistent {
                local,
                zone,
                gap_before,
                gap_after,
                ..
            } => (local, zone, gap_before, Some(gap_after)),
        };
        local.validate()?;
        zone.validate()?;
        first
            .validate()
            .map_err(|_| CivilTimeRefusal::InvalidResolutionInstant)?;
        if let Some(second) = second {
            second
                .validate()
                .map_err(|_| CivilTimeRefusal::InvalidResolutionInstant)?;
            if first.clock_basis != second.clock_basis || first.scale != second.scale {
                return Err(CivilTimeRefusal::IncomparableResolutionInstants);
            }
            if first.ticks >= second.ticks {
                return Err(CivilTimeRefusal::ReversedResolutionInstants);
            }
        }
        Ok(())
    }
}

fn validate_identity(value: &str, empty: CivilTimeRefusal) -> Result<(), CivilTimeRefusal> {
    if value.is_empty() {
        Err(empty)
    } else if value.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES {
        Err(CivilTimeRefusal::IdentityTooLarge)
    } else {
        Ok(())
    }
}

fn days_in_month(year: i32, month: u8) -> Option<u8> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if year.rem_euclid(400) == 0
            || (year.rem_euclid(4) == 0 && year.rem_euclid(100) != 0) =>
        {
            Some(29)
        }
        2 => Some(28),
        _ => None,
    }
}
