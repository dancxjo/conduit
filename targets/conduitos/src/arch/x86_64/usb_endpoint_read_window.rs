//! Admitted endpoint capture reservations under one native controller owner.
//! This owner publishes and completes transfers; it never polls or interprets reports.
#![allow(dead_code)] // Ordinary Root installation and native window proof are pending.
use super::super::{UsbDevice, dma::device_dma_pointer, endpoint_setup::ConfiguredInboundEndpoint};
use super::{EndpointNativeRefusal, transfer::validated_residual};
use crate::arch::x86_64::{hid_transfer_ring::publish, xhci::Event};
use crate::machine_membrane::selected_operation::SelectedOperationPlan;
use crate::usb_base::{
    endpoint_read_contract::EndpointReadContract,
    endpoint_read_factory::EndpointReadOperationFactory,
    endpoint_read_owner::{
        EndpointReadAttachment, EndpointReadCallOwner, NativeEndpointReadObservation,
        NativeEndpointReadSubmission,
    },
    endpoint_ring::{EndpointRingCursor, EndpointRingReservation},
};
use conduit_composite::KernelOperationFactory;
use conduit_core::{BaseCapabilityHandle, BaseCapabilityTable, BaseOperationClaim};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use core::{
    ptr::write_volatile,
    sync::atomic::{Ordering, fence},
};

pub(crate) struct EndpointReadWindowDma<'a, const N: usize> {
    pub ring: &'a mut [[u32; 4]; 64],
    pub buffers: &'a mut [[u8; 2048]; N],
    pub cursor: &'a mut EndpointRingCursor,
    pub ring_physical: u64,
    pub buffers_physical: u64,
}

pub(crate) struct EndpointReadWindowSelection<'a> {
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
    pub selected: SelectedOperationPlan<'a>,
}

struct Member {
    owner: EndpointReadCallOwner,
    node: NodeId,
}

struct Pending {
    member: usize,
    request: RequestId,
    submission: NativeEndpointReadSubmission,
    reservation: EndpointRingReservation,
}

pub(in crate::arch::x86_64::usb) struct WindowCompletion<'a> {
    pub node: NodeId,
    pub request: RequestId,
    pub ordinal: u64,
    pub encoded: &'a [u8],
}

pub(crate) struct EndpointReadWindow<'a, const N: usize> {
    device: UsbDevice,
    endpoint_dci: u8,
    dma: EndpointReadWindowDma<'a, N>,
    members: [Option<Member>; N],
    pending: [Option<Pending>; N],
    head: usize,
    count: usize,
    stopped: bool,
}

impl<'a, const N: usize> EndpointReadWindow<'a, N> {
    /// # Safety
    /// Root owns the actual controller, exact configured endpoint and coherent
    /// DMA regions. Every selection must reserve its share of this exact endpoint
    /// resource, with independently issued possession. Root retains all DMA after
    /// Drop or failure until hardware quiescence and dispatches only this
    /// controller's acknowledged events. This constructor grants no authority.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn bind_selected(
        maximum_ports: u8,
        device: UsbDevice,
        attachment: EndpointReadAttachment,
        configured: ConfiguredInboundEndpoint,
        dma: EndpointReadWindowDma<'a, N>,
        selections: [EndpointReadWindowSelection<'_>; N],
        contract: &EndpointReadContract,
    ) -> Result<Self, EndpointNativeRefusal> {
        use EndpointNativeRefusal as Error;
        if !(1..=8).contains(&N)
            || configured.slot != device.slot
            || configured.device_epoch != device.attachment_epoch
            || configured.endpoint_epoch != attachment.endpoint_generation
            || configured.dci != attachment.endpoint_dci
            || configured.ring_physical != dma.ring_physical
            || attachment.slot != device.slot
            || attachment.generation != u64::from(device.attachment_epoch)
            || attachment.resource_bytes < 3072
            || dma.cursor.ordinary_slots() != 63
            || dma.cursor.maximum_pending() != N
            || device.root_port == 0
            || device.root_port > maximum_ports
        {
            return Err(Error::Mapping);
        }
        validate_geometry::<N>(dma.ring_physical, dma.buffers_physical)?;
        device_dma_pointer(&device).map_err(|_| Error::Mapping)?;
        dma.cursor.ensure_idle().map_err(Error::Ring)?;
        for (index, selection) in selections.iter().enumerate() {
            if selection.claim != selections[0].claim
                || selections[..index]
                    .iter()
                    .any(|previous| previous.handle == selection.handle)
            {
                return Err(Error::Mapping);
            }
        }
        let factory =
            EndpointReadOperationFactory::prepare_contract().map_err(|_| Error::Mapping)?;
        let mut members: [Option<Member>; N] = core::array::from_fn(|_| None);
        for (index, selection) in selections.into_iter().enumerate() {
            let gear = selection
                .selected
                .fragment
                .placements
                .iter()
                .find(|gear| &gear.placement_id == selection.selected.placement_id)
                .ok_or(Error::Mapping)?;
            factory.budget(gear).map_err(|_| Error::Mapping)?;
            let owner = unsafe {
                EndpointReadCallOwner::bind_admitted(
                    selection.table,
                    selection.handle,
                    selection.claim,
                    attachment,
                    contract,
                    selection.selected,
                )
            }
            .map_err(Error::Ownership)?;
            let node = owner.binding_node();
            if members[..index]
                .iter()
                .flatten()
                .any(|member| member.node == node)
            {
                return Err(Error::Mapping);
            }
            members[index] = Some(Member { owner, node });
        }
        Ok(Self {
            device,
            endpoint_dci: attachment.endpoint_dci,
            dma,
            members,
            pending: core::array::from_fn(|_| None),
            head: 0,
            count: 0,
            stopped: false,
        })
    }

    pub fn target(&self) -> (u8, u8, u8) {
        (self.device.root_port, self.device.slot, self.endpoint_dci)
    }

    pub fn pending_count(&self) -> usize {
        self.count
    }

    /// Publish one exact admitted Host Call. The sole controller owner rings
    /// this endpoint after publication; no new read is invented by this window.
    pub fn begin(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), EndpointNativeRefusal> {
        use EndpointNativeRefusal as Error;
        if self.stopped {
            return Err(Error::Stop);
        }
        device_dma_pointer(&self.device).map_err(|_| Error::ProviderLost)?;
        self.dma.cursor.ensure_ready().map_err(Error::Ring)?;
        let index = self
            .members
            .iter()
            .position(|member| member.as_ref().is_some_and(|member| member.node == node))
            .ok_or(Error::Mapping)?;
        let submission = self.members[index]
            .as_mut()
            .expect("bound member")
            .owner
            .begin(node, call, request, input)
            .map_err(Error::Ownership)?;
        let reservation = self
            .dma
            .cursor
            .reserve(submission.length())
            .map_err(Error::Ring)?;
        if reservation.slot == 62 {
            publish(
                reservation.link(self.dma.ring_physical),
                |word, value| unsafe { write_volatile(&mut self.dma.ring[63][word], value) },
            );
        }
        let buffer = self.dma.buffers_physical + (index * 2048) as u64;
        publish(reservation.normal(buffer), |word, value| unsafe {
            write_volatile(&mut self.dma.ring[reservation.slot][word], value)
        });
        let tail = (self.head + self.count) % N;
        self.pending[tail] = Some(Pending {
            member: index,
            request,
            submission,
            reservation,
        });
        self.count += 1;
        Ok(())
    }

    /// Accept only the exact oldest completed TRB. Foreign, out-of-order or
    /// malformed events retain the entire capture window and its DMA.
    pub(in crate::arch::x86_64::usb) fn complete(
        &mut self,
        event: Event,
    ) -> Result<WindowCompletion<'_>, EndpointNativeRefusal> {
        use EndpointNativeRefusal as Error;
        if self.stopped {
            return Err(Error::Stop);
        }
        device_dma_pointer(&self.device).map_err(|_| Error::ProviderLost)?;
        let pending = self.pending[self.head].as_ref().ok_or(Error::Pending)?;
        let pointer = self.dma.ring_physical + (pending.reservation.slot * 16) as u64;
        let residual = validated_residual(event, self.device.slot, self.endpoint_dci, pointer)?;
        let actual = self
            .dma
            .cursor
            .actual(&pending.reservation, residual)
            .map_err(Error::Ring)?;
        fence(Ordering::Acquire);
        unsafe { self.dma.cursor.complete_quiesced(&pending.reservation) }.map_err(Error::Ring)?;
        let pending = self.pending[self.head]
            .take()
            .expect("validated pending transfer");
        self.head = (self.head + 1) % N;
        self.count -= 1;
        let member = self.members[pending.member].as_mut().expect("bound member");
        let encoded = unsafe {
            member.owner.finish_quiesced(
                &pending.submission,
                NativeEndpointReadObservation::Completed {
                    ordinal: pending.reservation.ordinal(),
                    actual,
                    input: &self.dma.buffers[pending.member][..usize::from(actual)],
                },
            )
        }
        .map_err(Error::Ownership)?;
        Ok(WindowCompletion {
            node: member.node,
            request: pending.request,
            ordinal: pending.reservation.ordinal(),
            encoded,
        })
    }

    /// Attempt every revocation, even if one possession has already failed.
    /// Root must still stop hardware; software refusal cannot release DMA.
    pub fn revoke_all(&mut self) -> Result<(), EndpointNativeRefusal> {
        let mut first = None;
        for member in self.members.iter_mut().flatten() {
            if let Err(error) = member.owner.revoke(member.node, HostCallId(0)) {
                first.get_or_insert(EndpointNativeRefusal::Ownership(error));
            }
        }
        first.map_or(Ok(()), Err)
    }

    /// # Safety
    /// Root acknowledged a stop covering this exact slot, endpoint and every
    /// retained transfer. Timeout, cancellation and loss alone are insufficient.
    pub unsafe fn release_stopped(&mut self) -> Result<(), EndpointNativeRefusal> {
        use EndpointNativeRefusal as Error;
        self.stopped = true;
        // Slot retirement belongs to the controller Root. Multiple endpoint
        // windows may share that slot and must all release their own DMA first.
        let mut first = None;
        while self.count != 0 {
            let pending = self.pending[self.head].as_ref().expect("retained transfer");
            unsafe { self.dma.cursor.complete_quiesced(&pending.reservation) }
                .map_err(Error::Ring)?;
            let pending = self.pending[self.head].take().expect("quiescent transfer");
            self.head = (self.head + 1) % N;
            self.count -= 1;
            if let Err(error) = unsafe {
                self.members[pending.member]
                    .as_mut()
                    .expect("bound member")
                    .owner
                    .discard_quiesced(&pending.submission)
            } {
                first.get_or_insert(Error::Ownership(error));
            }
        }
        first.map_or(Ok(()), Err)
    }
}

pub(in crate::arch::x86_64::usb) fn validate_geometry<const N: usize>(
    ring: u64,
    buffers: u64,
) -> Result<(), EndpointNativeRefusal> {
    use EndpointNativeRefusal::Mapping;
    if !(1..=8).contains(&N) || ring == 0 || ring & 63 != 0 || buffers == 0 {
        return Err(Mapping);
    }
    let ring_end = ring.checked_add(1024).ok_or(Mapping)?;
    let buffers_end = buffers.checked_add((N * 2048) as u64).ok_or(Mapping)?;
    if (ring & 0xffff) > 0xfc00 || (ring < buffers_end && buffers < ring_end) {
        return Err(Mapping);
    }
    for index in 0..N {
        if ((buffers + (index * 2048) as u64) & 0xffff) > 0xf800 {
            return Err(Mapping);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "usb_endpoint_read_window_tests.rs"]
mod tests;
