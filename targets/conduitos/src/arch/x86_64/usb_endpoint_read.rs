//! One controller consumer around exact admitted endpoint transfer state.
//! Root must configure the endpoint and retain coherent DMA before binding.
#![allow(dead_code)] // Not installed or advertised by the native Root yet.
use super::UsbDevice;
use crate::arch::x86_64::xhci::XhciReady;
use crate::{
    machine_membrane::selected_operation::SelectedOperationPlan,
    usb_base::{
        endpoint_read_contract::EndpointReadContract, endpoint_read_owner::EndpointReadAttachment,
    },
};
use conduit_core::{BaseCapabilityHandle, BaseCapabilityTable, BaseOperationClaim};
use conduit_kernel::{HostCallId, NodeId, RequestId};

#[path = "usb_endpoint_read_transfer.rs"]
mod transfer;
#[path = "usb_endpoint_read_window.rs"]
pub(super) mod window;
use transfer::EndpointReadTransfer;
pub(crate) use transfer::{EndpointNativeRefusal, EndpointReceiveDma};

pub(crate) struct UsbEndpointReadHostCall<'a> {
    controller: &'a mut XhciReady,
    transfer: EndpointReadTransfer<'a>,
}

impl<'a> UsbEndpointReadHostCall<'a> {
    /// # Safety
    /// Root exclusively owns this controller and configured inbound endpoint,
    /// exact device/endpoint generations and claim's provider/resource. The two
    /// supplied regions must be coherent DMA with these exact physical addresses
    /// and remain rooted until acknowledged stop, including after Drop/failure.
    /// The configured dequeue points to this ring, with initial cycle 1; the
    /// cursor must be that ring's retained producer. No other event consumer may
    /// run while this owner retains the controller. A grant is not confinement.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn bind_selected(
        controller: &'a mut XhciReady,
        device: UsbDevice,
        attachment: EndpointReadAttachment,
        configured: super::endpoint_setup::ConfiguredInboundEndpoint,
        dma: EndpointReceiveDma<'a>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &EndpointReadContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<Self, EndpointNativeRefusal> {
        let transfer = unsafe {
            EndpointReadTransfer::bind_selected(
                controller.maximum_ports(),
                device,
                attachment,
                configured,
                dma,
                table,
                handle,
                claim,
                contract,
                selected,
            )
        }?;
        Ok(Self {
            controller,
            transfer,
        })
    }

    pub fn ring_position(&self) -> (usize, u32) {
        self.transfer.ring_position()
    }

    /// Publish one admitted transfer and ring its exact endpoint once.
    pub fn begin(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), EndpointNativeRefusal> {
        self.transfer.begin(node, call, request, input)?;
        self.controller
            .ring_endpoint(self.transfer.device.slot, self.transfer.endpoint_dci);
        Ok(())
    }

    /// Examine at most one controller event. Absence retains the exact pending
    /// transfer. Wrong events/loss/error retain DMA until explicit native stop.
    pub fn poll(&mut self) -> Result<Option<&[u8]>, EndpointNativeRefusal> {
        self.transfer.ensure_pending()?;
        if self.controller.port_status(self.transfer.device.root_port) & 1 == 0 {
            return Err(EndpointNativeRefusal::ProviderLost);
        }
        let Some(event) = self.controller.poll_event() else {
            return Ok(None);
        };
        if event.event_type == 34 {
            return Err(
                if self.controller.port_status(self.transfer.device.root_port) & 1 == 0 {
                    EndpointNativeRefusal::ProviderLost
                } else {
                    EndpointNativeRefusal::ForeignCompletion
                },
            );
        }
        self.transfer.complete(event).map(Some)
    }

    /// Revoke first; failed hardware stop retains the pending reservation and
    /// DMA. Disable Slot is the bounded existing controller retirement path.
    pub fn cancel(&mut self, node: NodeId, call: HostCallId) -> Result<(), EndpointNativeRefusal> {
        self.transfer.revoke(node, call)?;
        self.controller
            .disable_removed_slot(self.transfer.device.slot)
            .map_err(|_| EndpointNativeRefusal::Stop)?;
        unsafe { self.transfer.release_stopped() }
    }
}
