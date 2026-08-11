//! HID boot-keyboard transcript extraction and exact correlation checks.

use super::{
    report::{GuestBootSign, GuestHidSign, GuestUsbSign, GuestXhciSign},
    ConduitosError,
};

pub(super) fn extract(serial: &str) -> Result<GuestHidSign, ConduitosError> {
    let signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_HID_SIGN "))
        .collect();
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-hid-sign",
            format!("expected one structured HID Sign, found {}", signs.len()),
        ));
    }
    serde_json::from_str(signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-hid-sign", error.to_string()))
}

pub(super) fn validate(
    boot: &GuestBootSign,
    xhci: &GuestXhciSign,
    usb: &GuestUsbSign,
    hid: &GuestHidSign,
) -> Result<(), ConduitosError> {
    if hid.schema != "conduit.conduitos.hid-boot-keyboard/v1"
        || hid.status != "transitions-observed"
        || hid.proof_class != "freestanding-emulator"
        || hid.controller_base_id != xhci.base_id
        || hid.boot_id != boot.boot_id
        || hid.device_instance_id != usb.device_instance_id
        || hid.interface_id != usb.first_interface_id
        || hid.endpoint_id != usb.first_endpoint_id
        || hid.interface_number != usb.first_interface_number
        || hid.endpoint_address != usb.first_endpoint_address
        || hid.endpoint_dci != 3
        || hid.endpoint_maximum_packet_size != 8
        || hid.endpoint_interval != usb.first_endpoint_interval
        || hid.set_protocol_transfers != 1
        || hid.interrupt_transfers != 2
        || hid.report_bytes != 8
        || hid.report_buffers != 2
        || hid.maximum_outstanding_interrupt_transfers != 2
        || hid.maximum_transitions_per_report != 20
        || hid.transfer_trbs != 48
        || hid.dma_bytes != 4096
        || hid.dma_alignment != 4096
        || hid.sign_slots != 8
        || hid.interrupt_poll_windows != 1024
        || hid.transition_count != 2
        || hid.first_usage_page != "keyboard-keypad"
        || hid.first_usage != 4
        || hid.first_state != "pressed"
        || hid.first_modifiers != 0
        || hid.second_usage_page != "keyboard-keypad"
        || hid.second_usage != 4
        || hid.second_state != "released"
        || hid.second_modifiers != 0
        || hid.layout_translation
        || hid.unicode_translation
        || hid.semantic_keyboard_offer
    {
        return Err(ConduitosError::refusal(
            "invalid-hid-sign",
            format!("HID Sign failed exact validation: {hid:?}"),
        ));
    }
    Ok(())
}
