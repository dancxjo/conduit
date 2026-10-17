//! Finite physical duration conversion; no Every/Count semantics or provider authority.
use crate::machine::BaseError;

pub const MAXIMUM_MILLISECONDS: u64 = 1000;
pub const PIT_FREQUENCY_HZ: u64 = 1_193_182;

/// Round upward so a representable physical count cannot fire before the
/// requested duration. Frequency is an exact numerator/denominator in Hertz.
pub fn duration_ticks(
    milliseconds: u64,
    numerator: u64,
    denominator: u64,
) -> Result<u64, BaseError> {
    if !(1..=MAXIMUM_MILLISECONDS).contains(&milliseconds) || numerator == 0 || denominator == 0 {
        return Err(BaseError::Unavailable);
    }
    let scale = u128::from(denominator) * 1000;
    let ticks = (u128::from(numerator) * u128::from(milliseconds)).div_ceil(scale);
    u64::try_from(ticks)
        .ok()
        .filter(|ticks| *ticks != 0)
        .ok_or(BaseError::Unavailable)
}

#[derive(Clone, Copy, Default)]
pub struct PitDuration {
    remaining: u64,
}

impl PitDuration {
    pub fn new(milliseconds: u64) -> Result<Self, BaseError> {
        Ok(Self {
            remaining: duration_ticks(milliseconds, PIT_FREQUENCY_HZ, 1)?,
        })
    }

    /// The PIT has a sixteen-bit count. Intermediate IRQs rearm this same
    /// logical timer; only the final chunk may publish its matching wake.
    pub fn next_count(&mut self) -> Option<u16> {
        if self.remaining == 0 {
            return None;
        }
        let count = self.remaining.min(u64::from(u16::MAX)) as u16;
        self.remaining -= u64::from(count);
        Some(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tour_timer_duration_rounds_up_and_preserves_exact_frequency_ratios() {
        assert_eq!(duration_ticks(120, 10_000_000, 1), Ok(1_200_000));
        assert_eq!(duration_ticks(120, PIT_FREQUENCY_HZ, 1), Ok(143_182));
        assert_eq!(duration_ticks(1, 1001, 1), Ok(2));
        assert_eq!(duration_ticks(1, 3001, 3), Ok(2));
        assert_eq!(duration_ticks(1000, u64::MAX, 1), Ok(u64::MAX));
        for parameters in [
            (0, 1, 1),
            (1001, 1, 1),
            (u64::MAX, 1, 1),
            (1, 0, 1),
            (1, 1, 0),
        ] {
            assert_eq!(
                duration_ticks(parameters.0, parameters.1, parameters.2),
                Err(BaseError::Unavailable)
            );
        }
    }

    #[test]
    fn tour_timer_pit_chunks_cover_the_complete_duration_before_final_wake() {
        let mut duration = PitDuration::new(120).unwrap();
        assert_eq!(duration.next_count(), Some(65_535));
        assert_eq!(duration.next_count(), Some(65_535));
        assert_eq!(duration.next_count(), Some(12_112));
        assert_eq!(duration.next_count(), None);
        for milliseconds in 1..=MAXIMUM_MILLISECONDS {
            let mut duration = PitDuration::new(milliseconds).unwrap();
            let mut sum = 0;
            let mut count = 0;
            while let Some(ticks) = duration.next_count() {
                assert_ne!(ticks, 0);
                sum += u64::from(ticks);
                count += 1;
            }
            assert_eq!(
                sum,
                duration_ticks(milliseconds, PIT_FREQUENCY_HZ, 1).unwrap()
            );
            assert!(count <= 19);
        }
    }
}
