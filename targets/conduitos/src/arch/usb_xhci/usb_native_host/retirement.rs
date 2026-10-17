//! Terminal native fault completion and acknowledged shared retirement.
use super::*;

impl UsbNativeHost<'_> {
    pub fn control_invoke(
        &mut self,
        index: UsbNativeCallIndex,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], UsbNativeHostRefusal> {
        self.idle_transport()?;
        self.validate_continuity()?;
        let bound = self
            .calls
            .get_mut(index.0)
            .ok_or(UsbNativeHostRefusal::Binding)?;
        let Transfer::Control(transfer) = &mut bound.transfer else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        self.playing = true;
        // Conservatively retain transport exclusivity on any refused invocation;
        // no later call may consume an event from an uncertain submission.
        self.uncertain_control = true;
        let stalled = match unsafe {
            transfer.execute(
                &mut self.controller,
                &self.device,
                bound.node,
                bound.call,
                request,
                input,
            )
        } {
            Ok(bytes) => {
                self.control_output[..bytes.len()].copy_from_slice(bytes);
                self.control_output_length = bytes.len();
                false
            }
            Err(UsbControlCallRefusal::Native(UsbError::ControlStall)) => true,
            Err(error) => return Err(UsbNativeHostRefusal::Control(error)),
        };
        if !stalled {
            self.validate_continuity()?;
            self.uncertain_control = false;
            return Ok(&self.control_output[..self.control_output_length]);
        }
        // This is terminal fault completion, not cancellation. The faulting
        // lease stays admitted until its exact disposition can complete after
        // acknowledged native stop. uncertain_control closes every effect now.
        self.controller
            .disable_removed_slot(self.device.slot)
            .map_err(|error| UsbNativeHostRefusal::Native(error.into()))?;
        self.stop_acknowledged = true;
        let Transfer::Control(transfer) = &mut self.calls[index.0].transfer else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        // SAFETY: original controller acknowledged slot stop; original device
        // and live marker remain owned, and this exact lease was not revoked.
        let bytes = unsafe { transfer.finish_stalled_quiesced(&self.device) }
            .map_err(UsbNativeHostRefusal::Control)?;
        self.control_output[..bytes.len()].copy_from_slice(bytes);
        self.control_output_length = bytes.len();
        // Complete every remaining software retirement before typed publication.
        // Any refusal here retains the marker and prevents publishing success.
        self.cancel()?;
        Ok(&self.control_output[..self.control_output_length])
    }

    // Only the original controller's acknowledged Disable Slot can settle an
    // uncertain endpoint transfer. Keep pending state and DMA intact on failure.
    fn stop_for_endpoint_fault(&mut self) -> Result<(), UsbNativeHostRefusal> {
        self.controller
            .disable_removed_slot(self.device.slot)
            .map_err(|error| UsbNativeHostRefusal::Native(error.into()))?;
        self.stop_acknowledged = true;
        Ok(())
    }

    pub(super) fn finish_read_fault(
        &mut self,
        index: UsbNativeCallIndex,
        disposition: EndpointReadDisposition,
    ) -> Result<&[u8], UsbNativeHostRefusal> {
        if self.pending_endpoint != Some(index.0) {
            return Err(UsbNativeHostRefusal::Pending);
        }
        let Some(BoundCall {
            transfer: Transfer::Endpoint(transfer),
            ..
        }) = self.calls.get(index.0)
        else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        transfer
            .validate_device(&self.device)
            .map_err(UsbNativeHostRefusal::Endpoint)?;
        self.stop_for_endpoint_fault()?;
        let Transfer::Endpoint(transfer) = &mut self.calls[index.0].transfer else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        // SAFETY: sole original controller acknowledged slot stop, marker and
        // exact attachment remain live, and this faulting lease was not revoked.
        let bytes = unsafe { transfer.finish_fault_quiesced(&self.device, disposition) }
            .map_err(UsbNativeHostRefusal::Endpoint)?;
        self.control_output[..bytes.len()].copy_from_slice(bytes);
        self.control_output_length = bytes.len();
        // Retire all remaining software possession before terminal publication.
        // No subsequent effect may run on the stopped controller attachment.
        self.cancel()?;
        Ok(&self.control_output[..self.control_output_length])
    }

    pub(super) fn finish_write_fault(
        &mut self,
        index: UsbNativeCallIndex,
        disposition: EndpointWriteDisposition,
    ) -> Result<&[u8], UsbNativeHostRefusal> {
        if self.pending_endpoint != Some(index.0) {
            return Err(UsbNativeHostRefusal::Pending);
        }
        let Some(BoundCall {
            transfer: Transfer::Write(transfer),
            ..
        }) = self.calls.get(index.0)
        else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        transfer
            .validate_device(&self.device)
            .map_err(UsbNativeHostRefusal::Write)?;
        self.stop_for_endpoint_fault()?;
        let Transfer::Write(transfer) = &mut self.calls[index.0].transfer else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        // SAFETY: sole original controller acknowledged slot stop, marker and
        // exact attachment remain live, and this faulting lease was not revoked.
        let bytes = unsafe { transfer.finish_fault_quiesced(&self.device, disposition) }
            .map_err(UsbNativeHostRefusal::Write)?;
        self.control_output[..bytes.len()].copy_from_slice(bytes);
        self.control_output_length = bytes.len();
        // Retire all remaining software possession before terminal publication.
        // No subsequent effect may run on the stopped controller attachment.
        self.cancel()?;
        Ok(&self.control_output[..self.control_output_length])
    }

    /// Revoke every selected call first; acknowledged slot stop covers all DMA.
    /// Failed stop or retirement retains ownership markers and outstanding state.
    pub fn cancel(&mut self) -> Result<(), UsbNativeHostRefusal> {
        if self.stopped {
            return Ok(());
        }
        // Cancellation closes admission before revocation or hardware stop can fail.
        self.cancellation_requested = true;
        if !self.possessions_revoked {
            let mut refusal = None;
            for bound in &mut self.calls {
                let result = match &mut bound.transfer {
                    Transfer::Control(transfer) => transfer
                        .revoke(bound.node, bound.call)
                        .map_err(UsbNativeHostRefusal::Control),
                    Transfer::Endpoint(transfer) => transfer
                        .revoke(bound.node, bound.call)
                        .map_err(UsbNativeHostRefusal::Endpoint),
                    Transfer::Write(transfer) => transfer
                        .revoke(bound.node, bound.call)
                        .map_err(UsbNativeHostRefusal::Write),
                };
                if let Err(error) = result
                    && refusal.is_none()
                {
                    refusal = Some(error);
                }
            }
            if let Some(error) = refusal {
                return Err(error);
            }
            self.possessions_revoked = true;
        }
        let dma = device_dma_pointer(&self.device).map_err(UsbNativeHostRefusal::Native)?;
        if !self.stop_acknowledged {
            self.controller
                .disable_removed_slot(self.device.slot)
                .map_err(|error| UsbNativeHostRefusal::Native(error.into()))?;
            self.stop_acknowledged = true;
        }
        for bound in &mut self.calls {
            // SAFETY: the sole original controller acknowledged slot stop. The
            // original attachment marker remains intact until all leases retire.
            unsafe {
                match &mut bound.transfer {
                    Transfer::Control(transfer) => transfer
                        .release_quiesced(&self.device)
                        .map_err(UsbNativeHostRefusal::Control)?,
                    Transfer::Endpoint(transfer) => transfer
                        .release_stopped(&self.device)
                        .map_err(UsbNativeHostRefusal::Endpoint)?,
                    Transfer::Write(transfer) => transfer
                        .release_stopped(&self.device)
                        .map_err(UsbNativeHostRefusal::Write)?,
                }
            }
        }
        if let Some(transfer) = &mut self.isochronous {
            // SAFETY: original controller acknowledged stop before any ISO lease retires.
            unsafe { transfer.release_stopped() }.map_err(UsbNativeHostRefusal::Isochronous)?;
        }
        unsafe {
            (*dma).owner_slot = 0;
        }
        self.pending_endpoint = None;
        self.pending_write = None;
        self.uncertain_control = false;
        self.uncertain_bulk = false;
        self.recovery_endpoint = None;
        self.stopped = true;
        Ok(())
    }
}
