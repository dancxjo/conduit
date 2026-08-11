use super::*;
use crate::arch::x86_64::xhci::XhciError;

fn blank_device() -> UsbDevice {
    device_from_descriptor(
        1,
        1,
        1,
        8,
        &[
            18, 1, 0, 2, 0, 0, 0, 8, 0x34, 0x12, 0x78, 0x56, 0, 1, 0, 0, 0, 1,
        ],
    )
    .unwrap()
}

const KEYBOARD_CONFIGURATION: [u8; 34] = [
    9, 2, 34, 0, 1, 1, 0, 0xa0, 50, 9, 4, 0, 0, 1, 3, 1, 1, 0, 9, 0x21, 0x11, 1, 0, 1, 0x22, 63, 0,
    7, 5, 0x81, 3, 8, 0, 10,
];

#[test]
fn bounded_configuration_retains_structural_truth_without_semantics() {
    let mut device = blank_device();
    parse_configuration(&KEYBOARD_CONFIGURATION, &mut device).unwrap();
    assert_eq!(device.configuration_value, 1);
    assert_eq!(device.interface_count, 1);
    assert_eq!(device.endpoint_count, 1);
    assert_eq!(device.interfaces[0].class, 3);
    assert_eq!(device.endpoints[0].address, 0x81);
}

#[test]
fn malformed_descriptor_chain_refuses() {
    let mut bytes = KEYBOARD_CONFIGURATION;
    bytes[18] = 0;
    assert_eq!(
        parse_configuration(&bytes, &mut blank_device()),
        Err(UsbError::MalformedDescriptor)
    );
}

#[test]
fn oversized_configuration_refuses_before_parsing() {
    let mut bytes = KEYBOARD_CONFIGURATION;
    bytes[2..4].copy_from_slice(&257_u16.to_le_bytes());
    assert_eq!(
        parse_configuration(&bytes, &mut blank_device()),
        Err(UsbError::OversizedConfiguration)
    );
}

#[test]
fn interface_and_endpoint_limits_are_finite() {
    assert_eq!(descriptor::MAX_INTERFACES, 4);
    assert_eq!(descriptor::MAX_ENDPOINTS, 8);
    assert_eq!(MAX_OUTSTANDING_CONTROL_TRANSFERS, 1);
    assert_eq!(MAX_ENUMERATION_RETRIES, 0);
}

#[test]
fn completion_identities_fail_separately() {
    let base = Event {
        event_type: 32,
        completion_code: 1,
        slot: 1,
        endpoint: 1,
        residual: 0,
        pointer: 16,
    };
    assert_eq!(
        validate_transfer_event(Event { slot: 2, ..base }, 1, 16),
        Err(UsbError::WrongSlot)
    );
    assert_eq!(
        validate_transfer_event(
            Event {
                endpoint: 2,
                ..base
            },
            1,
            16
        ),
        Err(UsbError::WrongEndpoint)
    );
    assert_eq!(
        validate_transfer_event(
            Event {
                pointer: 32,
                ..base
            },
            1,
            16
        ),
        Err(UsbError::WrongController)
    );
}

#[test]
fn short_packet_and_stall_are_explicit_control_outcomes() {
    let base = Event {
        event_type: 32,
        completion_code: 13,
        slot: 1,
        endpoint: 1,
        residual: 2,
        pointer: 16,
    };
    assert_eq!(validate_transfer_event(base, 1, 16), Ok(true));
    assert_eq!(
        validate_transfer_event(
            Event {
                completion_code: 6,
                ..base
            },
            1,
            16
        ),
        Err(UsbError::ControlStall)
    );
    assert_eq!(
        UsbError::from(XhciError::CommandTimeout),
        UsbError::ControlTimeout
    );
    assert_eq!(
        UsbError::from(XhciError::UnexpectedCompletion),
        UsbError::ControlError
    );
}

#[test]
fn port_reset_distinguishes_progress_failure_and_vanish() {
    assert_eq!(classify_port_reset(1 | (1 << 4)), None);
    assert_eq!(classify_port_reset(1 | 2), Some(Ok(())));
    assert_eq!(classify_port_reset(1), Some(Err(UsbError::PortResetFailed)));
    assert_eq!(classify_port_reset(0), Some(Err(UsbError::DeviceVanished)));
}

#[test]
fn excessive_interface_count_refuses() {
    let mut bytes = [0_u8; 61];
    bytes[..9].copy_from_slice(&[9, 2, 61, 0, 5, 1, 0, 0x80, 50]);
    for index in 0..5 {
        let offset = 9 + index * 9;
        bytes[offset..offset + 9].copy_from_slice(&[9, 4, index as u8, 0, 0, 3, 1, 1, 0]);
    }
    bytes[54..].copy_from_slice(&[7, 5, 0x81, 3, 8, 0, 10]);
    assert_eq!(
        parse_configuration(&bytes, &mut blank_device()),
        Err(UsbError::TooManyInterfaces)
    );
}

#[test]
fn excessive_endpoint_count_refuses() {
    let mut bytes = [0_u8; 81];
    bytes[..9].copy_from_slice(&[9, 2, 81, 0, 1, 1, 0, 0x80, 50]);
    bytes[9..18].copy_from_slice(&[9, 4, 0, 0, 9, 3, 1, 1, 0]);
    for index in 0..9 {
        let offset = 18 + index * 7;
        bytes[offset..offset + 7].copy_from_slice(&[7, 5, 0x81 + index as u8, 3, 8, 0, 10]);
    }
    assert_eq!(
        parse_configuration(&bytes, &mut blank_device()),
        Err(UsbError::TooManyEndpoints)
    );
}

#[test]
fn excessive_descriptor_record_count_refuses() {
    let mut bytes = [0_u8; 53];
    bytes[..9].copy_from_slice(&[9, 2, 53, 0, 1, 1, 0, 0x80, 50]);
    bytes[9..18].copy_from_slice(&[9, 4, 0, 0, 1, 3, 1, 1, 0]);
    bytes[18..25].copy_from_slice(&[7, 5, 0x81, 3, 8, 0, 10]);
    for index in 0..14 {
        let offset = 25 + index * 2;
        bytes[offset..offset + 2].copy_from_slice(&[2, 0x30]);
    }
    assert_eq!(
        parse_configuration(&bytes, &mut blank_device()),
        Err(UsbError::TooManyDescriptorRecords)
    );
}

#[test]
fn unsupported_configuration_without_endpoint_refuses() {
    let bytes = [9, 2, 18, 0, 1, 1, 0, 0x80, 50, 9, 4, 0, 0, 0, 3, 1, 1, 0];
    assert_eq!(
        parse_configuration(&bytes, &mut blank_device()),
        Err(UsbError::UnsupportedTopology)
    );
}

#[test]
fn reattachment_cannot_inherit_device_instance_identity() {
    let base = crate::identity::derive_base(&[1; 32], "xhci");
    let old = crate::identity::derive_usb_device(&[1; 32], &base, 1, 1, 1);
    let fresh = crate::identity::derive_usb_device(&[1; 32], &base, 1, 1, 2);
    assert_ne!(old, fresh);
}

#[test]
fn all_failures_are_machine_readable_and_distinct() {
    let errors = [
        UsbError::NoDevice,
        UsbError::PortResetTimeout,
        UsbError::MalformedDescriptor,
        UsbError::OversizedConfiguration,
        UsbError::ControlStall,
        UsbError::DeviceVanished,
        UsbError::StaleDeviceInstance,
    ];
    for error in errors {
        assert!(error.as_str().starts_with("usb-"));
    }
    assert_ne!(UsbError::NoDevice, UsbError::DeviceVanished);
    assert_ne!(UsbError::ControlStall, UsbError::ControlTimeout);
}
