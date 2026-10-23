//! Three actual resume/preservation controls and three terminal preempt controls.
use super::*;
pub(super) fn run() {
    for trial in 0..6 {
        let mut domain = arch::ServiceDomain::install_clock_fixture()
            .unwrap_or_else(|_| refuse("software-irq-install"));
        let cr4 = arch::ServiceDomain::proof_root_cr4();
        request(&mut domain, if trial < 3 { 23 } else { 17 }, 0, &[]);
        domain
            .arm_irq_fixture(trial)
            .unwrap_or_else(|_| refuse("software-irq-arm"));
        let returned = domain.enter(1);
        // Consume once even on a failed entry; never retry this specimen.
        let observed = domain.finish_irq_fixture();
        if observed.is_err() || arch::ServiceDomain::proof_root_cr4() != cr4 {
            refuse("software-irq-observation");
        }
        if trial < 3 {
            if returned != Ok(DomainReturn::Yielded) || exchange(&mut domain).status != 0 {
                refuse("software-irq-live-registers");
            }
        } else if returned != Ok(DomainReturn::Fault(DomainFault::WorkExhausted))
            || domain.enter(1).is_ok()
            || domain.arm_irq_fixture(trial).is_ok()
        {
            refuse("software-irq-terminal-preempt");
        }
        if domain.finish_irq_fixture().is_ok() {
            refuse("software-irq-duplicate-consume");
        }
        arch::early_write(if trial < 3 {
            b"CONDUIT_SERVICE_SOFTWARE_IRQ_CONTROL resume-gprs15-and-df-checked
"
        } else {
            b"CONDUIT_SERVICE_SOFTWARE_IRQ_CONTROL preempt-terminal-old-owner-refused
"
        });
    }
}
