//! One native attachment and event consumer shared by admitted transport calls.
//! Serial publication avoids an event router and never introduces class policy.
#![allow(dead_code)] // Root integration follows this ownership substrate.
use super::{
    UsbControlCallRefusal, UsbControlSelection, UsbDevice, UsbError,
    control_owner::UsbControlTransfer,
    device_dma_pointer,
    endpoint_read::{EndpointNativeRefusal, EndpointReadTransfer, EndpointReceiveDma},
    endpoint_setup::{ConfiguredInboundEndpoint, ConfiguredOutboundEndpoint},
    endpoint_write_transfer::{
        EndpointTransmitDma, EndpointWriteNativeRefusal, EndpointWriteTransfer,
    },
};
use crate::{
    arch::xhci_mechanism::XhciReady,
    machine_membrane::selected_operation::SelectedOperationPlan,
    usb_base::{
        endpoint_read_contract::EndpointReadContract, endpoint_read_owner::EndpointReadAttachment,
        endpoint_read_result::EndpointReadDisposition,
        endpoint_write_contract::EndpointWriteContract,
        endpoint_write_owner::EndpointWriteAttachment,
        endpoint_write_result::EndpointWriteDisposition,
    },
};
use alloc::vec::Vec;
use conduit_core::{BaseCapabilityHandle, BaseCapabilityTable, BaseOperationClaim};
use conduit_kernel::{HostCallId, NodeId, RequestId};

const MAXIMUM_CALLS: usize = 8;

/// Opaque Host-local ordinal; it neither describes a device nor grants possession.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UsbNativeCallIndex(usize);

#[derive(Debug)]
pub enum UsbNativeHostRefusal {
    Native(UsbError),
    Control(UsbControlCallRefusal),
    Endpoint(EndpointNativeRefusal),
    Write(EndpointWriteNativeRefusal),
    Isochronous(super::IsochronousOutRefusal),
    Capacity,
    Binding,
    Pending,
    NotReady,
}

struct BoundCall<'a> {
    node: NodeId,
    call: HostCallId,
    transfer: Transfer<'a>,
}
#[allow(clippy::large_enum_variant)] // At most eight preparation-reserved slots own fixed wire storage; Play never boxes a submission.
enum Transfer<'a> {
    Control(UsbControlTransfer),
    Endpoint(EndpointReadTransfer<'a>),
    Write(EndpointWriteTransfer<'a>),
}

/// Root retains the original move-only native attachment, every DMA owner and
/// the only controller event consumer until all submissions acknowledge stop.
pub(crate) struct UsbNativeHost<'a> {
    controller: XhciReady,
    device: UsbDevice,
    mapping: fn(u64) -> Option<u64>,
    calls: Vec<BoundCall<'a>>,
    pending_endpoint: Option<usize>,
    isochronous: Option<super::isochronous_out::IsochronousOutTransfer<'a>>,
    isochronous_started: bool,
    uncertain_control: bool,
    playing: bool,
    stop_acknowledged: bool,
    possessions_revoked: bool,
    cancellation_requested: bool,
    control_output: [u8; crate::usb_base::control_contract::CONTROL_MAXIMUM_BYTES as usize],
    control_output_length: usize,
    stopped: bool,
}

impl<'a> UsbNativeHost<'a> {
    /// # Safety
    /// Root owns this exact controller and move-only attachment exclusively;
    /// mapping and all later supplied coherent DMA remain rooted until actual
    /// acknowledged stop, including after failure or Drop. No other event
    /// consumer may run. Matching IDs or copied descriptors establish no ownership.
    pub unsafe fn new(
        controller: XhciReady,
        device: UsbDevice,
        mapping: fn(u64) -> Option<u64>,
    ) -> Result<Self, UsbNativeHostRefusal> {
        if device.slot == 0
            || device.attachment_epoch == 0
            || device.root_port == 0
            || device.root_port > controller.maximum_ports()
        {
            return Err(UsbNativeHostRefusal::Native(UsbError::StaleDeviceInstance));
        }
        device_dma_pointer(&device).map_err(UsbNativeHostRefusal::Native)?;
        Ok(Self {
            controller,
            device,
            mapping,
            calls: Vec::with_capacity(MAXIMUM_CALLS),
            pending_endpoint: None,
            isochronous: None,
            isochronous_started: false,
            uncertain_control: false,
            playing: false,
            stop_acknowledged: false,
            possessions_revoked: false,
            cancellation_requested: false,
            control_output: [0; crate::usb_base::control_contract::CONTROL_MAXIMUM_BYTES as usize],
            control_output_length: 0,
            stopped: false,
        })
    }

    fn preparing(&self, node: NodeId) -> Result<(), UsbNativeHostRefusal> {
        if self.playing || self.stop_acknowledged || self.stopped || self.cancellation_requested {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        if self.calls.len() == MAXIMUM_CALLS {
            return Err(UsbNativeHostRefusal::Capacity);
        }
        if self.calls.iter().any(|call| call.node == node) {
            return Err(UsbNativeHostRefusal::Binding);
        }
        Ok(())
    }

    /// # Safety
    /// Root owns this actual controller/attachment under the exact claim's
    /// provider/resource generation and envelope; matching claims alone do not
    /// establish native ownership. Mapping remains rooted through physical stop.
    pub unsafe fn add_control(
        &mut self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selected: UsbControlSelection<'_>,
    ) -> Result<UsbNativeCallIndex, UsbNativeHostRefusal> {
        let node = selected
            .lowered
            .nodes
            .iter()
            .find(|node| &node.placement_id == selected.placement)
            .ok_or(UsbNativeHostRefusal::Binding)?
            .node;
        self.preparing(node)?;
        // SAFETY: this sole Host retains the original native attachment/controller.
        let transfer = unsafe {
            UsbControlTransfer::bind_selected(
                &self.controller,
                &self.device,
                self.mapping,
                table,
                handle,
                claim,
                selected,
            )
        }
        .map_err(UsbNativeHostRefusal::Control)?;
        let index = UsbNativeCallIndex(self.calls.len());
        self.calls.push(BoundCall {
            node,
            call: HostCallId(0),
            transfer: Transfer::Control(transfer),
        });
        Ok(index)
    }

    /// # Safety
    /// Root supplied actual configured endpoint receipt and exclusive coherent
    /// DMA matching its exact physical addresses and native resource generation.
    /// Those regions remain rooted through every error until acknowledged stop.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn add_endpoint(
        &mut self,
        configured: ConfiguredInboundEndpoint,
        dma: EndpointReceiveDma<'a>,
        attachment: EndpointReadAttachment,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &EndpointReadContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<UsbNativeCallIndex, UsbNativeHostRefusal> {
        let node = selected
            .lowered
            .nodes
            .iter()
            .find(|node| &node.placement_id == selected.placement_id)
            .ok_or(UsbNativeHostRefusal::Binding)?
            .node;
        self.preparing(node)?;
        if self.calls.iter().any(|call| {
            matches!(&call.transfer, Transfer::Endpoint(endpoint)
            if endpoint.endpoint_dci == attachment.endpoint_dci)
        }) {
            return Err(UsbNativeHostRefusal::Binding);
        }
        // SAFETY: actual receipt/DMA and original attachment stay under one Host.
        let transfer = unsafe {
            EndpointReadTransfer::bind_selected(
                self.controller.maximum_ports(),
                &self.device,
                attachment,
                configured,
                dma,
                table,
                handle,
                claim,
                contract,
                selected,
            )
        }
        .map_err(UsbNativeHostRefusal::Endpoint)?;
        let index = UsbNativeCallIndex(self.calls.len());
        self.calls.push(BoundCall {
            node,
            call: HostCallId(0),
            transfer: Transfer::Endpoint(transfer),
        });
        Ok(index)
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe fn add_write(
        &mut self,
        configured: ConfiguredOutboundEndpoint,
        dma: EndpointTransmitDma<'a>,
        attachment: EndpointWriteAttachment,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &EndpointWriteContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<UsbNativeCallIndex, UsbNativeHostRefusal> {
        let node = selected
            .lowered
            .nodes
            .iter()
            .find(|node| &node.placement_id == selected.placement_id)
            .ok_or(UsbNativeHostRefusal::Binding)?
            .node;
        self.preparing(node)?;
        if self.calls.iter().any(|call| {
            matches!(&call.transfer, Transfer::Write(endpoint)
            if endpoint.endpoint_dci == attachment.endpoint_dci)
        }) {
            return Err(UsbNativeHostRefusal::Binding);
        }
        // SAFETY: actual receipt/DMA and original attachment stay under one Host.
        let transfer = unsafe {
            EndpointWriteTransfer::bind_selected(
                self.controller.maximum_ports(),
                &self.device,
                attachment,
                configured,
                dma,
                table,
                handle,
                claim,
                contract,
                selected,
            )
        }
        .map_err(UsbNativeHostRefusal::Write)?;
        let index = UsbNativeCallIndex(self.calls.len());
        self.calls.push(BoundCall {
            node,
            call: HostCallId(0),
            transfer: Transfer::Write(transfer),
        });
        Ok(index)
    }

    fn idle_transport(&self) -> Result<(), UsbNativeHostRefusal> {
        if self.stop_acknowledged || self.stopped || self.cancellation_requested {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        if self.pending_endpoint.is_some() || self.uncertain_control || self.isochronous_started {
            return Err(UsbNativeHostRefusal::Pending);
        }
        Ok(())
    }

    pub fn begin_endpoint(
        &mut self,
        index: UsbNativeCallIndex,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), UsbNativeHostRefusal> {
        self.idle_transport()?;
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
        if self.stopped || self.stop_acknowledged {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        if self.pending_endpoint != Some(index.0) {
            return Err(UsbNativeHostRefusal::Pending);
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
        if self.stopped || self.stop_acknowledged {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        if self.pending_endpoint != Some(index.0) {
            return Err(UsbNativeHostRefusal::Pending);
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
            self.pending_endpoint = None;
        }
        result.map(|()| Some(&self.control_output[..self.control_output_length]))
    }
}

#[path = "usb_native_host/retirement.rs"]
mod retirement;

#[path = "usb_native_host/isochronous.rs"]
mod isochronous;

#[cfg(test)]
#[path = "usb_native_host/stop_ack_tests.rs"]
mod stop_ack_tests;
