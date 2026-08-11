use super::*;
use crate::arch::x86_64::usb::descriptor::{UsbEndpoint, UsbInterface};

fn keyboard_device() -> UsbDevice {
    let mut device = super::super::usb::descriptor::device_from_descriptor(
        1,
        1,
        1,
        8,
        &[18, 1, 0, 2, 0, 0, 0, 8, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1],
    )
    .unwrap();
    device.interface_count = 1;
    device.endpoint_count = 1;
    device.interfaces[0] = UsbInterface {
        number: 0,
        alternate_setting: 0,
        class: 3,
        subclass: 1,
        protocol: 1,
        first_endpoint: 0,
        endpoint_count: 1,
    };
    device.endpoints[0] = UsbEndpoint {
        interface_index: 0,
        address: 0x81,
        direction_in: true,
        transfer_type: 3,
        maximum_packet_size: 8,
        interval: 7,
    };
    device
}

#[test]
fn exact_boot_keyboard_interface_and_endpoint_match() {
    let (interface, endpoint) = match_keyboard(&keyboard_device()).unwrap();
    assert_eq!(
        (interface.class, interface.subclass, interface.protocol),
        (3, 1, 1)
    );
    assert_eq!(endpoint.address, 0x81);
    assert_eq!(endpoint_dci(endpoint.address), Ok(3));
}

#[test]
fn interface_and_endpoint_refusals_are_distinct() {
    let mut device = keyboard_device();
    device.interfaces[0].class = 2;
    assert_eq!(match_keyboard(&device), Err(HidError::InterfaceAbsent));
    device.interfaces[0].class = 3;
    device.interfaces[0].subclass = 0;
    assert_eq!(match_keyboard(&device), Err(HidError::NonBootInterface));
    device.interfaces[0].subclass = 1;
    device.interfaces[0].protocol = 2;
    assert_eq!(match_keyboard(&device), Err(HidError::MouseProtocol));
    device.interfaces[0].protocol = 1;
    device.endpoints[0].direction_in = false;
    assert_eq!(match_keyboard(&device), Err(HidError::EndpointAbsent));
    device.endpoints[0].direction_in = true;
    device.endpoints[0].maximum_packet_size = 16;
    assert_eq!(
        match_keyboard(&device),
        Err(HidError::UnsupportedPacketSize)
    );

    let mut ambiguous = keyboard_device();
    ambiguous.interface_count = 2;
    ambiguous.interfaces[1] = ambiguous.interfaces[0];
    ambiguous.interfaces[1].number = 1;
    assert_eq!(
        match_keyboard(&ambiguous),
        Err(HidError::AmbiguousInterface)
    );

    let mut endpoints = keyboard_device();
    endpoints.endpoint_count = 2;
    endpoints.endpoints[1] = endpoints.endpoints[0];
    endpoints.endpoints[1].address = 0x82;
    assert_eq!(match_keyboard(&endpoints), Err(HidError::AmbiguousEndpoint));
}

#[test]
fn protocol_loss_and_queue_pressure_are_distinct() {
    assert_eq!(
        map_protocol_error(UsbError::ControlStall),
        HidError::SetProtocolStall
    );
    assert_eq!(
        map_protocol_error(UsbError::ControlTimeout),
        HidError::SetProtocolError
    );
    assert_eq!(ensure_device_present(0), Err(HidError::DeviceRemoved));
    assert_eq!(ensure_device_present(1), Ok(()));

    let mut queue = [HidKeyTransition::default(); 2];
    let mut count = 0;
    let transition = HidKeyTransition {
        usage: 4,
        pressed: true,
        modifiers: 0,
    };
    assert_eq!(
        retain_transition(&mut queue, &mut count, transition),
        Ok(())
    );
    assert_eq!(
        retain_transition(&mut queue, &mut count, transition),
        Ok(())
    );
    assert_eq!(
        retain_transition(&mut queue, &mut count, transition),
        Err(HidError::TransitionOverflow)
    );
}

#[test]
fn reports_derive_press_then_release_without_text() {
    let empty = parse_report(&[0; 8]).unwrap();
    let pressed = parse_report(&[0, 0, 4, 0, 0, 0, 0, 0]).unwrap();
    let (first, first_count) = derive_transitions(empty, pressed).unwrap();
    assert_eq!(first_count, 1);
    assert_eq!(
        first[0],
        HidKeyTransition {
            usage: 4,
            pressed: true,
            modifiers: 0
        }
    );
    let (second, second_count) = derive_transitions(pressed, empty).unwrap();
    assert_eq!(second_count, 1);
    assert_eq!(
        second[0],
        HidKeyTransition {
            usage: 4,
            pressed: false,
            modifiers: 0
        }
    );
}

#[test]
fn modifier_and_usage_order_is_canonical() {
    let previous = parse_report(&[0x02, 0, 6, 4, 0, 0, 0, 0]).unwrap();
    let current = parse_report(&[0x01, 0, 7, 5, 0, 0, 0, 0]).unwrap();
    let (events, count) = derive_transitions(previous, current).unwrap();
    assert_eq!(count, 6);
    let expected = [
        (0xe0, true),
        (0xe1, false),
        (4, false),
        (6, false),
        (5, true),
        (7, true),
    ];
    for (event, expected) in events[..count].iter().zip(expected) {
        assert_eq!((event.usage, event.pressed), expected);
    }
}

#[test]
fn malformed_rollover_and_duplicates_never_make_transitions() {
    assert_eq!(parse_report(&[0; 7]), Err(HidError::ShortReport));
    assert_eq!(
        parse_report(&[0, 1, 0, 0, 0, 0, 0, 0]),
        Err(HidError::ReservedByte)
    );
    assert_eq!(
        parse_report(&[0, 0, 1, 0, 0, 0, 0, 0]),
        Err(HidError::Rollover)
    );
    assert_eq!(
        parse_report(&[0, 0, 4, 4, 0, 0, 0, 0]),
        Err(HidError::DuplicateUsage)
    );
}

#[test]
fn completion_correlation_and_transfer_outcomes_are_distinct() {
    let event = Event {
        event_type: 32,
        completion_code: 1,
        slot: 1,
        endpoint: 3,
        residual: 0,
        pointer: 0x1000,
    };
    assert_eq!(validate_interrupt_event(event, 1, 3, 0x1000), Ok(()));
    assert_eq!(
        validate_interrupt_event(Event { slot: 2, ..event }, 1, 3, 0x1000),
        Err(HidError::WrongDevice)
    );
    assert_eq!(
        validate_interrupt_event(
            Event {
                endpoint: 5,
                ..event
            },
            1,
            3,
            0x1000
        ),
        Err(HidError::WrongEndpoint)
    );
    assert_eq!(
        validate_interrupt_event(
            Event {
                pointer: 0x2000,
                ..event
            },
            1,
            3,
            0x1000
        ),
        Err(HidError::WrongCompletion)
    );
    assert_eq!(
        validate_interrupt_event(
            Event {
                residual: 1,
                ..event
            },
            1,
            3,
            0x1000
        ),
        Err(HidError::ShortReport)
    );
    assert_eq!(
        validate_interrupt_event(
            Event {
                completion_code: 6,
                ..event
            },
            1,
            3,
            0x1000
        ),
        Err(HidError::TransferStall)
    );
}

#[test]
fn all_storage_and_work_limits_are_finite() {
    assert_eq!(BOOT_REPORT_BYTES, 8);
    assert_eq!(MAX_TRANSITIONS_PER_REPORT, 20);
    assert_eq!(MAX_SESSION_TRANSITIONS, 48);
    assert_eq!(MAX_SESSION_REPORTS, 48);
    assert_eq!(REPORT_BUFFERS, 2);
    assert_eq!(MAX_OUTSTANDING_INTERRUPT_TRANSFERS, 2);
    assert_eq!(INTERRUPT_TRANSFER_TRBS, 48);
    assert_eq!(INTERRUPT_POLL_WINDOWS, 1024);
    assert_eq!(core::mem::size_of::<HidDma>(), 4096);
}

#[test]
fn every_failure_has_a_distinct_machine_reason() {
    let failures = [
        HidError::InterfaceAbsent,
        HidError::AmbiguousInterface,
        HidError::NonBootInterface,
        HidError::MouseProtocol,
        HidError::EndpointAbsent,
        HidError::AmbiguousEndpoint,
        HidError::InvalidEndpoint,
        HidError::UnsupportedPacketSize,
        HidError::SetProtocolStall,
        HidError::SetProtocolError,
        HidError::ConfigureEndpointFailed,
        HidError::DmaAddressInvalid,
        HidError::TransferTimeout,
        HidError::TransferStall,
        HidError::TransferError,
        HidError::WrongDevice,
        HidError::WrongEndpoint,
        HidError::WrongCompletion,
        HidError::DeviceRemoved,
        HidError::ShortReport,
        HidError::ReservedByte,
        HidError::Rollover,
        HidError::DuplicateUsage,
        HidError::TransitionOverflow,
    ];
    for (index, failure) in failures.iter().enumerate() {
        assert!(failure.as_str().starts_with("hid-"));
        assert!(
            !failures[..index]
                .iter()
                .any(|prior| prior.as_str() == failure.as_str())
        );
    }
}
