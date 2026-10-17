//! Retained finite ISO OUT DMA. Root supplies admission; this leaf has no audio policy.
#![allow(dead_code)] // Checked ISO admission adapter is the next integration step.
use super::{
    UsbDevice, endpoint_setup::ConfiguredIsochronousOutboundEndpoint,
    endpoint_write_transfer::EndpointTransmitDma,
};
use crate::{
    arch::xhci_mechanism::{Event, isochronous, publish_transfer},
    usb_base::{
        endpoint_ring::{EndpointRingRefusal, EndpointRingReservation},
        isochronous_schedule::{
            IsochronousFrame, IsochronousOutGeometry, IsochronousOutObservation,
            IsochronousScheduleRefusal,
        },
    },
};
use core::ptr::write_volatile;
#[derive(Debug)]
pub enum IsochronousOutRefusal {
    Mapping,
    Binding,
    Pending,
    Faulted,
    Schedule(IsochronousScheduleRefusal),
    Ring(EndpointRingRefusal),
    ForeignCompletion,
    Residual,
    Completion(u8),
}
struct Pending {
    reservation: EndpointRingReservation,
    frame: u16,
    buffer: usize,
}
pub(super) struct IsochronousOutTransfer<'a> {
    dma: EndpointTransmitDma<'a>,
    geometry: IsochronousOutGeometry,
    slot: u8,
    attachment_epoch: u32,
    endpoint_epoch: u64,
    dci: u8,
    pending: [Option<Pending>; 8],
    head: usize,
    count: usize,
    next_frame: Option<u16>,
    faulted: bool,
}
impl<'a> IsochronousOutTransfer<'a> {
    /// # Safety
    /// Root has independently admitted this endpoint, schedule and complete resource
    /// envelope. The original controller, move-only receipt, eight buffers and
    /// cursor remain rooted through failure/Drop until actual slot-stop acknowledgement.
    /// Mapping must prove every page and end byte coherent and physically contiguous.
    pub unsafe fn bind(
        device: &UsbDevice,
        configured: ConfiguredIsochronousOutboundEndpoint,
        dma: EndpointTransmitDma<'a>,
        mapping: fn(u64) -> Option<u64>,
    ) -> Result<Self, IsochronousOutRefusal> {
        let c = configured.configured;
        if c.slot != device.slot
            || c.device_epoch != device.attachment_epoch
            || c.endpoint_epoch == 0
            || c.dci == 0
            || c.dci & 1 != 0
            || c.ring_physical != dma.ring_physical
            || dma.cursor.maximum_pending() != 8
            || dma.cursor.ordinary_slots() != 63
        {
            return Err(IsochronousOutRefusal::Binding);
        }
        for (virtual_start, physical, length, alignment) in [
            (
                dma.ring.as_ptr() as u64,
                dma.ring_physical,
                1024_u64,
                64_u64,
            ),
            (dma.buffer.as_ptr() as u64, dma.buffer_physical, 2048, 1),
        ] {
            if physical == 0
                || physical % alignment != 0
                || physical & 0xffff > 0x10000 - length
                || physical.checked_add(length).is_none()
            {
                return Err(IsochronousOutRefusal::Mapping);
            }
            // These small fixed regions span at most two 4 KiB pages.
            for offset in [
                0,
                (4096 - (virtual_start & 4095)).min(length - 1),
                length - 1,
            ] {
                if mapping(virtual_start + offset) != Some(physical + offset) {
                    return Err(IsochronousOutRefusal::Mapping);
                }
            }
        }
        let ring_end = dma.ring_physical + 1024;
        let buffer_end = dma.buffer_physical + 2048;
        if dma.ring_physical < buffer_end && dma.buffer_physical < ring_end {
            return Err(IsochronousOutRefusal::Mapping);
        }
        dma.cursor
            .ensure_idle()
            .map_err(IsochronousOutRefusal::Ring)?;
        Ok(Self {
            dma,
            geometry: configured.geometry,
            slot: device.slot,
            attachment_epoch: device.attachment_epoch,
            endpoint_epoch: c.endpoint_epoch,
            dci: c.dci,
            pending: core::array::from_fn(|_| None),
            head: 0,
            count: 0,
            next_frame: None,
            faulted: false,
        })
    }
    pub fn dci(&self) -> u8 {
        self.dci
    }
    pub fn geometry(&self) -> IsochronousOutGeometry {
        self.geometry
    }
    pub fn pending_count(&self) -> usize {
        self.count
    }
    pub fn validate(
        &self,
        device: &UsbDevice,
        endpoint_epoch: u64,
    ) -> Result<(), IsochronousOutRefusal> {
        if device.slot != self.slot
            || device.attachment_epoch != self.attachment_epoch
            || endpoint_epoch != self.endpoint_epoch
        {
            return Err(IsochronousOutRefusal::Binding);
        }
        super::device_dma_pointer(device).map_err(|_| IsochronousOutRefusal::Binding)?;
        Ok(())
    }
    pub fn begin(
        &mut self,
        frame: IsochronousFrame,
        input: &[u8],
        mfindex: u16,
        ist: u8,
    ) -> Result<(), IsochronousOutRefusal> {
        if self.faulted {
            return Err(IsochronousOutRefusal::Faulted);
        }
        if self.count == 8 {
            return Err(IsochronousOutRefusal::Pending);
        }
        if input.len() > usize::from(self.geometry.packet_bytes()) {
            return Err(IsochronousOutRefusal::Mapping);
        }
        frame
            .validate(mfindex, ist)
            .map_err(IsochronousOutRefusal::Schedule)?;
        // USB2 full-speed one-frame cadence. This verifies the caller's schedule;
        // it never selects a sample clock, skips, retries or repairs a missed frame.
        if self.next_frame.is_some_and(|next| next != frame.number()) {
            return Err(IsochronousOutRefusal::Schedule(
                IsochronousScheduleRefusal::Deadline,
            ));
        }
        let tail = (self.head + self.count) % 8;
        let reservation = self
            .dma
            .cursor
            .reserve_out(input.len() as u16)
            .map_err(IsochronousOutRefusal::Ring)?;
        let start = tail * 256;
        self.dma.buffer[start..start + input.len()].copy_from_slice(input);
        if reservation.slot == 62 {
            publish_transfer(
                reservation.link(self.dma.ring_physical),
                |word, value| unsafe { write_volatile(&mut self.dma.ring[63][word], value) },
            );
        }
        publish_transfer(
            isochronous::out_trb(
                self.dma.buffer_physical + start as u64,
                reservation.length,
                reservation.cycle,
                frame,
            ),
            |word, value| unsafe {
                write_volatile(&mut self.dma.ring[reservation.slot][word], value)
            },
        );
        self.pending[tail] = Some(Pending {
            reservation,
            frame: frame.number(),
            buffer: tail,
        });
        self.count += 1;
        self.next_frame = Some((frame.number() + 1) & 2047);
        Ok(())
    }
    pub fn event(
        &mut self,
        event: Event,
    ) -> Result<IsochronousOutObservation, IsochronousOutRefusal> {
        if event.event_type != 32 || event.slot != self.slot || event.endpoint != self.dci {
            return Err(IsochronousOutRefusal::ForeignCompletion);
        }
        if event.completion_code == 14 {
            self.faulted = true;
            return Ok(IsochronousOutObservation::Underrun);
        }
        let pending = self.pending[self.head]
            .as_ref()
            .ok_or(IsochronousOutRefusal::Pending)?;
        let observation = isochronous::observation(
            event,
            self.slot,
            self.dci,
            self.dma.ring_physical + (pending.reservation.slot * 16) as u64,
            pending.reservation.length,
            pending.frame,
        )
        .map_err(|error| match error {
            isochronous::IsochronousEventRefusal::Foreign => {
                IsochronousOutRefusal::ForeignCompletion
            }
            isochronous::IsochronousEventRefusal::Residual => IsochronousOutRefusal::Residual,
            isochronous::IsochronousEventRefusal::Completion(code) => {
                IsochronousOutRefusal::Completion(code)
            }
        })?;
        // Exact final event acknowledges only this oldest TD, never another buffer.
        unsafe { self.dma.cursor.complete_quiesced(&pending.reservation) }
            .map_err(IsochronousOutRefusal::Ring)?;
        debug_assert_eq!(pending.buffer, self.head);
        self.pending[self.head] = None;
        self.head = (self.head + 1) % 8;
        self.count -= 1;
        if matches!(
            observation,
            IsochronousOutObservation::MissedService { .. }
                | IsochronousOutObservation::Short { .. }
        ) {
            self.faulted = true;
        }
        Ok(observation)
    }
    /// # Safety
    /// Sole original controller acknowledged Disable Slot for this exact attachment.
    pub unsafe fn release_stopped(&mut self) -> Result<(), IsochronousOutRefusal> {
        while self.count != 0 {
            let pending = self.pending[self.head]
                .as_ref()
                .ok_or(IsochronousOutRefusal::Pending)?;
            unsafe { self.dma.cursor.complete_quiesced(&pending.reservation) }
                .map_err(IsochronousOutRefusal::Ring)?;
            self.pending[self.head] = None;
            self.head = (self.head + 1) % 8;
            self.count -= 1;
        }
        self.faulted = true;
        Ok(())
    }
}

#[cfg(test)]
#[path = "usb_isochronous_out/tests.rs"]
mod tests;

#[cfg(test)]
impl IsochronousOutTransfer<'_> {
    pub(crate) fn synthetic_original_wire(&self) -> &[u8] {
        let pending = self.pending[self.head]
            .as_ref()
            .expect("original pending TD");
        let start = pending.buffer * 256;
        &self.dma.buffer[start..start + usize::from(pending.reservation.length)]
    }
    pub(crate) fn synthetic_original_event(&self, code: u8, residual: u32) -> Event {
        let pending = self.pending[self.head]
            .as_ref()
            .expect("original pending TD");
        Event {
            event_type: 32,
            completion_code: code,
            slot: self.slot,
            endpoint: self.dci,
            residual,
            pointer: self.dma.ring_physical + (pending.reservation.slot * 16) as u64,
        }
    }
}
