//! Bounded pointer TD publication and exact completion without consuming peers.
use super::*;

pub(super) fn submit_report(
    controller: &mut XhciReady,
    device: &UsbDevice,
    ready: HidPointerReady,
    index: usize,
) {
    let ring = ready.dma_physical + core::mem::offset_of!(PointerDma, transfer_ring) as u64;
    let position =
        TransferPosition::at(index, POINTER_TRANSFER_REPORT_SLOTS, POINTER_REPORT_BUFFERS);
    let reports = ready.dma_physical + core::mem::offset_of!(PointerDma, reports) as u64;
    if index == 0 || position.slot == POINTER_TRANSFER_REPORT_SLOTS - 1 {
        publish(position.link(ring), |word, value| unsafe {
            write_volatile(
                core::ptr::addr_of_mut!(
                    POINTER_DMA.transfer_ring[POINTER_TRANSFER_REPORT_SLOTS][word]
                ),
                value,
            );
        });
    }
    publish(
        position.normal(reports, POINTER_REPORT_BYTES),
        |word, value| unsafe {
            write_volatile(
                core::ptr::addr_of_mut!(POINTER_DMA.transfer_ring[position.slot][word]),
                value,
            );
        },
    );
    controller.ring_endpoint(device.slot, ready.endpoint_dci);
}

fn completion_pointer(ready: HidPointerReady, index: usize) -> u64 {
    ready.dma_physical
        + core::mem::offset_of!(PointerDma, transfer_ring) as u64
        + (TransferPosition::at(index, POINTER_TRANSFER_REPORT_SLOTS, POINTER_REPORT_BUFFERS).slot
            * 16) as u64
}

pub(super) fn poll_report(
    controller: &mut XhciReady,
    device: &UsbDevice,
    ready: HidPointerReady,
    index: usize,
) -> Result<Option<()>, HidPointerError> {
    ensure_present(controller.port_status(device.root_port))?;
    let Some(event) = controller
        .poll_transfer_event(device.slot, ready.endpoint_dci)
        .map_err(controller_error)?
    else {
        return Ok(None);
    };
    validate_event(
        event,
        device.slot,
        ready.endpoint_dci,
        completion_pointer(ready, index),
    )?;
    ensure_present(controller.port_status(device.root_port))?;
    Ok(Some(()))
}

/// Preserve the existing exclusive receiver's exact finite wait bound. The
/// ordinary cooperative loop uses poll_report once and does not wait here.
pub(super) fn wait_report(
    controller: &mut XhciReady,
    device: &UsbDevice,
    ready: HidPointerReady,
    index: usize,
) -> Result<(), HidPointerError> {
    for _ in 0..POINTER_POLL_WINDOWS {
        ensure_present(controller.port_status(device.root_port))?;
        match controller.next_transfer_event(device.slot, ready.endpoint_dci) {
            Ok(event) => {
                validate_event(
                    event,
                    device.slot,
                    ready.endpoint_dci,
                    completion_pointer(ready, index),
                )?;
                ensure_present(controller.port_status(device.root_port))?;
                return Ok(());
            }
            Err(XhciError::CommandTimeout) => {}
            Err(error) => return Err(controller_error(error)),
        }
    }
    Err(HidPointerError::TransferTimeout)
}

fn controller_error(error: XhciError) -> HidPointerError {
    match error {
        XhciError::EventRetentionPressure => HidPointerError::CompletionPressure,
        _ => HidPointerError::TransferError,
    }
}

pub(super) fn ensure_present(port_status: u32) -> Result<(), HidPointerError> {
    if port_status & 1 == 0 {
        Err(HidPointerError::DeviceRemoved)
    } else {
        Ok(())
    }
}

pub(super) fn validate_event(
    event: Event,
    slot: u8,
    endpoint: u8,
    pointer: u64,
) -> Result<(), HidPointerError> {
    if event.event_data || event.event_type != 32 || event.pointer != pointer {
        return Err(HidPointerError::WrongCompletion);
    }
    if event.slot != slot {
        return Err(HidPointerError::WrongDevice);
    }
    if event.endpoint != endpoint {
        return Err(HidPointerError::WrongEndpoint);
    }
    match event.completion_code {
        1 if event.residual == 0 => Ok(()),
        6 => Err(HidPointerError::TransferStall),
        _ => Err(HidPointerError::TransferError),
    }
}
