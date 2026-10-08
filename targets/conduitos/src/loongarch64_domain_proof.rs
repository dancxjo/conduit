//! Independent PLV3 boundary checks following the ordinary product Play.
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
        (2, arch::entropy_storage_address(), DomainFault::Memory),
        (2, sibling_address, DomainFault::Memory),
        (
            3,
            arch::early_write as *const () as u64,
            DomainFault::Memory,
        ),
        (1, 0x1e02_0000, DomainFault::Memory),
        (1, 0x2000_0000, DomainFault::Memory),
        (2, 0x1fe0_01e0, DomainFault::Memory),
        // Root control register access is a privilege error at PLV3.
        (4, 0, DomainFault::PrivilegedOperation),
        (5, 0, DomainFault::PrivilegedOperation),
        (6, 0, DomainFault::WorkExhausted),
        (17, 0, DomainFault::WorkExhausted),
        (8, 0, DomainFault::PrivilegedOperation),
        (9, 0, DomainFault::PrivilegedOperation),
        (11, 0, DomainFault::InvalidInstruction),
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
            "CONDUIT_LOONGARCH64_DOMAIN_PROBE command={command} target={target:#x} result={returned:?}"
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
    let mut idle = arch::TextDomain::install().unwrap_or_else(|_| refuse("idle-install"));
    idle.probe(16, 0);
    match idle.enter(1) {
        Ok(DomainReturn::Fault(DomainFault::PrivilegedOperation)) => {}
        Ok(DomainReturn::Fault(DomainFault::WorkExhausted))
            if idle.cost().preemptions == 1 && idle.cost().interrupt_entries >= 3 => {}
        _ => refuse("idle-did-not-return"),
    }
    drop(idle);
    let mut stackless = arch::TextDomain::install().unwrap_or_else(|_| refuse("stackless-install"));
    stackless.probe(18, 0);
    if stackless.enter(1) != Ok(DomainReturn::Yielded) {
        refuse("stackless-gate");
    }
    drop(stackless);
    let mut stackless_loop =
        arch::TextDomain::install().unwrap_or_else(|_| refuse("stackless-loop-install"));
    stackless_loop.probe(19, 0);
    if stackless_loop.enter(1) != Ok(DomainReturn::Fault(DomainFault::WorkExhausted)) {
        refuse("stackless-loop");
    }
    drop(stackless_loop);
    let mut floating = arch::TextDomain::install().unwrap_or_else(|_| refuse("floating-install"));
    floating.probe(7, 0);
    if floating.enter(1) != Ok(DomainReturn::Yielded) {
        refuse("floating-state-not-restored");
    }
    arch::early_write(
        b"CONDUIT_LOONGARCH64_DOMAIN_FLOATING restored-f0-f31-fcsr-fcc-before-rust-and-irq-handler\n",
    );
    if private != 0x5a5a_5a5a
        || unsafe { (sibling_address as *const u64).read_volatile() } != sibling_before
    {
        refuse("private-state-changed");
    }
    arch::early_write(b"CONDUIT_LOONGARCH64_DOMAIN_NEGATIVES root-memory capability-memory entropy-memory sibling-memory root-entry fwcfg pci uart translation-register irq-mask loop floating-loop ertn iocsr breakpoint counter alternate-gate timer-control idle code-write data-execute stackless-gate stackless-loop\n");
}

fn refuse(reason: &str) -> ! {
    arch::early_write(b"CONDUIT_LOONGARCH64_DOMAIN_REFUSAL ");
    arch::early_write(reason.as_bytes());
    arch::early_write(b"\n");
    loop {
        core::hint::spin_loop();
    }
}
