//! Architecture proof appliance only: real control TDs across reusable cycles.
//! This scripts wire requests; it is not plotted enumeration/class acceptance.
use super::{ControlRing, UsbError, control::control_raw};
use crate::{
    arch::x86_64::{serial::early_write, xhci::XhciReady},
    sign_format::FixedText,
    usb_base::control_request::ControlTransferRequest,
};
use core::fmt::Write;

pub(super) fn run(controller: &mut XhciReady, ring: &mut ControlRing) -> Result<(), UsbError> {
    let (initial_enqueue, initial_cycle) = unsafe { (*ring.dma).control_cursor.position() };
    let mut cycle = initial_cycle;
    let mut wraps = 0;
    let mut short = 0;
    early_write(b"CONDUIT_USB_RING_PROOF start\n");
    for _ in 0..64 {
        // Fixed fixture overrequests the standard 18-octet device descriptor.
        let request = ControlTransferRequest::new([0x80, 6, 0, 1, 0, 0, 64, 0], &[], 256)
            .map_err(|_| UsbError::TransferPayload)?;
        let actual = control_raw(controller, ring, ring.slot, request)?;
        if actual != 18 {
            return Err(UsbError::MalformedCompletion);
        }
        short += 1;
        let (_, next_cycle) = unsafe { (*ring.dma).control_cursor.position() };
        if next_cycle != cycle {
            wraps += 1;
            cycle = next_cycle;
        }
    }
    if wraps < 4 {
        return Err(UsbError::TransferRingFull);
    }
    let (final_enqueue, final_cycle) = unsafe { (*ring.dma).control_cursor.position() };
    let mut sign = FixedText::new();
    writeln!(sign,"CONDUIT_USB_RING_SIGN {{\"schema\":\"conduit.conduitos.usb-control-ring/v1\",\"proof_class\":\"freestanding-emulator\",\"status\":\"completed\",\"root_port\":{},\"slot\":{},\"transfers\":64,\"short_transfers\":{},\"cycle_transitions\":{},\"initial_enqueue\":{},\"initial_cycle\":{},\"final_enqueue\":{},\"final_cycle\":{},\"ring_trbs\":32,\"maximum_in_flight\":1,\"dma_bytes\":{},\"fixture_protocol\":true}}",ring.root_port,ring.slot,short,wraps,initial_enqueue,initial_cycle,final_enqueue,final_cycle,core::mem::size_of::<super::UsbDma>()).map_err(|_|UsbError::ControlError)?;
    early_write(sign.as_bytes());
    Ok(())
}
