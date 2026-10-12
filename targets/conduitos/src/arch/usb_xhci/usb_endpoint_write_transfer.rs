//! Exact admitted transfer state and DMA publication, independent of event polling.
//! The controller owner supplies only acknowledged events and native stop.
#![allow(dead_code)] // Not installed or advertised by the native Root yet.
use super::UsbDevice;
use crate::arch::xhci_mechanism::{Event, publish_transfer};
use crate::{
    machine_membrane::selected_operation::SelectedOperationPlan,
    usb_base::{
        endpoint_ring::{EndpointRingCursor, EndpointRingRefusal, EndpointRingReservation},
        endpoint_write_contract::EndpointWriteContract,
        endpoint_write_owner::{
            EndpointWriteAttachment, EndpointWriteCallOwner, EndpointWriteOwnerRefusal,
            NativeEndpointWriteObservation, NativeEndpointWriteSubmission,
        },
        endpoint_write_result::EndpointWriteDisposition,
    },
};
use conduit_composite::KernelOperationFactory;
use conduit_core::{
    BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use core::{
    ptr::write_volatile,
    sync::atomic::{Ordering, fence},
};
use publish_transfer as publish;

#[derive(Debug)]
pub enum EndpointWriteNativeRefusal {
    Ownership(EndpointWriteOwnerRefusal),
    Ring(EndpointRingRefusal),
    Mapping,
    Pending,
    ForeignCompletion,
    Completion(u8),
    ProviderLost,
    Stop,
}

/// Storage remains rooted independently of the call owner's lifetime. Dropping
/// this owner cannot authorize freeing, clearing or reusing these DMA regions.
pub(crate) struct EndpointTransmitDma<'a> {
    pub ring: &'a mut [[u32; 4]; 64],
    pub buffer: &'a mut [u8; 2048],
    pub cursor: &'a mut EndpointRingCursor,
    pub ring_physical: u64,
    pub buffer_physical: u64,
}

#[derive(Clone, Copy)]
struct AttachmentIdentity {
    slot: u8,
    root_port: u8,
    attachment_epoch: u32,
    dma_slot: u8,
}

pub(in crate::arch::usb_xhci) struct EndpointWriteTransfer<'a> {
    attachment: AttachmentIdentity,
    pub(in crate::arch::usb_xhci) endpoint_dci: u8,
    dma: EndpointTransmitDma<'a>,
    owner: EndpointWriteCallOwner,
    pending: Option<(NativeEndpointWriteSubmission, EndpointRingReservation)>,
}

impl<'a> EndpointWriteTransfer<'a> {
    pub(in crate::arch::usb_xhci) fn retains_settlement_obligation(&self) -> bool {
        self.pending.is_some()
            || self.owner.has_pending_lease()
            || self.dma.cursor.ensure_idle().is_err()
    }

    pub(in crate::arch::usb_xhci) fn validate_settlement_idle(
        &self,
        device: &UsbDevice,
    ) -> Result<(), EndpointWriteNativeRefusal> {
        self.validate_device(device)?;
        if self.pending.is_some() || self.owner.has_pending_lease() {
            return Err(EndpointWriteNativeRefusal::Pending);
        }
        self.dma
            .cursor
            .ensure_idle()
            .map_err(EndpointWriteNativeRefusal::Ring)
    }

    /// # Safety
    /// Root exclusively owns the configured outbound endpoint,
    /// exact device/endpoint generations and claim's provider/resource. The two
    /// supplied regions must be coherent DMA with these exact physical addresses
    /// and remain rooted until acknowledged stop, including after Drop/failure.
    /// The configured dequeue points to this ring, with initial cycle 1; the
    /// cursor must be that ring's retained producer. Root supplies the actual
    /// controller port bound and only its acknowledged events and stop. A grant
    /// is not confinement.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn bind_selected(
        maximum_ports: u8,
        device: &UsbDevice,
        attachment: EndpointWriteAttachment,
        configured: super::endpoint_setup::ConfiguredOutboundEndpoint,
        dma: EndpointTransmitDma<'a>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &EndpointWriteContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<Self, EndpointWriteNativeRefusal> {
        if configured.slot != device.slot
            || configured.device_epoch != device.attachment_epoch
            || configured.endpoint_epoch != attachment.endpoint_generation
            || configured.dci != attachment.endpoint_dci
            || !attachment.endpoint_dci.is_multiple_of(2)
            || configured.ring_physical != dma.ring_physical
            || attachment.slot != device.slot
            || attachment.generation != u64::from(device.attachment_epoch)
            || dma.ring_physical == 0
            || dma.ring_physical & 63 != 0
            || dma.buffer_physical == 0
            || dma.ring_physical.checked_add(1024).is_none()
            || dma.buffer_physical.checked_add(2048).is_none()
            || (dma.ring_physical & 0xffff) > 0xfc00
            || (dma.buffer_physical & 0xffff) > 0xf800
            || attachment.resource_bytes < 3072
            || dma.cursor.ordinary_slots() != 63
            || device.root_port == 0
            || device.root_port > maximum_ports
        {
            return Err(EndpointWriteNativeRefusal::Mapping);
        }
        super::dma::device_dma_pointer(device).map_err(|_| EndpointWriteNativeRefusal::Mapping)?;
        let gear = selected
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selected.placement_id)
            .ok_or(EndpointWriteNativeRefusal::Mapping)?;
        crate::usb_base::endpoint_write_factory::EndpointWriteOperationFactory::prepare_contract()
            .map_err(|_| EndpointWriteNativeRefusal::Mapping)?
            .budget(gear)
            .map_err(|_| EndpointWriteNativeRefusal::Mapping)?;
        let ring_end = dma.ring_physical + 1024;
        let buffer_end = dma.buffer_physical + 2048;
        if dma.ring_physical < buffer_end && dma.buffer_physical < ring_end {
            return Err(EndpointWriteNativeRefusal::Mapping);
        }
        dma.cursor
            .ensure_idle()
            .map_err(EndpointWriteNativeRefusal::Ring)?;
        let owner = unsafe {
            EndpointWriteCallOwner::bind_admitted(
                table, handle, claim, attachment, contract, selected,
            )
        }
        .map_err(EndpointWriteNativeRefusal::Ownership)?;
        Ok(Self {
            attachment: AttachmentIdentity {
                slot: device.slot,
                root_port: device.root_port,
                attachment_epoch: device.attachment_epoch,
                dma_slot: device.dma_slot,
            },
            endpoint_dci: attachment.endpoint_dci,
            dma,
            owner,
            pending: None,
        })
    }

    /// Copied attachment facts are validation data, never a second native owner.
    pub(in crate::arch::usb_xhci) fn validate_device(
        &self,
        device: &UsbDevice,
    ) -> Result<(), EndpointWriteNativeRefusal> {
        if self.attachment.slot != device.slot
            || self.attachment.root_port != device.root_port
            || self.attachment.attachment_epoch != device.attachment_epoch
            || self.attachment.dma_slot != device.dma_slot
        {
            return Err(EndpointWriteNativeRefusal::Mapping);
        }
        super::dma::device_dma_pointer(device).map_err(|_| EndpointWriteNativeRefusal::Mapping)?;
        Ok(())
    }

    pub(in crate::arch::usb_xhci) fn ensure_pending(
        &self,
    ) -> Result<(), EndpointWriteNativeRefusal> {
        self.pending
            .as_ref()
            .map(|_| ())
            .ok_or(EndpointWriteNativeRefusal::Pending)
    }

    pub fn ring_position(&self) -> (usize, u32) {
        self.dma.cursor.position()
    }

    /// Publish one already admitted transfer; no polling, class logic or retry.
    pub fn begin(
        &mut self,
        device: &UsbDevice,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), EndpointWriteNativeRefusal> {
        self.validate_device(device)?;
        if self.pending.is_some() {
            return Err(EndpointWriteNativeRefusal::Pending);
        }
        self.dma
            .cursor
            .ensure_ready()
            .map_err(EndpointWriteNativeRefusal::Ring)?;
        let submission = self
            .owner
            .begin(node, call, request, input)
            .map_err(EndpointWriteNativeRefusal::Ownership)?;
        let reservation = self
            .dma
            .cursor
            .reserve_out(submission.length())
            .map_err(EndpointWriteNativeRefusal::Ring)?;
        // Retain exact admitted payload in Root-owned DMA before publication.
        self.dma.buffer[..usize::from(submission.length())].copy_from_slice(submission.wire());
        // The controller may still be parked at the preceding cycle's Link.
        // Publish its new cycle only with the final ordinary TRB, never slot 0.
        if reservation.slot == 62 {
            publish(
                reservation.link(self.dma.ring_physical),
                |word, value| unsafe { write_volatile(&mut self.dma.ring[63][word], value) },
            );
        }
        publish(
            reservation.normal(self.dma.buffer_physical),
            |word, value| unsafe {
                write_volatile(&mut self.dma.ring[reservation.slot][word], value)
            },
        );
        self.pending = Some((submission, reservation));
        Ok(())
    }

    /// Describe only the exact live reservation for the sole native event router.
    /// This key conveys no authority and never retires DMA.
    pub(in crate::arch::usb_xhci) fn pending_event_key(
        &self,
        device: &UsbDevice,
    ) -> Result<(u8, u8, u64), EndpointWriteNativeRefusal> {
        self.validate_device(device)?;
        let (_, reservation) = self
            .pending
            .as_ref()
            .ok_or(EndpointWriteNativeRefusal::Pending)?;
        Ok((
            self.attachment.slot,
            self.endpoint_dci,
            self.dma.ring_physical + (reservation.slot * 16) as u64,
        ))
    }

    /// Validate the exact completion before retiring its DMA reservation.
    pub(in crate::arch::usb_xhci) fn complete(
        &mut self,
        device: &UsbDevice,
        event: Event,
    ) -> Result<&[u8], EndpointWriteNativeRefusal> {
        self.validate_device(device)?;
        let (_, reservation) = self
            .pending
            .as_ref()
            .ok_or(EndpointWriteNativeRefusal::Pending)?;
        let pointer = self.dma.ring_physical + (reservation.slot * 16) as u64;
        let residual = validated_residual(event, self.attachment.slot, self.endpoint_dci, pointer)?;
        let actual = self
            .dma
            .cursor
            .actual(reservation, residual)
            .map_err(EndpointWriteNativeRefusal::Ring)?;
        // Exact final Transfer Event acknowledges this TRB's DMA quiescence.
        fence(Ordering::Acquire);
        unsafe { self.dma.cursor.complete_quiesced(reservation) }
            .map_err(EndpointWriteNativeRefusal::Ring)?;
        let (submission, _) = self.pending.take().expect("validated pending transfer");
        unsafe {
            self.owner.finish_quiesced(
                &submission,
                NativeEndpointWriteObservation::Completed { actual },
            )
        }
        .map_err(EndpointWriteNativeRefusal::Ownership)
    }

    /// # Safety
    /// The sole original controller has acknowledged stop for this exact slot.
    /// A stall event or disconnected-port observation alone grants no retirement.
    pub(in crate::arch::usb_xhci) unsafe fn finish_fault_quiesced(
        &mut self,
        device: &UsbDevice,
        disposition: EndpointWriteDisposition,
    ) -> Result<&[u8], EndpointWriteNativeRefusal> {
        self.validate_device(device)?;
        let (submission, reservation) = self
            .pending
            .as_ref()
            .ok_or(EndpointWriteNativeRefusal::Pending)?;
        // Preserve the exact admitted lease until its typed terminal settles.
        fence(Ordering::Acquire);
        unsafe { self.dma.cursor.complete_quiesced(reservation) }
            .map_err(EndpointWriteNativeRefusal::Ring)?;
        let bytes = unsafe {
            self.owner.finish_quiesced(
                submission,
                NativeEndpointWriteObservation::Disposition(disposition),
            )
        }
        .map_err(EndpointWriteNativeRefusal::Ownership)?;
        self.pending = None;
        Ok(bytes)
    }

    pub(in crate::arch::usb_xhci) fn revoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), EndpointWriteNativeRefusal> {
        self.owner
            .revoke(node, call)
            .map_err(EndpointWriteNativeRefusal::Ownership)
    }

    /// Root must acknowledge native stop before this retirement step. The Host
    /// clears attachment ownership only after every transfer has retired.
    pub(in crate::arch::usb_xhci) unsafe fn release_stopped(
        &mut self,
        device: &UsbDevice,
    ) -> Result<(), EndpointWriteNativeRefusal> {
        self.validate_device(device)?;
        if let Some((submission, reservation)) = self.pending.as_ref() {
            unsafe { self.dma.cursor.complete_quiesced(reservation) }
                .map_err(EndpointWriteNativeRefusal::Ring)?;
            match unsafe { self.owner.discard_quiesced(submission) } {
                Ok(())
                | Err(EndpointWriteOwnerRefusal::Capability(
                    BaseCapabilityRefusal::StaleCompletion,
                )) => {}
                Err(error) => return Err(EndpointWriteNativeRefusal::Ownership(error)),
            }
            self.pending = None;
        }
        Ok(())
    }
}

pub(super) fn validated_residual(
    event: Event,
    slot: u8,
    endpoint: u8,
    pointer: u64,
) -> Result<u32, EndpointWriteNativeRefusal> {
    if event.event_type != 32
        || event.slot != slot
        || event.endpoint != endpoint
        || event.pointer != pointer
    {
        return Err(EndpointWriteNativeRefusal::ForeignCompletion);
    }
    if event.completion_code != 1 {
        return Err(EndpointWriteNativeRefusal::Completion(
            event.completion_code,
        ));
    }
    Ok(event.residual)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreign_stall_events_cannot_authorize_fault_retirement() {
        let exact = Event {
            event_type: 32,
            completion_code: 6,
            slot: 2,
            endpoint: 2,
            residual: 0,
            pointer: 0x4010,
        };
        assert!(matches!(
            validated_residual(exact, 2, exact.endpoint, 0x4010),
            Err(EndpointWriteNativeRefusal::Completion(6))
        ));
        for event in [
            Event {
                event_type: 34,
                ..exact
            },
            Event { slot: 3, ..exact },
            Event {
                endpoint: 4,
                ..exact
            },
            Event {
                pointer: 0x4020,
                ..exact
            },
        ] {
            assert!(matches!(
                validated_residual(event, 2, exact.endpoint, 0x4010),
                Err(EndpointWriteNativeRefusal::ForeignCompletion)
            ));
        }
    }

    #[test]
    fn outbound_completion_requires_exact_identity_and_success() {
        let event = Event {
            event_type: 32,
            completion_code: 1,
            slot: 2,
            endpoint: 4,
            residual: 0,
            pointer: 0x4010,
        };
        assert_eq!(validated_residual(event, 2, 4, 0x4010).unwrap(), 0);
        for changed in [
            Event { slot: 3, ..event },
            Event {
                endpoint: 5,
                ..event
            },
            Event {
                pointer: 0x4020,
                ..event
            },
            Event {
                event_type: 33,
                ..event
            },
        ] {
            assert!(matches!(
                validated_residual(changed, 2, 4, 0x4010),
                Err(EndpointWriteNativeRefusal::ForeignCompletion)
            ));
        }
        for code in [0, 2, 6, 13, 19, 26, 255] {
            assert!(
                matches!(validated_residual(Event {completion_code: code, ..event}, 2, 4, 0x4010), Err(EndpointWriteNativeRefusal::Completion(actual)) if actual == code)
            );
        }
    }
}
