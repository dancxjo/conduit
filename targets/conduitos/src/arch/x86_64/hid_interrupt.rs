//! One reusable, admitted interrupt ring; exact report completion is unchanged.
use core::{
    ptr::write_volatile,
    sync::atomic::{Ordering, fence},
};

use super::{
    BOOT_REPORT_BYTES, HID_DMA, HidDma, HidError, INTERRUPT_POLL_WINDOWS, REPORT_BUFFERS,
    TRANSFER_RING_REPORT_SLOTS, ensure_device_present, validate_interrupt_event,
};
use crate::arch::x86_64::{serial, usb::UsbDevice, xhci::XhciReady};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Position {
    slot: usize,
    buffer: usize,
    cycle: u32,
}
impl Position {
    fn at(sequence: usize) -> Self {
        Self {
            slot: sequence % TRANSFER_RING_REPORT_SLOTS,
            buffer: sequence % REPORT_BUFFERS,
            cycle: 1 ^ ((sequence / TRANSFER_RING_REPORT_SLOTS) & 1) as u32,
        }
    }
    fn normal(self, reports: u64) -> [u32; 4] {
        let address = reports + (self.buffer * BOOT_REPORT_BYTES) as u64;
        [
            address as u32,
            (address >> 32) as u32,
            BOOT_REPORT_BYTES as u32,
            (1 << 10) | (1 << 5) | self.cycle,
        ]
    }
    fn link(self, ring: u64) -> [u32; 4] {
        // xHCI 1.2b §4.9.2, §6.4.4.1: one Link TRB toggles cycle at wrap.
        // https://cdrdv2-public.intel.com/625472/625472_xHCI_Rev1_2b.pdf
        [
            ring as u32,
            (ring >> 32) as u32,
            0,
            (6 << 10) | (1 << 1) | self.cycle,
        ]
    }
}

/// Publish payload before granting the controller ownership through cycle.
/// The x86 coherent DMA boundary supplies hardware ordering; release prevents
/// compiler reordering of payload after the final control-word write.
fn publish(trb: [u32; 4], mut write: impl FnMut(usize, u32)) {
    for (word, value) in trb[..3].iter().copied().enumerate() {
        write(word, value);
    }
    fence(Ordering::Release);
    write(3, trb[3]);
}

fn enqueue(sequence: usize, ring: u64, reports: u64) {
    let position = Position::at(sequence);
    // The Link is installed before publishing the last ordinary TRB of each
    // cycle. The controller cannot reach it until that new tail is owned.
    if sequence == 0 || position.slot == TRANSFER_RING_REPORT_SLOTS - 1 {
        publish(position.link(ring), |word, value| unsafe {
            write_volatile(
                core::ptr::addr_of_mut!(HID_DMA.transfer_ring[TRANSFER_RING_REPORT_SLOTS][word]),
                value,
            );
        });
    }
    publish(position.normal(reports), |word, value| unsafe {
        write_volatile(
            core::ptr::addr_of_mut!(HID_DMA.transfer_ring[position.slot][word]),
            value,
        );
    });
}

/// Arms an exact report slot without waiting for a completion.  The caller may
/// now service already-admitted finite work before polling this transfer.
pub(super) fn submit_report(
    controller: &mut XhciReady,
    device: &UsbDevice,
    dci: u8,
    index: usize,
    dma_physical: u64,
) -> Result<(), HidError> {
    let ring = dma_physical + core::mem::offset_of!(HidDma, transfer_ring) as u64;
    let reports = dma_physical + core::mem::offset_of!(HidDma, reports) as u64;
    if index == 0 {
        for report_index in 0..REPORT_BUFFERS {
            enqueue(report_index, ring, reports);
        }
        controller.ring_endpoint(device.slot, dci);
        serial::early_write(b"CONDUIT_BOOT_STAGE hid-awaiting-qemu-key\n");
    } else if index >= REPORT_BUFFERS {
        // Only completed slots are reused: at most two initial reports are in
        // flight, then each next report is submitted after its predecessor.
        enqueue(index, ring, reports);
        controller.ring_endpoint(device.slot, dci);
    }
    Ok(())
}

/// Polls one already-armed report completion.  `Ok(None)` is ordinary: it is
/// not input loss and it leaves the physical transfer armed.
pub(super) fn poll_report(
    controller: &mut XhciReady,
    device: &UsbDevice,
    dci: u8,
    index: usize,
    dma_physical: u64,
) -> Result<Option<()>, HidError> {
    let ring = dma_physical + core::mem::offset_of!(HidDma, transfer_ring) as u64;
    ensure_device_present(controller.port_status(device.root_port))?;
    let event = match controller.poll_event() {
        Some(event) if event.event_type == 34 => return Err(HidError::DeviceRemoved),
        Some(event) => event,
        None => return Ok(None),
    };
    validate_interrupt_event(
        event,
        device.slot,
        dci,
        ring + (Position::at(index).slot * 16) as u64,
    )?;
    ensure_device_present(controller.port_status(device.root_port))?;
    Ok(Some(()))
}

pub(super) fn receive_report(
    controller: &mut XhciReady,
    device: &UsbDevice,
    dci: u8,
    index: usize,
    dma_physical: u64,
) -> Result<(), HidError> {
    submit_report(controller, device, dci, index, dma_physical)?;
    for _ in 0..u64::from(INTERRUPT_POLL_WINDOWS) * u64::from(super::super::xhci::POLL_STEPS) {
        if poll_report(controller, device, dci, index, dma_physical)?.is_some() {
            return Ok(());
        }
    }
    Err(HidError::TransferTimeout)
}

#[cfg(test)]
#[path = "hid_interrupt_tests.rs"]
mod tests;
