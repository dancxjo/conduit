//! Pure civil projection of an explicitly supplied clock observation.
//! Sampling a clock remains an admitted Host Call. Timezone preparation uses
//! immutable compiled IANA rules and needs no filesystem or process authority.
use chrono::{Datelike, Offset, TimeZone, Timelike};

mod snapshot;
pub use snapshot::PreparedAmbientClockWeather;
mod wire;
pub use wire::PreparedAmbientClockWire;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbientClockRefusal {
    UnknownTimeZone,
    TimestampOutOfRange,
    UnrepresentableOffset,
    NativeContract,
}

#[derive(Debug, Clone, Copy)]
pub struct PreparedAmbientClock {
    timezone: chrono_tz::Tz,
}

/// Fixed storage for one projection. The adapter can copy these labels into
/// its separately admitted typed observation and expose the exact rule set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmbientClockReading {
    date: [u8; 10],
    time: [u8; 8],
    pub timezone: &'static str,
    pub rule_set: &'static str,
    pub utc_offset_minutes: i64,
    pub observed_at_unix_ms: u64,
}

impl PreparedAmbientClock {
    pub fn prepare(timezone: &str) -> Result<Self, AmbientClockRefusal> {
        if timezone.is_empty() || timezone.len() > 64 {
            return Err(AmbientClockRefusal::UnknownTimeZone);
        }
        Ok(Self {
            timezone: timezone
                .parse()
                .map_err(|_| AmbientClockRefusal::UnknownTimeZone)?,
        })
    }

    pub fn project(&self, unix_millis: u64) -> Result<AmbientClockReading, AmbientClockRefusal> {
        let timestamp =
            i64::try_from(unix_millis).map_err(|_| AmbientClockRefusal::TimestampOutOfRange)?;
        let local = self
            .timezone
            .timestamp_millis_opt(timestamp)
            .single()
            .ok_or(AmbientClockRefusal::TimestampOutOfRange)?;
        let year = local.year();
        if !(0..=9999).contains(&year) {
            return Err(AmbientClockRefusal::TimestampOutOfRange);
        }
        let offset_seconds = local.offset().fix().local_minus_utc();
        if offset_seconds % 60 != 0 || !(-840 * 60..=840 * 60).contains(&offset_seconds) {
            return Err(AmbientClockRefusal::UnrepresentableOffset);
        }
        let mut date = *b"0000-00-00";
        decimal(&mut date[..4], year as u32);
        decimal(&mut date[5..7], local.month());
        decimal(&mut date[8..], local.day());
        let mut time = *b"00:00:00";
        decimal(&mut time[..2], local.hour());
        decimal(&mut time[3..5], local.minute());
        decimal(&mut time[6..], local.second());
        Ok(AmbientClockReading {
            date,
            time,
            timezone: self.timezone.name(),
            rule_set: chrono_tz::IANA_TZDB_VERSION,
            utc_offset_minutes: i64::from(offset_seconds / 60),
            observed_at_unix_ms: unix_millis,
        })
    }
}

impl AmbientClockReading {
    pub fn local_date(&self) -> &str {
        core::str::from_utf8(&self.date).expect("fixed ASCII civil date")
    }

    pub fn local_time(&self) -> &str {
        core::str::from_utf8(&self.time).expect("fixed ASCII civil time")
    }
}

fn decimal(output: &mut [u8], mut value: u32) {
    for digit in output.iter_mut().rev() {
        *digit = b'0' + (value % 10) as u8;
        value /= 10;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> u64 {
        chrono::Utc
            .with_ymd_and_hms(year, month, day, hour, minute, second)
            .single()
            .unwrap()
            .timestamp_millis() as u64
    }

    #[test]
    fn seattle_spring_gap_and_autumn_fold_keep_exact_offsets() {
        let clock = PreparedAmbientClock::prepare("America/Los_Angeles").unwrap();
        for (timestamp, date, time, offset) in [
            (utc(2026, 3, 8, 9, 59, 59), "2026-03-08", "01:59:59", -480),
            (utc(2026, 3, 8, 10, 0, 0), "2026-03-08", "03:00:00", -420),
            (utc(2026, 11, 1, 8, 59, 59), "2026-11-01", "01:59:59", -420),
            (utc(2026, 11, 1, 9, 0, 0), "2026-11-01", "01:00:00", -480),
        ] {
            let reading = clock.project(timestamp).unwrap();
            assert_eq!(reading.local_date(), date);
            assert_eq!(reading.local_time(), time);
            assert_eq!(reading.utc_offset_minutes, offset);
            assert_eq!(reading.observed_at_unix_ms, timestamp);
            assert_eq!(reading.timezone, "America/Los_Angeles");
            assert_eq!(reading.rule_set, chrono_tz::IANA_TZDB_VERSION);
        }
    }

    #[test]
    fn local_date_rollover_and_leap_day_follow_the_selected_zone() {
        let clock = PreparedAmbientClock::prepare("America/Los_Angeles").unwrap();
        let before = clock.project(utc(2024, 3, 1, 7, 59, 59)).unwrap();
        let after = clock.project(utc(2024, 3, 1, 8, 0, 0)).unwrap();
        assert_eq!(
            (before.local_date(), before.local_time()),
            ("2024-02-29", "23:59:59")
        );
        assert_eq!(
            (after.local_date(), after.local_time()),
            ("2024-03-01", "00:00:00")
        );
    }

    #[test]
    fn unknown_zones_and_unrepresentable_instants_refuse() {
        for zone in ["", "Seattle", "../etc/localtime"] {
            assert_eq!(
                PreparedAmbientClock::prepare(zone).unwrap_err(),
                AmbientClockRefusal::UnknownTimeZone
            );
        }
        let clock = PreparedAmbientClock::prepare("UTC").unwrap();
        assert_eq!(
            clock.project(u64::MAX),
            Err(AmbientClockRefusal::TimestampOutOfRange)
        );
        assert_eq!(
            clock.project(utc(10000, 1, 1, 0, 0, 0)),
            Err(AmbientClockRefusal::TimestampOutOfRange)
        );
    }
}
