//! Ordinary CPL3 entry, terminal gate, and hard timer return to Root.
use super::{domain_budget, domain_memory, gdt, idt};
use crate::protected_region::DomainRefusal;
use core::{
    arch::global_asm,
    sync::atomic::{AtomicBool, Ordering},
};

#[cfg(feature = "ordinary-domain-proof")]
mod trap_observation;

static ACTIVE: AtomicBool = AtomicBool::new(false);
#[repr(C)]
pub(super) struct TransitionReturn {
    pub value: u64,
    pub origin: u64,
}
unsafe extern "C" {
    fn conduitos_ordinary_enter(entry: u64, stack: u64, frame: u64, cr3: u64) -> TransitionReturn;
    fn conduitos_ordinary_gate();
    fn conduitos_ordinary_gp();
    fn conduitos_ordinary_pf();
    fn conduitos_ordinary_ud();
    fn conduitos_ordinary_nm();
    fn conduitos_ordinary_exception_0();
    fn conduitos_ordinary_exception_1();
    fn conduitos_ordinary_exception_3();
    fn conduitos_ordinary_exception_4();
    fn conduitos_ordinary_exception_5();
    fn conduitos_ordinary_exception_10();
    fn conduitos_ordinary_exception_11();
    fn conduitos_ordinary_exception_12();
    fn conduitos_ordinary_exception_16();
    fn conduitos_ordinary_exception_17();
    fn conduitos_ordinary_exception_19();
    fn conduitos_ordinary_exception_21();
    fn conduitos_ordinary_exception_30();
    fn conduitos_ordinary_timer();
    fn conduitos_ordinary_budget();
}

/// Root-created context from an installed, exclusively owned address space.
pub(super) struct EntryContext {
    pub entry: u64,
    pub stack: u64,
    pub frame: u64,
    pub cr3: u64,
    pub trap_stack: u64,
}

pub(super) fn enter(
    space: &domain_memory::AddressSpace,
) -> Result<TransitionReturn, DomainRefusal> {
    enter_context(EntryContext {
        entry: space.entry,
        stack: domain_memory::USER_STACK_TOP - 8,
        frame: domain_memory::USER_FRAME,
        cr3: space.cr3,
        trap_stack: space.trap_stack_top(),
    })
}

pub(super) fn enter_context(context: EntryContext) -> Result<TransitionReturn, DomainRefusal> {
    enter_context_with_budget(
        context,
        domain_budget::EntryBudget::Ordinary,
        #[cfg(feature = "ordinary-domain-proof")]
        None,
    )
}

pub(super) fn enter_context_with_budget(
    context: EntryContext,
    profile: domain_budget::EntryBudget,
    #[cfg(feature = "ordinary-domain-proof")] permit: Option<super::clock_boundary::Permit>,
) -> Result<TransitionReturn, DomainRefusal> {
    run_context(
        context,
        profile,
        #[cfg(feature = "ordinary-domain-proof")]
        permit,
        None,
    )
    .map(|(returned, _)| returned)
}

pub(super) fn enter_context_with_receipt(
    context: EntryContext,
    profile: domain_budget::EntryBudget,
    identity: domain_budget::EntryIdentity,
    #[cfg(feature = "ordinary-domain-proof")] permit: Option<super::clock_boundary::Permit>,
) -> Result<domain_budget::NativeEntryReceipt, DomainRefusal> {
    run_context(
        context,
        profile,
        #[cfg(feature = "ordinary-domain-proof")]
        permit,
        Some(identity),
    )?
    .1
    .ok_or(DomainRefusal::BackendFailure)
}
fn run_context(
    context: EntryContext,
    profile: domain_budget::EntryBudget,
    #[cfg(feature = "ordinary-domain-proof")] permit: Option<super::clock_boundary::Permit>,
    identity: Option<domain_budget::EntryIdentity>,
) -> Result<(TransitionReturn, Option<domain_budget::NativeEntryReceipt>), DomainRefusal> {
    let _interrupts = super::cpu::InterruptMask::new();
    if ACTIVE.swap(true, Ordering::AcqRel) {
        return Err(DomainRefusal::InvalidLifecycle);
    }
    unsafe {
        gdt::set_ring0_stack(context.trap_stack);
        idt::install_handler(
            0x80,
            conduitos_ordinary_gate as *const () as u64,
            idt::USER_INTERRUPT_GATE,
        );
        idt::install_handler(13, conduitos_ordinary_gp as *const () as u64, 0x8e);
        idt::install_handler(14, conduitos_ordinary_pf as *const () as u64, 0x8e);
        idt::install_handler(6, conduitos_ordinary_ud as *const () as u64, 0x8e);
        idt::install_handler(7, conduitos_ordinary_nm as *const () as u64, 0x8e);
        idt::install_handler(0, conduitos_ordinary_exception_0 as *const () as u64, 0x8e);
        idt::install_handler(1, conduitos_ordinary_exception_1 as *const () as u64, 0x8e);
        idt::install_handler(3, conduitos_ordinary_exception_3 as *const () as u64, 0x8e);
        idt::install_handler(4, conduitos_ordinary_exception_4 as *const () as u64, 0x8e);
        idt::install_handler(5, conduitos_ordinary_exception_5 as *const () as u64, 0x8e);
        idt::install_handler(
            10,
            conduitos_ordinary_exception_10 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            11,
            conduitos_ordinary_exception_11 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            12,
            conduitos_ordinary_exception_12 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            16,
            conduitos_ordinary_exception_16 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            17,
            conduitos_ordinary_exception_17 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            19,
            conduitos_ordinary_exception_19 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            21,
            conduitos_ordinary_exception_21 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            30,
            conduitos_ordinary_exception_30 as *const () as u64,
            0x8e,
        );

        idt::install_handler(
            super::TIMER_IRQ_VECTOR,
            conduitos_ordinary_timer as *const () as u64,
            0x8e,
        );
    }
    unsafe {
        idt::install_handler(
            domain_budget::VECTOR,
            conduitos_ordinary_budget as *const () as u64,
            0x8e,
        );
    }
    let budget = match domain_budget::Budget::arm(profile) {
        Ok(budget) => budget,
        Err(error) => {
            ACTIVE.store(false, Ordering::Release);
            return Err(error);
        }
    };
    #[cfg(feature = "ordinary-domain-proof")]
    let clock = match super::clock_boundary::begin(permit, &context) {
        Ok(clock) => clock,
        Err(error) => {
            drop(budget);
            ACTIVE.store(false, Ordering::Release);
            return Err(error);
        }
    };
    let result = unsafe {
        conduitos_ordinary_enter(context.entry, context.stack, context.frame, context.cr3)
    };
    #[cfg(feature = "ordinary-domain-proof")]
    if let Some(observed) = super::clock_boundary::finish(clock) {
        crate::ordinary_domain_proof::observe_clock_boundary(observed, result.value, result.origin);
    }
    #[cfg(feature = "ordinary-domain-proof")]
    crate::ordinary_domain_proof::observe_trap_envelope(
        result.value,
        result.origin,
        trap_observation::take(),
    );
    #[cfg(feature = "ordinary-domain-proof")]
    crate::ordinary_domain_proof::observe_transition_return(result.value, result.origin);
    // Snapshot while the live Budget and masked exclusive entry still exist.
    // Keep the capture Result local: even a refused observation must restore
    // RTC routing and release ACTIVE before propagating its error.
    let receipt = identity.map(|identity| budget.capture_receipt(identity, &result));
    drop(budget);
    ACTIVE.store(false, Ordering::Release);
    #[cfg(feature = "ordinary-domain-proof")]
    super::super::domain_context_probe::finish()?;
    Ok((result, receipt.transpose()?))
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_ordinary_timer_handler(user: u64, budget: u64, saved: *const u64) -> u64 {
    #[cfg(feature = "ordinary-domain-proof")]
    let sampled = unsafe { super::irq_envelope::before_handler(saved, user, budget) };
    #[cfg(feature = "ordinary-domain-proof")]
    super::super::domain_context_probe::observe_irq();
    let disposition = if budget != 0 {
        u64::from(domain_budget::interrupt(
            user & 3 == 3 && ACTIVE.load(Ordering::Acquire),
        ))
    } else {
        if user & 3 == 3 {
            domain_budget::source_interrupt();
        }
        // Source timer facts remain Source facts even while a domain is running.
        super::irq::conduitos_timer_irq_handler();
        0
    };
    if disposition != 0 && budget != 0 && user & 3 == 3 && ACTIVE.load(Ordering::Acquire) {
        unsafe { domain_budget::observe_fault_pc(saved) };
    }
    #[cfg(feature = "ordinary-domain-proof")]
    if sampled {
        unsafe {
            super::irq_envelope::after_handler(saved, disposition);
        }
    }
    disposition
}

global_asm!(
    include_str!("domain_transition/entry_return.S"),
    proof_envelope = const cfg!(feature = "ordinary-domain-proof") as u8,
    user_data = const gdt::USER_DATA_SELECTOR,
    user_code = const gdt::USER_CODE_SELECTOR,
);
