//! Independent EL0 boundary checks following the ordinary product Play.
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

pub fn run(plan: &conduit_core::Plan, offer: &crate::offer::HostOffer<'_>) {
    gates::run(plan, offer);
    morse::run(plan, offer);
    boundary_entries();
    timer::run();
}

fn boundary_entries() {
    let private = 0x5a5a_5a5a_u64;
    let capabilities = crate::protection_domain::KernelCapabilityTable::new(7)
        .unwrap_or_else(|_| refuse("capability-table"));
    let mut sibling = arch::TextDomain::install().unwrap_or_else(|_| refuse("sibling"));
    sibling
        .input(b"sentinel")
        .unwrap_or_else(|_| refuse("sibling-input"));
    let sibling_address = sibling.private_frame_address();
    let sibling_before = unsafe { (sibling_address as *const u64).read_volatile() };
    for (command, target, expected) in [
        (1, &private as *const u64 as u64, DomainFault::Memory),
        (1, &capabilities as *const _ as u64, DomainFault::Memory),
        (2, sibling_address, DomainFault::Memory),
        (
            3,
            arch::early_write as *const () as u64,
            DomainFault::Memory,
        ),
        (1, 0x0800_0000, DomainFault::Memory),
        (2, 0x0900_0000, DomainFault::Memory),
        // EL0 access to this EL1-only register is an undefined instruction;
        // retain the processor's classification instead of inventing a trap.
        (4, 0, DomainFault::InvalidInstruction),
        (5, 0, DomainFault::PrivilegedOperation),
        (6, 0, DomainFault::WorkExhausted),
        (17, 0, DomainFault::WorkExhausted),
        (8, 0, DomainFault::InvalidInstruction),
        (9, 0, DomainFault::InvalidInstruction),
        (10, 0, DomainFault::InvalidInstruction),
        (11, 0, DomainFault::PrivilegedOperation),
        (12, 0, DomainFault::PrivilegedOperation),
        (13, 0, DomainFault::InvalidGate),
        (14, 0, DomainFault::PrivilegedOperation),
        (2, crate::domain_image::USER_TEXT_START, DomainFault::Memory),
        (
            3,
            crate::domain_image::USER_TEXT_START + 0x10000,
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
            "CONDUIT_AARCH64_DOMAIN_PROBE command={command} target={target:#x} result={returned:?}"
        );
        arch::early_write(sign.as_bytes());
        if returned != Ok(DomainReturn::Fault(expected)) {
            refuse("boundary-entry-did-not-fault");
        }
        if domain.cost().scheduler_returns != 1 {
            refuse("boundary-entry-did-not-return");
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
    arch::early_write(
        b"CONDUIT_AARCH64_DOMAIN_FLOATING restored-q0-q31-fpcr-fpsr-before-rust-and-irq-handler\n",
    );
    if private != 0x5a5a_5a5a
        || unsafe { (sibling_address as *const u64).read_volatile() } != sibling_before
    {
        refuse("private-state-changed");
    }
    arch::early_write(b"CONDUIT_AARCH64_DOMAIN_NEGATIVES root-memory capability-memory sibling-memory root-entry gic uart translation-register irq-mask loop floating-loop eret hvc smc breakpoint counter alternate-gate timer-control code-write data-execute\n");
}

fn refuse(reason: &str) -> ! {
    arch::early_write(b"CONDUIT_AARCH64_DOMAIN_REFUSAL ");
    arch::early_write(reason.as_bytes());
    arch::early_write(b"\n");
    loop {
        core::hint::spin_loop();
    }
}
