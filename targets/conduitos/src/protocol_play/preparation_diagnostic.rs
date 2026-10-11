//! Bounded preparation diagnostics; preserve every original refusal.
use super::ProtocolCallRefusal;
use core::fmt::{Debug, Write};

pub(super) fn observe<E: Debug>(stage: &str, error: E) -> E {
    if crate::allocation::BOOT_ARENA.capacity() != 0 {
        let mut line = crate::sign_format::FixedText::new();
        if writeln!(
            line,
            "CONDUIT_PROTOCOL_PLAN_REFUSAL stage={stage} reason={error:?}"
        )
        .is_ok()
        {
            crate::arch::early_write(line.as_bytes());
        }
    }
    error
}

pub(super) fn invalid<E: Debug>(stage: &str, error: E) -> ProtocolCallRefusal {
    observe(stage, error);
    ProtocolCallRefusal::InvalidPlan
}
