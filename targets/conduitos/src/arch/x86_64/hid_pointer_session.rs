//! One exact attachment's pending receive and normalized portable sample state.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PointerAttachment {
    slot: u8,
    root_port: u8,
    dma_slot: u8,
    epoch: u32,
}

impl PointerAttachment {
    pub(super) const fn new(device: &UsbDevice) -> Self {
        Self {
            slot: device.slot,
            root_port: device.root_port,
            dma_slot: device.dma_slot,
            epoch: device.attachment_epoch,
        }
    }
}

pub const fn start_pointer_session(ready: HidPointerReady) -> HidPointerSession {
    HidPointerSession {
        ready,
        next_transfer: 0,
        pending_transfer: None,
        position_x: 500_000,
        position_y: 500_000,
        primary_pressed: false,
        sequence: 0,
    }
}

impl HidPointerSession {
    pub const fn position(&self) -> (i64, i64) {
        (self.position_x, self.position_y)
    }
    pub const fn primary_pressed(&self) -> bool {
        self.primary_pressed
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub const fn physical_transfers(&self) -> usize {
        self.next_transfer
    }
    pub const fn ring_wraps(&self) -> usize {
        self.next_transfer / POINTER_TRANSFER_REPORT_SLOTS
    }
    pub const fn followup_pending(&self) -> bool {
        self.pending_transfer.is_some()
    }

    fn validate_attachment(&self, device: &UsbDevice) -> Result<(), HidPointerError> {
        if self.ready.attachment != PointerAttachment::new(device) {
            return Err(HidPointerError::StaleAttachment);
        }
        Ok(())
    }

    /// Publish one already-admitted physical report without waiting. A second
    /// begin refuses rather than overwriting an original pending transfer.
    pub fn begin_followup(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
    ) -> Result<(), HidPointerError> {
        self.validate_attachment(device)?;
        if self.pending_transfer.is_some() {
            return Err(HidPointerError::TransferOverflow);
        }
        let index = self.next_transfer;
        index
            .checked_add(1)
            .ok_or(HidPointerError::TransferOverflow)?;
        transfer::ensure_present(controller.port_status(device.root_port))?;
        transfer::submit_report(controller, device, self.ready, index);
        self.pending_transfer = Some(index);
        Ok(())
    }

    /// Absence is idle, not input loss. Foreign completions remain retained for
    /// their original endpoint; malformed own completion leaves this TD fenced.
    pub fn poll_followup(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
    ) -> Result<Option<NormalizedPointerSample>, HidPointerError> {
        self.validate_attachment(device)?;
        let index = self
            .pending_transfer
            .ok_or(HidPointerError::TransferOverflow)?;
        if transfer::poll_report(controller, device, self.ready, index)?.is_none() {
            return Ok(None);
        }
        self.consume_report(index).map(Some)
    }

    /// Exclusive legacy callers use the same admitted publication/completion,
    /// retaining their established finite wait bound without dropping peers.
    pub fn receive(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
    ) -> Result<NormalizedPointerSample, HidPointerError> {
        self.begin_followup(controller, device)?;
        let index = self
            .pending_transfer
            .ok_or(HidPointerError::TransferOverflow)?;
        transfer::wait_report(controller, device, self.ready, index)?;
        self.consume_report(index)
    }

    fn consume_report(&mut self, index: usize) -> Result<NormalizedPointerSample, HidPointerError> {
        let next_transfer = index
            .checked_add(1)
            .ok_or(HidPointerError::TransferOverflow)?;
        let report = unsafe { POINTER_DMA.reports[index % POINTER_REPORT_BUFFERS] };
        let sample = apply_report(self.position_x, self.position_y, self.sequence, report)?;
        self.position_x = sample.position_x;
        self.position_y = sample.position_y;
        self.primary_pressed = sample.primary_pressed;
        self.sequence = sample.sequence;
        self.next_transfer = next_transfer;
        self.pending_transfer = None;
        Ok(sample)
    }
}

#[cfg(test)]
#[path = "hid_pointer_session_tests.rs"]
mod tests;
