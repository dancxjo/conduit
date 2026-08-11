//! USB-specific transcript validation and real absence proof.

use std::process::{Command, Stdio};

use super::{
    profile::Paths,
    report::{GuestBootSign, GuestUsbSign, GuestXhciSign},
    ConduitosError,
};

pub(super) fn extract(serial: &str) -> Result<GuestUsbSign, ConduitosError> {
    let signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_USB_SIGN "))
        .collect();
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-usb-sign",
            format!("expected one structured USB Sign, found {}", signs.len()),
        ));
    }
    serde_json::from_str(signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-usb-sign", error.to_string()))
}

pub(super) fn validate(
    boot: &GuestBootSign,
    xhci: &GuestXhciSign,
    sign: &GuestUsbSign,
) -> Result<(), ConduitosError> {
    let exact_id =
        |value: &str| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if sign.schema != "conduit.conduitos.usb-device/v1"
        || sign.status != "configured"
        || sign.proof_class != "freestanding-emulator"
        || sign.controller_base_id != xhci.base_id
        || sign.boot_id != boot.boot_id
        || !exact_id(&sign.device_instance_id)
        || !exact_id(&sign.first_interface_id)
        || !exact_id(&sign.first_endpoint_id)
        || sign.device_instance_id == sign.controller_base_id
        || sign.first_interface_id == sign.device_instance_id
        || sign.first_endpoint_id == sign.first_interface_id
        || sign.root_port != 1
        || sign.slot == 0
        || sign.address == 0
        || sign.attachment_epoch != 1
        || sign.ep0_maximum_packet_size == 0
        || sign.configuration_value == 0
        || sign.configuration_bytes < 9
        || sign.descriptor_records < 3
        || sign.interface_count == 0
        || sign.endpoint_count == 0
        || sign.first_endpoint_address & 0x0f == 0
        || sign.configuration_limit_bytes != 256
        || sign.interface_limit != 4
        || sign.endpoint_limit != 8
        || sign.descriptor_record_limit != 16
        || sign.outstanding_control_transfer_limit != 1
        || sign.enumeration_retries != 0
        || sign.control_transfers != 5
        || sign.short_packets > sign.control_transfers
        || sign.transfer_trbs != 32
        || sign.dma_bytes != 8192
        || sign.dma_alignment != 4096
        || sign.port_poll_steps != 2_000_000
        || sign.sign_slots != 12
        || sign.semantic_keyboard_offer
    {
        return Err(ConduitosError::refusal(
            "invalid-usb-sign",
            format!("USB Sign failed exact validation: {sign:?}"),
        ));
    }
    Ok(())
}

pub(super) fn prove_absent(paths: &Paths) -> Result<String, ConduitosError> {
    let output = Command::new("qemu-system-x86_64")
        .args([
            "-M",
            "q35",
            "-cpu",
            "max",
            "-m",
            "64M",
            "-smp",
            "1",
            "-display",
            "none",
            "-vga",
            "none",
            "-monitor",
            "none",
            "-serial",
            "stdio",
            "-no-reboot",
            "-net",
            "none",
            "-rtc",
            "base=2026-08-09T00:00:00,clock=vm",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
            "-cdrom",
            paths.iso.to_str().unwrap(),
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    let serial = String::from_utf8(output.stdout)
        .map_err(|error| ConduitosError::refusal("malformed-usb-refusal", error.to_string()))?;
    let expected = "\"status\":\"refused\",\"reason\":\"usb-device-absent\"";
    if output.status.code() != Some(35)
        || !serial.contains("CONDUIT_BOOT_STAGE xhci-ready")
        || !serial.contains(expected)
        || serial.contains("CONDUIT_USB_SIGN")
        || serial.contains("CONDUIT_KEYBOARD_SIGN")
    {
        return Err(ConduitosError::refusal(
            "usb-absence-not-refused",
            format!("status {}; serial {serial}", output.status),
        ));
    }
    Ok("usb-device-absent".to_owned())
}
