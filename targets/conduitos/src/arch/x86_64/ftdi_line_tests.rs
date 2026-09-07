use super::*;
use crate::arch::x86_64::usb::descriptor::{UsbInterface, device_from_descriptor};

fn device() -> UsbDevice {
    let mut device = device_from_descriptor(
        3,
        3,
        3,
        8,
        &[
            18, 1, 0, 2, 0, 0, 0, 8, 0x03, 0x04, 0x01, 0x60, 0, 4, 1, 2, 3, 1,
        ],
    )
    .unwrap();
    device.interface_count = 1;
    device.endpoint_count = 2;
    device.interfaces[0] = UsbInterface {
        number: 0,
        alternate_setting: 0,
        class: 0xff,
        subclass: 0xff,
        protocol: 0xff,
        first_endpoint: 0,
        endpoint_count: 2,
    };
    device.endpoints[0] = UsbEndpoint {
        interface_index: 0,
        address: 0x81,
        direction_in: true,
        transfer_type: 2,
        maximum_packet_size: 64,
        interval: 0,
    };
    device.endpoints[1] = UsbEndpoint {
        interface_index: 0,
        address: 0x02,
        direction_in: false,
        transfer_type: 2,
        maximum_packet_size: 64,
        interval: 0,
    };
    device
}

#[test]
fn qemu_ft232bm_identity_and_bulk_pair_are_exact() {
    let (interface, input, output) = match_ftdi(&device()).unwrap();
    assert_eq!(
        (interface.class, interface.subclass, interface.protocol),
        (0xff, 0xff, 0xff)
    );
    assert_eq!(
        (input.address, endpoint_dci(input.address).unwrap()),
        (0x81, 3)
    );
    assert_eq!(
        (output.address, endpoint_dci(output.address).unwrap()),
        (0x02, 4)
    );

    let mut wrong = device();
    wrong.product_id = 0x6002;
    assert_eq!(match_ftdi(&wrong), Err(FtdiLineError::WrongDevice));
    let mut malformed = device();
    malformed.endpoints[1].maximum_packet_size = 512;
    assert_eq!(match_ftdi(&malformed), Err(FtdiLineError::InvalidEndpoint));
}

#[test]
fn transfer_completion_classes_and_bounds_remain_distinct() {
    let event = Event {
        pointer: 0x1000,
        completion_code: 13,
        event_type: 32,
        slot: 3,
        endpoint: 3,
        residual: 17,
    };
    assert_eq!(validate_event(event, 3, 3, 0x1000, 64), Ok(47));
    assert_eq!(
        validate_event(Event { slot: 2, ..event }, 3, 3, 0x1000, 64),
        Err(FtdiLineError::WrongSlot)
    );
    assert_eq!(
        validate_event(
            Event {
                endpoint: 4,
                ..event
            },
            3,
            3,
            0x1000,
            64
        ),
        Err(FtdiLineError::WrongEndpoint)
    );
    assert_eq!(
        validate_event(
            Event {
                completion_code: 6,
                ..event
            },
            3,
            3,
            0x1000,
            64
        ),
        Err(FtdiLineError::TransferStall)
    );
}

#[test]
fn every_refusal_is_machine_readable_and_storage_is_fixed() {
    let failures = [
        FtdiLineError::WrongDevice,
        FtdiLineError::InterfaceAbsent,
        FtdiLineError::AmbiguousInterface,
        FtdiLineError::EndpointAbsent,
        FtdiLineError::AmbiguousEndpoint,
        FtdiLineError::InvalidEndpoint,
        FtdiLineError::ConfigureEndpointsFailed,
        FtdiLineError::DmaAddressInvalid,
        FtdiLineError::EmptyPayload,
        FtdiLineError::OversizedPayload,
        FtdiLineError::TransferOverflow,
        FtdiLineError::TransferTimeout,
        FtdiLineError::TransferStall,
        FtdiLineError::TransferError,
        FtdiLineError::WrongCompletion,
        FtdiLineError::WrongSlot,
        FtdiLineError::WrongEndpoint,
        FtdiLineError::DeviceRemoved,
        FtdiLineError::InvalidStatus,
    ];
    for (index, failure) in failures.iter().enumerate() {
        assert!(failure.as_str().starts_with("ftdi-line-"));
        assert!(
            !failures[..index]
                .iter()
                .any(|prior| prior.as_str() == failure.as_str())
        );
    }
    assert!(core::mem::size_of::<FtdiDma>() <= 4096);
    assert_eq!(core::mem::align_of::<FtdiDma>(), 4096);
    assert_eq!(FTDI_TRANSFER_TRBS, 32);
    assert_eq!(FTDI_PAYLOAD_BYTES, 62);
}
