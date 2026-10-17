//! EL0 entry and exact restoration of the EL1 product caller.
use super::{domain_budget, domain_memory};
use crate::protected_region::DomainRefusal;
use core::{
    arch::asm,
    sync::atomic::{AtomicBool, Ordering},
};

#[path = "domain_transition_asm.rs"]
mod assembly;
static ACTIVE: AtomicBool = AtomicBool::new(false);

#[repr(C)]
pub(super) struct TransitionReturn {
    pub value: u64,
    pub origin: u64,
}
#[repr(C)]
struct Entry {
    entry: u64,
    stack: u64,
    frame: u64,
    ttbr0: u64,
    ttbr1: u64,
    tcr: u64,
    root_floating: u64,
    user_floating: u64,
    flags: u64,
}
unsafe extern "C" {
    fn conduitos_aarch64_domain_enter(entry: *const Entry) -> TransitionReturn;
    static conduitos_aarch64_domain_vectors: u8;
}

struct Irqs(u64);
impl Irqs {
    fn mask() -> Self {
        let flags;
        unsafe {
            asm!("mrs {0}, daif", "msr daifset, #15", out(reg) flags, options(nostack));
        }
        Self(flags)
    }
}
impl Drop for Irqs {
    fn drop(&mut self) {
        unsafe {
            asm!("msr daif, {0}", in(reg) self.0, options(nostack));
        }
    }
}
struct Active;
impl Drop for Active {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}
struct Vectors {
    old: u64,
    stack: u64,
}
impl Vectors {
    fn install(stack: u64) -> Result<Self, DomainRefusal> {
        let (selection, floating): (u64, u64);
        unsafe {
            asm!("mrs {0}, spsel", "mrs {1}, cpacr_el1",
                out(reg) selection, out(reg) floating, options(nostack));
        }
        // The existing Limine product runs Root at EL1t. Keep its SP_EL0
        // untouched while installing a distinct bounded exception stack.
        if super::current_el() != 1 || selection != 0 || floating & 0x0333_0000 != 0x0030_0000 {
            return Err(DomainRefusal::Unsupported);
        }
        let (old, old_stack): (u64, u64);
        unsafe {
            asm!("mrs {old}, vbar_el1", "msr spsel, #1", "mov {old_stack}, sp",
                "mov sp, {stack}", "msr spsel, #0", "msr vbar_el1, {vectors}", "isb",
                old = out(reg) old, old_stack = out(reg) old_stack,
                stack = in(reg) stack,
                vectors = in(reg) core::ptr::addr_of!(conduitos_aarch64_domain_vectors),
                options(nostack));
        }
        Ok(Self {
            old,
            stack: old_stack,
        })
    }
}
impl Drop for Vectors {
    fn drop(&mut self) {
        unsafe {
            asm!("msr vbar_el1, {vectors}", "msr spsel, #1", "mov sp, {stack}",
                "msr spsel, #0", "isb", vectors = in(reg) self.old,
                stack = in(reg) self.stack, options(nostack));
        }
    }
}

pub(super) fn enter(
    space: &domain_memory::AddressSpace,
) -> Result<TransitionReturn, DomainRefusal> {
    let _irqs = Irqs::mask();
    if ACTIVE.swap(true, Ordering::AcqRel) {
        return Err(DomainRefusal::InvalidLifecycle);
    }
    let _active = Active;
    let _vectors = Vectors::install(space.trap_stack_top())?;
    let _budget = domain_budget::Budget::arm()?;
    let (root_floating, user_floating) = space.floating_states();
    let (isar1, isar2, debug): (u64, u64, u64);
    unsafe {
        asm!("mrs {0}, id_aa64isar1_el1", "mrs {1}, S3_0_C0_C6_2",
            "mrs {2}, id_aa64dfr0_el1", out(reg) isar1, out(reg) isar2,
            out(reg) debug, options(nostack));
    }
    let pauth = isar1 & 0xff00_0ff0 != 0 || isar2 & 0xff00 != 0;
    let pmu = !matches!((debug >> 8) & 15, 0 | 15);
    let entry = Entry {
        entry: space.entry,
        stack: domain_memory::USER_STACK_TOP,
        frame: domain_memory::USER_FRAME,
        ttbr0: space.ttbr0,
        ttbr1: space.ttbr1,
        tcr: space.tcr,
        root_floating,
        user_floating,
        flags: u64::from(pauth) | (u64::from(pmu) << 1),
    };
    Ok(unsafe { conduitos_aarch64_domain_enter(&entry) })
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_aarch64_domain_irq(user: u64) -> u64 {
    let acknowledge = unsafe { super::read32(super::GICC_BASE + 0x00c) };
    let irq = acknowledge & 0x3ff;
    if irq == domain_budget::IRQ {
        let expired = domain_budget::interrupt(user != 0);
        unsafe {
            super::write32(super::GICC_BASE + 0x010, acknowledge);
        }
        u64::from(expired)
    } else {
        if user != 0 && irq == super::VIRTUAL_TIMER_IRQ {
            domain_budget::source_interrupt();
        }
        super::service_interrupt(acknowledge);
        0
    }
}
