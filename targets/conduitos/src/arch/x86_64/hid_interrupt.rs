//! One reusable, admitted interrupt ring; exact report completion is unchanged.
use core::ptr::write_volatile;

use super::super::hid_transfer_ring::{TransferPosition, publish};
use super::{
    BOOT_REPORT_BYTES, HID_DMA, HidDma, HidError, INTERRUPT_POLL_WINDOWS, REPORT_BUFFERS,
    TRANSFER_RING_REPORT_SLOTS, ensure_device_present, validate_interrupt_event,
};
use crate::arch::x86_64::{serial, usb::UsbDevice, xhci::XhciReady};

struct Position;
impl Position {
    fn at(sequence: usize) -> TransferPosition {
        TransferPosition::at(sequence, TRANSFER_RING_REPORT_SLOTS, REPORT_BUFFERS)
    }
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
    publish(
        position.normal(reports, BOOT_REPORT_BYTES),
        |word, value| unsafe {
            write_volatile(
                core::ptr::addr_of_mut!(HID_DMA.transfer_ring[position.slot][word]),
                value,
            );
        },
    );
}

/// Publish the next fixed receive window without waiting for completion.
///
/// `REPORT_BUFFERS` is the admitted physical buffering bound. Windows are
/// published only at aligned sequence boundaries so a later `begin_followup`
/// for the already-owned sibling slot is a no-op rather than a duplicate TRB.
/// This keeps the controller collecting the next report while the host services
/// semantic work or a dirty frame.
pub(super) fn submit_report(
    controller: &mut XhciReady,
    device: &UsbDevice,
    dci: u8,
    index: usize,
    dma_physical: u64,
) -> Result<(), HidError> {
    let ring = dma_physical + core::mem::offset_of!(HidDma, transfer_ring) as u64;
    let reports = dma_physical + core::mem::offset_of!(HidDma, reports) as u64;
    let (start, end) = report_window(index)?;
    for report_index in start..end {
        enqueue(report_index, ring, reports);
    }
    controller.ring_endpoint(device.slot, dci);
    if index == 0 {
        serial::early_write(b"CONDUIT_BOOT_STAGE hid-awaiting-qemu-key\n");
    }
    Ok(())
}

fn report_window(index: usize) -> Result<(usize, usize), HidError> {
    if index == 0 {
        return Ok((0, REPORT_BUFFERS));
    }
    let start = index
        .checked_add(REPORT_BUFFERS - 1)
        .ok_or(HidError::TransferOverflow)?;
    let end = start.checked_add(1).ok_or(HidError::TransferOverflow)?;
    Ok((start, end))
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
