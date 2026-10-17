//! Exact U-mode entry below the shared execution-region backend.
use super::{domain_budget, domain_memory};
use crate::protected_region::DomainRefusal;
use core::sync::atomic::{AtomicBool, Ordering};
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
    satp: u64,
    trap: u64,
    root_floating: u64,
    user_floating: u64,
}
unsafe extern "C" {
    fn conduitos_riscv64_domain_enter(entry: *const Entry) -> TransitionReturn;
}
struct Interrupts(u64);
impl Interrupts {
    fn mask() -> Self {
        let status;
        unsafe {
            core::arch::asm!("csrrci {0}, sstatus, 2", out(reg) status, options(nostack));
        }
        Self(status)
    }
}
impl Drop for Interrupts {
    fn drop(&mut self) {
        if self.0 & 2 != 0 {
            super::enable_interrupts();
        } else {
            super::disable_interrupts();
        }
    }
}
struct Active;
impl Drop for Active {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}
pub(super) fn enter(
    space: &domain_memory::AddressSpace,
) -> Result<TransitionReturn, DomainRefusal> {
    let _interrupts = Interrupts::mask();
    if ACTIVE.swap(true, Ordering::AcqRel) {
        return Err(DomainRefusal::InvalidLifecycle);
    }
    let _active = Active;
    let status: u64;
    unsafe {
        core::arch::asm!("csrr {0}, sstatus", out(reg) status, options(nostack));
    }
    // This profile saves scalar F/D state and refuses active vector/extension
    // state rather than passing unreviewed register banks into user execution.
    if status & 0x6000 == 0 || status & 0x18600 != 0 {
        return Err(DomainRefusal::Unsupported);
    }
    let _budget = domain_budget::Budget::arm()?;
    let (root_floating, user_floating) = space.floating_states();
    let entry = Entry {
        entry: space.entry,
        stack: domain_memory::USER_STACK_TOP,
        frame: domain_memory::USER_FRAME,
        satp: space.satp,
        trap: space.trap_stack_top(),
        root_floating,
        user_floating,
    };
    Ok(unsafe { conduitos_riscv64_domain_enter(&entry) })
}
#[unsafe(no_mangle)]
extern "C" fn conduitos_riscv64_domain_irq_handler() -> u64 {
    u64::from(domain_budget::interrupt(true))
}
