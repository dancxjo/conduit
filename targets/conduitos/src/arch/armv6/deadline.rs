//! Bounded system-timer observation in calibrated milliseconds.
use crate::monotonic_clock::{codec::ClockDisposition, owner::MonotonicDeadlineProvider};

pub trait CounterRegisters {
    fn high(&mut self) -> u32;
    fn low(&mut self) -> u32;
}

pub struct DeadlineClock<R> {
    registers: R,
    last: u64,
    revoked: bool,
}
impl<R: CounterRegisters> DeadlineClock<R> {
    /// # Safety
    /// Root owns this clock realization and its BCM2835 one-microsecond
    /// calibration. Register reads must be bounded and ordered.
    pub unsafe fn admit(registers: R) -> Self {
        Self {
            registers,
            last: 0,
            revoked: false,
        }
    }
    fn observe(&mut self) -> Result<u64, ClockDisposition> {
        if self.revoked {
            return Err(ClockDisposition::ProviderLost);
        }
        // The low register wraps after about 71 minutes. A high/low/high sample
        // refuses torn observations rather than widening only the low word.
        for _ in 0..3 {
            let high = self.registers.high();
            let low = self.registers.low();
            if high == self.registers.high() {
                let now = ((u64::from(high) << 32) | u64::from(low)) / 1000;
                if now < self.last {
                    self.revoked = true;
                    return Err(ClockDisposition::ProviderLost);
                }
                self.last = now;
                return Ok(now);
            }
        }
        self.revoked = true;
        Err(ClockDisposition::Unavailable)
    }
}
impl<R: CounterRegisters> MonotonicDeadlineProvider for DeadlineClock<R> {
    fn poll_until(&mut self, deadline_ms: u64) -> Result<Option<u64>, ClockDisposition> {
        let now = self.observe()?;
        if deadline_ms > now.saturating_add(60_000) {
            return Err(ClockDisposition::UnsupportedDeadline);
        }
        Ok((now >= deadline_ms).then_some(now))
    }
    fn sample_now(&mut self) -> Result<u64, ClockDisposition> {
        self.observe()
    }
    fn revoke(&mut self) {
        self.revoked = true;
    }
}

#[cfg(target_arch = "arm")]
pub struct SystemTimer {
    _owned: (),
}
#[cfg(target_arch = "arm")]
impl SystemTimer {
    /// # Safety
    /// Root admits the BCM2835 system-timer mapping and calibrated clock basis.
    pub unsafe fn admit() -> DeadlineClock<Self> {
        unsafe { DeadlineClock::admit(Self { _owned: () }) }
    }
    fn read(offset: usize) -> u32 {
        unsafe {
            core::arch::asm!("mcr p15, 0, {zero}, c7, c10, 4", zero=in(reg) 0u32, options(nostack,preserves_flags));
            let value = core::ptr::read_volatile((0x2000_3000 + offset) as *const u32);
            core::arch::asm!("mcr p15, 0, {zero}, c7, c10, 4", zero=in(reg) 0u32, options(nostack,preserves_flags));
            value
        }
    }
}
#[cfg(target_arch = "arm")]
impl CounterRegisters for SystemTimer {
    fn high(&mut self) -> u32 {
        Self::read(8)
    }
    fn low(&mut self) -> u32 {
        Self::read(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::VecDeque;
    struct Registers {
        high: VecDeque<u32>,
        low: VecDeque<u32>,
    }
    impl CounterRegisters for Registers {
        fn high(&mut self) -> u32 {
            self.high.pop_front().unwrap()
        }
        fn low(&mut self) -> u32 {
            self.low.pop_front().unwrap()
        }
    }
    #[test]
    fn retries_torn_rollover_and_converts_the_full_counter() {
        let mut clock = unsafe {
            DeadlineClock::admit(Registers {
                high: [0, 1, 1, 1, 1, 1].into(),
                low: [u32::MAX, 2000, 3000].into(),
            })
        };
        assert_eq!(clock.sample_now(), Ok(((1u64 << 32) + 2000) / 1000));
        assert_eq!(
            clock.poll_until(((1u64 << 32) + 3000) / 1000),
            Ok(Some(((1u64 << 32) + 3000) / 1000))
        );
        clock.revoke();
        assert_eq!(clock.sample_now(), Err(ClockDisposition::ProviderLost));
    }
    #[test]
    fn unstable_or_regressing_counter_revokes_in_finite_work() {
        let mut unstable = unsafe {
            DeadlineClock::admit(Registers {
                high: [0, 1, 1, 2, 2, 3].into(),
                low: [0, 0, 0].into(),
            })
        };
        assert_eq!(unstable.sample_now(), Err(ClockDisposition::Unavailable));
        assert_eq!(unstable.sample_now(), Err(ClockDisposition::ProviderLost));
        let mut regressing = unsafe {
            DeadlineClock::admit(Registers {
                high: [0, 0, 0, 0].into(),
                low: [2000, 1000].into(),
            })
        };
        assert_eq!(regressing.sample_now(), Ok(2));
        assert_eq!(regressing.sample_now(), Err(ClockDisposition::ProviderLost));
    }
}
