//! Independent IA-32 hostile entries following the ordinary product Play.
use crate::{
    arch,
    protected_region::{DomainBackend, DomainFault, DomainReturn},
};

#[path = "ordinary_domain_proof/gates.rs"]
mod gates;
#[path = "ordinary_domain_proof/morse.rs"]
mod morse;
#[path = "ordinary_domain_proof/timer.rs"]
mod timer;
#[path = "ordinary_domain_proof/timer_runtime.rs"]
mod timer_runtime;

pub fn run(plan: &conduit_core::Plan, offer: &crate::offer::HostOffer<'_>) {
    gates::run(plan, offer);
    morse::run(plan, offer);
    timer_runtime::run(offer);
    hostile_entries();
    timer::run();
}

fn hostile_entries() {
    let private = 0x5a5a_5a5a_u32;
    let capabilities = crate::protection_domain::KernelCapabilityTable::new(7)
        .unwrap_or_else(|_| refuse("capability-table"));
    let mut sibling = arch::TextDomain::install().unwrap_or_else(|_| refuse("sibling"));
    sibling
        .input(b"sentinel")
        .unwrap_or_else(|_| refuse("sibling-input"));
    let sibling_address = sibling.private_frame_address();
    let sibling_before = unsafe { (sibling_address as *const u32).read_volatile() };
    for (command, target, expected) in [
        (1, &private as *const u32 as u64, DomainFault::Memory),
        (1, &capabilities as *const _ as u64, DomainFault::Memory),
        (2, sibling_address, DomainFault::Memory),
        (
            3,
            arch::early_write as *const () as u64,
            DomainFault::Memory,
        ),
        (1, 0xfee0_0000, DomainFault::Memory),
        (4, 0, DomainFault::PrivilegedOperation),
        (5, 0, DomainFault::PrivilegedOperation),
        (6, 0, DomainFault::WorkExhausted),
        (17, 0, DomainFault::WorkExhausted),
        (8, 0, DomainFault::InvalidInstruction),
        (9, 0, DomainFault::PrivilegedOperation),
        (10, 0, DomainFault::InvalidInstruction),
        (11, 0, DomainFault::PrivilegedOperation),
        (12, 0, DomainFault::InvalidInstruction),
        (14, 0, DomainFault::PrivilegedOperation),
        (
            2,
            crate::domain_image::IA32_USER_TEXT_START,
            DomainFault::Memory,
        ),
        (
            3,
            crate::domain_image::IA32_USER_TEXT_START + 0x10000,
            DomainFault::Memory,
        ),
    ] {
        let mut domain = arch::TextDomain::install().unwrap_or_else(|_| refuse("install"));
        domain.probe(command, target);
        let returned = domain.enter(1);
        let mut sign = crate::sign_format::FixedText::new();
        use core::fmt::Write;
        let _ = writeln!(
            sign,
            "CONDUIT_IA32_DOMAIN_PROBE command={command} target={target:#x} result={returned:?}"
        );
        arch::early_write(sign.as_bytes());
        if returned != Ok(DomainReturn::Fault(expected)) {
            refuse("hostile-entry-did-not-fault");
        }
        if domain.cost().scheduler_returns != 1 {
            refuse("hostile-entry-did-not-return");
        }
        if matches!(command, 6 | 17)
            && (domain.cost().preemptions != 1 || domain.cost().interrupt_entries < 3)
        {
            refuse("loop-did-not-preempt");
        }
    }
    let mut floating = arch::TextDomain::install().unwrap_or_else(|_| refuse("floating-install"));
    floating.probe(7, 0);
    if floating.enter(1) != Ok(DomainReturn::Yielded) {
        refuse("floating-state-not-restored");
    }
    arch::early_write(b"CONDUIT_IA32_DOMAIN_FLOATING restored-x87-mxcsr-xmm-before-rust\n");
    if private != 0x5a5a_5a5a
        || unsafe { (sibling_address as *const u32).read_volatile() } != sibling_before
    {
        refuse("private-state-changed");
    }
    arch::early_write(b"CONDUIT_IA32_DOMAIN_NEGATIVES root-memory capability-memory sibling-memory root-entry mmio ports cli loop direction-flag syscall sysenter divide breakpoint single-step rdtsc code-write data-execute\n");
}

fn refuse(reason: &str) -> ! {
    arch::early_write(b"CONDUIT_IA32_DOMAIN_REFUSAL ");
    arch::early_write(reason.as_bytes());
    arch::early_write(b"\n");
    loop {
        core::hint::spin_loop();
    }
}
