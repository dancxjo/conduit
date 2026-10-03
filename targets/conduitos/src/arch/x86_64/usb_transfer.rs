//! Exact xHCI completion correlation for USB control transfers.

use super::UsbError;
use crate::arch::x86_64::xhci::Event;

/// Controller residuals are 24-bit counts, not narrowed request lengths.
/// A residual outside the submitted envelope is malformed hardware truth;
/// saturating subtraction would falsely report a successful short transfer.
pub(super) fn transferred_bytes(requested: u16, residual: u32) -> Result<usize, UsbError> {
    u32::from(requested)
        .checked_sub(residual)
        .map(|bytes| bytes as usize)
        .ok_or(UsbError::MalformedCompletion)
}

pub(super) fn validate_transfer_event(
    event: Event,
    slot: u8,
    pointer: u64,
) -> Result<bool, UsbError> {
    if event.event_type != 32 || event.pointer != pointer {
        return Err(UsbError::WrongController);
    }
    if event.slot != slot {
        return Err(UsbError::WrongSlot);
    }
    if event.endpoint != 1 {
        return Err(UsbError::WrongEndpoint);
    }
    match event.completion_code {
        1 => Ok(false),
        13 => Ok(true),
        6 => Err(UsbError::ControlStall),
        _ => Err(UsbError::ControlError),
    }
}
