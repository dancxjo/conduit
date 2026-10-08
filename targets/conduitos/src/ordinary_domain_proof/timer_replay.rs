//! A terminal product owner must refuse before any trusted provider access.
use crate::{
    composition::MachineRunError,
    machine::{
        BaseError, IdleBase, InterruptBase, InterruptState, KernelInterest, MonotonicClockBase,
        SerialBase, TimerBase, TimerToken,
    },
    protected_region::DomainRefusal,
    tour_timer_plan::PreparedTourTimerPlan,
};

pub(super) fn verify(prepared: &mut PreparedTourTimerPlan) {
    let returned = prepared.kernel.run(
        &mut NoAccess,
        &mut NoAccess,
        &mut NoAccess,
        &mut NoAccess,
        &mut NoAccess,
    );
    if !matches!(
        returned,
        Err(MachineRunError::ProtectionDomain(
            DomainRefusal::InvalidLifecycle
        ))
    ) {
        super::refuse("timer-product-terminal-owner-replayed");
    }
    crate::arch::early_write(b"CONDUIT_DOMAIN_TIMER_REPLAY refused-before-provider-access\n");
}

struct NoAccess;
fn touched() -> ! {
    super::refuse("timer-product-replay-provider-touched")
}
impl MonotonicClockBase for NoAccess {
    fn now(&mut self) -> u64 {
        touched()
    }
    fn provider_generation(&self) -> Option<u64> {
        touched()
    }
}
impl TimerBase for NoAccess {
    fn arm(&mut self, _: KernelInterest) -> Result<TimerToken, BaseError> {
        touched()
    }
    fn arm_after_milliseconds(
        &mut self,
        _: KernelInterest,
        _: u64,
    ) -> Result<TimerToken, BaseError> {
        touched()
    }
    fn cancel(&mut self, _: TimerToken) -> Result<KernelInterest, BaseError> {
        touched()
    }
    fn take_wake(&mut self) -> Result<Option<KernelInterest>, BaseError> {
        touched()
    }
    fn wake_count(&self) -> u32 {
        touched()
    }
    fn provider_generation(&self) -> Option<u64> {
        touched()
    }
}
impl SerialBase for NoAccess {
    fn present(&mut self, _: &[u8]) -> Result<(), BaseError> {
        touched()
    }
    fn presentation_count(&self) -> u32 {
        touched()
    }
    fn provider_generation(&self) -> Option<u64> {
        touched()
    }
}
impl InterruptBase for NoAccess {
    fn enable(&mut self) {
        touched()
    }
    fn disable(&mut self) -> InterruptState {
        touched()
    }
    fn restore(&mut self, _: InterruptState) {
        touched()
    }
    fn is_enabled(&self) -> bool {
        touched()
    }
}
impl IdleBase for NoAccess {
    fn wait_for_interrupt(&mut self) -> Result<(), BaseError> {
        touched()
    }
    fn idle_count(&self) -> u32 {
        touched()
    }
}
