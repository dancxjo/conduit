//! Bounded QEMU xHCI detach/reattach proof for one planned keyboard realization.

use alloc::format;

use conduit_kernel::scheduler::{HostOperationRequest, SchedulerError, SchedulerStatus};

use crate::{
    arch::{self, HidError, HidKeyboardSession, UsbDevice, XhciReady},
    boot::{self, BootRecord},
    identity::{self, BootIdentities},
    keyboard_bridge,
    keyboard_offer::KeyboardRealization,
    keyboard_text_plan,
    keyboard_text_play::{KeyboardTextKernel, KeyboardTextRequestKind},
    offer::HostOffer,
};

pub struct HotplugProofInputs<'a> {
    pub record: &'a BootRecord,
    pub identities: &'a BootIdentities,
    pub controller: &'a mut XhciReady,
    pub d1: UsbDevice,
    pub d1_session: HidKeyboardSession,
    pub d1_offer: HostOffer<'a>,
    pub controller_id: [u8; 32],
    pub build_id: &'a str,
}

pub fn run(mut inputs: HotplugProofInputs<'_>) -> ! {
    match prove(&mut inputs) {
        Ok(()) => arch::deterministic_exit(true),
        Err(reason) => {
            arch::early_write(b"CONDUIT_HOTPLUG_REFUSAL ");
            arch::early_write(reason.as_bytes());
            arch::early_write(b"\n");
            arch::deterministic_exit(false)
        }
    }
}

fn prove(inputs: &mut HotplugProofInputs<'_>) -> Result<(), &'static str> {
    let record = inputs.record;
    let identities = inputs.identities;
    let controller = &mut *inputs.controller;
    let d1 = &inputs.d1;
    let d1_session = &mut inputs.d1_session;
    let d1_offer = &inputs.d1_offer;
    let controller_id = inputs.controller_id;
    let build_id = inputs.build_id;
    let p1 = keyboard_text_plan::prepare(identities, d1_offer, build_id)
        .map_err(|_| "d1-plan-refused")?;
    let immutable_p1 = p1.plan.clone();
    let mut x = KeyboardTextKernel::prepare(&p1, 3).map_err(|_| "d1-play-refused")?;

    for stage in [b"hotplug-d1-key-down\n".as_slice(), b"hotplug-d1-key-up\n"] {
        arch::early_write(b"CONDUIT_BOOT_STAGE ");
        arch::early_write(stage);
        let (transitions, count) = d1_session
            .receive_followup(controller, d1)
            .map_err(|_| "d1-report-refused")?;
        if count != 1 {
            return Err("d1-transition-count-invalid");
        }
        let event = keyboard_bridge::portable_key_event(
            transitions[0].usage(),
            transitions[0].pressed(),
            transitions[0].modifiers(),
        )
        .map_err(|_| "d1-event-invalid")?;
        let request = drive_to_keyboard(&mut x)?;
        x.complete_keyboard(request, event)
            .map_err(|_| "d1-completion-refused")?;
    }
    let pending = drive_to_keyboard(&mut x)?;
    arch::early_write(b"CONDUIT_BOOT_STAGE hotplug-d1-transfer-pending\n");
    let mut detached = false;
    for _ in 0..4 {
        match d1_session.receive_followup(controller, d1) {
            Err(HidError::DeviceRemoved) => {
                detached = true;
                break;
            }
            Ok(_) => {}
            Err(error) => return Err(error.as_str()),
        }
    }
    if !detached {
        return Err("detach-not-observed-as-device-loss");
    }
    x.fail_keyboard_device_removed(pending)
        .map_err(|_| "device-loss-not-delivered")?;
    let failed = (0..64).any(|_| matches!(x.step(), Err(SchedulerError::OperationFailed(_))));
    if !failed || p1.plan != immutable_p1 {
        return Err("d1-play-not-terminal-or-plan-mutated");
    }
    let stale_completions =
        arch::retire_removed_device(controller, d1).map_err(|_| "d1-retirement-refused")?;
    arch::early_write(b"CONDUIT_BOOT_STAGE hotplug-d1-retired\n");

    arch::wait_for_attachment_state(controller, d1.root_port, true)
        .map_err(|_| "d2-attachment-timeout")?;
    let d2 = arch::enumerate_one_at_epoch(controller, boot::executable_physical_address, 2)
        .map_err(|_| "d2-enumeration-refused")?;
    let d2_ready = arch::prepare_boot_keyboard(controller, &d2, boot::executable_physical_address)
        .map_err(|_| "d2-hid-refused")?;
    let d2_ids = realization(
        identities,
        controller_id,
        &d2,
        d2_ready.report_buffers,
        d2_ready.transition_slots,
        d2_ready.operation_slots,
    );
    let d1_ids = d1_offer.keyboard.ok_or("d1-offer-missing")?.realization;
    if d1_ids.device_id == d2_ids.device_id
        || d1.vendor_id != d2.vendor_id
        || d1.product_id != d2.product_id
    {
        return Err("reattachment-identity-invalid");
    }
    let mut d2_offer = HostOffer::new(
        identities,
        build_id,
        arch::feature_basis(),
        record.runtime_arena.length,
    );
    d2_offer.generation = d1_offer.generation + 1;
    let d2_offer = d2_offer
        .with_keyboard(d2_ids, build_id)
        .map_err(|_| "d2-offer-refused")?;
    let p2 = keyboard_text_plan::prepare(identities, &d2_offer, build_id)
        .map_err(|_| "d2-plan-refused")?;
    if p1.plan.plan_id == p2.plan.plan_id
        || p1.source_document_id != p2.source_document_id
        || p1.checked_form_id != p2.checked_form_id
        || p1.expanded_form_id != p2.expanded_form_id
        || keyboard_text_plan::validate(&p1.plan, &p2.advertisement, &d2_offer, build_id).is_ok()
    {
        return Err("fresh-plan-or-stale-plan-check-invalid");
    }

    arch::early_write(b"CONDUIT_BOOT_STAGE hotplug-d2-key-down\n");
    let mut d2_session = arch::receive_first_boot_keyboard_report(controller, &d2, d2_ready)
        .map_err(|_| "d2-first-report-refused")?;
    arch::early_write(b"CONDUIT_BOOT_STAGE hotplug-d2-key-up\n");
    let (followup, count) = d2_session
        .receive_followup(controller, &d2)
        .map_err(|_| "d2-second-report-refused")?;
    let pair = replacement_report_pair(d2_session.transitions(), &followup[..count])?;
    let events = pair.map(|transition| {
        keyboard_bridge::portable_key_event(
            transition.usage(),
            transition.pressed(),
            transition.modifiers(),
        )
    });
    let [Ok(first), Ok(second)] = events else {
        return Err("d2-event-invalid");
    };
    let events = [first, second];
    let mut y = KeyboardTextKernel::prepare(&p2, 2).map_err(|_| "d2-play-refused")?;
    for event in events {
        let request = drive_to_keyboard(&mut y)?;
        y.complete_keyboard(request, event)
            .map_err(|_| "d2-completion-refused")?;
    }
    if finish(&mut y)? != 1 {
        return Err("d2-play-result-invalid");
    }
    let sign = format!(
        "CONDUIT_HOTPLUG_SIGN {{\"schema\":\"conduit.conduitos.keyboard-hotplug/v1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"source_document_id\":\"{}\",\"checked_form_id\":\"{}\",\"expanded_form_id\":\"{}\",\"d1_device_id\":\"{}\",\"p1_plan_id\":\"{}\",\"x_active_play_id\":\"{}\",\"x_terminal\":\"failed-device-removed\",\"p1_immutable\":true,\"fabricated_semantic_events\":0,\"stale_completions_retired\":{},\"d2_device_id\":\"{}\",\"p2_plan_id\":\"{}\",\"y_active_play_id\":\"{}\",\"same_form\":true,\"same_host\":true,\"same_boot\":true,\"stale_plan_refused\":true,\"semantic_topology_stable\":true,\"usb_hid_in_form\":false,\"completed\":true}}\n",
        identity::hex(&identities.host),
        identity::hex(&identities.boot),
        p1.source_document_id.as_str(),
        p1.checked_form_id.as_str(),
        p1.expanded_form_id.as_str(),
        identity::hex(&d1_ids.device_id),
        p1.plan.plan_id.as_str(),
        p1.active_play.active_play_id.as_str(),
        stale_completions,
        identity::hex(&d2_ids.device_id),
        p2.plan.plan_id.as_str(),
        p2.active_play.active_play_id.as_str(),
    );
    arch::early_write(sign.as_bytes());
    arch::early_write(b"CONDUIT_BOOT_STAGE hotplug-completed\n");
    Ok(())
}

fn drive_to_keyboard(
    kernel: &mut KeyboardTextKernel,
) -> Result<HostOperationRequest, &'static str> {
    for _ in 0..128 {
        while let Some(request) = kernel.next_host_request() {
            match kernel
                .request_kind(request)
                .map_err(|_| "request-kind-invalid")?
            {
                KeyboardTextRequestKind::Keyboard => return Ok(request),
                KeyboardTextRequestKind::Keymap => kernel.complete_keymap(request),
                KeyboardTextRequestKind::Upper => kernel.complete_upper(request),
                KeyboardTextRequestKind::Presentation => {
                    kernel.complete_presentation(request).map(|_| ())
                }
            }
            .map_err(|_| "semantic-operation-refused")?;
        }
        if !matches!(
            kernel.step(),
            Ok(SchedulerStatus::Progress { .. }) | Ok(SchedulerStatus::Idle)
        ) {
            return Err("play-ended-before-keyboard-request");
        }
    }
    Err("keyboard-request-timeout")
}

fn finish(kernel: &mut KeyboardTextKernel) -> Result<u8, &'static str> {
    let mut presentations = 0_u8;
    for _ in 0..256 {
        while let Some(request) = kernel.next_host_request() {
            match kernel
                .request_kind(request)
                .map_err(|_| "request-kind-invalid")?
            {
                KeyboardTextRequestKind::Keyboard => return Err("unexpected-extra-key-request"),
                KeyboardTextRequestKind::Keymap => kernel.complete_keymap(request),
                KeyboardTextRequestKind::Upper => kernel.complete_upper(request),
                KeyboardTextRequestKind::Presentation => {
                    presentations += 1;
                    kernel.complete_presentation(request).map(|_| ())
                }
            }
            .map_err(|_| "semantic-operation-refused")?;
        }
        match kernel.step() {
            Ok(SchedulerStatus::Complete) => return Ok(presentations),
            Ok(SchedulerStatus::Progress { .. }) | Ok(SchedulerStatus::Idle) => {}
            _ => return Err("d2-kernel-terminal-invalid"),
        }
    }
    Err("d2-kernel-timeout")
}

fn realization(
    identities: &BootIdentities,
    controller_id: [u8; 32],
    device: &UsbDevice,
    report_buffers: u16,
    transition_slots: u16,
    operation_slots: u16,
) -> KeyboardRealization {
    let device_id = identity::derive_usb_device(
        &identities.boot,
        &controller_id,
        device.root_port,
        device.slot,
        device.attachment_epoch,
    );
    let interface = device.interfaces[0];
    let interface_id =
        identity::derive_usb_interface(&device_id, interface.number, interface.alternate_setting);
    let endpoint_id = identity::derive_usb_endpoint(&interface_id, device.endpoints[0].address);
    KeyboardRealization {
        controller_id,
        device_id,
        interface_id,
        endpoint_id,
        report_buffers,
        transition_slots,
        operation_slots,
    }
}

fn replacement_report_pair(
    initial: &[arch::HidKeyTransition],
    followup: &[arch::HidKeyTransition],
) -> Result<[arch::HidKeyTransition; 2], &'static str> {
    let ([first], [second]) = (initial, followup) else {
        return Err("d2-transition-count-invalid");
    };
    if !first.pressed() || second.pressed() || first.usage() != second.usage() {
        return Err("d2-transition-sequence-invalid");
    }
    Ok([*first, *second])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_uses_initial_press_and_returned_release_without_retained_history() {
        let down = arch::HidKeyTransition::new(7, true, 0);
        let up = arch::HidKeyTransition::new(7, false, 0);
        assert_eq!(replacement_report_pair(&[down], &[up]), Ok([down, up]));
        for (initial, followup) in [
            (&[][..], &[up][..]),
            (&[down][..], &[][..]),
            (&[down, up][..], &[up][..]),
        ] {
            assert_eq!(
                replacement_report_pair(initial, followup),
                Err("d2-transition-count-invalid")
            );
        }
        for pair in [
            [up, down],
            [down, down],
            [down, arch::HidKeyTransition::new(8, false, 0)],
        ] {
            assert_eq!(
                replacement_report_pair(&pair[..1], &pair[1..]),
                Err("d2-transition-sequence-invalid")
            );
        }
    }
}
