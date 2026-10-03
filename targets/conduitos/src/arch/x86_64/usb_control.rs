//! Bounded native control-transfer machinery, independent of class/probe policy.
//!
//! Legacy enumeration supplies requests today. Plot-selected callers must bind
//! admitted endpoint/DMA possession before using this machinery; this module
//! alone neither offers a Host Call nor grants authority.

use core::ptr::write_volatile;

use super::transfer::{setup_transfer_type, transferred_bytes, validate_transfer_event};
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
    let ControlRequest {
        request_type,
        request,
        value,
        index,
        length,
        input,
    } = request;
    if usize::from(length) > MAX_CONFIGURATION_BYTES {
        return Err(UsbError::TransferEnvelope);
    }
    let count = if length == 0 { 2 } else { 3 };
    if ring.enqueue + count >= TRANSFER_TRBS {
        return Err(UsbError::TransferRingFull);
    }
    let setup = [
        u32::from(request_type) | (u32::from(request) << 8) | (u32::from(value) << 16),
        u32::from(index) | (u32::from(length) << 16),
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
            (3 << 10) | (u32::from(input) << 16) | ring.cycle,
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
    controller.ring_endpoint(slot, 1);
    let mut event = controller.next_event()?;
    for _ in 0..TRANSFER_TRBS {
        if event.event_type != 34 {
            break;
        }
        event = controller.next_event()?;
    }
    if validate_transfer_event(event, slot, ring.physical + (status_index * 16) as u64)? {
        ring.short_packets = ring.short_packets.saturating_add(1);
    }
    if controller.port_status(ring.root_port) & 1 == 0 {
        return Err(UsbError::DeviceVanished);
    }
    transferred_bytes(length, event.residual)
}

fn put_transfer(dma: *mut UsbDma, index: usize, trb: [u32; 4]) {
    unsafe { write_volatile(core::ptr::addr_of_mut!((*dma).transfer_ring[index]), trb) };
}
