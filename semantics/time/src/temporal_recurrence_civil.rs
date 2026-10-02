//! Bounded civil recurrence expansion over explicit timezone resolution truth.

use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    date_key, CivilFoldPolicy, CivilGapPolicy, CivilResolutionChoice, CivilResolutionPolicy,
    LocalDate, LocalDateTime, OccurrenceInstant, RecurrenceDefinition, RecurrenceExpansion,
    RecurrenceOccurrence, RecurrenceRefusal, RecurrenceRule, RecurrenceUntil, RecurrenceWindow,
    TemporalInstant, WeekdaySet, ZonedResolution,
};

pub const MAXIMUM_CIVIL_RECURRENCE_SCAN_DAYS: u32 = 36_600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CivilOccurrenceResolution {
    pub ordinal: u32,
    pub resolution: ZonedResolution,
}

impl RecurrenceDefinition {
    pub fn expand_civil(
        &self,
        request: &RecurrenceExpansion,
        resolutions: &[CivilOccurrenceResolution],
        policy: CivilResolutionPolicy,
    ) -> Result<Vec<RecurrenceOccurrence>, RecurrenceRefusal> {
        self.validate()?;
        request.validate()?;
        let RecurrenceRule::CivilWeekdays(rule) = &self.rule else {
            return Err(RecurrenceRefusal::WrongWindowKind);
        };
        let first_date = rule.first_date();
        let local_time = rule.local_time();
        let zone = rule.zone();
        let weekdays = rule.weekdays();
        let excluded_dates = rule.excluded_dates();
        let RecurrenceWindow::Wall(window) = &request.window else {
            return Err(RecurrenceRefusal::WrongWindowKind);
        };
        let start = window.start();
        let end = window.end();
        validate_resolution_set(resolutions, self.maximum_occurrences)?;

        let mut occurrences = Vec::with_capacity(request.maximum_results as usize);
        let mut date = *first_date;
        let mut scanned_days = 0_u32;
        for ordinal in 0..self.maximum_occurrences {
            date = next_selected_date(date, *weekdays, &mut scanned_days)?;
            if self.until.as_ref().is_some_and(|until| {
                matches!(until, RecurrenceUntil::CivilDate(value) if date_key(date) > date_key(*value.value()))
            }) {
                break;
            }
            let date_is_excluded = excluded_dates
                .binary_search_by_key(&date_key(date), |candidate| date_key(*candidate))
                .is_ok();
            if self.excluded_ordinals.binary_search(&ordinal).is_err() && !date_is_excluded {
                let supplied = find_resolution(resolutions, ordinal)?;
                let local = LocalDateTime::new(date, *local_time)
                    .map_err(|_| RecurrenceRefusal::InvalidRule)?;
                let choices = choose_resolution(supplied, &local, zone, policy)?;
                for (instant, choice, suffix) in choices {
                    if wall_in_window(&instant, start, end)? {
                        if occurrences.len() == request.maximum_results as usize {
                            return Err(RecurrenceRefusal::WorkLimitExceeded);
                        }
                        occurrences.push(
                            self.occurrence_with_suffix(
                                ordinal,
                                OccurrenceInstant::civil(
                                    instant,
                                    local.clone(),
                                    choice,
                                    zone.clone(),
                                )
                                .map_err(|_| RecurrenceRefusal::InvalidCivilResolution)?,
                                suffix,
                            )?,
                        );
                    }
                }
            }
            date = next_date(date)?;
            scanned_days = scanned_days
                .checked_add(1)
                .ok_or(RecurrenceRefusal::ArithmeticOverflow)?;
        }
        Ok(occurrences)
    }
}

fn validate_resolution_set(
    resolutions: &[CivilOccurrenceResolution],
    maximum_occurrences: u32,
) -> Result<(), RecurrenceRefusal> {
    if resolutions.len() > maximum_occurrences as usize
        || resolutions.iter().any(|entry| {
            entry.ordinal >= maximum_occurrences || entry.resolution.validate().is_err()
        })
        || resolutions
            .windows(2)
            .any(|pair| pair[0].ordinal >= pair[1].ordinal)
    {
        return Err(RecurrenceRefusal::InvalidCivilResolution);
    }
    Ok(())
}

fn find_resolution(
    resolutions: &[CivilOccurrenceResolution],
    ordinal: u32,
) -> Result<&ZonedResolution, RecurrenceRefusal> {
    resolutions
        .binary_search_by_key(&ordinal, |entry| entry.ordinal)
        .ok()
        .map(|index| &resolutions[index].resolution)
        .ok_or(RecurrenceRefusal::CivilResolutionRequired)
}

type ChosenResolution<'a> = (TemporalInstant, CivilResolutionChoice, &'a str);

fn choose_resolution<'a>(
    resolution: &ZonedResolution,
    expected_local: &LocalDateTime,
    expected_zone: &crate::NamedTimeZone,
    policy: CivilResolutionPolicy,
) -> Result<Vec<ChosenResolution<'a>>, RecurrenceRefusal> {
    let (local, zone) = match resolution {
        ZonedResolution::Unique { local, zone, .. }
        | ZonedResolution::Ambiguous { local, zone, .. }
        | ZonedResolution::Nonexistent { local, zone, .. } => (local, zone),
    };
    let local =
        LocalDateTime::try_from(*local).map_err(|_| RecurrenceRefusal::InvalidCivilResolution)?;
    let zone = crate::NamedTimeZone::try_from(zone.clone())
        .map_err(|_| RecurrenceRefusal::InvalidCivilResolution)?;
    if &local != expected_local || &zone != expected_zone {
        return Err(RecurrenceRefusal::CivilResolutionMismatch);
    }
    let mut selected = Vec::with_capacity(2);
    match resolution {
        ZonedResolution::Unique { instant, .. } => {
            selected.push((native_instant(instant)?, CivilResolutionChoice::Unique, ""));
        }
        ZonedResolution::Ambiguous { earlier, later, .. } => match policy.fold() {
            CivilFoldPolicy::Earlier => selected.push((
                native_instant(earlier)?,
                CivilResolutionChoice::FoldEarlier,
                "/fold/earlier",
            )),
            CivilFoldPolicy::Later => selected.push((
                native_instant(later)?,
                CivilResolutionChoice::FoldLater,
                "/fold/later",
            )),
            CivilFoldPolicy::Both => {
                selected.push((
                    native_instant(earlier)?,
                    CivilResolutionChoice::FoldEarlier,
                    "/fold/earlier",
                ));
                selected.push((
                    native_instant(later)?,
                    CivilResolutionChoice::FoldLater,
                    "/fold/later",
                ));
            }
            CivilFoldPolicy::Refuse => return Err(RecurrenceRefusal::CivilResolutionRequired),
        },
        ZonedResolution::Nonexistent {
            gap_before,
            gap_after,
            ..
        } => match policy.gap() {
            CivilGapPolicy::Skip => {}
            CivilGapPolicy::UseBefore => selected.push((
                native_instant(gap_before)?,
                CivilResolutionChoice::GapBefore,
                "/gap/before",
            )),
            CivilGapPolicy::UseAfter => selected.push((
                native_instant(gap_after)?,
                CivilResolutionChoice::GapAfter,
                "/gap/after",
            )),
            CivilGapPolicy::Refuse => return Err(RecurrenceRefusal::CivilResolutionRequired),
        },
    }
    Ok(selected)
}

fn native_instant(
    value: &conduit_core::TemporalInstant,
) -> Result<TemporalInstant, RecurrenceRefusal> {
    TemporalInstant::try_from(value.clone()).map_err(|_| RecurrenceRefusal::InvalidCivilResolution)
}

fn wall_in_window(
    instant: &TemporalInstant,
    start: &TemporalInstant,
    end: &TemporalInstant,
) -> Result<bool, RecurrenceRefusal> {
    if instant.clock_basis != start.clock_basis
        || instant.clock_basis != end.clock_basis
        || instant.scale != start.scale
        || instant.scale != end.scale
    {
        return Err(RecurrenceRefusal::IncomparableWindow);
    }
    if instant.uncertainty_ticks != 0 {
        return Err(RecurrenceRefusal::InvalidRule);
    }
    Ok(instant.ticks >= start.ticks && instant.ticks <= end.ticks)
}

fn next_selected_date(
    mut date: LocalDate,
    weekdays: WeekdaySet,
    scanned_days: &mut u32,
) -> Result<LocalDate, RecurrenceRefusal> {
    while !weekdays.contains(weekday(date)) {
        if *scanned_days == MAXIMUM_CIVIL_RECURRENCE_SCAN_DAYS {
            return Err(RecurrenceRefusal::WorkLimitExceeded);
        }
        date = next_date(date)?;
        *scanned_days += 1;
    }
    Ok(date)
}

fn next_date(date: LocalDate) -> Result<LocalDate, RecurrenceRefusal> {
    let (year, month, day) = (date.year(), date.month(), date.day());
    if let Ok(next) = LocalDate::new(year, month, day.saturating_add(1)) {
        return Ok(next);
    }
    if month < 12 {
        LocalDate::new(year, month + 1, 1).map_err(|_| RecurrenceRefusal::ArithmeticOverflow)
    } else {
        LocalDate::new(
            year.checked_add(1)
                .ok_or(RecurrenceRefusal::ArithmeticOverflow)?,
            1,
            1,
        )
        .map_err(|_| RecurrenceRefusal::ArithmeticOverflow)
    }
}

pub(crate) fn weekday(date: LocalDate) -> WeekdaySet {
    let mut year = i64::from(date.year());
    let month = i64::from(date.month());
    let day = i64::from(date.day());
    year -= i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_number = era * 146_097 + year_of_era * 365 + year_of_era / 4 - year_of_era / 100
        + day_of_year
        - 719_468;
    match (day_number + 3).rem_euclid(7) {
        0 => WeekdaySet::MONDAY,
        1 => WeekdaySet::TUESDAY,
        2 => WeekdaySet::WEDNESDAY,
        3 => WeekdaySet::THURSDAY,
        4 => WeekdaySet::FRIDAY,
        5 => WeekdaySet::SATURDAY,
        _ => WeekdaySet::SUNDAY,
    }
}
