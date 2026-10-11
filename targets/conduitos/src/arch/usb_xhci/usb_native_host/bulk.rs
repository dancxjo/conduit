//! Bounded bulk publication and exact completion under the sole controller owner.
use super::*;

impl UsbNativeHost<'_> {
    pub fn begin_endpoint(
        &mut self,
        index: UsbNativeCallIndex,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), UsbNativeHostRefusal> {
        self.idle_transport()?;
        self.validate_continuity()?;
        let bound = self
            .calls
            .get_mut(index.0)
            .ok_or(UsbNativeHostRefusal::Binding)?;
        let Transfer::Endpoint(transfer) = &mut bound.transfer else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        // Keep transport exclusivity even if publication refuses after authority
        // acceptance; only an acknowledged stop may settle uncertain DMA state.
        self.playing = true;
        self.pending_endpoint = Some(index.0);
        transfer
            .begin(&self.device, bound.node, bound.call, request, input)
            .map_err(UsbNativeHostRefusal::Endpoint)?;
        self.controller
            .ring_endpoint(self.device.slot, transfer.endpoint_dci);
        Ok(())
    }

    /// At most one event, routed only to the exact sole pending endpoint.
    pub fn poll_endpoint(
        &mut self,
        index: UsbNativeCallIndex,
    ) -> Result<Option<&[u8]>, UsbNativeHostRefusal> {
        if self.stopped || self.stop_acknowledged || self.uncertain_bulk {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        if self.pending_write.is_some() || self.pending_endpoint != Some(index.0) {
            return Err(UsbNativeHostRefusal::Pending);
        }
        if self.controller.port_status(self.device.root_port) & 1 != 0 {
            self.validate_continuity()?;
        }
        let bound = self
            .calls
            .get_mut(index.0)
            .ok_or(UsbNativeHostRefusal::Binding)?;
        let Transfer::Endpoint(transfer) = &mut bound.transfer else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        transfer
            .validate_device(&self.device)
            .map_err(UsbNativeHostRefusal::Endpoint)?;
        transfer
            .ensure_pending()
            .map_err(UsbNativeHostRefusal::Endpoint)?;
        if self.controller.port_status(self.device.root_port) & 1 == 0 {
            return self
                .finish_read_fault(index, EndpointReadDisposition::ProviderLost)
                .map(Some);
        }
        let Some(event) = self.controller.poll_event() else {
            return Ok(None);
        };
        if let Err(error) = super::super::attachment::validate_attachment_continuity(
            self.controller.port_status(self.device.root_port),
        ) {
            self.uncertain_bulk = true;
            self.uncertain_control = true;
            return Err(UsbNativeHostRefusal::Native(error));
        }
        let result = transfer
            .complete(&self.device, event)
            .map(|bytes| {
                self.control_output[..bytes.len()].copy_from_slice(bytes);
                self.control_output_length = bytes.len();
            })
            .map_err(UsbNativeHostRefusal::Endpoint);
        if matches!(
            result,
            Err(UsbNativeHostRefusal::Endpoint(
                EndpointNativeRefusal::Completion(6)
            ))
        ) {
            return self
                .finish_read_fault(index, EndpointReadDisposition::Stalled)
                .map(Some);
        }
        if result.is_ok() {
            self.validate_continuity()?;
            self.pending_endpoint = None;
        }
        result.map(|()| Some(&self.control_output[..self.control_output_length]))
    }

    pub fn begin_write(
        &mut self,
        index: UsbNativeCallIndex,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), UsbNativeHostRefusal> {
        self.idle_transport()?;
        self.validate_continuity()?;
        let bound = self
            .calls
            .get_mut(index.0)
            .ok_or(UsbNativeHostRefusal::Binding)?;
        let Transfer::Write(transfer) = &mut bound.transfer else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        // Keep transport exclusivity even if publication refuses after authority
        // acceptance; only an acknowledged stop may settle uncertain DMA state.
        self.playing = true;
        self.pending_endpoint = Some(index.0);
        transfer
            .begin(&self.device, bound.node, bound.call, request, input)
            .map_err(UsbNativeHostRefusal::Write)?;
        self.controller
            .ring_endpoint(self.device.slot, transfer.endpoint_dci);
        Ok(())
    }

    /// At most one event, routed only to the exact sole pending endpoint.
    pub fn poll_write(
        &mut self,
        index: UsbNativeCallIndex,
    ) -> Result<Option<&[u8]>, UsbNativeHostRefusal> {
        if self.stopped || self.stop_acknowledged || self.uncertain_bulk {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        if self.pending_write.is_some() || self.pending_endpoint != Some(index.0) {
            return Err(UsbNativeHostRefusal::Pending);
        }
        if self.controller.port_status(self.device.root_port) & 1 != 0 {
            self.validate_continuity()?;
        }
        let bound = self
            .calls
            .get_mut(index.0)
            .ok_or(UsbNativeHostRefusal::Binding)?;
        let Transfer::Write(transfer) = &mut bound.transfer else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        transfer
            .validate_device(&self.device)
            .map_err(UsbNativeHostRefusal::Write)?;
        transfer
            .ensure_pending()
            .map_err(UsbNativeHostRefusal::Write)?;
        if self.controller.port_status(self.device.root_port) & 1 == 0 {
            return self
                .finish_write_fault(index, EndpointWriteDisposition::ProviderLost)
                .map(Some);
        }
        let Some(event) = self.controller.poll_event() else {
            return Ok(None);
        };
        if let Err(error) = super::super::attachment::validate_attachment_continuity(
            self.controller.port_status(self.device.root_port),
        ) {
            self.uncertain_bulk = true;
            self.uncertain_control = true;
            return Err(UsbNativeHostRefusal::Native(error));
        }
        let result = transfer
            .complete(&self.device, event)
            .map(|bytes| {
                self.control_output[..bytes.len()].copy_from_slice(bytes);
                self.control_output_length = bytes.len();
            })
            .map_err(UsbNativeHostRefusal::Write);
        if matches!(
            result,
            Err(UsbNativeHostRefusal::Write(
                EndpointWriteNativeRefusal::Completion(6)
            ))
        ) {
            return self
                .finish_write_fault(index, EndpointWriteDisposition::Stalled)
                .map(Some);
        }
        if result.is_ok() {
            self.validate_continuity()?;
            self.pending_endpoint = None;
        }
        result.map(|()| Some(&self.control_output[..self.control_output_length]))
    }
}
