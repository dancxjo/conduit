//! Reusable bounded-report session above validated HID boot reports.

use super::{
    BootReport, HID_DMA, HidError, HidKeyTransition, HidKeyboardReady, HidProof,
    MAX_SESSION_TRANSITIONS, MAX_TRANSITIONS_PER_REPORT, derive_transitions, receive_report,
};
use crate::arch::x86_64::{usb::UsbDevice, xhci::XhciReady};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HidKeyboardSession {
    ready: HidKeyboardReady,
    previous: BootReport,
    initial: [HidKeyTransition; MAX_TRANSITIONS_PER_REPORT],
    initial_count: usize,
    transition_count: usize,
    next_report_index: usize,
}

impl HidKeyboardSession {
    pub fn transitions(&self) -> &[HidKeyTransition] {
        &self.initial[..self.initial_count]
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
    Ok(HidKeyboardSession {
        ready,
        previous: current,
        initial: transitions,
        initial_count: count,
        transition_count: count,
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
        self.previous = current;
        self.next_report_index += 1;
        self.transition_count = self
            .transition_count
            .checked_add(count)
            .ok_or(HidError::TransitionOverflow)?;
        Ok((transitions, count))
    }

    pub fn receive_until(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
        expected_transitions: usize,
    ) -> Result<(), HidError> {
        self.receive_until_observing(controller, device, expected_transitions, |_| {})
    }

    pub fn receive_until_observing(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
        expected_transitions: usize,
        mut observe: impl FnMut(HidKeyTransition),
    ) -> Result<(), HidError> {
        if expected_transitions > MAX_SESSION_TRANSITIONS
            || expected_transitions < self.transition_count
        {
            return Err(HidError::TransitionOverflow);
        }
        while self.transition_count < expected_transitions {
            let (transitions, count) = self.receive_followup(controller, device)?;
            for transition in transitions[..count].iter().copied() {
                observe(transition);
            }
        }
        if self.transition_count != expected_transitions {
            return Err(HidError::TransitionOverflow);
        }
        Ok(())
    }

    pub fn scripted_initial_proof(
        &self,
        followup: &[HidKeyTransition],
    ) -> Result<HidProof, HidError> {
        if self.initial_count != 1
            || followup.len() != 1
            || self.initial[0].usage != 4
            || !self.initial[0].pressed
            || followup[0].usage != 4
            || followup[0].pressed
        {
            return Err(HidError::ProofSequenceMismatch);
        }
        Ok(HidProof {
            interface_number: self.ready.interface_number,
            endpoint_address: self.ready.endpoint_address,
            endpoint_dci: self.ready.endpoint_dci,
            endpoint_maximum_packet_size: self.ready.endpoint_maximum_packet_size,
            endpoint_interval: self.ready.endpoint_interval,
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
            transition_count: 2,
            transitions: [self.initial[0], followup[0]],
        })
    }
}

pub fn finish_boot_keyboard(
    controller: &mut XhciReady,
    device: &UsbDevice,
    mut session: HidKeyboardSession,
) -> Result<HidProof, HidError> {
    let (followup, count) = session.receive_followup(controller, device)?;
    session.scripted_initial_proof(&followup[..count])
}
