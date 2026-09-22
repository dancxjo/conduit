//! Explicit authority gate for the Create UART electrical boundary.

use embassy_rp::gpio::Output;
use portable_atomic::{AtomicBool, Ordering};

use crate::create_play::{self, RequestState};

static TRANSLATOR_ENABLED: AtomicBool = AtomicBool::new(false);
static PHYSICAL_EMERGENCY_LATCHED: AtomicBool = AtomicBool::new(false);

pub fn authorized() -> bool {
    !emergency_latched()
        && matches!(
        create_play::snapshot().state,
        RequestState::Preparing
            | RequestState::Pending
            | RequestState::Active
            | RequestState::Withdrawal
        )
}

pub fn set_translator(translator_oe: &mut Output<'static>, enabled: bool) {
    let enabled = enabled && !emergency_latched();
    if enabled {
        translator_oe.set_high();
    } else {
        translator_oe.set_low();
    }
    TRANSLATOR_ENABLED.store(enabled, Ordering::Release);
}

/// Latch Pete's target-specific local actuation boundary off. GPIO19 was
/// configured as the translator output during boot and remains owned by the
/// Create task; the RP2040 atomic SIO clear is the only cross-task operation.
pub fn latch_physical_emergency() {
    PHYSICAL_EMERGENCY_LATCHED.store(true, Ordering::Release);
    unsafe {
        core::ptr::write_volatile(0xd000_0018 as *mut u32, 1 << 19);
    }
    TRANSLATOR_ENABLED.store(false, Ordering::Release);
}

pub fn emergency_latched() -> bool {
    PHYSICAL_EMERGENCY_LATCHED.load(Ordering::Acquire)
}

pub fn translator_enabled() -> bool {
    TRANSLATOR_ENABLED.load(Ordering::Acquire)
}
