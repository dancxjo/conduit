//! One independently admitted IN and OUT transfer under the original event owner.
use super::event_routing::{PendingEventKey, route_event};
use super::*;

pub(super) struct BulkTerminal {
    pub(super) index: Option<UsbNativeCallIndex>,
    pub(super) bytes:
        [u8; crate::usb_base::endpoint_read_contract::ENDPOINT_READ_MAXIMUM_BYTES as usize],
    pub(super) length: usize,
}
impl BulkTerminal {
    pub(super) fn empty() -> Self {
        Self {
            index: None,
            bytes: [0; crate::usb_base::endpoint_read_contract::ENDPOINT_READ_MAXIMUM_BYTES
                as usize],
            length: 0,
        }
    }
}

/// DMA plus this direction's preparation-owned terminal record. Existing
/// capability-table/validator admission remains separately accounted.
pub(super) const fn endpoint_resource_bytes() -> u64 {
    3072 + core::mem::size_of::<BulkTerminal>() as u64
}

impl UsbNativeHost<'_> {
    /// Preparation only: the fixed records are absent in control/ISO hosts.
    pub(in crate::arch::usb_xhci) fn prepare_independent(
        &mut self,
    ) -> Result<(), UsbNativeHostRefusal> {
        if self.playing
            || self.stopped
            || self.cancellation_requested
            || self.bulk_terminals.is_some()
        {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        self.bulk_terminals = Some(alloc::boxed::Box::new(core::array::from_fn(|_| {
            BulkTerminal::empty()
        })));
        Ok(())
    }

    fn admit_independent(&self, direction: usize) -> Result<(), UsbNativeHostRefusal> {
        if self.bulk_terminals.is_none()
            || self.stop_acknowledged
            || self.stopped
            || self.cancellation_requested
        {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        if self.uncertain_control
            || self.uncertain_bulk
            || self.recovery_endpoint.is_some()
            || self.isochronous_started
            || self
                .bulk_terminals
                .as_ref()
                .is_some_and(|slots| slots.iter().any(|slot| slot.index.is_some()))
            || [self.pending_endpoint, self.pending_write][direction].is_some()
        {
            return Err(UsbNativeHostRefusal::Pending);
        }
        // A legacy write occupies pending_endpoint; never mix its serial API
        // with independently routed transport.
        if let Some(index) = self.pending_endpoint
            && !matches!(self.calls[index].transfer, Transfer::Endpoint(_))
        {
            return Err(UsbNativeHostRefusal::Pending);
        }
        Ok(())
    }

    pub fn begin_independent_read(
        &mut self,
        index: UsbNativeCallIndex,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), UsbNativeHostRefusal> {
        self.admit_independent(0)?;
        self.validate_continuity()?;
        let bound = self
            .calls
            .get_mut(index.0)
            .ok_or(UsbNativeHostRefusal::Binding)?;
        let Transfer::Endpoint(transfer) = &mut bound.transfer else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        self.playing = true;
        self.pending_endpoint = Some(index.0);
        self.uncertain_bulk = true;
        transfer
            .begin(&self.device, bound.node, bound.call, request, input)
            .map_err(UsbNativeHostRefusal::Endpoint)?;
        self.controller
            .ring_endpoint(self.device.slot, transfer.endpoint_dci);
        self.uncertain_bulk = false;
        Ok(())
    }

    pub fn begin_independent_write(
        &mut self,
        index: UsbNativeCallIndex,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), UsbNativeHostRefusal> {
        self.admit_independent(1)?;
        self.validate_continuity()?;
        let bound = self
            .calls
            .get_mut(index.0)
            .ok_or(UsbNativeHostRefusal::Binding)?;
        let Transfer::Write(transfer) = &mut bound.transfer else {
            return Err(UsbNativeHostRefusal::Binding);
        };
        self.playing = true;
        self.pending_write = Some(index.0);
        self.uncertain_bulk = true;
        transfer
            .begin(&self.device, bound.node, bound.call, request, input)
            .map_err(UsbNativeHostRefusal::Write)?;
        self.controller
            .ring_endpoint(self.device.slot, transfer.endpoint_dci);
        self.uncertain_bulk = false;
        Ok(())
    }

    pub(super) fn independent_keys(
        &self,
    ) -> Result<[Option<PendingEventKey>; 2], UsbNativeHostRefusal> {
        let mut keys = [None, None];
        for (direction, index) in [self.pending_endpoint, self.pending_write]
            .into_iter()
            .enumerate()
        {
            let Some(index) = index else {
                continue;
            };
            let key = match &self.calls[index].transfer {
                Transfer::Endpoint(transfer) if direction == 0 => transfer
                    .pending_event_key(&self.device)
                    .map_err(UsbNativeHostRefusal::Endpoint)?,
                Transfer::Write(transfer) if direction == 1 => transfer
                    .pending_event_key(&self.device)
                    .map_err(UsbNativeHostRefusal::Write)?,
                _ => return Err(UsbNativeHostRefusal::Binding),
            };
            keys[direction] = Some(PendingEventKey {
                index: UsbNativeCallIndex(index),
                slot: key.0,
                endpoint: key.1,
                pointer: key.2,
            });
        }
        Ok(keys)
    }

    /// At most one event consumption and one exact completed call publication.
    /// No transfer is retired by matching metadata alone.
    pub fn poll_independent(
        &mut self,
    ) -> Result<Option<(UsbNativeCallIndex, &[u8])>, UsbNativeHostRefusal> {
        if let Some(position) = self
            .bulk_terminals
            .as_ref()
            .and_then(|slots| slots.iter().position(|slot| slot.index.is_some()))
        {
            if !self.bulk_terminals_ready {
                return Err(UsbNativeHostRefusal::NotReady);
            }
            let slot = &mut self
                .bulk_terminals
                .as_mut()
                .ok_or(UsbNativeHostRefusal::NotReady)?[position];
            let index = slot.index.take().ok_or(UsbNativeHostRefusal::Binding)?;
            return Ok(Some((index, &slot.bytes[..slot.length])));
        }
        if self.stopped
            || self.stop_acknowledged
            || self.cancellation_requested
            || self.uncertain_bulk
        {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        let keys = self.independent_keys()?;
        if keys.iter().all(Option::is_none) {
            return Ok(None);
        }
        if !self.independent_continuity()? {
            return self.poll_independent();
        }
        let Some(event) = self.controller.poll_event() else {
            return Ok(None);
        };
        if !self.independent_continuity()? {
            return self.poll_independent();
        }
        self.complete_independent_event(event, keys)
    }

    pub(super) fn complete_independent_event(
        &mut self,
        event: crate::arch::xhci_mechanism::Event,
        keys: [Option<PendingEventKey>; 2],
    ) -> Result<Option<(UsbNativeCallIndex, &[u8])>, UsbNativeHostRefusal> {
        if self.stopped
            || self.stop_acknowledged
            || self.cancellation_requested
            || self.uncertain_bulk
        {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        if !self.independent_continuity()? {
            return self.poll_independent();
        }
        let index = match route_event(event, keys) {
            Ok(index) => index,
            Err(_) => {
                self.uncertain_bulk = true;
                return Err(UsbNativeHostRefusal::Binding);
            }
        };
        let result = match &mut self.calls[index.0].transfer {
            Transfer::Endpoint(transfer) => transfer
                .complete(&self.device, event)
                .map_err(UsbNativeHostRefusal::Endpoint),
            Transfer::Write(transfer) => transfer
                .complete(&self.device, event)
                .map_err(UsbNativeHostRefusal::Write),
            _ => return Err(UsbNativeHostRefusal::Binding),
        };
        match result {
            Ok(bytes) => {
                self.control_output[..bytes.len()].copy_from_slice(bytes);
                self.control_output_length = bytes.len();
                self.validate_continuity()?;
                if self.pending_endpoint == Some(index.0) {
                    self.pending_endpoint = None;
                }
                if self.pending_write == Some(index.0) {
                    self.pending_write = None;
                }
                Ok(Some((
                    index,
                    &self.control_output[..self.control_output_length],
                )))
            }
            Err(UsbNativeHostRefusal::Endpoint(EndpointNativeRefusal::Completion(6)))
            | Err(UsbNativeHostRefusal::Write(EndpointWriteNativeRefusal::Completion(6))) => {
                if !self.finish_single_stalled_endpoint(index)? {
                    self.finish_independent_fault(Some(index))?;
                }
                self.poll_independent()
            }
            Err(error) => {
                self.uncertain_bulk = true;
                Err(error)
            }
        }
    }

    fn independent_continuity(&mut self) -> Result<bool, UsbNativeHostRefusal> {
        self.independent_continuity_status(self.controller.port_status(self.device.root_port))
    }

    pub(super) fn independent_continuity_status(
        &mut self,
        status: u32,
    ) -> Result<bool, UsbNativeHostRefusal> {
        // Every pre-completion checkpoint uses one actual observation. Loss
        // between event polls must settle the original admitted calls just as
        // loss observed at the first checkpoint does. Connected replacement
        // remains a stale-identity refusal, never a provider-lost completion.
        if status & 1 == 0 {
            self.finish_independent_fault(None)?;
            return Ok(false);
        }
        self.validate_continuity_status(status)?;
        Ok(true)
    }

    fn finish_independent_fault(
        &mut self,
        stalled: Option<UsbNativeCallIndex>,
    ) -> Result<(), UsbNativeHostRefusal> {
        // Exact event routing/validated original attachment precedes physical stop.
        // A failed acknowledgement leaves both original leases and DMA rooted.
        self.uncertain_bulk = true;
        self.controller
            .disable_removed_slot(self.device.slot)
            .map_err(|error| UsbNativeHostRefusal::Native(error.into()))?;
        self.stop_acknowledged = true;
        for (direction, index) in [self.pending_endpoint, self.pending_write]
            .into_iter()
            .enumerate()
        {
            let Some(index) = index else {
                continue;
            };
            let bytes = match &mut self.calls[index].transfer {
                Transfer::Endpoint(transfer) if direction == 0 => unsafe {
                    transfer.finish_fault_quiesced(
                        &self.device,
                        if stalled == Some(UsbNativeCallIndex(index)) {
                            EndpointReadDisposition::Stalled
                        } else {
                            EndpointReadDisposition::ProviderLost
                        },
                    )
                }
                .map_err(UsbNativeHostRefusal::Endpoint)?,
                Transfer::Write(transfer) if direction == 1 => unsafe {
                    transfer.finish_fault_quiesced(
                        &self.device,
                        if stalled == Some(UsbNativeCallIndex(index)) {
                            EndpointWriteDisposition::Stalled
                        } else {
                            EndpointWriteDisposition::ProviderLost
                        },
                    )
                }
                .map_err(UsbNativeHostRefusal::Write)?,
                _ => return Err(UsbNativeHostRefusal::Binding),
            };
            let slot = &mut self
                .bulk_terminals
                .as_mut()
                .ok_or(UsbNativeHostRefusal::NotReady)?[direction];
            slot.bytes[..bytes.len()].copy_from_slice(bytes);
            slot.length = bytes.len();
            slot.index = Some(UsbNativeCallIndex(index));
        }
        // No terminal publication until every transfer/software possession settles.
        self.cancel()?;
        self.bulk_terminals_ready = true;
        Ok(())
    }
}
