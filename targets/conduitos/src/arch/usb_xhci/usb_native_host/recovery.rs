//! Single, exactly halted queue retirement. Class recovery remains checked Source.
use super::*;

impl UsbNativeHost<'_> {
    pub(super) fn finish_single_stalled_endpoint(
        &mut self,
        index: UsbNativeCallIndex,
    ) -> Result<bool, UsbNativeHostRefusal> {
        let pending = [self.pending_endpoint, self.pending_write];
        if pending.iter().filter(|entry| entry.is_some()).count() != 1
            || !pending.contains(&Some(index.0))
            || self.uncertain_control
            || self.uncertain_bulk
            || self.recovery_endpoint.is_some()
            || self.isochronous.is_some()
            || self.cancellation_requested
            || self.stop_acknowledged
            || self.stopped
            || self
                .bulk_terminals
                .as_ref()
                .is_none_or(|slots| slots.iter().any(|slot| slot.index.is_some()))
        {
            return Ok(false);
        }
        self.validate_continuity()?;
        // Every other selected queue, including EP0, must be genuinely idle.
        for (position, bound) in self.calls.iter().enumerate() {
            if position == index.0 {
                continue;
            }
            let idle = match &bound.transfer {
                Transfer::Control(t) => t.validate_settlement_idle(&self.device).is_ok(),
                Transfer::Endpoint(t) => t.validate_settlement_idle(&self.device).is_ok(),
                Transfer::Write(t) => t.validate_settlement_idle(&self.device).is_ok(),
            };
            if !idle {
                return Ok(false);
            }
        }
        let endpoint = match &self.calls[index.0].transfer {
            Transfer::Endpoint(t) => t.endpoint_dci,
            Transfer::Write(t) => t.endpoint_dci,
            _ => return Err(UsbNativeHostRefusal::Binding),
        };
        // This path is called only after complete() validated an exact original
        // STALL event. Reset Endpoint moves that Halted queue to Stopped. A
        // failed or foreign acknowledgement retains its original lease and DMA.
        self.uncertain_bulk = true;
        self.recovery_endpoint = Some(index.0);
        unsafe {
            self.controller
                .reset_original_halted_endpoint(self.device.slot, endpoint)
        }
        .map_err(|error| UsbNativeHostRefusal::Native(error.into()))?;
        self.validate_continuity()?;
        let (direction, bytes) = match &mut self.calls[index.0].transfer {
            Transfer::Endpoint(t) => (
                0,
                unsafe { t.finish_fault_quiesced(&self.device, EndpointReadDisposition::Stalled) }
                    .map_err(UsbNativeHostRefusal::Endpoint)?,
            ),
            Transfer::Write(t) => (
                1,
                unsafe { t.finish_fault_quiesced(&self.device, EndpointWriteDisposition::Stalled) }
                    .map_err(UsbNativeHostRefusal::Write)?,
            ),
            _ => return Err(UsbNativeHostRefusal::Binding),
        };
        let terminal = &mut self
            .bulk_terminals
            .as_mut()
            .ok_or(UsbNativeHostRefusal::NotReady)?[direction];
        terminal.bytes[..bytes.len()].copy_from_slice(bytes);
        terminal.length = bytes.len();
        terminal.index = Some(index);
        self.pending_endpoint = None;
        self.pending_write = None;
        self.uncertain_bulk = false;
        self.bulk_terminals_ready = true;
        Ok(true)
    }

    /// # Safety
    /// Dispatcher has verified that every original bulk Kernel obligation and
    /// token has retired. Native idle alone never authorizes queue reuse.
    pub(in crate::arch::usb_xhci) unsafe fn prepare_endpoint_recovery(
        &mut self,
    ) -> Result<(), UsbNativeHostRefusal> {
        let Some(index) = self.recovery_endpoint else {
            return Ok(());
        };
        if self.pending_endpoint.is_some()
            || self.pending_write.is_some()
            || self.uncertain_control
            || self.uncertain_bulk
            || self.stopped
            || self.stop_acknowledged
            || self.cancellation_requested
            || self
                .bulk_terminals
                .as_ref()
                .is_some_and(|slots| slots.iter().any(|slot| slot.index.is_some()))
        {
            return Err(UsbNativeHostRefusal::Pending);
        }
        self.validate_continuity()?;
        let (endpoint, (address, cycle)) = match &self.calls[index].transfer {
            Transfer::Endpoint(t) => (
                t.endpoint_dci,
                t.retired_dequeue(&self.device)
                    .map_err(UsbNativeHostRefusal::Endpoint)?,
            ),
            Transfer::Write(t) => (
                t.endpoint_dci,
                t.retired_dequeue(&self.device)
                    .map_err(UsbNativeHostRefusal::Write)?,
            ),
            _ => return Err(UsbNativeHostRefusal::Binding),
        };
        self.uncertain_bulk = true;
        unsafe {
            self.controller.set_original_stopped_endpoint_dequeue(
                self.device.slot,
                endpoint,
                address,
                cycle,
            )
        }
        .map_err(|error| UsbNativeHostRefusal::Native(error.into()))?;
        self.validate_continuity()?;
        self.recovery_endpoint = None;
        self.uncertain_bulk = false;
        Ok(())
    }
}
