//! Exact PLV3 entry and one finite Root timer budget per entry.
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
    pgdl: u64,
    pgdh: u64,
    trap: u64,
    root_floating: u64,
    user_floating: u64,
    code: u64,
    refill: u64,
}
unsafe extern "C" {
    fn conduitos_loongarch64_domain_enter(entry: *const Entry) -> TransitionReturn;
}
struct Interrupts(usize);
impl Interrupts {
    fn mask() -> Self {
        let status = super::read_csr::<0>();
        super::disable_interrupts();
        Self(status)
    }
}
impl Drop for Interrupts {
    fn drop(&mut self) {
        if self.0 & 4 != 0 {
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
    domain_memory::enable_no_execute()?;
    let _budget = domain_budget::Budget::arm()?;
    let (root_floating, user_floating) = space.floating_states();
    let entry = Entry {
        entry: space.entry,
        stack: domain_memory::USER_STACK_TOP,
        frame: domain_memory::USER_FRAME,
        pgdl: space.pgdl,
        pgdh: space.pgdh,
        trap: space.trap_stack_top(),
        root_floating,
        user_floating,
        code: space.code_address(),
        refill: super::domain_refill::physical_entry()?,
    };
    let returned = unsafe { conduitos_loongarch64_domain_enter(&entry) };
    if returned.origin == 4 {
        Err(DomainRefusal::Unsupported)
    } else {
        Ok(returned)
    }
}
#[unsafe(no_mangle)]
extern "C" fn conduitos_loongarch64_domain_irq_handler() -> u64 {
    u64::from(domain_budget::interrupt(true))
}
