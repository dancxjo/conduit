//! Exact admitted transfer state and DMA publication, independent of event polling.
//! The controller owner supplies only acknowledged events and native stop.
#![allow(dead_code)] // Not installed or advertised by the native Root yet.
use super::super::UsbDevice;
use crate::arch::x86_64::{hid_transfer_ring::publish, xhci::Event};
use crate::{
    machine_membrane::selected_operation::SelectedOperationPlan,
    usb_base::{
        endpoint_read_contract::EndpointReadContract,
        endpoint_read_owner::{
            EndpointReadAttachment, EndpointReadCallOwner, EndpointReadOwnerRefusal,
            NativeEndpointReadObservation, NativeEndpointReadSubmission,
        },
        endpoint_ring::{EndpointRingCursor, EndpointRingRefusal, EndpointRingReservation},
    },
};
use conduit_composite::KernelOperationFactory;
use conduit_core::{BaseCapabilityHandle, BaseCapabilityTable, BaseOperationClaim};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use core::{
    ptr::write_volatile,
    sync::atomic::{Ordering, fence},
};

#[derive(Debug)]
pub(crate) enum EndpointNativeRefusal {
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

pub(super) struct EndpointReadTransfer<'a> {
    pub(super) device: UsbDevice,
    pub(super) endpoint_dci: u8,
    dma: EndpointReceiveDma<'a>,
    owner: EndpointReadCallOwner,
    pending: Option<(NativeEndpointReadSubmission, EndpointRingReservation)>,
}

impl<'a> EndpointReadTransfer<'a> {
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
        device: UsbDevice,
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
        super::super::dma::device_dma_pointer(&device)
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
            device,
            endpoint_dci: attachment.endpoint_dci,
            dma,
            owner,
            pending: None,
        })
    }

    pub(super) fn ensure_pending(&self) -> Result<(), EndpointNativeRefusal> {
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
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), EndpointNativeRefusal> {
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

    /// Validate the exact completion before retiring its DMA reservation.
    pub(super) fn complete(&mut self, event: Event) -> Result<&[u8], EndpointNativeRefusal> {
        let (_, reservation) = self
            .pending
            .as_ref()
            .ok_or(EndpointNativeRefusal::Pending)?;
        let pointer = self.dma.ring_physical + (reservation.slot * 16) as u64;
        let residual = validated_residual(event, self.device.slot, self.endpoint_dci, pointer)?;
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

    pub(super) fn revoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), EndpointNativeRefusal> {
        self.owner
            .revoke(node, call)
            .map_err(EndpointNativeRefusal::Ownership)
    }

    /// Root must acknowledge the native stop before this retirement step.
    pub(super) unsafe fn release_stopped(&mut self) -> Result<(), EndpointNativeRefusal> {
        let device_dma = super::super::dma::device_dma_pointer(&self.device)
            .map_err(|_| EndpointNativeRefusal::Mapping)?;
        unsafe {
            (*device_dma).owner_slot = 0;
        }
        if let Some((submission, reservation)) = self.pending.take() {
            unsafe { self.dma.cursor.complete_quiesced(&reservation) }
                .map_err(EndpointNativeRefusal::Ring)?;
            unsafe { self.owner.discard_quiesced(&submission) }
                .map_err(EndpointNativeRefusal::Ownership)?;
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
