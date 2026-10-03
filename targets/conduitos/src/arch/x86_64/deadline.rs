//! A candidate deadline is admitted only when x86 reports an invariant TSC
//! with a numeric frequency. Raw TSC ticks are not milliseconds.

use core::arch::x86_64::__cpuid;

use super::cpu::{feature_basis, read_tsc};

#[derive(Clone, Copy)]
pub struct CandidateDeadline {
    start_ticks: u64,
    end_ticks: u64,
    ticks_per_millisecond: u64,
}

impl CandidateDeadline {
    pub fn admit(timeout_millis: u32) -> Option<Self> {
        if !feature_basis().invariant_tsc || __cpuid(0).eax < 0x15 {
            return None;
        }
        let frequency = __cpuid(0x15);
        let ticks_per_millisecond =
            calibrated_ticks_per_millisecond(frequency.eax, frequency.ebx, frequency.ecx)?;
        let start_ticks = read_tsc();
        let end_ticks = start_ticks
            .checked_add(ticks_per_millisecond.checked_mul(u64::from(timeout_millis))?)?;
        Some(Self {
            start_ticks,
            end_ticks,
            ticks_per_millisecond,
        })
    }

    pub fn elapsed_millis(self) -> Option<i64> {
        self.elapsed_at(read_tsc())
    }

    fn elapsed_at(self, observed: u64) -> Option<i64> {
        if observed >= self.end_ticks || observed < self.start_ticks {
            return None;
        }
        i64::try_from((observed - self.start_ticks) / self.ticks_per_millisecond).ok()
    }
}

fn calibrated_ticks_per_millisecond(
    denominator: u32,
    numerator: u32,
    crystal_hz: u32,
) -> Option<u64> {
    if denominator == 0 || numerator == 0 || crystal_hz == 0 {
        return None;
    }
    let ticks_per_millisecond = u64::from(crystal_hz)
        .checked_mul(u64::from(numerator))?
        .checked_div(u64::from(denominator))?
        .checked_div(1_000)?;
    (ticks_per_millisecond > 0).then_some(ticks_per_millisecond)
}

#[cfg(test)]
mod tests {
    use super::{CandidateDeadline, calibrated_ticks_per_millisecond};

    #[test]
    fn frequency_requires_exact_nonzero_cpuid_ratio() {
        assert_eq!(
            calibrated_ticks_per_millisecond(1, 100, 24_000_000),
            Some(2_400_000)
        );
        assert_eq!(calibrated_ticks_per_millisecond(0, 100, 24_000_000), None);
        assert_eq!(calibrated_ticks_per_millisecond(1, 100, 0), None);
    }

    #[test]
    fn elapsed_deadline_refuses_at_limit_and_counter_regression() {
        let deadline = CandidateDeadline {
            start_ticks: 1_000,
            end_ticks: 3_000,
            ticks_per_millisecond: 100,
        };
        assert_eq!(deadline.elapsed_at(1_000), Some(0));
        assert_eq!(deadline.elapsed_at(2_999), Some(19));
        assert_eq!(deadline.elapsed_at(3_000), None);
        assert_eq!(deadline.elapsed_at(999), None);
    }
}
