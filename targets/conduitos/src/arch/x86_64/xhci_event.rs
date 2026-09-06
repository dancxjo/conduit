//! Consume an xHCI event only after the controller publishes its cycle bit.

use core::sync::atomic::{Ordering, fence};

use super::Event;

/// The controller owns the payload until control's cycle matches the consumer.
/// Read ownership first, then acquire the published payload. Reading the whole
/// TRB before testing its cycle can mix an old pointer with a newly written
/// control word when the controller reuses a ring slot.
pub(super) fn read_owned_event(
    cycle: u32,
    mut read_word: impl FnMut(usize) -> u32,
) -> Option<Event> {
    let control = read_word(3);
    if control & 1 != cycle {
        return None;
    }
    // This module is x86-only: coherent DMA and x86 load ordering provide the
    // hardware ordering; acquire also keeps payload loads after ownership.
    fence(Ordering::Acquire);
    let low = read_word(0);
    let high = read_word(1);
    let status = read_word(2);
    Some(Event {
        event_type: ((control >> 10) & 0x3f) as u8,
        completion_code: (status >> 24) as u8,
        slot: (control >> 24) as u8,
        endpoint: ((control >> 16) & 0x1f) as u8,
        residual: status & 0x00ff_ffff,
        pointer: u64::from(low) | (u64::from(high) << 32),
    })
}

#[cfg(test)]
#[path = "xhci_event_tests.rs"]
mod tests;
