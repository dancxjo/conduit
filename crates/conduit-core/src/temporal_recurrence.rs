//! Finite recurrence semantics with no ambient clock or timezone resolver.

use alloc::{format, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{
    LocalDate, LocalTime, MonotonicDuration, MonotonicInstant, NamedTimeZone, TemporalInstant,
    MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

pub const MAXIMUM_RECURRENCE_OCCURRENCES: u32 = 4_096;
pub const MAXIMUM_RECURRENCE_EXCEPTIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurrenceDefinition {
    pub identity: String,
    pub rule: RecurrenceRule,
    pub maximum_occurrences: u32,
    pub excluded_ordinals: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecurrenceRule {
    OneShot {
        at: TemporalInstant,
    },
    FixedElapsed {
        first: MonotonicInstant,
        every: MonotonicDuration,
    },
    CivilWeekdays {
        first_date: LocalDate,
        local_time: LocalTime,
        zone: NamedTimeZone,
        weekdays: WeekdaySet,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeekdaySet(u8);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecurrenceWindow {
    Wall {
        start: TemporalInstant,
        end: TemporalInstant,
    },
    Monotonic {
        start: MonotonicInstant,
        end: MonotonicInstant,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurrenceExpansion {
    pub maximum_results: u32,
    pub window: RecurrenceWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurrenceOccurrence {
    pub identity: String,
    pub recurrence_identity: String,
    pub ordinal: u32,
    pub at: OccurrenceInstant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OccurrenceInstant {
    Wall(TemporalInstant),
    Monotonic(MonotonicInstant),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RecurrenceRefusal {
    InvalidIdentity,
    InvalidRule,
    InvalidLimit,
    InvalidExceptions,
    InvalidWindow,
    IncomparableWindow,
    WrongWindowKind,
    WorkLimitExceeded,
    ArithmeticOverflow,
    CivilResolutionRequired,
}

impl WeekdaySet {
    pub const MONDAY: Self = Self(1 << 0);
    pub const TUESDAY: Self = Self(1 << 1);
    pub const WEDNESDAY: Self = Self(1 << 2);
    pub const THURSDAY: Self = Self(1 << 3);
    pub const FRIDAY: Self = Self(1 << 4);
    pub const SATURDAY: Self = Self(1 << 5);
    pub const SUNDAY: Self = Self(1 << 6);
    pub const WEEKDAYS: Self = Self((1 << 5) - 1);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, weekday: Self) -> bool {
        self.0 & weekday.0 != 0
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    fn validate(self) -> Result<(), RecurrenceRefusal> {
        if self.0 == 0 || self.0 & !0x7f != 0 {
            Err(RecurrenceRefusal::InvalidRule)
        } else {
            Ok(())
        }
    }
}

impl RecurrenceDefinition {
    pub fn validate(&self) -> Result<(), RecurrenceRefusal> {
        validate_identity(&self.identity)?;
        if self.maximum_occurrences == 0
            || self.maximum_occurrences > MAXIMUM_RECURRENCE_OCCURRENCES
        {
            return Err(RecurrenceRefusal::InvalidLimit);
        }
        if self.excluded_ordinals.len() > MAXIMUM_RECURRENCE_EXCEPTIONS
            || self
                .excluded_ordinals
                .iter()
                .any(|ordinal| *ordinal >= self.maximum_occurrences)
            || self
                .excluded_ordinals
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(RecurrenceRefusal::InvalidExceptions);
        }
        match &self.rule {
            RecurrenceRule::OneShot { at } => {
                at.validate().map_err(|_| RecurrenceRefusal::InvalidRule)?;
                if self.maximum_occurrences != 1 {
                    return Err(RecurrenceRefusal::InvalidLimit);
                }
            }
            RecurrenceRule::FixedElapsed { first, every } => {
                first
                    .validate()
                    .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                if every.ticks() == 0 || every.scale() != first.clock().scale() {
                    return Err(RecurrenceRefusal::InvalidRule);
                }
            }
            RecurrenceRule::CivilWeekdays {
                first_date,
                local_time,
                zone,
                weekdays,
            } => {
                first_date
                    .validate()
                    .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                local_time
                    .validate()
                    .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                zone.validate()
                    .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                weekdays.validate()?;
            }
        }
        Ok(())
    }

    pub fn expand(
        &self,
        request: &RecurrenceExpansion,
    ) -> Result<Vec<RecurrenceOccurrence>, RecurrenceRefusal> {
        self.validate()?;
        request.validate()?;
        if request.maximum_results > self.maximum_occurrences {
            return Err(RecurrenceRefusal::WorkLimitExceeded);
        }
        match (&self.rule, &request.window) {
            (RecurrenceRule::OneShot { at }, RecurrenceWindow::Wall { start, end }) => {
                let mut occurrences = Vec::with_capacity(1);
                if self.excluded_ordinals.binary_search(&0).is_err()
                    && wall_in_window(at, start, end)?
                    && request.maximum_results > 0
                {
                    occurrences.push(self.occurrence(0, OccurrenceInstant::Wall(at.clone()))?);
                }
                Ok(occurrences)
            }
            (
                RecurrenceRule::FixedElapsed { first, every },
                RecurrenceWindow::Monotonic { start, end },
            ) => {
                ensure_same_monotonic_clock(first, start)?;
                ensure_same_monotonic_clock(first, end)?;
                if start.ticks() > end.ticks() {
                    return Err(RecurrenceRefusal::InvalidWindow);
                }
                let mut occurrences = Vec::with_capacity(request.maximum_results as usize);
                for ordinal in 0..self.maximum_occurrences {
                    let offset = every
                        .ticks()
                        .checked_mul(u64::from(ordinal))
                        .ok_or(RecurrenceRefusal::ArithmeticOverflow)?;
                    let at = first
                        .deadline_after(MonotonicDuration::new(offset, every.scale()))
                        .map_err(|_| RecurrenceRefusal::ArithmeticOverflow)?
                        .instant()
                        .clone();
                    if at.ticks() > end.ticks() {
                        break;
                    }
                    if at.ticks() >= start.ticks()
                        && self.excluded_ordinals.binary_search(&ordinal).is_err()
                    {
                        if occurrences.len() == request.maximum_results as usize {
                            return Err(RecurrenceRefusal::WorkLimitExceeded);
                        }
                        occurrences
                            .push(self.occurrence(ordinal, OccurrenceInstant::Monotonic(at))?);
                    }
                }
                Ok(occurrences)
            }
            (RecurrenceRule::CivilWeekdays { .. }, _) => {
                Err(RecurrenceRefusal::CivilResolutionRequired)
            }
            _ => Err(RecurrenceRefusal::WrongWindowKind),
        }
    }

    fn occurrence(
        &self,
        ordinal: u32,
        at: OccurrenceInstant,
    ) -> Result<RecurrenceOccurrence, RecurrenceRefusal> {
        let identity = format!("{}/occurrence/{ordinal}", self.identity);
        validate_identity(&identity)?;
        Ok(RecurrenceOccurrence {
            identity,
            recurrence_identity: self.identity.clone(),
            ordinal,
            at,
        })
    }
}

impl RecurrenceExpansion {
    pub fn validate(&self) -> Result<(), RecurrenceRefusal> {
        if self.maximum_results == 0 || self.maximum_results > MAXIMUM_RECURRENCE_OCCURRENCES {
            return Err(RecurrenceRefusal::InvalidLimit);
        }
        match &self.window {
            RecurrenceWindow::Wall { start, end } => {
                start
                    .validate()
                    .map_err(|_| RecurrenceRefusal::InvalidWindow)?;
                end.validate()
                    .map_err(|_| RecurrenceRefusal::InvalidWindow)?;
                if start.clock_basis != end.clock_basis || start.scale != end.scale {
                    return Err(RecurrenceRefusal::IncomparableWindow);
                }
                if start.uncertainty_ticks != 0
                    || end.uncertainty_ticks != 0
                    || start.ticks > end.ticks
                {
                    return Err(RecurrenceRefusal::InvalidWindow);
                }
            }
            RecurrenceWindow::Monotonic { start, end } => {
                ensure_same_monotonic_clock(start, end)?;
                if start.ticks() > end.ticks() {
                    return Err(RecurrenceRefusal::InvalidWindow);
                }
            }
        }
        Ok(())
    }
}

fn wall_in_window(
    at: &TemporalInstant,
    start: &TemporalInstant,
    end: &TemporalInstant,
) -> Result<bool, RecurrenceRefusal> {
    if at.clock_basis != start.clock_basis
        || at.clock_basis != end.clock_basis
        || at.scale != start.scale
        || at.scale != end.scale
    {
        return Err(RecurrenceRefusal::IncomparableWindow);
    }
    if at.uncertainty_ticks != 0 {
        return Err(RecurrenceRefusal::InvalidRule);
    }
    Ok(at.ticks >= start.ticks && at.ticks <= end.ticks)
}

fn ensure_same_monotonic_clock(
    left: &MonotonicInstant,
    right: &MonotonicInstant,
) -> Result<(), RecurrenceRefusal> {
    if left.clock().host_id() != right.clock().host_id()
        || left.clock().boot_id() != right.clock().boot_id()
        || left.clock().basis_id() != right.clock().basis_id()
        || left.clock().scale() != right.clock().scale()
    {
        Err(RecurrenceRefusal::IncomparableWindow)
    } else {
        Ok(())
    }
}

fn validate_identity(identity: &str) -> Result<(), RecurrenceRefusal> {
    if identity.is_empty() || identity.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES {
        Err(RecurrenceRefusal::InvalidIdentity)
    } else {
        Ok(())
    }
}
