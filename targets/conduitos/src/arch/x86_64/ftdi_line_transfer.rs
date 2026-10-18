//! Exact FTDI transfer publication and completion; no class policy or input loss.
use super::*;

pub(super) fn submit(
    controller: &mut XhciReady,
    device: &UsbDevice,
    ready: FtdiLineReady,
    sequence: usize,
    length: usize,
    direction: TransferDirection,
) -> Result<usize, FtdiLineError> {
    let (dci, ring_offset, buffer_offset) = match direction {
        TransferDirection::Input => (
            ready.input_dci,
            core::mem::offset_of!(FtdiDma, input_ring),
            core::mem::offset_of!(FtdiDma, input_packet),
        ),
        TransferDirection::Output => (
            ready.output_dci,
            core::mem::offset_of!(FtdiDma, output_ring),
            core::mem::offset_of!(FtdiDma, output_packet),
        ),
    };
    let ring = ready.dma_physical + ring_offset as u64;
    let buffer = ready.dma_physical + buffer_offset as u64;
    let position = TransferPosition::at(sequence, FTDI_TRANSFER_SLOTS, 1);
    let trb = ring + (position.slot * 16) as u64;
    if sequence == 0 || position.slot == FTDI_TRANSFER_SLOTS - 1 {
        publish(position.link(ring), |word, value| unsafe {
            let destination = match direction {
                TransferDirection::Input => {
                    core::ptr::addr_of_mut!(FTDI_DMA.input_ring[FTDI_TRANSFER_SLOTS][word])
                }
                TransferDirection::Output => {
                    core::ptr::addr_of_mut!(FTDI_DMA.output_ring[FTDI_TRANSFER_SLOTS][word])
                }
            };
            write_volatile(destination, value);
        });
    }
    publish(position.normal_at(buffer, length), |word, value| unsafe {
        let destination = match direction {
            TransferDirection::Input => {
                core::ptr::addr_of_mut!(FTDI_DMA.input_ring[position.slot][word])
            }
            TransferDirection::Output => {
                core::ptr::addr_of_mut!(FTDI_DMA.output_ring[position.slot][word])
            }
        };
        write_volatile(destination, value);
    });
    controller.ring_endpoint(device.slot, dci);
    let mut completed = None;
    for _ in 0..FTDI_POLL_WINDOWS {
        ensure_present(controller.port_status(device.root_port))?;
        match controller.next_transfer_event(device.slot, dci) {
            Ok(event) => {
                completed = Some(event);
                break;
            }
            Err(XhciError::CommandTimeout) => {}
            Err(XhciError::EventRetentionPressure) => {
                return Err(FtdiLineError::CompletionPressure);
            }
            Err(_) => return Err(FtdiLineError::TransferError),
        }
    }
    let event = completed.ok_or(FtdiLineError::TransferTimeout)?;
    ensure_present(controller.port_status(device.root_port))?;
    validate_event(event, device.slot, dci, trb, length)
}
