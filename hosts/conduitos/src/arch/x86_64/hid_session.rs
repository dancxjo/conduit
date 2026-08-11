//! Bounded two-report session above validated HID boot reports.

use super::{
    BootReport, HID_DMA, HidError, HidKeyTransition, HidKeyboardReady, HidProof,
    MAX_TRANSITIONS_PER_REPORT, derive_transitions, receive_report, retain_transition,
};
use crate::arch::x86_64::{usb::UsbDevice, xhci::XhciReady};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HidKeyboardSession {
    ready: HidKeyboardReady,
    previous: BootReport,
    observed: [HidKeyTransition; MAX_TRANSITIONS_PER_REPORT],
    observed_count: usize,
    next_report_index: usize,
}

impl HidKeyboardSession {
    pub fn transitions(&self) -> &[HidKeyTransition] {
        &self.observed[..self.observed_count]
    }
}

pub fn receive_first_boot_keyboard_report(
    controller: &mut XhciReady,
    device: &UsbDevice,
    ready: HidKeyboardReady,
) -> Result<HidKeyboardSession, HidError> {
    receive_report(
        controller,
        device,
        ready.endpoint_dci,
        0,
        ready.dma_physical,
    )?;
    super::super::serial::early_write(b"CONDUIT_BOOT_STAGE hid-press-report\n");
    let current = super::parse_report(unsafe { &HID_DMA.reports[0] })?;
    let (transitions, count) = derive_transitions(BootReport::default(), current)?;
    let mut observed = [HidKeyTransition::default(); MAX_TRANSITIONS_PER_REPORT];
    observed[..count].copy_from_slice(&transitions[..count]);
    Ok(HidKeyboardSession {
        ready,
        previous: current,
        observed,
        observed_count: count,
        next_report_index: 1,
    })
}

impl HidKeyboardSession {
    pub fn receive_followup(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
    ) -> Result<([HidKeyTransition; MAX_TRANSITIONS_PER_REPORT], usize), HidError> {
        let index = self.next_report_index;
        if index >= super::MAX_SESSION_REPORTS {
            return Err(HidError::TransferOverflow);
        }
        receive_report(
            controller,
            device,
            self.ready.endpoint_dci,
            index,
            self.ready.dma_physical,
        )?;
        super::super::serial::early_write(b"CONDUIT_BOOT_STAGE hid-release-report\n");
        let current =
            super::parse_report(unsafe { &HID_DMA.reports[index % super::REPORT_BUFFERS] })?;
        let (transitions, count) = derive_transitions(self.previous, current)?;
        for transition in transitions[..count].iter().copied() {
            retain_transition(&mut self.observed, &mut self.observed_count, transition)?;
        }
        self.previous = current;
        self.next_report_index += 1;
        Ok((transitions, count))
    }
}

pub fn finish_boot_keyboard(
    controller: &mut XhciReady,
    device: &UsbDevice,
    mut session: HidKeyboardSession,
) -> Result<HidProof, HidError> {
    session.receive_followup(controller, device)?;
    if session.observed_count != 2
        || session.observed[0].usage != 4
        || !session.observed[0].pressed
        || session.observed[1].usage != 4
        || session.observed[1].pressed
    {
        return Err(HidError::TransferError);
    }
    Ok(HidProof {
        interface_number: session.ready.interface_number,
        endpoint_address: session.ready.endpoint_address,
        endpoint_dci: session.ready.endpoint_dci,
        endpoint_maximum_packet_size: session.ready.endpoint_maximum_packet_size,
        endpoint_interval: session.ready.endpoint_interval,
        set_protocol_transfers: 1,
        interrupt_transfers: super::REPORT_BUFFERS as u8,
        report_bytes: super::BOOT_REPORT_BYTES as u8,
        report_buffers: super::REPORT_BUFFERS as u8,
        maximum_outstanding_interrupt_transfers: super::MAX_OUTSTANDING_INTERRUPT_TRANSFERS,
        maximum_transitions_per_report: MAX_TRANSITIONS_PER_REPORT as u8,
        transfer_trbs: super::INTERRUPT_TRANSFER_TRBS as u8,
        dma_bytes: core::mem::size_of::<super::HidDma>() as u16,
        dma_alignment: core::mem::align_of::<super::HidDma>() as u16,
        sign_slots: super::HID_SIGN_SLOTS,
        interrupt_poll_windows: super::INTERRUPT_POLL_WINDOWS,
        transition_count: session.observed_count as u8,
        transitions: [session.observed[0], session.observed[1]],
    })
}
