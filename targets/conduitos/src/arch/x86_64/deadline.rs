//! A candidate deadline needs a hardware counter with a numeric frequency.
//! The invariant TSC's CPUID frequency is preferred; ACPI-discovered HPET
//! supplies the same elapsed-millisecond contract under QEMU TCG.

mod monotonic;
pub use monotonic::{NativeMonotonicDeadlineClock, admitted_monotonic_deadline_clock};

use core::{
    arch::x86_64::__cpuid,
    ptr::{read_volatile, write_volatile},
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
};

use super::cpu::{feature_basis, read_tsc};

const HPET_CAPABILITIES: usize = 0;
const HPET_CONFIGURATION: usize = 0x10;
const HPET_MAIN_COUNTER: usize = 0xf0;
const HPET_COUNTER_64_BIT: u64 = 1 << 13;
const HPET_ENABLE: u64 = 1;
const FEMTOSECONDS_PER_MILLISECOND: u128 = 1_000_000_000_000;
const MAXIMUM_HPET_PERIOD_FS: u32 = 100_000_000;
const HPET_STARTUP_READS: usize = 100_000;

// The address is published last, after the period and observed counter
// progress. Machine initialization runs once before product admission.
static HPET_ADDRESS: AtomicUsize = AtomicUsize::new(0);
static HPET_PERIOD_FS: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy)]
pub enum CandidateDeadline {
    Tsc {
        start_ticks: u64,
        end_ticks: u64,
        ticks_per_millisecond: u64,
    },
    Hpet {
        address: usize,
        start_ticks: u64,
        period_fs: u32,
        timeout_fs: u128,
    },
}

/// Install only a checked ACPI memory-mapped HPET whose 64-bit counter
/// actually advances. Missing/unsupported hardware leaves TSC admission
/// intact and otherwise fails closed at the candidate boundary.
pub(super) fn initialize_hpet(
    physical: Option<u64>,
    hhdm: u64,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> bool {
    let Some(physical) = physical else {
        return false;
    };
    let Ok(address) = super::mmio::map(physical, hhdm, image_virtual_to_physical) else {
        return false;
    };
    let capabilities = unsafe { read_register(address, HPET_CAPABILITIES) };
    let period_fs = (capabilities >> 32) as u32;
    if capabilities & HPET_COUNTER_64_BIT == 0 || !valid_hpet_period(period_fs) {
        return false;
    }
    let configuration = unsafe { read_register(address, HPET_CONFIGURATION) };
    unsafe { write_register(address, HPET_CONFIGURATION, configuration | HPET_ENABLE) };
    let start = unsafe { read_register(address, HPET_MAIN_COUNTER) };
    let mut advanced = false;
    for _ in 0..HPET_STARTUP_READS {
        if unsafe { read_register(address, HPET_MAIN_COUNTER) } > start {
            advanced = true;
            break;
        }
        core::hint::spin_loop();
    }
    if !advanced {
        return false;
    }
    HPET_PERIOD_FS.store(period_fs, Ordering::Relaxed);
    HPET_ADDRESS.store(address, Ordering::Release);
    true
}

impl CandidateDeadline {
    pub fn admit(timeout_millis: u32) -> Option<Self> {
        if timeout_millis == 0 {
            return None;
        }
        if let Some(tsc) = admit_tsc(timeout_millis) {
            return Some(tsc);
        }
        let address = HPET_ADDRESS.load(Ordering::Acquire);
        if address == 0 {
            return None;
        }
        let period_fs = HPET_PERIOD_FS.load(Ordering::Relaxed);
        if !valid_hpet_period(period_fs) {
            return None;
        }
        let start_ticks = unsafe { read_register(address, HPET_MAIN_COUNTER) };
        let timeout_fs = hpet_timeout_fs(start_ticks, period_fs, timeout_millis)?;
        Some(Self::Hpet {
            address,
            start_ticks,
            period_fs,
            timeout_fs,
        })
    }

    pub fn elapsed_millis(self) -> Option<i64> {
        match self {
            Self::Tsc { .. } => self.elapsed_at(read_tsc()),
            Self::Hpet { address, .. } => {
                self.elapsed_at(unsafe { read_register(address, HPET_MAIN_COUNTER) })
            }
        }
    }

    fn elapsed_at(self, observed: u64) -> Option<i64> {
        match self {
            Self::Tsc {
                start_ticks,
                end_ticks,
                ticks_per_millisecond,
            } => {
                if observed >= end_ticks || observed < start_ticks {
                    return None;
                }
                i64::try_from((observed - start_ticks) / ticks_per_millisecond).ok()
            }
            Self::Hpet {
                start_ticks,
                period_fs,
                timeout_fs,
                ..
            } => {
                // Admission proved the counter cannot wrap before this exact
                // deadline. A regressed observation still refuses.
                let ticks = observed.checked_sub(start_ticks)?;
                let elapsed_fs = u128::from(ticks) * u128::from(period_fs);
                (elapsed_fs < timeout_fs)
                    .then(|| i64::try_from(elapsed_fs / FEMTOSECONDS_PER_MILLISECOND).ok())?
            }
        }
    }
}

fn admit_tsc(timeout_millis: u32) -> Option<CandidateDeadline> {
    if !feature_basis().invariant_tsc || __cpuid(0).eax < 0x15 {
        return None;
    }
    let frequency = __cpuid(0x15);
    let ticks_per_millisecond =
        calibrated_ticks_per_millisecond(frequency.eax, frequency.ebx, frequency.ecx)?;
    let start_ticks = read_tsc();
    let end_ticks =
        start_ticks.checked_add(ticks_per_millisecond.checked_mul(u64::from(timeout_millis))?)?;
    Some(CandidateDeadline::Tsc {
        start_ticks,
        end_ticks,
        ticks_per_millisecond,
    })
}

fn valid_hpet_period(period_fs: u32) -> bool {
    (1..=MAXIMUM_HPET_PERIOD_FS).contains(&period_fs)
}

fn hpet_timeout_fs(start_ticks: u64, period_fs: u32, timeout_millis: u32) -> Option<u128> {
    if !valid_hpet_period(period_fs) || timeout_millis == 0 {
        return None;
    }
    let timeout_fs = u128::from(timeout_millis) * FEMTOSECONDS_PER_MILLISECOND;
    let ticks_to_deadline = timeout_fs
        .checked_add(u128::from(period_fs) - 1)?
        .checked_div(u128::from(period_fs))?;
    start_ticks.checked_add(u64::try_from(ticks_to_deadline).ok()?)?;
    Some(timeout_fs)
}

unsafe fn read_register(address: usize, offset: usize) -> u64 {
    unsafe { read_volatile((address + offset) as *const u64) }
}

unsafe fn write_register(address: usize, offset: usize, value: u64) {
    unsafe { write_volatile((address + offset) as *mut u64, value) }
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
    use super::{
        CandidateDeadline, FEMTOSECONDS_PER_MILLISECOND, calibrated_ticks_per_millisecond,
        hpet_timeout_fs, valid_hpet_period,
    };

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
        let deadline = CandidateDeadline::Tsc {
            start_ticks: 1_000,
            end_ticks: 3_000,
            ticks_per_millisecond: 100,
        };
        assert_eq!(deadline.elapsed_at(1_000), Some(0));
        assert_eq!(deadline.elapsed_at(2_999), Some(19));
        assert_eq!(deadline.elapsed_at(3_000), None);
        assert_eq!(deadline.elapsed_at(999), None);
    }

    #[test]
    fn hpet_elapsed_uses_period_and_refuses_at_limit_or_wrap() {
        let deadline = CandidateDeadline::Hpet {
            address: 0,
            start_ticks: 5_000,
            period_fs: 100_000_000,
            timeout_fs: 2_000 * FEMTOSECONDS_PER_MILLISECOND,
        };
        assert_eq!(deadline.elapsed_at(5_000), Some(0));
        assert_eq!(deadline.elapsed_at(5_000 + 19_999_999), Some(1_999));
        assert_eq!(deadline.elapsed_at(5_000 + 20_000_000), None);
        assert_eq!(deadline.elapsed_at(4_999), None);
        assert!(valid_hpet_period(100_000_000));
        assert!(!valid_hpet_period(0));
        assert!(!valid_hpet_period(100_000_001));
        assert_eq!(
            hpet_timeout_fs(0, 100_000_000, 2_000),
            Some(2_000 * FEMTOSECONDS_PER_MILLISECOND)
        );
        assert_eq!(hpet_timeout_fs(u64::MAX - 9, 100_000_000, 2_000), None);
        assert_eq!(hpet_timeout_fs(0, 1, u32::MAX), None);
    }
}
