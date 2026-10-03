//! Bounded native control-transfer machinery, independent of class/probe policy.
//!
//! Legacy enumeration supplies requests today. Plot-selected callers must bind
//! admitted endpoint/DMA possession before using this machinery; this module
//! alone neither offers a Host Call nor grants authority.

use crate::usb_base::control_request::{ControlRequestRefusal, ControlTransferRequest};
use core::ptr::write_volatile;

use super::transfer::{ControlCompletion, setup_transfer_type};
use super::{MAX_CONFIGURATION_BYTES, TRANSFER_TRBS, UsbDma, UsbError};
use crate::arch::x86_64::xhci::XhciReady;

pub(super) struct ControlRing {
    pub(super) enqueue: usize,
    pub(super) cycle: u32,
    pub(super) physical: u64,
    pub(super) buffer_physical: u64,
    pub(super) root_port: u8,
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
    let length = request.length();
    let input = request.input();
    if usize::from(length) > MAX_CONFIGURATION_BYTES {
        return Err(UsbError::TransferEnvelope);
    }
    let count = if length == 0 { 2 } else { 3 };
    if ring.enqueue + count >= TRANSFER_TRBS {
        return Err(UsbError::TransferRingFull);
    }
    stage_output(ring.dma, &request)?;
    let octets = request.setup();
    let setup_pointer = ring.physical + (ring.enqueue * 16) as u64;
    let data_pointer = (length != 0).then_some(setup_pointer + 16);
    let setup = [
        u32::from_le_bytes(octets[..4].try_into().expect("fixed setup low")),
        u32::from_le_bytes(octets[4..].try_into().expect("fixed setup high")),
        8,
        (2 << 10) | (1 << 6) | setup_transfer_type(input, length) | ring.cycle,
    ];
    put_transfer(ring.dma, ring.enqueue, setup);
    ring.enqueue += 1;
    if length != 0 {
        let data = [
            ring.buffer_physical as u32,
            (ring.buffer_physical >> 32) as u32,
            u32::from(length),
            (3 << 10) | (u32::from(input) << 16) | (u32::from(input) << 2) | ring.cycle,
        ];
        put_transfer(ring.dma, ring.enqueue, data);
        ring.enqueue += 1;
    }
    let status_index = ring.enqueue;
    put_transfer(
        ring.dma,
        status_index,
        [
            0,
            0,
            0,
            (4 << 10) | (u32::from(!input || length == 0) << 16) | (1 << 5) | ring.cycle,
        ],
    );
    ring.enqueue += 1;
    let mut completion = ControlCompletion::new(
        slot,
        setup_pointer,
        data_pointer,
        ring.physical + (status_index * 16) as u64,
        length,
        input,
    );
    controller.ring_endpoint(slot, 1);
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

fn put_transfer(dma: *mut UsbDma, index: usize, trb: [u32; 4]) {
    unsafe { write_volatile(core::ptr::addr_of_mut!((*dma).transfer_ring[index]), trb) };
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
