//! One admitted endpoint's read custody and PID state, independent of bus storage.
//! Root lends its original controller for one bounded call and retains this
//! context through retirement. No report or class interpretation happens here.
#![allow(dead_code)] // Installed by the native product Root, never by request data.
#[derive(Clone, Copy)]
pub(crate) struct RootEndpointRoute {
    pub slot: u8,
    pub generation: u64,
    pub endpoint_generation: u64,
    pub endpoint: EndpointRoute,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Dwc2ReadRefusal {
    Binding,
    Ownership(EndpointReadOwnerRefusal),
    Native(PacketError),
}

use super::{
    endpoint_read_owner::{
        EndpointReadAttachment, EndpointReadCallOwner, EndpointReadOwnerRefusal,
        NativeEndpointReadObservation, NativeEndpointReadSubmission,
    },
    endpoint_read_result::EndpointReadDisposition,
};
use crate::arch::dwc2::{
    Controller, EndpointRoute, PacketError, PacketPid, RegisterIo, TransferType,
};
use conduit_core::BaseCapabilityRefusal;
use conduit_kernel::{HostCallId, NodeId, RequestId};
pub(crate) struct Dwc2EndpointReadContext {
    owner: EndpointReadCallOwner,
    route: RootEndpointRoute,
    buffer: [u8; 64],
    pending: Option<NativeEndpointReadSubmission>,
    pid: PacketPid,
    ordinal: u64,
    lost: bool,
}
impl Dwc2EndpointReadContext {
    /// # Safety
    /// Root retains the exact original controller, attachment, endpoint and native
    /// resource generation. Source completed configuration/endpoint reset so its
    /// first expected PID is DATA0. Retain this owner through physical retirement;
    /// matching copied IDs are not possession and dropping it acknowledges none.
    /// Every execute/discard call must borrow that same original controller.
    pub unsafe fn bind_admitted<I: RegisterIo>(
        controller: &Controller<I>,
        owner: EndpointReadCallOwner,
        route: RootEndpointRoute,
    ) -> Result<Self, Dwc2ReadRefusal> {
        let attachment = owner.native_attachment();
        if !controller.is_prepared()
            || !controller.is_quiescent()
            || owner.has_pending_lease()
            || !route.endpoint.valid()
            || route.endpoint.address == 0
            || route.endpoint.endpoint == 0
            || route.endpoint.transfer_type != TransferType::Interrupt
            || attachment.slot != route.slot
            || attachment.generation != route.generation
            || attachment.endpoint_generation != route.endpoint_generation
            || attachment.endpoint_dci != route.endpoint.endpoint * 2 + 1
            || attachment.maximum_data_bytes != u16::from(route.endpoint.maximum_packet)
            || attachment.resource_bytes < 64
        {
            return Err(Dwc2ReadRefusal::Binding);
        }
        Ok(Self {
            owner,
            route,
            buffer: [0; 64],
            pending: None,
            pid: PacketPid::Data0,
            ordinal: 0,
            lost: false,
        })
    }
    pub fn execute<I: RegisterIo>(
        &mut self,
        controller: &mut Controller<I>,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<&[u8], Dwc2ReadRefusal> {
        if !controller.is_prepared() {
            return Err(Dwc2ReadRefusal::Native(PacketError::NotPrepared));
        }
        if !controller.is_quiescent() {
            return Err(Dwc2ReadRefusal::Native(PacketError::Retirement));
        }
        if self.pending.is_some() {
            return Err(Dwc2ReadRefusal::Ownership(
                EndpointReadOwnerRefusal::Pressure,
            ));
        }
        if self.lost {
            return Err(Dwc2ReadRefusal::Native(PacketError::ProviderLost));
        }
        let next = self
            .ordinal
            .checked_add(1)
            .ok_or(Dwc2ReadRefusal::Ownership(
                EndpointReadOwnerRefusal::SequenceExhausted,
            ))?;
        let submission = self
            .owner
            .begin(node, call, request_id, input)
            .map_err(Dwc2ReadRefusal::Ownership)?;
        let attachment = submission.attachment();
        self.pending = Some(submission);
        if !self.matches(attachment) {
            return Err(Dwc2ReadRefusal::Binding);
        }
        let length = self
            .pending
            .as_ref()
            .expect("accepted read retained")
            .length();
        let observation = if length != u16::from(self.route.endpoint.maximum_packet) {
            NativeEndpointReadObservation::Disposition(EndpointReadDisposition::Unsupported)
        } else {
            match controller.packet_on(
                self.route.endpoint,
                self.pid,
                true,
                &mut self.buffer[..usize::from(length)],
            ) {
                Ok(actual) => {
                    // Physical ACK advances the endpoint even if later software
                    // completion is revoked. Never replay an accepted report.
                    let ordinal = self.ordinal;
                    self.ordinal = next;
                    self.pid = if self.pid == PacketPid::Data0 {
                        PacketPid::Data1
                    } else {
                        PacketPid::Data0
                    };
                    NativeEndpointReadObservation::Completed {
                        ordinal,
                        actual: actual as u16,
                        input: &self.buffer[..actual],
                    }
                }
                Err(PacketError::Nak | PacketError::Nyet) if controller.is_quiescent() => {
                    NativeEndpointReadObservation::Disposition(EndpointReadDisposition::Timeout)
                }
                Err(PacketError::Stall) if controller.is_quiescent() => {
                    NativeEndpointReadObservation::Disposition(EndpointReadDisposition::Stalled)
                }
                Err(PacketError::ProviderLost) if controller.is_quiescent() => {
                    self.lost = true;
                    NativeEndpointReadObservation::Disposition(
                        EndpointReadDisposition::ProviderLost,
                    )
                }
                Err(reason) => {
                    self.lost = true;
                    return Err(Dwc2ReadRefusal::Native(reason));
                }
            }
        };
        if !controller.is_quiescent() {
            return Err(Dwc2ReadRefusal::Native(PacketError::Retirement));
        }
        let submission = self.pending.take().expect("completed read retained");
        // SAFETY: sole original PIO owner, exact retained lease, and acknowledged
        // completion/NAK/halt (including TT settlement). No memory DMA exists.
        unsafe { self.owner.finish_quiesced(&submission, observation) }
            .map_err(Dwc2ReadRefusal::Ownership)
    }
    fn matches(&self, attachment: EndpointReadAttachment) -> bool {
        attachment.slot == self.route.slot
            && attachment.generation == self.route.generation
            && attachment.endpoint_generation == self.route.endpoint_generation
            && attachment.endpoint_dci == self.route.endpoint.endpoint * 2 + 1
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), Dwc2ReadRefusal> {
        self.owner
            .revoke(node, call)
            .map_err(Dwc2ReadRefusal::Ownership)
    }
    pub fn discard_retired<I: RegisterIo>(
        &mut self,
        controller: &Controller<I>,
    ) -> Result<(), Dwc2ReadRefusal> {
        if !controller.is_quiescent() {
            return Err(Dwc2ReadRefusal::Native(PacketError::Retirement));
        }
        if let Some(submission) = self.pending.as_ref() {
            // SAFETY: confirmed original controller/TT retirement, never timeout alone.
            match unsafe { self.owner.discard_quiesced(submission) } {
                Ok(())
                | Err(EndpointReadOwnerRefusal::Capability(
                    BaseCapabilityRefusal::StaleCompletion,
                )) => {}
                Err(reason) => return Err(Dwc2ReadRefusal::Ownership(reason)),
            }
            self.pending = None;
        }
        Ok(())
    }
    pub fn retains_submission(&self) -> bool {
        self.pending.is_some() || self.owner.has_pending_lease()
    }
}
