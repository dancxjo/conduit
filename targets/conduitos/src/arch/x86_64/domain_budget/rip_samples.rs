//! Diagnostic-only Root RTC samples. No guest bytes, clocks or authority.
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
const CAPACITY: usize = 64;
struct Samples {
    active: AtomicBool,
    count: AtomicUsize,
    overflow: AtomicBool,
    pcs: [AtomicU64; CAPACITY],
}
impl Samples {
    const fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            count: AtomicUsize::new(0),
            overflow: AtomicBool::new(false),
            pcs: [const { AtomicU64::new(0) }; CAPACITY],
        }
    }
    fn arm(&self, active: bool) {
        self.active.store(false, Ordering::Release);
        self.count.store(0, Ordering::Relaxed);
        self.overflow.store(false, Ordering::Relaxed);
        self.active.store(active, Ordering::Release);
    }
    fn record(&self, pc: u64) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }
        let index = self.count.load(Ordering::Relaxed);
        if index == CAPACITY {
            self.overflow.store(true, Ordering::Relaxed);
            return;
        }
        self.pcs[index].store(pc, Ordering::Relaxed);
        self.count.store(index + 1, Ordering::Release);
    }
    fn finish(&self) -> Observation {
        self.active.store(false, Ordering::Release);
        let count = self.count.load(Ordering::Acquire);
        let mut result = Observation {
            count,
            overflow: self.overflow.load(Ordering::Relaxed),
            pcs: [0; CAPACITY],
        };
        for (output, input) in result.pcs[..count].iter_mut().zip(&self.pcs[..count]) {
            *output = input.load(Ordering::Relaxed);
        }
        result
    }
}
static SAMPLES: Samples = Samples::new();
pub(in crate::arch::x86_64) struct Observation {
    pub count: usize,
    pub overflow: bool,
    pub pcs: [u64; CAPACITY],
}
pub(in crate::arch::x86_64) struct Capture;
impl Capture {
    /// Serialized native entry and masked IRQ transitions exclusively own this
    /// fixed Root storage. The guest cannot enable it or reset any budget.
    pub fn arm(active: bool) -> Self {
        SAMPLES.arm(active);
        Self
    }
    pub fn finish(self) -> Observation {
        SAMPLES.finish()
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        SAMPLES.active.store(false, Ordering::Release);
    }
}
/// Caller has observed one increment of the existing RTC user counter. The
/// pointer is the real Root IRQ frame, never a guest-provided address.
pub(in crate::arch::x86_64) unsafe fn observe(saved: *const u64) {
    if !SAMPLES.active.load(Ordering::Acquire) || saved.is_null() {
        return;
    }
    if unsafe { saved.add(17).read() } & 3 != 3 {
        return;
    }
    SAMPLES.record(unsafe { saved.add(16).read() });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_samples_cannot_overflow_or_cross_entry() {
        let samples = Samples::new();
        samples.arm(true);
        for pc in 1..=65 {
            samples.record(pc);
        }
        let result = samples.finish();
        assert_eq!(result.count, 64);
        assert!(result.overflow);
        assert_eq!(result.pcs[0], 1);
        assert_eq!(result.pcs[63], 64);
        samples.record(99);
        assert_eq!(samples.finish().pcs[63], 64);
        samples.arm(true);
        samples.record(777);
        let result = samples.finish();
        assert_eq!(result.count, 1);
        assert!(!result.overflow);
        assert_eq!(result.pcs[0], 777);
        assert_eq!(result.pcs[1], 0);
        samples.arm(false);
        samples.record(999);
        assert_eq!(samples.finish().count, 0);
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    #[test]
    fn observation_uses_only_live_root_cpl3_frames() {
        let capture = Capture::arm(true);
        unsafe { observe(core::ptr::null()) };
        let mut saved = [0u64; 21];
        saved[16] = 0x401234;
        unsafe { observe(saved.as_ptr()) };
        saved[17] = 3;
        unsafe { observe(saved.as_ptr()) };
        let result = capture.finish();
        assert_eq!(result.count, 1);
        assert_eq!(result.pcs[0], 0x401234);
        let capture = Capture::arm(false);
        unsafe { observe(saved.as_ptr()) };
        assert_eq!(capture.finish().count, 0);
    }
}
