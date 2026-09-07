//! Exact xHCI completion correlation for USB control transfers.

use super::UsbError;
use crate::arch::x86_64::xhci::Event;

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
