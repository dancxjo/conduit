//! Finite no_std USB descriptor model and parser.

use super::UsbError;

pub(super) const MAX_CONFIGURATION_BYTES: usize = 256;
pub(super) const MAX_INTERFACES: usize = 4;
pub(super) const MAX_ENDPOINTS: usize = 8;
pub(super) const MAX_DESCRIPTOR_RECORDS: usize = 16;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UsbEndpoint {
    pub interface_index: u8,
    pub address: u8,
    pub direction_in: bool,
    pub transfer_type: u8,
    pub maximum_packet_size: u16,
    pub interval: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UsbInterface {
    pub number: u8,
    pub alternate_setting: u8,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub first_endpoint: u8,
    pub endpoint_count: u8,
}

#[derive(Debug, Eq, PartialEq)]
pub struct UsbDevice {
    pub root_port: u8,
    pub slot: u8,
    pub address: u8,
    pub attachment_epoch: u32,
    pub usb_version: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub ep0_maximum_packet_size: u16,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_version: u16,
    pub configuration_value: u8,
    pub configuration_bytes: u16,
    pub descriptor_records: u8,
    pub interface_count: u8,
    pub endpoint_count: u8,
    pub interfaces: [UsbInterface; MAX_INTERFACES],
    pub endpoints: [UsbEndpoint; MAX_ENDPOINTS],
    pub control_transfers: u8,
    pub short_packets: u8,
    pub outstanding_control_transfer_limit: u8,
    pub enumeration_retries: u8,
    pub sign_slots: u8,
    pub configuration_limit_bytes: u16,
    pub interface_limit: u8,
    pub endpoint_limit: u8,
    pub descriptor_record_limit: u8,
    pub transfer_trbs: u8,
    pub dma_bytes: u16,
    pub dma_alignment: u16,
    pub port_poll_steps: u32,
}

pub(super) fn validate_header(bytes: &[u8], length: usize, kind: u8) -> Result<(), UsbError> {
    if bytes.len() < length || usize::from(bytes[0]) != length || bytes[1] != kind {
        Err(UsbError::MalformedDescriptor)
    } else {
        Ok(())
    }
}

pub(super) fn device_from_descriptor(
    port: u8,
    slot: u8,
    address: u8,
    ep0: u16,
    bytes: &[u8],
) -> Result<UsbDevice, UsbError> {
    Ok(UsbDevice {
        root_port: port,
        slot,
        address,
        attachment_epoch: 1,
        usb_version: u16::from_le_bytes([bytes[2], bytes[3]]),
        device_class: bytes[4],
        device_subclass: bytes[5],
        device_protocol: bytes[6],
        ep0_maximum_packet_size: ep0,
        vendor_id: u16::from_le_bytes([bytes[8], bytes[9]]),
        product_id: u16::from_le_bytes([bytes[10], bytes[11]]),
        device_version: u16::from_le_bytes([bytes[12], bytes[13]]),
        configuration_value: 0,
        configuration_bytes: 0,
        descriptor_records: 0,
        interface_count: 0,
        endpoint_count: 0,
        interfaces: [UsbInterface::default(); MAX_INTERFACES],
        endpoints: [UsbEndpoint::default(); MAX_ENDPOINTS],
        control_transfers: 0,
        short_packets: 0,
        outstanding_control_transfer_limit: 0,
        enumeration_retries: 0,
        sign_slots: 0,
        configuration_limit_bytes: MAX_CONFIGURATION_BYTES as u16,
        interface_limit: MAX_INTERFACES as u8,
        endpoint_limit: MAX_ENDPOINTS as u8,
        descriptor_record_limit: MAX_DESCRIPTOR_RECORDS as u8,
        transfer_trbs: 0,
        dma_bytes: 0,
        dma_alignment: 0,
        port_poll_steps: 0,
    })
}

pub(super) fn parse_configuration(bytes: &[u8], device: &mut UsbDevice) -> Result<(), UsbError> {
    validate_header(bytes, 9, 2)?;
    let total = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
    if total > MAX_CONFIGURATION_BYTES {
        return Err(UsbError::OversizedConfiguration);
    }
    if total != bytes.len() || bytes[4] == 0 {
        return Err(UsbError::MalformedDescriptor);
    }
    device.configuration_value = bytes[5];
    device.configuration_bytes = total as u16;
    let mut offset = 0;
    let mut current_interface = None;
    while offset < total {
        if offset + 2 > total {
            return Err(UsbError::MalformedDescriptor);
        }
        let length = usize::from(bytes[offset]);
        let kind = bytes[offset + 1];
        if length < 2 || offset + length > total {
            return Err(UsbError::MalformedDescriptor);
        }
        device.descriptor_records = device
            .descriptor_records
            .checked_add(1)
            .ok_or(UsbError::TooManyDescriptorRecords)?;
        if usize::from(device.descriptor_records) > MAX_DESCRIPTOR_RECORDS {
            return Err(UsbError::TooManyDescriptorRecords);
        }
        match kind {
            4 => {
                if length < 9 {
                    return Err(UsbError::MalformedDescriptor);
                }
                let index = usize::from(device.interface_count);
                if index == MAX_INTERFACES {
                    return Err(UsbError::TooManyInterfaces);
                }
                device.interfaces[index] = UsbInterface {
                    number: bytes[offset + 2],
                    alternate_setting: bytes[offset + 3],
                    class: bytes[offset + 5],
                    subclass: bytes[offset + 6],
                    protocol: bytes[offset + 7],
                    first_endpoint: device.endpoint_count,
                    endpoint_count: 0,
                };
                device.interface_count += 1;
                current_interface = Some(index);
            }
            5 => {
                if length < 7 {
                    return Err(UsbError::MalformedDescriptor);
                }
                let interface = current_interface.ok_or(UsbError::UnsupportedTopology)?;
                let endpoint = usize::from(device.endpoint_count);
                if endpoint == MAX_ENDPOINTS {
                    return Err(UsbError::TooManyEndpoints);
                }
                let address = bytes[offset + 2];
                device.endpoints[endpoint] = UsbEndpoint {
                    interface_index: interface as u8,
                    address,
                    direction_in: address & 0x80 != 0,
                    transfer_type: bytes[offset + 3] & 3,
                    maximum_packet_size: u16::from_le_bytes([bytes[offset + 4], bytes[offset + 5]])
                        & 0x7ff,
                    interval: bytes[offset + 6],
                };
                device.endpoint_count += 1;
                device.interfaces[interface].endpoint_count += 1;
            }
            _ => {}
        }
        offset += length;
    }
    if usize::from(device.interface_count) != usize::from(bytes[4]) || device.endpoint_count == 0 {
        return Err(UsbError::UnsupportedTopology);
    }
    Ok(())
}
