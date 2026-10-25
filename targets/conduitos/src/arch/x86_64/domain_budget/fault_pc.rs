//! Optional Root-only terminal RTC observation. Never execution authority.
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::arch::x86_64) struct FaultPc {
    pub rip: u64,
    pub rsp: u64,
}
/// One exclusively owned native entry; interrupts are masked during reset/read.
/// Atomics avoid mutable references to IRQ-shared storage.
struct Snapshot {
    valid: AtomicBool,
    rip: AtomicU64,
    rsp: AtomicU64,
}
impl Snapshot {
    const fn new() -> Self {
        Self {
            valid: AtomicBool::new(false),
            rip: AtomicU64::new(0),
            rsp: AtomicU64::new(0),
        }
    }
    fn reset(&self) {
        self.valid.store(false, Ordering::Release);
        self.rip.store(0, Ordering::Relaxed);
        self.rsp.store(0, Ordering::Relaxed);
    }
    fn record(&self, pc: FaultPc) {
        self.rip.store(pc.rip, Ordering::Relaxed);
        self.rsp.store(pc.rsp, Ordering::Relaxed);
        self.valid.store(true, Ordering::Release);
    }
    fn read(&self) -> Option<FaultPc> {
        self.valid.load(Ordering::Acquire).then(|| FaultPc {
            rip: self.rip.load(Ordering::Relaxed),
            rsp: self.rsp.load(Ordering::Relaxed),
        })
    }
}
static SNAPSHOT: Snapshot = Snapshot::new();
pub(super) fn reset() {
    SNAPSHOT.reset();
}
pub(super) fn read() -> Option<FaultPc> {
    SNAPSHOT.read()
}
/// Caller has established an active terminal RTC disposition. The saved pointer
/// is the Root IRQ stack after fifteen GPR pushes and the vector marker.
/// Read RSP only after the hardware CS proves a privilege-changing CPL3 frame.
pub(super) unsafe fn observe(saved: *const u64) {
    if saved.is_null() {
        return;
    }
    let cs = unsafe { saved.add(17).read() };
    if cs & 3 != 3 {
        return;
    }
    SNAPSHOT.record(FaultPc {
        rip: unsafe { saved.add(16).read() },
        rsp: unsafe { saved.add(19).read() },
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_reset_prevents_cross_entry_reuse_and_copy_is_immutable() {
        let snapshot = Snapshot::new();
        assert_eq!(snapshot.read(), None);
        let pc = FaultPc {
            rip: 0x401234,
            rsp: 0x7ff000,
        };
        snapshot.record(pc);
        let receipt = snapshot.read();
        snapshot.reset();
        assert_eq!(snapshot.read(), None);
        assert_eq!(receipt, Some(pc));
    }
    #[test]
    fn only_cpl3_frame_can_supply_terminal_pc() {
        // One test owns global snapshot, just as one native entry owns it.
        reset();
        unsafe { observe(core::ptr::null()) };
        let mut saved = [0u64; 21];
        saved[16] = 0x401234;
        saved[19] = 0x7ff000;
        unsafe { observe(saved.as_ptr()) };
        assert_eq!(read(), None);
        saved[17] = 0x23;
        unsafe { observe(saved.as_ptr()) };
        assert_eq!(
            read(),
            Some(FaultPc {
                rip: saved[16],
                rsp: saved[19]
            })
        );
        reset();
        assert_eq!(read(), None);
    }
}
