//! Exact offset-only civil conversion without ambient timezone rules.

use alloc::string::String;

use crate::{
    CivilTimeRefusal, LocalDate, LocalDateTime, LocalTime, TemporalInstant, TemporalScale,
    UtcOffsetSeconds,
};

pub const UNIX_UTC_CLOCK_BASIS: &str = "time/unix-utc@1";

const NANOS_PER_SECOND: i128 = 1_000_000_000;
const SECONDS_PER_DAY: i128 = 86_400;
const UNIX_EPOCH_DAY_OFFSET: i128 = 719_468;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CivilConversionRefusal {
    InvalidCivilTime(CivilTimeRefusal),
    InvalidInstant,
    WrongClockBasis,
    UncertainInstant,
    Inexact,
    BeforeUnixEpoch,
    Overflow,
}

impl LocalDateTime {
    pub fn to_offset_instant(
        self,
        offset: UtcOffsetSeconds,
        scale: TemporalScale,
    ) -> Result<TemporalInstant, CivilConversionRefusal> {
        self.validate()
            .map_err(CivilConversionRefusal::InvalidCivilTime)?;
        offset
            .validate()
            .map_err(CivilConversionRefusal::InvalidCivilTime)?;

        let local_seconds = days_from_civil(self.date)
            .checked_mul(SECONDS_PER_DAY)
            .and_then(|value| {
                value.checked_add(
                    i128::from(self.time.hour()) * 3_600
                        + i128::from(self.time.minute()) * 60
                        + i128::from(self.time.second()),
                )
            })
            .ok_or(CivilConversionRefusal::Overflow)?;
        let utc_seconds = local_seconds
            .checked_sub(i128::from(offset.seconds()))
            .ok_or(CivilConversionRefusal::Overflow)?;
        let utc_nanos = utc_seconds
            .checked_mul(NANOS_PER_SECOND)
            .and_then(|value| value.checked_add(i128::from(self.time.nanosecond())))
            .ok_or(CivilConversionRefusal::Overflow)?;
        if utc_nanos < 0 {
            return Err(CivilConversionRefusal::BeforeUnixEpoch);
        }

        let factor = nanos_per_tick(scale);
        if utc_nanos % factor != 0 {
            return Err(CivilConversionRefusal::Inexact);
        }
        let ticks =
            u64::try_from(utc_nanos / factor).map_err(|_| CivilConversionRefusal::Overflow)?;
        Ok(TemporalInstant {
            ticks,
            scale,
            clock_basis: String::from(UNIX_UTC_CLOCK_BASIS),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        })
    }

    pub fn from_offset_instant(
        instant: &TemporalInstant,
        offset: UtcOffsetSeconds,
    ) -> Result<Self, CivilConversionRefusal> {
        instant
            .validate()
            .map_err(|_| CivilConversionRefusal::InvalidInstant)?;
        offset
            .validate()
            .map_err(CivilConversionRefusal::InvalidCivilTime)?;
        if instant.clock_basis != UNIX_UTC_CLOCK_BASIS {
            return Err(CivilConversionRefusal::WrongClockBasis);
        }
        if instant.uncertainty_ticks != 0 {
            return Err(CivilConversionRefusal::UncertainInstant);
        }

        let utc_nanos = i128::from(instant.ticks)
            .checked_mul(nanos_per_tick(instant.scale))
            .ok_or(CivilConversionRefusal::Overflow)?;
        let local_nanos = utc_nanos
            .checked_add(i128::from(offset.seconds()) * NANOS_PER_SECOND)
            .ok_or(CivilConversionRefusal::Overflow)?;
        let local_seconds = local_nanos.div_euclid(NANOS_PER_SECOND);
        let nanosecond = u32::try_from(local_nanos.rem_euclid(NANOS_PER_SECOND))
            .map_err(|_| CivilConversionRefusal::Overflow)?;
        let days = local_seconds.div_euclid(SECONDS_PER_DAY);
        let seconds_in_day = local_seconds.rem_euclid(SECONDS_PER_DAY);
        let date = civil_from_days(days)?;
        let hour =
            u8::try_from(seconds_in_day / 3_600).map_err(|_| CivilConversionRefusal::Overflow)?;
        let minute = u8::try_from((seconds_in_day % 3_600) / 60)
            .map_err(|_| CivilConversionRefusal::Overflow)?;
        let second =
            u8::try_from(seconds_in_day % 60).map_err(|_| CivilConversionRefusal::Overflow)?;
        let time = LocalTime::new(hour, minute, second, nanosecond)
            .map_err(CivilConversionRefusal::InvalidCivilTime)?;
        Ok(Self::new(date, time))
    }
}

const fn nanos_per_tick(scale: TemporalScale) -> i128 {
    match scale {
        TemporalScale::Seconds => NANOS_PER_SECOND,
        TemporalScale::Milliseconds => 1_000_000,
        TemporalScale::Microseconds => 1_000,
        TemporalScale::Nanoseconds => 1,
    }
}

fn days_from_civil(date: LocalDate) -> i128 {
    let month = i128::from(date.month());
    let year = i128::from(date.year()) - i128::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + i128::from(date.day()) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - UNIX_EPOCH_DAY_OFFSET
}

fn civil_from_days(days: i128) -> Result<LocalDate, CivilConversionRefusal> {
    let zero_day = days
        .checked_add(UNIX_EPOCH_DAY_OFFSET)
        .ok_or(CivilConversionRefusal::Overflow)?;
    let era = zero_day.div_euclid(146_097);
    let day_of_era = zero_day - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year_of_era + era * 400 + i128::from(month <= 2);
    LocalDate::new(
        i32::try_from(year).map_err(|_| CivilConversionRefusal::Overflow)?,
        u8::try_from(month).map_err(|_| CivilConversionRefusal::Overflow)?,
        u8::try_from(day).map_err(|_| CivilConversionRefusal::Overflow)?,
    )
    .map_err(CivilConversionRefusal::InvalidCivilTime)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date_time(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        nanosecond: u32,
    ) -> LocalDateTime {
        LocalDateTime::new(
            LocalDate::new(year, month, day).unwrap(),
            LocalTime::new(hour, minute, second, nanosecond).unwrap(),
        )
    }

    #[test]
    fn utc_epoch_and_explicit_offset_name_the_same_instant() {
        let utc = date_time(1970, 1, 1, 0, 0, 0, 0)
            .to_offset_instant(UtcOffsetSeconds::new(0).unwrap(), TemporalScale::Seconds)
            .unwrap();
        let local = date_time(1970, 1, 1, 1, 0, 0, 0)
            .to_offset_instant(
                UtcOffsetSeconds::new(3_600).unwrap(),
                TemporalScale::Seconds,
            )
            .unwrap();
        assert_eq!(utc, local);
        assert_eq!(utc.ticks, 0);
        assert_eq!(utc.clock_basis, UNIX_UTC_CLOCK_BASIS);
    }

    #[test]
    fn leap_day_and_offset_date_crossing_round_trip() {
        let source = date_time(2024, 2, 29, 0, 15, 30, 123_456_000);
        let offset = UtcOffsetSeconds::new(5_400).unwrap();
        let instant = source
            .to_offset_instant(offset, TemporalScale::Microseconds)
            .unwrap();
        assert_eq!(
            LocalDateTime::from_offset_instant(&instant, offset),
            Ok(source)
        );
        let previous_date = LocalDateTime::from_offset_instant(
            &date_time(1970, 1, 1, 0, 0, 0, 0)
                .to_offset_instant(UtcOffsetSeconds::new(0).unwrap(), TemporalScale::Seconds)
                .unwrap(),
            UtcOffsetSeconds::new(-3_600).unwrap(),
        )
        .unwrap();
        assert_eq!(previous_date, date_time(1969, 12, 31, 23, 0, 0, 0));
    }

    #[test]
    fn conversion_refuses_precision_loss_and_pre_epoch_instants() {
        assert_eq!(
            date_time(1970, 1, 1, 0, 0, 0, 1)
                .to_offset_instant(UtcOffsetSeconds::new(0).unwrap(), TemporalScale::Seconds),
            Err(CivilConversionRefusal::Inexact)
        );
        assert_eq!(
            date_time(1969, 12, 31, 23, 59, 59, 999_999_999).to_offset_instant(
                UtcOffsetSeconds::new(0).unwrap(),
                TemporalScale::Nanoseconds,
            ),
            Err(CivilConversionRefusal::BeforeUnixEpoch)
        );
    }

    #[test]
    fn reverse_conversion_requires_exact_unix_utc_truth() {
        let mut instant = date_time(2024, 1, 1, 0, 0, 0, 0)
            .to_offset_instant(UtcOffsetSeconds::new(0).unwrap(), TemporalScale::Seconds)
            .unwrap();
        instant.uncertainty_ticks = 1;
        assert_eq!(
            LocalDateTime::from_offset_instant(&instant, UtcOffsetSeconds::new(0).unwrap()),
            Err(CivilConversionRefusal::UncertainInstant)
        );
        instant.uncertainty_ticks = 0;
        instant.clock_basis = String::from("clock/other@1");
        assert_eq!(
            LocalDateTime::from_offset_instant(&instant, UtcOffsetSeconds::new(0).unwrap()),
            Err(CivilConversionRefusal::WrongClockBasis)
        );
    }

    #[test]
    fn nanosecond_scale_refuses_unsigned_tick_overflow() {
        assert_eq!(
            date_time(2554, 7, 22, 0, 0, 0, 0).to_offset_instant(
                UtcOffsetSeconds::new(0).unwrap(),
                TemporalScale::Nanoseconds,
            ),
            Err(CivilConversionRefusal::Overflow)
        );
    }
}
