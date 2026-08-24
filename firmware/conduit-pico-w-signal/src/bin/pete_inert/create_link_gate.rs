//! Explicit authority gate for the Create UART electrical boundary.

use portable_atomic::{AtomicBool, Ordering};

use crate::create_play::{self, RequestState};

static TRANSLATOR_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn authorized() -> bool {
    matches!(
        create_play::snapshot().state,
        RequestState::Preparing
            | RequestState::Pending
            | RequestState::Active
            | RequestState::Withdrawal
    )
}

pub fn set_translator(enabled: bool) {
    TRANSLATOR_ENABLED.store(enabled, Ordering::Release);
}

pub fn translator_enabled() -> bool {
    TRANSLATOR_ENABLED.load(Ordering::Acquire)
}
