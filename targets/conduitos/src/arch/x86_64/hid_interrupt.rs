//! One reusable, admitted interrupt ring; exact report completion is unchanged.
use core::ptr::write_volatile;

use super::super::hid_transfer_ring::{TransferPosition, publish};
use super::{
    BOOT_REPORT_BYTES, HID_DMA, HidDma, HidError, INTERRUPT_POLL_WINDOWS, REPORT_BUFFERS,
    TRANSFER_RING_REPORT_SLOTS, ensure_device_present, validate_interrupt_event,
};
use crate::arch::x86_64::{
    serial,
    usb::UsbDevice,
    xhci::{XhciError, XhciReady},
};

#[path = "hid_transfer_identity.rs"]
mod identity;

/// Root is the sole keyboard runway owner; pending DMA cannot be reset.
pub(super) fn ensure_idle() -> Result<(), HidError> {
    unsafe { (&*core::ptr::addr_of!(identity::INTERESTS)).ensure_idle() }
}

struct Position;
impl Position {
    fn at(sequence: usize) -> TransferPosition {
        TransferPosition::at(sequence, TRANSFER_RING_REPORT_SLOTS, REPORT_BUFFERS)
    }
}

fn enqueue(sequence: usize, cookie: u64, ring: u64, reports: u64) {
    let position = Position::at(sequence);
    // The Link is installed before publishing the last ordinary TRB of each
    // cycle. The controller cannot reach it until that new tail is owned.
    if sequence == 0 || position.slot == TRANSFER_RING_REPORT_SLOTS - 1 {
        publish(position.link(ring), |word, value| unsafe {
            write_volatile(
                core::ptr::addr_of_mut!(
                    HID_DMA.transfer_ring[TRANSFER_RING_REPORT_SLOTS * 2][word]
                ),
                value,
            );
        });
    }
    // Publish the terminal Event Data first; Normal ownership exposes the TD.
    publish(position.event_data(cookie), |word, value| unsafe {
        write_volatile(
            core::ptr::addr_of_mut!(HID_DMA.transfer_ring[position.slot * 2 + 1][word]),
            value,
        );
    });
    publish(
        position.chained_normal(reports, BOOT_REPORT_BYTES),
        |word, value| unsafe {
            write_volatile(
                core::ptr::addr_of_mut!(HID_DMA.transfer_ring[position.slot * 2][word]),
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
    // Exclusive Root metadata follows the same sole owner as HID_DMA.
    let interests = unsafe { &mut *core::ptr::addr_of_mut!(identity::INTERESTS) };
    interests.arm(start, end, device.attachment_epoch, &identity::SERIAL)?;
    for report_index in start..end {
        let cookie = interests.cookie(report_index, device.attachment_epoch)?;
        enqueue(report_index, cookie, ring, reports);
    }
    #[cfg(feature = "native-latency-observation")]
    super::super::hid_observation::submitted(start, end);
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
    #[cfg(feature = "native-latency-observation")]
    super::super::hid_observation::poll();
    let _ = dma_physical;
    let interests = unsafe { &mut *core::ptr::addr_of_mut!(identity::INTERESTS) };
    let cookie = interests.cookie(index, device.attachment_epoch)?;
    ensure_device_present(controller.port_status(device.root_port))?;
    let Some(event) =
        controller
            .poll_transfer_event(device.slot, dci)
            .map_err(|error| match error {
                XhciError::EventRetentionPressure => HidError::CompletionPressure,
                _ => HidError::TransferError,
            })?
    else {
        #[cfg(feature = "native-latency-observation")]
        super::super::hid_observation::empty_poll();
        return Ok(None);
    };
    validate_interrupt_event(event, device.slot, dci, cookie)?;
    ensure_device_present(controller.port_status(device.root_port))?;
    interests.retire(index, cookie)?;
    #[cfg(feature = "native-latency-observation")]
    super::super::hid_observation::completed(index);
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
    let budget = super::super::xhci::ReceiveBudget::new();
    for _ in 0..u64::from(INTERRUPT_POLL_WINDOWS) * u64::from(super::super::xhci::POLL_STEPS) {
        if poll_report(controller, device, dci, index, dma_physical)?.is_some() {
            return Ok(());
        }
        if budget.expired() {
            return Err(HidError::TransferTimeout);
        }
        budget.idle(controller);
    }
    Err(HidError::TransferTimeout)
}

#[cfg(test)]
#[path = "hid_interrupt_tests.rs"]
mod tests;
