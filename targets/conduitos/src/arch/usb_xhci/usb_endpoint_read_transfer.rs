//! Exact admitted transfer state and DMA publication, independent of event polling.
//! The controller owner supplies only acknowledged events and native stop.
#![allow(dead_code)] // Not installed or advertised by the native Root yet.
use super::super::UsbDevice;
use crate::arch::xhci_mechanism::{Event, publish_transfer};
use crate::{
    machine_membrane::selected_operation::SelectedOperationPlan,
    usb_base::{
        endpoint_read_contract::EndpointReadContract,
        endpoint_read_owner::{
            EndpointReadAttachment, EndpointReadCallOwner, EndpointReadOwnerRefusal,
            NativeEndpointReadObservation, NativeEndpointReadSubmission,
        },
        endpoint_read_result::EndpointReadDisposition,
        endpoint_ring::{EndpointRingCursor, EndpointRingRefusal, EndpointRingReservation},
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
pub enum EndpointNativeRefusal {
    Ownership(EndpointReadOwnerRefusal),
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
pub(crate) struct EndpointReceiveDma<'a> {
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

pub(in crate::arch::usb_xhci) struct EndpointReadTransfer<'a> {
    attachment: AttachmentIdentity,
    pub(in crate::arch::usb_xhci) endpoint_dci: u8,
    dma: EndpointReceiveDma<'a>,
    owner: EndpointReadCallOwner,
    pending: Option<(NativeEndpointReadSubmission, EndpointRingReservation)>,
}

impl<'a> EndpointReadTransfer<'a> {
    pub(in crate::arch::usb_xhci) fn validate_settlement_idle(
        &self,
        device: &UsbDevice,
    ) -> Result<(), EndpointNativeRefusal> {
        self.validate_device(device)?;
        if self.pending.is_some() || self.owner.has_pending_lease() {
            return Err(EndpointNativeRefusal::Pending);
        }
        self.dma
            .cursor
            .ensure_idle()
            .map_err(EndpointNativeRefusal::Ring)
    }

    /// # Safety
    /// Root exclusively owns the configured inbound endpoint,
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
        attachment: EndpointReadAttachment,
        configured: super::super::endpoint_setup::ConfiguredInboundEndpoint,
        dma: EndpointReceiveDma<'a>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &EndpointReadContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<Self, EndpointNativeRefusal> {
        if configured.slot != device.slot
            || configured.device_epoch != device.attachment_epoch
            || configured.endpoint_epoch != attachment.endpoint_generation
            || configured.dci != attachment.endpoint_dci
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
            return Err(EndpointNativeRefusal::Mapping);
        }
        super::super::dma::device_dma_pointer(device)
            .map_err(|_| EndpointNativeRefusal::Mapping)?;
        let gear = selected
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selected.placement_id)
            .ok_or(EndpointNativeRefusal::Mapping)?;
        crate::usb_base::endpoint_read_factory::EndpointReadOperationFactory::prepare_contract()
            .map_err(|_| EndpointNativeRefusal::Mapping)?
            .budget(gear)
            .map_err(|_| EndpointNativeRefusal::Mapping)?;
        let ring_end = dma.ring_physical + 1024;
        let buffer_end = dma.buffer_physical + 2048;
        if dma.ring_physical < buffer_end && dma.buffer_physical < ring_end {
            return Err(EndpointNativeRefusal::Mapping);
        }
        dma.cursor
            .ensure_idle()
            .map_err(EndpointNativeRefusal::Ring)?;
        let owner = unsafe {
            EndpointReadCallOwner::bind_admitted(
                table, handle, claim, attachment, contract, selected,
            )
        }
        .map_err(EndpointNativeRefusal::Ownership)?;
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
    ) -> Result<(), EndpointNativeRefusal> {
        if self.attachment.slot != device.slot
            || self.attachment.root_port != device.root_port
            || self.attachment.attachment_epoch != device.attachment_epoch
            || self.attachment.dma_slot != device.dma_slot
        {
            return Err(EndpointNativeRefusal::Mapping);
        }
        super::super::dma::device_dma_pointer(device)
            .map_err(|_| EndpointNativeRefusal::Mapping)?;
        Ok(())
    }

    pub(in crate::arch::usb_xhci) fn ensure_pending(&self) -> Result<(), EndpointNativeRefusal> {
        self.pending
            .as_ref()
            .map(|_| ())
            .ok_or(EndpointNativeRefusal::Pending)
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
    ) -> Result<(), EndpointNativeRefusal> {
        self.validate_device(device)?;
        if self.pending.is_some() {
            return Err(EndpointNativeRefusal::Pending);
        }
        self.dma
            .cursor
            .ensure_ready()
            .map_err(EndpointNativeRefusal::Ring)?;
        let submission = self
            .owner
            .begin(node, call, request, input)
            .map_err(EndpointNativeRefusal::Ownership)?;
        let reservation = self
            .dma
            .cursor
            .reserve(submission.length())
            .map_err(EndpointNativeRefusal::Ring)?;
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
    ) -> Result<(u8, u8, u64), EndpointNativeRefusal> {
        self.validate_device(device)?;
        let (_, reservation) = self
            .pending
            .as_ref()
            .ok_or(EndpointNativeRefusal::Pending)?;
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
    ) -> Result<&[u8], EndpointNativeRefusal> {
        self.validate_device(device)?;
        let (_, reservation) = self
            .pending
            .as_ref()
            .ok_or(EndpointNativeRefusal::Pending)?;
        let pointer = self.dma.ring_physical + (reservation.slot * 16) as u64;
        let residual = validated_residual(event, self.attachment.slot, self.endpoint_dci, pointer)?;
        let actual = self
            .dma
            .cursor
            .actual(reservation, residual)
            .map_err(EndpointNativeRefusal::Ring)?;
        // Exact final Transfer Event acknowledges this TRB's DMA quiescence.
        fence(Ordering::Acquire);
        unsafe { self.dma.cursor.complete_quiesced(reservation) }
            .map_err(EndpointNativeRefusal::Ring)?;
        let (submission, reservation) = self.pending.take().expect("validated pending transfer");
        unsafe {
            self.owner.finish_quiesced(
                &submission,
                NativeEndpointReadObservation::Completed {
                    ordinal: reservation.ordinal(),
                    actual,
                    input: &self.dma.buffer[..usize::from(actual)],
                },
            )
        }
        .map_err(EndpointNativeRefusal::Ownership)
    }

    /// # Safety
    /// The sole original controller has acknowledged stop for this exact slot.
    /// A stall event or disconnected-port observation alone grants no retirement.
    pub(in crate::arch::usb_xhci) unsafe fn finish_fault_quiesced(
        &mut self,
        device: &UsbDevice,
        disposition: EndpointReadDisposition,
    ) -> Result<&[u8], EndpointNativeRefusal> {
        self.validate_device(device)?;
        let (submission, reservation) = self
            .pending
            .as_ref()
            .ok_or(EndpointNativeRefusal::Pending)?;
        // Preserve the exact admitted lease until its typed terminal settles.
        fence(Ordering::Acquire);
        unsafe { self.dma.cursor.complete_quiesced(reservation) }
            .map_err(EndpointNativeRefusal::Ring)?;
        let bytes = unsafe {
            self.owner.finish_quiesced(
                submission,
                NativeEndpointReadObservation::Disposition(disposition),
            )
        }
        .map_err(EndpointNativeRefusal::Ownership)?;
        self.pending = None;
        Ok(bytes)
    }

    pub(in crate::arch::usb_xhci) fn revoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), EndpointNativeRefusal> {
        self.owner
            .revoke(node, call)
            .map_err(EndpointNativeRefusal::Ownership)
    }

    /// Root must acknowledge native stop before this retirement step. The Host
    /// clears attachment ownership only after every transfer has retired.
    pub(in crate::arch::usb_xhci) unsafe fn release_stopped(
        &mut self,
        device: &UsbDevice,
    ) -> Result<(), EndpointNativeRefusal> {
        self.validate_device(device)?;
        if let Some((submission, reservation)) = self.pending.as_ref() {
            unsafe { self.dma.cursor.complete_quiesced(reservation) }
                .map_err(EndpointNativeRefusal::Ring)?;
            match unsafe { self.owner.discard_quiesced(submission) } {
                Ok(())
                | Err(EndpointReadOwnerRefusal::Capability(
                    BaseCapabilityRefusal::StaleCompletion,
                )) => {}
                Err(error) => return Err(EndpointNativeRefusal::Ownership(error)),
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
) -> Result<u32, EndpointNativeRefusal> {
    if event.event_type != 32
        || event.slot != slot
        || event.endpoint != endpoint
        || event.pointer != pointer
    {
        return Err(EndpointNativeRefusal::ForeignCompletion);
    }
    if !matches!(event.completion_code, 1 | 13) {
        return Err(EndpointNativeRefusal::Completion(event.completion_code));
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
            endpoint: 3,
            residual: 0,
            pointer: 0x4010,
        };
        assert!(matches!(
            validated_residual(exact, 2, exact.endpoint, 0x4010),
            Err(EndpointNativeRefusal::Completion(6))
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
                Err(EndpointNativeRefusal::ForeignCompletion)
            ));
        }
    }

    #[test]
    fn only_the_exact_native_transfer_event_can_supply_a_received_extent() {
        let event = Event {
            event_type: 32,
            completion_code: 13,
            slot: 2,
            endpoint: 3,
            residual: 2040,
            pointer: 0x4010,
        };
        assert_eq!(validated_residual(event, 2, 3, 0x4010).unwrap(), 2040);
        assert_eq!(
            validated_residual(
                Event {
                    completion_code: 1,
                    residual: 0,
                    ..event
                },
                2,
                3,
                0x4010
            )
            .unwrap(),
            0
        );
        for changed in [
            Event {
                event_type: 33,
                ..event
            },
            Event { slot: 3, ..event },
            Event {
                endpoint: 5,
                ..event
            },
            Event {
                pointer: 0x4020,
                ..event
            },
        ] {
            assert!(matches!(
                validated_residual(changed, 2, 3, 0x4010),
                Err(EndpointNativeRefusal::ForeignCompletion)
            ));
        }
        for code in [0, 2, 6, 19, 26, 255] {
            assert!(
                matches!(validated_residual(Event { completion_code: code, ..event }, 2, 3, 0x4010), Err(EndpointNativeRefusal::Completion(actual)) if actual == code)
            );
        }
    }
}
