//! Preserve one admitted Source timer wake during independent User preemption.
use super::refuse;
use crate::arch;

pub(super) fn run() {
    use crate::{
        machine::{KernelInterest, TimerBase},
        protected_region::{DomainBackend, DomainFault, DomainReturn},
    };
    use conduit_kernel::{BoundedValueRef, NodeId, RequestId, ValueRef};
    let interest = KernelInterest {
        node: NodeId(0),
        request: RequestId(71),
        input: BoundedValueRef::new(
            ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 4,
            },
            4,
        )
        .unwrap_or_else(|_| refuse("timer-fixture-interest")),
    };
    let mut domain = arch::TextDomain::install().unwrap_or_else(|_| refuse("timer-fixture-domain"));
    domain.probe(6, 0);
    let mut timer = arch::Timer::new();
    timer
        .arm(interest)
        .unwrap_or_else(|_| refuse("timer-fixture-arm"));
    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    ))]
    arch::start_pending_source_timer();
    let returned = domain.enter(1);
    let mut sign = crate::sign_format::FixedText::new();
    use core::fmt::Write;
    let _ = writeln!(
        sign,
        "CONDUIT_DOMAIN_TIMER_COUNTS source={} interrupts={}",
        domain.cost().source_timer_interrupts,
        domain.cost().interrupt_entries
    );
    arch::early_write(sign.as_bytes());
    if returned != Ok(DomainReturn::Fault(DomainFault::WorkExhausted))
        || domain.cost().source_timer_interrupts != 1
        || timer.take_wake() != Ok(Some(interest))
        || timer.take_wake() != Ok(None)
        || timer.wake_count() != 1
    {
        refuse("pending-source-timer-lost");
    }
    arch::early_write(
        b"CONDUIT_DOMAIN_TIMER_COEXISTENCE source-wake-once user-irq budget-preemption
",
    );
}
