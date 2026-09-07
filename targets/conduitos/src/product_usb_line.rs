//! Ordinary-product publication of one exact current QEMU FTDI Line.

use alloc::{format, string::String};
use conduit_core::{BaseInstanceId, BootId, HostId, LineId, LinkBindingId, LinkEndpointId, SignId};

use crate::{
    arch::{FtdiLineReady, UsbDevice},
    identity::{self, BootIdentities},
    usb_line_offer::{
        AdmittedUsbLineBasis, UsbLineIdentity, UsbLineObservation, UsbLineRealization,
        UsbLineState, offer_usb_ftdi_line,
    },
};

pub const QEMU_PEER_HOST_ID: &str = "host/qemu-product-journey-ftdi-peer";
pub const QEMU_PEER_BOOT_ID: &str = "boot/qemu-product-journey-ftdi-peer/1";

pub fn current_offer_sign(
    identities: &BootIdentities,
    controller_id: [u8; 32],
    device: &UsbDevice,
    ready: FtdiLineReady,
) -> Result<String, &'static str> {
    let (line, observation) = current_line(identities, controller_id, device, ready)?;
    Ok(format!(
        "CONDUIT_USB_LINE_SIGN {{\"schema\":\"conduit.conduitos/usb-line@1\",\"status\":\"current\",\"proof_class\":\"freestanding-emulator\",\"line_id\":\"{}\",\"binding_id\":\"{}\",\"base_instance_id\":\"{}\",\"source_host_id\":\"{}\",\"source_boot_id\":\"{}\",\"source_endpoint_id\":\"{}\",\"sink_host_id\":\"{}\",\"sink_boot_id\":\"{}\",\"sink_endpoint_id\":\"{}\",\"state_sign_id\":\"{}\",\"controller_id\":\"{}\",\"device_id\":\"{}\",\"interface_id\":\"{}\",\"input_endpoint_id\":\"{}\",\"output_endpoint_id\":\"{}\",\"attachment_epoch\":{},\"maximum_in_flight_items\":{},\"maximum_payload_bytes\":{},\"maximum_buffered_bytes\":{},\"maximum_frame_bytes\":{}}}\n",
        line.line_id.as_str(),
        line.binding.binding_id.as_str(),
        line.binding.base_instance_id.as_str(),
        line.binding.source.host_id.as_str(),
        line.binding.source.boot_id.as_str(),
        line.binding.source.endpoint_id.as_str(),
        line.binding.sink.host_id.as_str(),
        line.binding.sink.boot_id.as_str(),
        line.binding.sink.endpoint_id.as_str(),
        line.availability.sign_id.as_str(),
        identity::hex(&observation.realization.controller_id),
        identity::hex(&observation.realization.device_id),
        identity::hex(&observation.realization.interface_id),
        identity::hex(&observation.realization.input_endpoint_id),
        identity::hex(&observation.realization.output_endpoint_id),
        observation.realization.attachment_epoch,
        line.binding.limits.maximum_in_flight_items,
        line.binding.limits.maximum_payload_bytes,
        line.binding.limits.maximum_buffered_bytes,
        line.binding.limits.maximum_frame_bytes,
    ))
}

pub fn lost_sign(
    identities: &BootIdentities,
    controller_id: [u8; 32],
    device: &UsbDevice,
    ready: FtdiLineReady,
) -> Result<String, &'static str> {
    let (line, mut observation) = current_line(identities, controller_id, device, ready)?;
    observation.state = UsbLineState::Lost;
    let lost_state = identity::derive_base(&identities.boot, "conduitos/usb-ftdi/lost/1");
    observation.state_sign_id = SignId::from(identity::hex(&lost_state));
    let admitted = AdmittedUsbLineBasis::from_offer(
        &line,
        &UsbLineObservation {
            state: UsbLineState::Current,
            state_sign_id: line.availability.sign_id.clone(),
            ..observation.clone()
        },
    )
    .map_err(|_| "usb-line-admission-invalid")?;
    if !matches!(
        admitted.validate_current(&observation),
        Err(crate::usb_line_offer::UsbLineOfferError::Lost)
    ) {
        return Err("usb-line-stale-current-not-refused");
    }
    Ok(format!(
        "CONDUIT_USB_LINE_SIGN {{\"schema\":\"conduit.conduitos/usb-line@1\",\"status\":\"lost\",\"proof_class\":\"freestanding-emulator\",\"line_id\":\"{}\",\"binding_id\":\"{}\",\"base_instance_id\":\"{}\",\"source_host_id\":\"{}\",\"source_boot_id\":\"{}\",\"sink_host_id\":\"{}\",\"sink_boot_id\":\"{}\",\"state_sign_id\":\"{}\",\"device_id\":\"{}\",\"attachment_epoch\":{},\"reason\":\"device-removed\",\"stale_current_refused\":true}}\n",
        line.line_id.as_str(),
        line.binding.binding_id.as_str(),
        line.binding.base_instance_id.as_str(),
        line.binding.source.host_id.as_str(),
        line.binding.source.boot_id.as_str(),
        line.binding.sink.host_id.as_str(),
        line.binding.sink.boot_id.as_str(),
        observation.state_sign_id.as_str(),
        identity::hex(&observation.realization.device_id),
        observation.realization.attachment_epoch,
    ))
}

fn current_line(
    identities: &BootIdentities,
    controller_id: [u8; 32],
    device: &UsbDevice,
    ready: FtdiLineReady,
) -> Result<(conduit_core::LineOffer, UsbLineObservation), &'static str> {
    let device_id = identity::derive_usb_device(
        &identities.boot,
        &controller_id,
        device.root_port,
        device.slot,
        device.attachment_epoch,
    );
    let interface_id = identity::derive_usb_interface(&device_id, ready.interface_number, 0);
    let input_endpoint_id =
        identity::derive_usb_endpoint(&interface_id, ready.input_endpoint_address);
    let output_endpoint_id =
        identity::derive_usb_endpoint(&interface_id, ready.output_endpoint_address);
    let base_instance = identity::derive_base(&identities.boot, "conduitos/usb-ftdi/0");
    let state_sign = identity::derive_base(&identities.boot, "conduitos/usb-ftdi/current/1");
    let observation = UsbLineObservation {
        realization: UsbLineRealization {
            controller_id,
            device_id,
            interface_id,
            input_endpoint_id,
            output_endpoint_id,
            attachment_epoch: device.attachment_epoch,
            input_dci: ready.input_dci,
            output_dci: ready.output_dci,
            packet_bytes: ready.packet_bytes,
            payload_bytes: ready.payload_bytes,
            transfer_trbs_per_direction: ready.transfer_trbs_per_direction,
        },
        base_instance_id: BaseInstanceId::from(identity::hex(&base_instance)),
        state: UsbLineState::Current,
        state_sign_id: SignId::from(identity::hex(&state_sign)),
    };
    let line_id = identity::derive_base(&identities.boot, "conduitos/usb-ftdi/line/1");
    let binding_id = identity::derive_base(&identities.boot, "conduitos/usb-ftdi/binding/1");
    let source_endpoint =
        identity::derive_base(&identities.boot, "conduitos/usb-ftdi/source-endpoint/1");
    let line = offer_usb_ftdi_line(
        UsbLineIdentity {
            line_id: LineId::from(identity::hex(&line_id)),
            binding_id: LinkBindingId::from(identity::hex(&binding_id)),
            base_instance_id: observation.base_instance_id.clone(),
            source_host_id: HostId::from(identity::hex(&identities.host)),
            source_boot_id: BootId::from(identity::hex(&identities.boot)),
            source_endpoint_id: LinkEndpointId::from(identity::hex(&source_endpoint)),
            sink_host_id: HostId::from(QEMU_PEER_HOST_ID),
            sink_boot_id: BootId::from(QEMU_PEER_BOOT_ID),
            sink_endpoint_id: LinkEndpointId::from("endpoint/qemu-product-journey-ftdi-peer"),
        },
        &observation,
    )
    .map_err(|_| "usb-line-offer-invalid")?;
    AdmittedUsbLineBasis::from_offer(&line, &observation)
        .map_err(|_| "usb-line-admission-invalid")?;
    Ok((line, observation))
}
