//! Ordinary product service for one exact pre-admission USB connectivity seam.

use alloc::{format, vec};
use conduit_core::{
    BaseInstanceId, BootId, ConnectionId, FragmentId, HostId, KindId, LineId, LinkBindingId,
    LinkEndpointId, PlacementId, PlannedConnection, PortId, PortTemporal, SignId, bind_active_play,
};
use conduit_wire::{SessionBinding, SessionMessage, SessionRole};

use crate::{
    arch::{FtdiLineReady, FtdiLineSession, UsbDevice, XhciReady, start_ftdi_line_session},
    identity::{self, BootIdentities},
    usb_line_offer::{
        AdmittedUsbLineBasis, UsbLineIdentity, UsbLineObservation, UsbLineRealization,
        UsbLineState, offer_usb_ftdi_line,
    },
    usb_line_session::{ReceivedSessionMessage, UsbLineSession, UsbLineSessionError},
};

pub const HARNESS_HOST_ID: &str = "host/qemu-usb-line-peer";
pub const HARNESS_BOOT_ID: &str = "boot/qemu-usb-line-peer/1";
pub const LINE_VALUE: &[u8] = b"HELLO USB LINE";

pub struct ProductUsbLine {
    observation: UsbLineObservation,
    basis: AdmittedUsbLineBasis,
    carrier: FtdiLineSession,
    session: UsbLineSession,
}

pub fn prepare(
    identities: &BootIdentities,
    controller_id: [u8; 32],
    device: &UsbDevice,
    ready: FtdiLineReady,
) -> Result<ProductUsbLine, &'static str> {
    let device_id = identity::derive_usb_device(
        &identities.boot,
        &controller_id,
        device.root_port,
        device.slot,
        device.attachment_epoch,
    );
    let interface_id = identity::derive_usb_interface(&device_id, ready.interface_number, 0);
    let realization = UsbLineRealization {
        controller_id,
        device_id,
        interface_id,
        input_endpoint_id: identity::derive_usb_endpoint(
            &interface_id,
            ready.input_endpoint_address,
        ),
        output_endpoint_id: identity::derive_usb_endpoint(
            &interface_id,
            ready.output_endpoint_address,
        ),
        attachment_epoch: device.attachment_epoch,
        input_dci: ready.input_dci,
        output_dci: ready.output_dci,
        packet_bytes: ready.packet_bytes,
        payload_bytes: ready.payload_bytes,
        transfer_trbs_per_direction: ready.transfer_trbs_per_direction,
    };
    let base = identity::derive_base(&identities.boot, "conduitos/qemu-usb-ftdi/0");
    let suffix = identity::hex(&base);
    let base_instance_id = BaseInstanceId::from(format!("base/usb-ftdi/{suffix}"));
    let observation = UsbLineObservation {
        realization,
        base_instance_id: base_instance_id.clone(),
        state: UsbLineState::Current,
        state_sign_id: SignId::from(format!("sign/usb-ftdi/{suffix}/current")),
    };
    let source_host = HostId::from(identity::hex(&identities.host));
    let source_boot = BootId::from(identity::hex(&identities.boot));
    let line_identity = UsbLineIdentity {
        line_id: LineId::from(format!("line/usb-ftdi/{suffix}")),
        binding_id: LinkBindingId::from(format!("binding/usb-ftdi/{suffix}")),
        base_instance_id,
        source_host_id: source_host.clone(),
        source_boot_id: source_boot.clone(),
        source_endpoint_id: LinkEndpointId::from(format!("endpoint/usb-ftdi/{suffix}/guest")),
        sink_host_id: HostId::from(HARNESS_HOST_ID),
        sink_boot_id: BootId::from(HARNESS_BOOT_ID),
        sink_endpoint_id: LinkEndpointId::from("endpoint/qemu-usb-line-peer/ftdi"),
    };
    let offer = offer_usb_ftdi_line(line_identity, &observation)
        .map_err(|_| "product-usb-line-offer-refused")?;
    let basis = AdmittedUsbLineBasis::from_offer(&offer, &observation)
        .map_err(|_| "product-usb-line-basis-refused")?;
    let line = offer.admitted_line();
    let connection = PlannedConnection {
        connection_id: ConnectionId::from("connection/usb-line/connectivity-seam"),
        source_placement_id: PlacementId::from("placement/conduitos/usb-line-source"),
        source_port_id: PortId::from("value"),
        sink_placement_id: PlacementId::from("placement/harness/usb-line-sink"),
        sink_port_id: PortId::from("value"),
        value_kind: KindId::from("info/text@1"),
        temporal: PortTemporal::Value,
        selected_line: Some(line.clone()),
        admitted_lines: vec![line],
        item_capacity: 1,
        byte_capacity: crate::usb_line_offer::USB_LINE_MAXIMUM_PAYLOAD_BYTES,
    };
    let plan_id = conduit_core::PlanId::from("plan/usb-line/pre-admission-connectivity-seam");
    let binding = SessionBinding::from_planned_connection(
        plan_id,
        FragmentId::from("fragment/conduitos/usb-line-source"),
        FragmentId::from("fragment/harness/usb-line-sink"),
        &connection,
    )
    .map_err(|_| "product-usb-line-session-binding-refused")?;
    debug_assert_eq!(
        binding.source_active_play_id,
        bind_active_play(&binding.plan_id, &source_host, &source_boot, 0).active_play_id
    );
    Ok(ProductUsbLine {
        observation,
        basis,
        carrier: start_ftdi_line_session(ready),
        session: UsbLineSession::new(binding, SessionRole::Source)
            .map_err(|_| "product-usb-line-session-refused")?,
    })
}

impl ProductUsbLine {
    pub fn run(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
        body_id: &str,
        mut manifest: impl FnMut(
            crate::front_door::ConnectivityStatus,
            &str,
            Option<&str>,
        ) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        manifest(
            crate::front_door::ConnectivityStatus::Current,
            self.session.binding().attachment.line_id.as_str(),
            None,
        )?;
        emit("usb-line-current", self, body_id, None);
        crate::arch::early_write(b"CONDUIT_BOOT_STAGE usb-line-current\n");
        let binding = self.session.binding().clone();
        self.send(controller, device, binding.hello_frame().message)?;
        self.expect(controller, device, ReceivedSessionMessage::Hello)?;
        self.send(controller, device, SessionMessage::Ready)?;
        self.expect(controller, device, ReceivedSessionMessage::Ready)?;
        manifest(
            crate::front_door::ConnectivityStatus::PeerAttached,
            self.session.binding().attachment.line_id.as_str(),
            None,
        )?;
        emit("peer-attached", self, body_id, None);
        crate::arch::early_write(b"CONDUIT_BOOT_STAGE peer-attached\n");
        self.send(
            controller,
            device,
            SessionMessage::Offered {
                sequence: 0,
                payload: LINE_VALUE,
            },
        )?;
        self.expect(controller, device, ReceivedSessionMessage::Accepted(0))?;
        self.expect(controller, device, ReceivedSessionMessage::Delivered(0))?;
        manifest(
            crate::front_door::ConnectivityStatus::ValueVisible,
            self.session.binding().attachment.line_id.as_str(),
            Some("HELLO USB LINE"),
        )?;
        emit("line-value-visible", self, body_id, Some("HELLO USB LINE"));
        crate::arch::early_write(b"CONDUIT_BOOT_STAGE line-value-visible\n");
        let loss = self.session.receive(
            &mut self.carrier,
            controller,
            device,
            &self.basis,
            &self.observation,
        );
        if !matches!(
            loss,
            Err(UsbLineSessionError::Carrier(
                crate::arch::FtdiLineError::DeviceRemoved
            ))
        ) {
            return Err("product-usb-line-removal-not-observed");
        }
        self.observation.state = UsbLineState::Lost;
        self.observation.state_sign_id = SignId::from("sign/usb-ftdi/lost");
        if !matches!(
            self.session.send(
                &mut self.carrier,
                controller,
                device,
                &self.basis,
                &self.observation,
                SessionMessage::Ready,
            ),
            Err(UsbLineSessionError::Current(
                crate::usb_line_offer::UsbLineOfferError::Lost
            ))
        ) {
            return Err("product-usb-line-stale-session-not-refused");
        }
        manifest(
            crate::front_door::ConnectivityStatus::Lost,
            self.session.binding().attachment.line_id.as_str(),
            None,
        )?;
        emit("line-lost", self, body_id, None);
        crate::arch::early_write(b"CONDUIT_BOOT_STAGE line-lost\n");
        Ok(())
    }

    fn send(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
        message: SessionMessage<'_>,
    ) -> Result<(), &'static str> {
        self.session
            .send(
                &mut self.carrier,
                controller,
                device,
                &self.basis,
                &self.observation,
                message,
            )
            .map_err(|_| "product-usb-line-send-refused")
    }

    fn expect(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
        expected: ReceivedSessionMessage,
    ) -> Result<(), &'static str> {
        let found = self
            .session
            .receive(
                &mut self.carrier,
                controller,
                device,
                &self.basis,
                &self.observation,
            )
            .map_err(|_| "product-usb-line-receive-refused")?;
        (found == expected)
            .then_some(())
            .ok_or("product-usb-line-message-unexpected")
    }
}

fn emit(status: &str, line: &ProductUsbLine, body_id: &str, value: Option<&str>) {
    let binding = line.session.binding();
    let value = value.map_or_else(|| "null".into(), |value| format!("\"{value}\""));
    crate::arch::early_write(
        format!(
            "CONDUIT_USB_LINE_SIGN {{\"schema\":\"conduit.conduitos.usb-line/v1\",\"status\":\"{status}\",\"line_id\":\"{}\",\"binding_id\":\"{}\",\"base_instance_id\":\"{}\",\"plan_id\":\"{}\",\"source_active_play_id\":\"{}\",\"sink_active_play_id\":\"{}\",\"source_host_id\":\"{}\",\"source_boot_id\":\"{}\",\"sink_host_id\":\"{}\",\"sink_boot_id\":\"{}\",\"body_id\":\"{body_id}\",\"value\":{value},\"proof_class\":\"freestanding-emulator\",\"membership\":\"not-requested\",\"bounded\":true}}\n",
            binding.attachment.line_id.as_str(),
            binding.attachment.link_binding_id.as_str(),
            binding.attachment.base_instance_id.as_str(),
            binding.plan_id.as_str(),
            binding.source_active_play_id.as_str(),
            binding.sink_active_play_id.as_str(),
            binding.source.host_id.as_str(),
            binding.source.boot_id.as_str(),
            binding.sink.host_id.as_str(),
            binding.sink.boot_id.as_str(),
        )
        .as_bytes(),
    );
}
