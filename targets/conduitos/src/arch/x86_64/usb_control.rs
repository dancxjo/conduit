//! Bounded native control-transfer machinery, independent of class/probe policy.
//!
//! Legacy enumeration supplies requests today. Plot-selected callers must bind
//! admitted endpoint/DMA possession before using this machinery; this module
//! alone neither offers a Host Call nor grants authority.

use crate::usb_base::control_request::{ControlRequestRefusal, ControlTransferRequest};
use crate::usb_base::control_ring::{RingRefusal, RingReservation};
use core::ptr::write_volatile;
use core::sync::atomic::{Ordering, fence};

use super::transfer::{ControlCompletion, setup_transfer_type};
use super::{MAX_CONFIGURATION_BYTES, TRANSFER_TRBS, UsbDma, UsbError};
use crate::arch::x86_64::xhci::XhciReady;

pub(super) struct ControlRing {
    pub(super) physical: u64,
    pub(super) buffer_physical: u64,
    pub(super) root_port: u8,
    pub(super) slot: u8,
    pub(super) short_packets: u8,
    pub(super) dma: *mut UsbDma,
}

#[derive(Clone, Copy)]
pub(super) struct ControlRequest {
    pub(super) request_type: u8,
    pub(super) request: u8,
    pub(super) value: u16,
    pub(super) index: u16,
    pub(super) length: u16,
    pub(super) input: bool,
}

pub(super) fn control(
    controller: &mut XhciReady,
    ring: &mut ControlRing,
    slot: u8,
    request: ControlRequest,
) -> Result<usize, UsbError> {
    let setup_word = u64::from(request.request_type)
        | (u64::from(request.request) << 8)
        | (u64::from(request.value) << 16)
        | (u64::from(request.index) << 32)
        | (u64::from(request.length) << 48);
    let raw = ControlTransferRequest::new(
        setup_word.to_le_bytes(),
        &[],
        MAX_CONFIGURATION_BYTES as u16,
    )
    .map_err(|reason| match reason {
        ControlRequestRefusal::DataEnvelope => UsbError::TransferEnvelope,
        _ => UsbError::TransferPayload,
    })?;
    if request.length != 0 && raw.input() != request.input {
        return Err(UsbError::TransferPayload);
    }
    control_raw(controller, ring, slot, raw)
}

/// Raw, class-neutral native operation. The caller owns the exact admitted
/// endpoint/DMA lifetime; a request value does not select or authorize a slot.
/// Product Host Call binding and cancellation are not installed by this seam.
pub(super) fn control_raw(
    controller: &mut XhciReady,
    ring: &mut ControlRing,
    slot: u8,
    request: ControlTransferRequest<'_>,
) -> Result<usize, UsbError> {
    if slot == 0
        || ring.root_port == 0
        || slot != ring.slot
        || unsafe {
            (*ring.dma).owner_slot != slot || (*ring.dma).owner_root_port != ring.root_port
        }
    {
        return Err(UsbError::WrongSlot);
    }
    let (reservation, mut completion) = prepare_transfer(ring, &request)?;
    controller.ring_endpoint(slot, 1);
    let result = wait_completion(controller, ring, &mut completion);
    match result {
        Ok(bytes) => {
            // SAFETY: the exact TD's successful final Status Stage was observed.
            unsafe { (&mut (*ring.dma).control_cursor).complete_quiesced(&reservation) }
                .map_err(ring_refusal)?;
            Ok(bytes)
        }
        Err(error) => {
            // Failed/foreign events and timeout do not acknowledge DMA stop.
            unsafe {
                (&mut (*ring.dma).control_cursor).retain_uncertain();
            }
            Err(error)
        }
    }
}

fn prepare_transfer(
    ring: &mut ControlRing,
    request: &ControlTransferRequest<'_>,
) -> Result<(RingReservation, ControlCompletion), UsbError> {
    let length = request.length();
    let input = request.input();
    if usize::from(length) > MAX_CONFIGURATION_BYTES {
        return Err(UsbError::TransferEnvelope);
    }
    let count = if length == 0 { 2 } else { 3 };
    let reservation = unsafe { (&mut (*ring.dma).control_cursor).reserve(TRANSFER_TRBS, count) }
        .map_err(ring_refusal)?;
    if let Err(error) = stage_output(ring.dma, request) {
        unsafe {
            (&mut (*ring.dma).control_cursor).retain_uncertain();
        }
        return Err(error);
    }
    let start = reservation.start;
    let cycle = reservation.cycle;
    let octets = request.setup();
    let setup_pointer = ring.physical + (start * 16) as u64;
    let data_pointer = (length != 0).then_some(setup_pointer + 16);
    let setup = [
        u32::from_le_bytes(octets[..4].try_into().expect("fixed setup low")),
        u32::from_le_bytes(octets[4..].try_into().expect("fixed setup high")),
        8,
        (2 << 10) | (1 << 6) | setup_transfer_type(input, length) | cycle,
    ];
    // Keep the first TRB unavailable until every payload/control word is ready.
    let mut held_setup = setup;
    held_setup[3] ^= 1;
    put_transfer(ring.dma, start, held_setup);
    if length != 0 {
        let data = [
            ring.buffer_physical as u32,
            (ring.buffer_physical >> 32) as u32,
            u32::from(length),
            (3 << 10) | (u32::from(input) << 16) | (u32::from(input) << 2) | cycle,
        ];
        put_transfer(ring.dma, start + 1, data);
    }
    let status_index = start + reservation.count - 1;
    put_transfer(
        ring.dma,
        status_index,
        [
            0,
            0,
            0,
            (4 << 10) | (u32::from(!input || length == 0) << 16) | (1 << 5) | cycle,
        ],
    );
    // The following free slot may never have been used in an earlier cycle.
    // Explicitly withhold it before exposing this TD: zero-filled tail slots
    // otherwise become falsely owned when the consumer cycle toggles to zero.
    publish_control(ring.dma, start + reservation.count, cycle ^ 1);
    publish_control(ring.dma, start, setup[3]);
    publish_link(ring, &reservation);
    let completion = ControlCompletion::new(
        ring.slot,
        setup_pointer,
        data_pointer,
        ring.physical + (status_index * 16) as u64,
        length,
        input,
    );
    Ok((reservation, completion))
}

fn wait_completion(
    controller: &mut XhciReady,
    ring: &mut ControlRing,
    completion: &mut ControlCompletion,
) -> Result<usize, UsbError> {
    for _ in 0..TRANSFER_TRBS {
        let event = controller.next_event()?;
        if let Some(result) = completion.observe(event)? {
            if result.short {
                ring.short_packets = ring.short_packets.saturating_add(1);
            }
            if controller.port_status(ring.root_port) & 1 == 0 {
                return Err(UsbError::DeviceVanished);
            }
            return Ok(result.bytes);
        }
    }
    Err(UsbError::ControlTimeout)
}

fn ring_refusal(refusal: RingRefusal) -> UsbError {
    match refusal {
        RingRefusal::Geometry => UsbError::TransferRingGeometry,
        RingRefusal::Pending => UsbError::TransferRingFull,
        RingRefusal::Uncertain | RingRefusal::StaleCompletion => UsbError::TransferRingUncertain,
        RingRefusal::Exhausted => UsbError::TransferSequenceExhausted,
    }
}

fn put_transfer(dma: *mut UsbDma, index: usize, trb: [u32; 4]) {
    for (word, value) in trb[..3].iter().copied().enumerate() {
        unsafe {
            write_volatile(
                core::ptr::addr_of_mut!((*dma).transfer_ring[index][word]),
                value,
            );
        }
    }
    publish_control(dma, index, trb[3]);
}

fn publish_control(dma: *mut UsbDma, index: usize, control: u32) {
    // x86 coherent DMA: release plus ordered stores publish payload before C.
    fence(Ordering::Release);
    unsafe {
        write_volatile(
            core::ptr::addr_of_mut!((*dma).transfer_ring[index][3]),
            control,
        );
    }
}

fn publish_link(ring: &ControlRing, reservation: &RingReservation) {
    if let Some((index, cycle)) = reservation.link {
        // xHCI 6.4.4.1: Link TRB type 6 with Toggle Cycle returns to the head.
        // Publish it after the complete new TD; no controller-visible partial TD.
        put_transfer(
            ring.dma,
            index,
            [
                ring.physical as u32,
                (ring.physical >> 32) as u32,
                0,
                (6 << 10) | (1 << 1) | cycle,
            ],
        );
    }
}

/// Native buffer geometry is independently enforced even if request validation
/// was performed with a broader bound. Only an owned, valid DMA pointer may be
/// supplied here; it is never decoded from request octets.
fn stage_output(dma: *mut UsbDma, request: &ControlTransferRequest<'_>) -> Result<(), UsbError> {
    if usize::from(request.length()) > MAX_CONFIGURATION_BYTES {
        return Err(UsbError::TransferEnvelope);
    }
    for (index, byte) in request.output().iter().enumerate() {
        unsafe {
            write_volatile(core::ptr::addr_of_mut!((*dma).descriptor[index]), *byte);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "usb_control_tests.rs"]
mod tests;
