use super::*;
use crate::arch::x86_64::usb::descriptor::{UsbEndpoint, UsbInterface};

fn pointer_device() -> UsbDevice {
    let mut device = crate::arch::x86_64::usb::descriptor::device_from_descriptor(
        2,
        2,
        2,
        8,
        &[18, 1, 0, 2, 0, 0, 0, 8, 0x27, 0x06, 1, 0, 0, 1, 0, 0, 0, 1],
    )
    .unwrap();
    device.dma_slot = 1;
    device.interface_count = 1;
    device.interfaces[0] = UsbInterface {
        number: 0,
        alternate_setting: 0,
        class: 3,
        subclass: 1,
        protocol: 2,
        first_endpoint: 0,
        endpoint_count: 1,
    };
    device.endpoint_count = 1;
    device.endpoints[0] = UsbEndpoint {
        interface_index: 0,
        address: 0x81,
        direction_in: true,
        transfer_type: 3,
        maximum_packet_size: 4,
        interval: 10,
    };
    device
}

#[test]
fn exact_boot_pointer_interface_and_endpoint_match() {
    let device = pointer_device();
    let (interface, endpoint) = match_pointer(&device).unwrap();
    assert_eq!(interface.protocol, 2);
    assert_eq!(endpoint.maximum_packet_size, 4);
    assert_eq!(endpoint_dci(endpoint.address), Ok(3));
}

#[test]
fn keyboard_and_non_hid_interfaces_do_not_become_pointer_truth() {
    let mut device = pointer_device();
    device.interfaces[0].protocol = 1;
    assert_eq!(
        match_pointer(&device),
        Err(HidPointerError::NonBootInterface)
    );
    device.interfaces[0].class = 2;
    assert_eq!(
        match_pointer(&device),
        Err(HidPointerError::InterfaceAbsent)
    );
}

#[test]
fn reports_produce_bounded_motion_and_distinct_button_states() {
    let hover = apply_report(500_000, 500_000, 0, [0, 100, (-100_i8) as u8, 0]).unwrap();
    assert_eq!((hover.position_x, hover.position_y), (900_000, 100_000));
    assert!(!hover.primary_pressed);
    let pressed = apply_report(hover.position_x, hover.position_y, 1, [1, 127, 127, 0]).unwrap();
    assert_eq!(
        (pressed.position_x, pressed.position_y),
        (1_000_000, 608_000)
    );
    assert!(pressed.primary_pressed);
    let released = apply_report(pressed.position_x, pressed.position_y, 2, [0, 0, 0, 0]).unwrap();
    assert!(!released.primary_pressed);
    assert_eq!(released.sequence, 3);
    assert_eq!(released.queue_capacity, POINTER_QUEUE_CAPACITY);
}

#[test]
fn malformed_loss_and_completion_fail_distinctly() {
    assert_eq!(
        apply_report(0, 0, 0, [0x80, 0, 0, 0]),
        Err(HidPointerError::ReservedButtons)
    );
    assert_eq!(ensure_present(0), Err(HidPointerError::DeviceRemoved));
    let event = Event {
        event_type: 32,
        completion_code: 1,
        slot: 2,
        endpoint: 3,
        residual: 0,
        pointer: 0x1000,
    };
    assert_eq!(
        validate_event(event, 1, 3, 0x1000),
        Err(HidPointerError::WrongDevice)
    );
    assert_eq!(
        validate_event(
            Event {
                residual: 1,
                ..event
            },
            2,
            3,
            0x1000
        ),
        Err(HidPointerError::TransferError)
    );
}

#[test]
fn every_refusal_is_machine_readable() {
    for error in [
        HidPointerError::InterfaceAbsent,
        HidPointerError::AmbiguousInterface,
        HidPointerError::NonBootInterface,
        HidPointerError::EndpointAbsent,
        HidPointerError::AmbiguousEndpoint,
        HidPointerError::InvalidEndpoint,
        HidPointerError::UnsupportedPacketSize,
        HidPointerError::SetProtocolFailed,
        HidPointerError::ConfigureEndpointFailed,
        HidPointerError::DmaAddressInvalid,
        HidPointerError::TransferOverflow,
        HidPointerError::TransferTimeout,
        HidPointerError::TransferStall,
        HidPointerError::TransferError,
        HidPointerError::WrongDevice,
        HidPointerError::WrongEndpoint,
        HidPointerError::WrongCompletion,
        HidPointerError::DeviceRemoved,
        HidPointerError::ReservedButtons,
        HidPointerError::SequenceOverflow,
    ] {
        assert!(error.as_str().starts_with("hid-pointer-"));
    }
}

#[test]
fn storage_and_work_are_fixed() {
    assert_eq!(core::mem::size_of::<PointerDma>(), 4096);
    assert_eq!(core::mem::align_of::<PointerDma>(), 4096);
    assert_eq!(POINTER_REPORT_BUFFERS, 2);
    assert_eq!(POINTER_TRANSFER_TRBS, 64);
    assert_eq!(POINTER_POLL_WINDOWS, 1_024);
}
