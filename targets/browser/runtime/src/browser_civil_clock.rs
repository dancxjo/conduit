//! Bounded browser civil clock replies.

use conduit_core::{LocalDate, LocalDateTime, LocalTime, TemporalScale, UtcOffsetSeconds};

pub const CIVIL_CLOCK_OPERATION: &str = "conduit.host/browser-civil-clock@1";
pub(crate) const CIVIL_CLOCK_RESOURCE: &str = "conduit.resource/browser-civil-clock-slot@1";
pub(crate) const CIVIL_CLOCK_POOL: &str = "browser/civil-clock";
pub const CIVIL_CLOCK_REPLY_BYTES: usize = 28;

#[derive(Debug, PartialEq, Eq)]
pub struct CivilClockReply<'a> {
    pub observed_at_unix_ms: u64,
    pub utc_offset_minutes: i16,
    pub local_date: &'a str,
    pub local_time: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CivilClockRefusal {
    Length,
    Timestamp,
    Offset,
    CivilLabel,
    InconsistentInstant,
}

impl<'a> CivilClockReply<'a> {
    pub fn decode(bytes: &'a [u8]) -> Result<Self, CivilClockRefusal> {
        let bytes: &[u8; CIVIL_CLOCK_REPLY_BYTES] =
            bytes.try_into().map_err(|_| CivilClockRefusal::Length)?;
        let observed_at_unix_ms = u64::from_le_bytes(bytes[..8].try_into().unwrap());
        if observed_at_unix_ms > 253_402_300_799_999 {
            return Err(CivilClockRefusal::Timestamp);
        }
        let utc_offset_minutes = i16::from_le_bytes([bytes[8], bytes[9]]);
        if !(-840..=840).contains(&utc_offset_minutes) {
            return Err(CivilClockRefusal::Offset);
        }
        let offset = UtcOffsetSeconds::new(i32::from(utc_offset_minutes) * 60)
            .map_err(|_| CivilClockRefusal::Offset)?;
        let local_date = label(&bytes[10..20], b'-', [4, 7])?;
        let local_time = label(&bytes[20..], b':', [2, 5])?;
        let date = LocalDate::new(
            number(&local_date[..4])?,
            number(&local_date[5..7])?,
            number(&local_date[8..])?,
        )
        .map_err(|_| CivilClockRefusal::CivilLabel)?;
        let time = LocalTime::new(
            number(&local_time[..2])?,
            number(&local_time[3..5])?,
            number(&local_time[6..])?,
            0,
        )
        .map_err(|_| CivilClockRefusal::CivilLabel)?;
        let instant = LocalDateTime::new(date, time)
            .to_offset_instant(offset, TemporalScale::Seconds)
            .map_err(|_| CivilClockRefusal::InconsistentInstant)?;
        if instant.ticks != observed_at_unix_ms / 1000 {
            return Err(CivilClockRefusal::InconsistentInstant);
        }
        Ok(Self {
            observed_at_unix_ms,
            utc_offset_minutes,
            local_date,
            local_time,
        })
    }
}

fn label(bytes: &[u8], separator: u8, positions: [usize; 2]) -> Result<&str, CivilClockRefusal> {
    if !bytes.iter().enumerate().all(|(index, byte)| {
        if positions.contains(&index) {
            *byte == separator
        } else {
            byte.is_ascii_digit()
        }
    }) {
        return Err(CivilClockRefusal::CivilLabel);
    }
    core::str::from_utf8(bytes).map_err(|_| CivilClockRefusal::CivilLabel)
}

fn number<T: core::str::FromStr>(digits: &str) -> Result<T, CivilClockRefusal> {
    digits.parse().map_err(|_| CivilClockRefusal::CivilLabel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fold_reply() -> [u8; CIVIL_CLOCK_REPLY_BYTES] {
        let mut bytes = [0; CIVIL_CLOCK_REPLY_BYTES];
        bytes[..8].copy_from_slice(&1_793_523_600_876_u64.to_le_bytes());
        bytes[8..10].copy_from_slice(&(-480_i16).to_le_bytes());
        bytes[10..].copy_from_slice(b"2026-11-0101:00:00");
        bytes
    }

    #[test]
    fn decodes_the_browser_fold_reply() {
        assert_eq!(
            CivilClockReply::decode(&fold_reply()),
            Ok(CivilClockReply {
                observed_at_unix_ms: 1_793_523_600_876,
                utc_offset_minutes: -480,
                local_date: "2026-11-01",
                local_time: "01:00:00",
            }),
        );
    }

    #[test]
    fn refuses_wrong_extents_and_inconsistent_civil_values() {
        let mut bytes = fold_reply();
        assert_eq!(
            CivilClockReply::decode(&bytes[..27]),
            Err(CivilClockRefusal::Length)
        );
        bytes[8..10].copy_from_slice(&(-420_i16).to_le_bytes());
        assert_eq!(
            CivilClockReply::decode(&bytes),
            Err(CivilClockRefusal::InconsistentInstant)
        );
        bytes[10..20].copy_from_slice(b"2026-02-29");
        assert_eq!(
            CivilClockReply::decode(&bytes),
            Err(CivilClockRefusal::CivilLabel)
        );
        bytes[8..10].copy_from_slice(&841_i16.to_le_bytes());
        assert_eq!(
            CivilClockReply::decode(&bytes),
            Err(CivilClockRefusal::Offset)
        );
    }
}
