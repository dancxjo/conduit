//! Finite recurrence semantics with no ambient clock or timezone resolver.

use alloc::{format, vec::Vec};

use crate::{
    LocalDate, MonotonicDuration, MonotonicInstant, OccurrenceInstant, RecurrenceDefinition,
    RecurrenceExpansion, RecurrenceOccurrence, RecurrenceRefusal, RecurrenceRule, RecurrenceUntil,
    RecurrenceWindow, TemporalInstant, WeekdaySet, MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

pub const MAXIMUM_RECURRENCE_OCCURRENCES: u32 = 4_096;
pub const MAXIMUM_RECURRENCE_EXCEPTIONS: usize = 256;

impl WeekdaySet {
    fn validate(self) -> Result<(), RecurrenceRefusal> {
        if *self.get() == 0 || *self.get() & !0x7f != 0 {
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
                .iter()
                .zip(self.excluded_ordinals.iter().skip(1))
                .any(|(left, right)| left >= right)
        {
            return Err(RecurrenceRefusal::InvalidExceptions);
        }
        match &self.rule {
            RecurrenceRule::OneShot(rule) => {
                let at = rule.at();
                at.validate().map_err(|_| RecurrenceRefusal::InvalidRule)?;
                if self.maximum_occurrences != 1 {
                    return Err(RecurrenceRefusal::InvalidLimit);
                }
                match &self.until {
                    Some(RecurrenceUntil::Wall(until)) => {
                        let until = until.value();
                        until
                            .validate()
                            .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                        wall_comparable(at, until)?;
                    }
                    None => {}
                    _ => return Err(RecurrenceRefusal::InvalidRule),
                }
            }
            RecurrenceRule::FixedElapsed(rule) => {
                let first = rule.first();
                let every = rule.every();
                first
                    .validate()
                    .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                if every.ticks() == 0 || every.scale() != *first.clock().scale() {
                    return Err(RecurrenceRefusal::InvalidRule);
                }
                match &self.until {
                    Some(RecurrenceUntil::Monotonic(until)) => {
                        let until = until.value();
                        until
                            .validate()
                            .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                        ensure_same_monotonic_clock(first, until)?;
                    }
                    None => {}
                    _ => return Err(RecurrenceRefusal::InvalidRule),
                }
            }
            RecurrenceRule::CivilWeekdays(rule) => {
                let first_date = rule.first_date();
                let local_time = rule.local_time();
                let zone = rule.zone();
                let weekdays = rule.weekdays();
                let excluded_dates = rule.excluded_dates();
                first_date
                    .validate()
                    .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                local_time
                    .validate()
                    .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                zone.validate()
                    .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                weekdays.validate()?;
                if excluded_dates.len() > MAXIMUM_RECURRENCE_EXCEPTIONS
                    || excluded_dates.iter().any(|date| date.validate().is_err())
                    || excluded_dates
                        .iter()
                        .zip(excluded_dates.iter().skip(1))
                        .any(|(left, right)| date_key(*left) >= date_key(*right))
                    || excluded_dates
                        .iter()
                        .any(|date| date_key(*date) < date_key(*first_date))
                    || excluded_dates.iter().any(|date| {
                        !weekdays.contains(crate::temporal_recurrence_civil::weekday(*date))
                    })
                {
                    return Err(RecurrenceRefusal::InvalidExceptions);
                }
                match &self.until {
                    Some(RecurrenceUntil::CivilDate(until)) => {
                        let until = until.value();
                        until
                            .validate()
                            .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                        if date_key(*until) < date_key(*first_date) {
                            return Err(RecurrenceRefusal::InvalidRule);
                        }
                    }
                    None => {}
                    _ => return Err(RecurrenceRefusal::InvalidRule),
                }
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
        match (&self.rule, &request.window) {
            (RecurrenceRule::OneShot(rule), RecurrenceWindow::Wall(window)) => {
                let at = rule.at();
                let start = window.start();
                let end = window.end();
                let mut occurrences = Vec::with_capacity(1);
                if self.excluded_ordinals.binary_search(&0).is_err()
                    && self.until.as_ref().is_none_or(
                        |until| matches!(until, RecurrenceUntil::Wall(value) if at.ticks <= value.value().ticks),
                    )
                    && wall_in_window(at, start, end)?
                    && request.maximum_results > 0
                {
                    occurrences.push(self.occurrence(
                        0,
                        OccurrenceInstant::wall(at.clone())
                            .map_err(|_| RecurrenceRefusal::InvalidRule)?,
                    )?);
                }
                Ok(occurrences)
            }
            (RecurrenceRule::FixedElapsed(rule), RecurrenceWindow::Monotonic(window)) => {
                let first = rule.first();
                let every = rule.every();
                let start = window.start();
                let end = window.end();
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
                    let duration = MonotonicDuration::new(offset, every.scale())
                        .map_err(|_| RecurrenceRefusal::ArithmeticOverflow)?;
                    let at = first
                        .after(duration)
                        .map_err(|_| RecurrenceRefusal::ArithmeticOverflow)?;
                    if self.until.as_ref().is_some_and(|until| {
                        matches!(until, RecurrenceUntil::Monotonic(value) if at.ticks() > value.value().ticks())
                    }) {
                        break;
                    }
                    if at.ticks() > end.ticks() {
                        break;
                    }
                    if at.ticks() >= start.ticks()
                        && self.excluded_ordinals.binary_search(&ordinal).is_err()
                    {
                        if occurrences.len() == request.maximum_results as usize {
                            return Err(RecurrenceRefusal::WorkLimitExceeded);
                        }
                        occurrences.push(
                            self.occurrence(
                                ordinal,
                                OccurrenceInstant::monotonic(at)
                                    .map_err(|_| RecurrenceRefusal::InvalidRule)?,
                            )?,
                        );
                    }
                }
                Ok(occurrences)
            }
            (RecurrenceRule::CivilWeekdays(_), _) => {
                Err(RecurrenceRefusal::CivilResolutionRequired)
            }
            _ => Err(RecurrenceRefusal::WrongWindowKind),
        }
    }

    pub(crate) fn occurrence(
        &self,
        ordinal: u32,
        at: OccurrenceInstant,
    ) -> Result<RecurrenceOccurrence, RecurrenceRefusal> {
        self.occurrence_with_suffix(ordinal, at, "")
    }

    pub(crate) fn occurrence_with_suffix(
        &self,
        ordinal: u32,
        at: OccurrenceInstant,
        suffix: &str,
    ) -> Result<RecurrenceOccurrence, RecurrenceRefusal> {
        let identity = format!("{}/occurrence/{ordinal}{suffix}", self.identity);
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
            RecurrenceWindow::Wall(window) => {
                let start = window.start();
                let end = window.end();
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
            RecurrenceWindow::Monotonic(window) => {
                let start = window.start();
                let end = window.end();
                ensure_same_monotonic_clock(start, end)?;
                if start.ticks() > end.ticks() {
                    return Err(RecurrenceRefusal::InvalidWindow);
                }
            }
        }
        Ok(())
    }
}

impl RecurrenceOccurrence {
    pub fn validate(&self) -> Result<(), RecurrenceRefusal> {
        validate_identity(&self.identity)?;
        validate_identity(&self.recurrence_identity)?;
        match &self.at {
            OccurrenceInstant::Wall(value) => value
                .value()
                .validate()
                .map_err(|_| RecurrenceRefusal::InvalidRule),
            OccurrenceInstant::Monotonic(value) => value
                .value()
                .validate()
                .map_err(|_| RecurrenceRefusal::InvalidRule),
            OccurrenceInstant::Civil(value) => {
                let local = value.local();
                let zone = value.zone();
                let instant = value.instant();
                local
                    .validate()
                    .map_err(|_| RecurrenceRefusal::InvalidCivilResolution)?;
                zone.validate()
                    .map_err(|_| RecurrenceRefusal::InvalidCivilResolution)?;
                instant
                    .validate()
                    .map_err(|_| RecurrenceRefusal::InvalidCivilResolution)
            }
        }
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

fn wall_comparable(
    left: &TemporalInstant,
    right: &TemporalInstant,
) -> Result<(), RecurrenceRefusal> {
    if left.clock_basis != right.clock_basis || left.scale != right.scale {
        Err(RecurrenceRefusal::IncomparableWindow)
    } else {
        Ok(())
    }
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

pub(crate) const fn date_key(date: LocalDate) -> (i32, u8, u8) {
    (date.year(), date.month(), date.day())
}
