//! One machine-owned MSI-X readiness lane. Root remains the sole event consumer.
//! IRQ facts never authorize report bytes, attachment identity, or semantic input.
use super::XhciReady;
#[cfg(target_os = "none")]
use core::sync::atomic::AtomicBool;
#[cfg(any(target_os = "none", feature = "native-latency-observation"))]
use core::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_os = "none")]
use core::{
    ptr::{read_volatile, write_volatile},
    sync::atomic::AtomicUsize,
};

#[cfg(target_os = "none")]
const VECTOR: u8 = 0x21;
#[cfg(target_os = "none")]
static CONFIGURED: AtomicBool = AtomicBool::new(false);
#[cfg(any(target_os = "none", feature = "native-latency-observation"))]
static SEEN: AtomicU64 = AtomicU64::new(0);
#[cfg(any(target_os = "none", feature = "native-latency-observation"))]
static IRQ_TICKS: AtomicU64 = AtomicU64::new(0);
#[cfg(any(target_os = "none", feature = "native-latency-observation"))]
static WAKE_TICKS: AtomicU64 = AtomicU64::new(0);
#[cfg(any(target_os = "none", feature = "native-latency-observation"))]
static IDLE_ENTRIES: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "none")]
static OPERATIONAL: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_os = "none")]
static INTERRUPTER: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_os = "none")]
static BAR: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "none")]
static RTC_FAILED: AtomicBool = AtomicBool::new(false);

#[cfg(any(test, target_os = "none"))]
fn source_ready(status: u32, erdp: u32) -> bool {
    status & (1 << 3) != 0 && erdp & 8 != 0
}

/// An MSI-X table entry must be wholly inside the already mapped device page.
/// Only BAR0 is mapped by this provider; other BIRs explicitly fall back.
#[cfg(any(test, target_os = "none"))]
fn table_entry(mmio: usize, bar: u64, bir: u8, offset: u32) -> Option<usize> {
    if bir != 0 || offset & 7 != 0 {
        return None;
    }
    let extent = (bar & 0x1f_ffff)
        .checked_add(u64::from(offset))?
        .checked_add(16)?;
    if extent > 0x20_0000 {
        return None;
    }
    mmio.checked_add(offset as usize)
}

#[cfg(target_os = "none")]
pub(super) fn configure(pci: &super::pci::PciController, mmio: usize, ready: &XhciReady) {
    use crate::arch::x86_64::{cpu, interrupt_controller, serial};
    let _mask = cpu::InterruptMask::new();
    let _ = serial::append_boot_diagnostic(
        b"CONDUIT_HID_WAKE_ROUTE bounded-poll qualification-pending\n",
    );
    let Some(destination) = interrupt_controller::msi_destination() else {
        return;
    };
    let Some((capability, _, bir, offset)) = super::pci::observe_msix(pci) else {
        return;
    };
    let Some(table) = table_entry(mmio, pci.bar, bir, offset) else {
        return;
    };
    let control = super::pci::msix_control(pci, capability);
    // An already enabled table is owned elsewhere, not a reservation for us.
    if control & (1 << 15) != 0 || CONFIGURED.load(Ordering::Acquire) {
        return;
    }
    let (operational, interrupter) = ready.interrupt_registers();
    unsafe {
        write_volatile((table + 12) as *mut u32, 1);
        write_volatile(
            table as *mut u32,
            0xfee0_0000 | (u32::from(destination) << 12),
        );
        write_volatile((table + 4) as *mut u32, 0);
        write_volatile((table + 8) as *mut u32, u32::from(VECTOR));
    }
    OPERATIONAL.store(operational, Ordering::Relaxed);
    INTERRUPTER.store(interrupter, Ordering::Relaxed);
    BAR.store(pci.bar, Ordering::Relaxed);
    CONFIGURED.store(true, Ordering::Release);
    super::pci::set_msix_control(pci, capability, (control | (1 << 15)) & !(1 << 14));
    unsafe {
        write_volatile((table + 12) as *mut u32, 0);
    }
    let readback = super::pci::msix_control(pci, capability);
    if readback & 0xc000 != 0x8000
        || unsafe { read_volatile((table + 8) as *const u32) } != u32::from(VECTOR)
    {
        unsafe {
            write_volatile((table + 12) as *mut u32, 1);
        }
        super::pci::set_msix_control(pci, capability, control);
        CONFIGURED.store(false, Ordering::Release);
        return;
    }
    unsafe {
        // Clear only this source's W1C pending bits, retaining interrupter IE.
        write_volatile((operational + 4) as *mut u32, 1 << 3);
        write_volatile(interrupter as *mut u32, 3);
        let command = read_volatile(operational as *const u32);
        write_volatile(operational as *mut u32, command | (1 << 2));
    }
    let _ = serial::append_boot_diagnostic(
        b"CONDUIT_HID_WAKE_ROUTE msix-awaiting-first-irq bounded-poll\n",
    );
}

#[unsafe(no_mangle)]
pub(super) extern "C" fn conduitos_xhci_irq_handler() {
    #[cfg(target_os = "none")]
    {
        if CONFIGURED.load(Ordering::Acquire) {
            let operational = OPERATIONAL.load(Ordering::Relaxed);
            let interrupter = INTERRUPTER.load(Ordering::Relaxed);
            // QEMU MSI-X automatically clears IMAN.IP on delivery. EINT/EHB
            // identify readiness; requiring IP would discard every valid IRQ.
            let status = unsafe { read_volatile((operational + 4) as *const u32) };
            let erdp = unsafe { read_volatile((interrupter + 0x18) as *const u32) };
            if source_ready(status, erdp) {
                IRQ_TICKS.store(
                    crate::native_observation::counter_ticks().unwrap_or(0),
                    Ordering::Relaxed,
                );
                if SEEN.fetch_add(1, Ordering::Release) == u64::MAX {
                    CONFIGURED.store(false, Ordering::Release);
                }
            }
            unsafe {
                write_volatile((operational + 4) as *mut u32, 1 << 3);
                write_volatile(
                    interrupter as *mut u32,
                    if CONFIGURED.load(Ordering::Acquire) {
                        3
                    } else {
                        1
                    },
                );
            }
        }
        crate::arch::x86_64::interrupt_controller::end_device_interrupt();
    }
}

/// One synchronous machine-only idle interval; no callback or eligible Play is
/// entered while the RTC lease lives. Unqualified delivery retains polling.
pub(crate) fn idle_after_poll(controller: &XhciReady) -> bool {
    #[cfg(target_os = "none")]
    {
        use crate::arch::x86_64::{cpu, domain_budget::RootIdleLease, serial};
        if !CONFIGURED.load(Ordering::Acquire)
            || BAR.load(Ordering::Acquire) != controller.bar_physical
        {
            return false;
        }
        let _mask = cpu::InterruptMask::new();
        // Allow the first actual device IRQ to qualify itself without sleeping
        // on an unproven route or consuming an additional command-ring slot.
        cpu::enable_interrupts();
        unsafe {
            core::arch::asm!("nop", options(nostack));
        }
        cpu::disable_interrupts();
        if SEEN.load(Ordering::Acquire) == 0 || RTC_FAILED.load(Ordering::Acquire) {
            return false;
        }
        if controller.has_observed_event() {
            return true;
        }
        let before = SEEN.load(Ordering::Acquire);
        let Some(lease) = RootIdleLease::try_arm() else {
            RTC_FAILED.store(true, Ordering::Release);
            let _ = serial::append_boot_diagnostic(
                b"CONDUIT_HID_WAKE_ROUTE rtc-unqualified bounded-poll\n",
            );
            return false;
        };
        // CLI protects all three observations through STI's interrupt shadow.
        // A device event after the cycle check either interrupts HLT or is
        // observed next turn after the independently qualified RTC wakes us.
        if lease.may_idle()
            && SEEN.load(Ordering::Acquire) == before
            && !controller.has_observed_event()
        {
            IDLE_ENTRIES.fetch_add(1, Ordering::Relaxed);
            cpu::interruptible_idle();
            WAKE_TICKS.store(
                crate::native_observation::counter_ticks().unwrap_or(0),
                Ordering::Relaxed,
            );
        }
        drop(lease); // restores RTC/PIC/LINT before any eligible Root execution
        true
    }
    #[cfg(not(target_os = "none"))]
    {
        let _ = controller;
        false
    }
}

#[cfg(feature = "native-latency-observation")]
pub(in crate::arch::x86_64) fn observation() -> (&'static str, u64, u64, u64, u64, u32) {
    let count = SEEN.load(Ordering::Acquire);
    let idle = IDLE_ENTRIES.load(Ordering::Relaxed);
    let route = if idle != 0 {
        "msix-rtc"
    } else if count != 0 {
        "msix-observed-poll"
    } else {
        "bounded-poll"
    };
    #[cfg(target_os = "none")]
    let qualification = crate::arch::x86_64::domain_budget::qualification_polls();
    #[cfg(not(target_os = "none"))]
    let qualification = 0;
    (
        route,
        count,
        IRQ_TICKS.load(Ordering::Relaxed),
        WAKE_TICKS.load(Ordering::Relaxed),
        idle,
        qualification,
    )
}

#[cfg(test)]
mod tests {
    use super::{source_ready, table_entry};
    #[test]
    fn msix_source_qualification_does_not_require_auto_cleared_iman_ip() {
        assert!(source_ready(8, 8));
        assert!(!source_ready(0, 8));
        assert!(!source_ready(8, 0));
        assert!(!source_ready(u32::MAX & !8, u32::MAX));
    }
    #[test]
    fn msix_table_cannot_escape_the_reserved_device_mapping() {
        assert_eq!(table_entry(0x1000, 0x400000, 0, 0x3000), Some(0x4000));
        assert_eq!(table_entry(0x1000, 0x400000, 1, 0x3000), None);
        assert_eq!(table_entry(0x1000, 0x400000, 0, 0x3001), None);
        assert_eq!(table_entry(0x1000, 0x5ffff8, 0, 0), None);
        assert_eq!(table_entry(usize::MAX, 0x400000, 0, 8), None);
    }
}

/// Synchronous callers retain both the original finite poll budget and a
/// physical deadline when the calibrated native clock is available. Idling
/// must not turn a finite polling bound into billions of sleep intervals.
pub(in crate::arch::x86_64) struct ReceiveBudget {
    #[cfg(target_os = "none")]
    clock: Option<crate::arch::x86_64::deadline::CandidateDeadline>,
}
impl ReceiveBudget {
    pub(in crate::arch::x86_64) fn new() -> Self {
        Self {
            #[cfg(target_os = "none")]
            clock: crate::arch::x86_64::deadline::CandidateDeadline::admit_duration(30_000),
        }
    }
    pub(in crate::arch::x86_64) fn expired(&self) -> bool {
        #[cfg(target_os = "none")]
        {
            self.clock.is_some_and(|clock| {
                !clock
                    .elapsed_millis()
                    .is_some_and(|elapsed| (0..30_000).contains(&elapsed))
            })
        }
        #[cfg(not(target_os = "none"))]
        {
            false
        }
    }
    pub(in crate::arch::x86_64) fn idle(&self, controller: &XhciReady) {
        #[cfg(target_os = "none")]
        if self.clock.is_some() {
            idle_after_poll(controller);
        }
        #[cfg(not(target_os = "none"))]
        let _ = controller;
    }
}
