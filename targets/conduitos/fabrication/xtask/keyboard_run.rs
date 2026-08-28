//! Portable keyboard transcript extraction and exact realization checks.

use super::{
    report::{GuestBootSign, GuestHidSign, GuestKeyboardSign, GuestUsbSign, GuestXhciSign},
    ConduitosError,
};

pub(super) fn extract(serial: &str) -> Result<GuestKeyboardSign, ConduitosError> {
    let signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_KEYBOARD_SIGN "))
        .collect();
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-keyboard-sign",
            format!(
                "expected one structured keyboard Sign, found {}",
                signs.len()
            ),
        ));
    }
    serde_json::from_str(signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-keyboard-sign", error.to_string()))
}

pub(super) fn validate(
    boot: &GuestBootSign,
    xhci: &GuestXhciSign,
    usb: &GuestUsbSign,
    hid: &GuestHidSign,
    keyboard: &GuestKeyboardSign,
    observatory: &conduit_observatory::ObservatorySnapshot,
) -> Result<(), ConduitosError> {
    let exact_id =
        |value: &str| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    let advertised = observatory.hosts.first().is_some_and(|host| {
        host.advertisement.capabilities.iter().any(|capability| {
            capability.kind_id.as_str() == "input/keyboard"
                && capability.kind_contract_revision.as_str() == "conduit.input/keyboard@1"
                && capability.implementation.implementation_id.as_str()
                    == "conduitos/usb-hid-keyboard@1"
                && capability.implementation.execution_profile_id.as_str()
                    == "conduitos/usb-input-cooperative@1"
                && capability.implementation.artifact_id.as_str()
                    == format!("conduitos-build/{}", boot.build_id)
                && capability.inputs.is_empty()
                && capability.outputs.len() == 1
                && capability.outputs[0].port_id.as_str() == "key"
                && capability.outputs[0].value_kind.as_str() == "input/key-event@1"
                && capability.host_operations.len() == 1
                && capability.authority_requirements.is_empty()
                && capability.resource_requirements.len() == 8
        })
    });
    if keyboard.schema != "conduit.conduitos.keyboard-offer/v1"
        || keyboard.status != "completed"
        || keyboard.proof_class != "freestanding-emulator"
        || keyboard.host_id != boot.host_id
        || keyboard.boot_id != boot.boot_id
        || keyboard.offer_generation != 1
        || keyboard.kind != "input/keyboard"
        || keyboard.contract_revision != "conduit.input/keyboard@1"
        || keyboard.implementation != "conduitos/usb-hid-keyboard@1"
        || keyboard.execution_profile != "conduitos/usb-input-cooperative@1"
        || keyboard.artifact_build != boot.build_id
        || keyboard.controller_base_id != xhci.base_id
        || keyboard.device_instance_id != usb.device_instance_id
        || keyboard.interface_id != hid.interface_id
        || keyboard.endpoint_id != hid.endpoint_id
        || !exact_id(&keyboard.plan_id)
        || !exact_id(&keyboard.active_play_id)
        || keyboard.plan_id == keyboard.active_play_id
        || keyboard.resource_bindings != 8
        || keyboard.report_buffers != 2
        || keyboard.transition_slots != 8
        || keyboard.operation_slots != 2
        || keyboard.cord_item_capacity != 1
        || keyboard.cord_byte_capacity != 3
        || keyboard.event_count != 2
        || keyboard.first_value != [4, 0, 0]
        || keyboard.second_value != [4, 1, 0]
        || keyboard.semantic_usb_facts
        || keyboard.layout_translation
        || keyboard.unicode_translation
        || !keyboard.completed
        || !advertised
    {
        return Err(ConduitosError::refusal(
            "invalid-keyboard-sign",
            format!("keyboard Sign failed exact validation: {keyboard:?}"),
        ));
    }
    Ok(())
}
