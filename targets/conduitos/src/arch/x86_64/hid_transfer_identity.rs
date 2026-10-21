//! Identity metadata for the sole existing Root-owned keyboard runway.
//! This does not acknowledge DMA stop, cancel transfers, or permit runway reset.
use super::super::{HidError, REPORT_BUFFERS};
use core::sync::atomic::{AtomicU32, Ordering};

pub(super) static SERIAL: AtomicU32 = AtomicU32::new(0);
pub(super) static mut INTERESTS: Interests = Interests::new();

#[derive(Clone, Copy)]
pub(super) struct Interests {
    pending: [Option<(usize, u64)>; REPORT_BUFFERS],
}
impl Interests {
    pub(super) const fn new() -> Self {
        Self {
            pending: [None; REPORT_BUFFERS],
        }
    }
    pub(super) fn ensure_idle(&self) -> Result<(), HidError> {
        if self.pending.iter().any(Option::is_some) {
            Err(HidError::TransferOverflow)
        } else {
            Ok(())
        }
    }
    pub(super) fn arm(
        &mut self,
        start: usize,
        end: usize,
        epoch: u32,
        serial: &AtomicU32,
    ) -> Result<(), HidError> {
        if epoch == 0 {
            return Err(HidError::StaleAttachment);
        }
        let count = end
            .checked_sub(start)
            .filter(|n| (1..=REPORT_BUFFERS).contains(n))
            .ok_or(HidError::TransferOverflow)?;
        if start == 0 {
            self.ensure_idle()?;
        }
        for index in start..end {
            if self.pending[index % REPORT_BUFFERS].is_some() {
                return Err(HidError::TransferOverflow);
            }
        }
        // The complete range is reserved before the first controller-owned TRB.
        // Never reset this allocator on prepare or session restart.
        let previous = serial
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(count as u32)
            })
            .map_err(|_| HidError::TransferOverflow)?;
        for (offset, index) in (start..end).enumerate() {
            let cookie = (u64::from(epoch) << 32) | u64::from(previous + offset as u32 + 1);
            self.pending[index % REPORT_BUFFERS] = Some((index, cookie));
        }
        Ok(())
    }
    pub(super) fn cookie(&self, index: usize, epoch: u32) -> Result<u64, HidError> {
        match self.pending[index % REPORT_BUFFERS] {
            Some((original, cookie)) if original == index && cookie >> 32 == u64::from(epoch) => {
                Ok(cookie)
            }
            _ => Err(HidError::WrongCompletion),
        }
    }
    pub(super) fn retire(&mut self, index: usize, cookie: u64) -> Result<(), HidError> {
        if self.pending[index % REPORT_BUFFERS] != Some((index, cookie)) {
            return Err(HidError::WrongCompletion);
        }
        self.pending[index % REPORT_BUFFERS] = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generation_does_not_alias_after_ring_wrap_or_runway_restart() {
        let serial = AtomicU32::new(0);
        let mut interests = Interests::new();
        let mut old = 0;
        for index in 0..1024 {
            interests.arm(index, index + 1, 7, &serial).unwrap();
            let cookie = interests.cookie(index, 7).unwrap();
            assert!(cookie > old);
            if old != 0 {
                assert_eq!(interests.retire(index, old), Err(HidError::WrongCompletion));
            }
            interests.retire(index, cookie).unwrap();
            assert!(interests.cookie(index, 7).is_err());
            assert!(interests.retire(index, cookie).is_err());
            old = cookie;
        }
        interests.arm(0, 8, 7, &serial).unwrap();
        assert!(interests.cookie(0, 7).unwrap() > old);
        assert_eq!(
            interests.arm(0, 8, 7, &serial),
            Err(HidError::TransferOverflow)
        );
        assert!(interests.cookie(0, 8).is_err());
    }
    #[test]
    fn exhaustion_and_busy_refuse_the_entire_window_without_partial_interest() {
        let serial = AtomicU32::new(u32::MAX - 7);
        let mut interests = Interests::new();
        assert_eq!(
            interests.arm(0, 8, 1, &serial),
            Err(HidError::TransferOverflow)
        );
        assert_eq!(serial.load(Ordering::Relaxed), u32::MAX - 7);
        interests.ensure_idle().unwrap();
        assert_eq!(
            interests.arm(0, 1, 0, &serial),
            Err(HidError::StaleAttachment)
        );
        interests.arm(0, 7, 1, &serial).unwrap();
        assert_eq!(serial.load(Ordering::Relaxed), u32::MAX);
        assert_eq!(
            interests.arm(7, 8, 1, &serial),
            Err(HidError::TransferOverflow)
        );
        assert!(interests.cookie(7, 1).is_err());
    }
}
