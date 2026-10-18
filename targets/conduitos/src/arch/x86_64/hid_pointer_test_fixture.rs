use super::*;
use crate::arch::x86_64::usb::descriptor::{UsbEndpoint, UsbInterface};

pub(super) fn pointer_device() -> UsbDevice {
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

pub(super) fn pointer_ready(device: &UsbDevice) -> HidPointerReady {
    HidPointerReady {
        interface_number: 0,
        endpoint_address: 0x81,
        endpoint_dci: 3,
        endpoint_interval: 7,
        report_buffers: POINTER_REPORT_BUFFERS as u8,
        transfer_trbs: POINTER_TRANSFER_TRBS as u8,
        dma_physical: 0x1000,
        attachment: session::PointerAttachment::new(device),
    }
}
