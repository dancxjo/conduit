//! Exact xHCI completion correlation for USB control transfers.

use super::UsbError;
use crate::arch::x86_64::xhci::Event;

/// xHCI Setup Stage TRT (6.4.1.2.1): no data=0, OUT=2, IN=3.
/// This encodes controller machinery, not a USB class request or its payload.
pub(super) const fn setup_transfer_type(input: bool, length: u16) -> u32 {
    let transfer_type = if length == 0 {
        0
    } else if input {
        3
    } else {
        2
    };
    transfer_type << 16
}

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

/// Hardware stages for one submitted control operation. A short Data Stage is
/// not the final Status Stage, and its residue must survive the later success.
pub(super) struct ControlCompletion {
    slot: u8,
    setup_pointer: u64,
    data_pointer: Option<u64>,
    status_pointer: u64,
    requested: u16,
    input: bool,
    short_residual: Option<u32>,
    finished: bool,
}

pub(super) struct ControlTransferResult {
    pub(super) bytes: usize,
    pub(super) short: bool,
}

impl ControlCompletion {
    pub(super) const fn new(
        slot: u8,
        setup_pointer: u64,
        data_pointer: Option<u64>,
        status_pointer: u64,
        requested: u16,
        input: bool,
    ) -> Self {
        Self {
            slot,
            setup_pointer,
            data_pointer,
            status_pointer,
            requested,
            input,
            short_residual: None,
            finished: false,
        }
    }

    pub(super) fn observe(
        &mut self,
        event: Event,
    ) -> Result<Option<ControlTransferResult>, UsbError> {
        if self.finished {
            return Err(UsbError::MalformedCompletion);
        }
        if event.event_type == 34 {
            return Ok(None);
        }
        let data = self.data_pointer == Some(event.pointer);
        let status = event.pointer == self.status_pointer;
        if !data && !status && event.pointer != self.setup_pointer {
            return Err(UsbError::WrongController);
        }
        let short = validate_transfer_event(event, self.slot, event.pointer)?;
        if data && short && self.input && self.requested != 0 {
            if self.short_residual.is_some() {
                return Err(UsbError::MalformedCompletion);
            }
            transferred_bytes(self.requested, event.residual)?;
            self.short_residual = Some(event.residual);
            return Ok(None);
        }
        if !status || short || event.residual != 0 {
            return Err(UsbError::MalformedCompletion);
        }
        let bytes = transferred_bytes(self.requested, self.short_residual.unwrap_or(0))?;
        self.finished = true;
        Ok(Some(ControlTransferResult {
            bytes,
            short: self.short_residual.is_some(),
        }))
    }
}

#[cfg(test)]
#[path = "usb_completion_tests.rs"]
mod completion_tests;
