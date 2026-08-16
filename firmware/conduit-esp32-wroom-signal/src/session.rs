//! Exact Plan-scoped Conduit session above the ESP32 GATT mechanism.

use conduit_bluetooth::{
    BleGattProfile, BleReassembler, MAXIMUM_BLE_FRAME_BYTES, MAXIMUM_BLE_GATT_PACKET_BYTES,
    encode_fragment, fragment_count,
};
use conduit_core::{
    BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId, HostId, KindId,
    LinkBindingId, LinkEndpointId, PROTOCOL_VERSION, PlanId, bind_active_play,
};
use conduit_wire::{
    LineAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits, SessionMachine,
    SessionMessage, SessionRole, decode_session_frame, encode_session_frame_into,
};
use heapless::Vec;

const PAYLOAD_BYTES: u32 = 96;
const MAXIMUM_REPLY_PACKETS: usize = 24;
type ReplyPacket = Vec<u8, MAXIMUM_BLE_GATT_PACKET_BYTES>;
pub type ReplyPackets = Vec<ReplyPacket, MAXIMUM_REPLY_PACKETS>;

pub struct ConduitBleSession {
    binding: SessionBinding,
    machine: SessionMachine,
    reassembler: BleReassembler,
    send_sequence: u8,
}

impl ConduitBleSession {
    pub fn new(boot: &crate::receipts::BootIdentity) -> Result<Self, &'static str> {
        let binding = binding(boot);
        let machine = SessionMachine::new(binding.clone(), SessionRole::Sink)
            .map_err(|_| "invalid-binding")?;
        Ok(Self {
            binding,
            machine,
            reassembler: BleReassembler::new(BleGattProfile::FIRST),
            send_sequence: 0,
        })
    }

    pub fn next_sequence(&self) -> u64 {
        self.machine.next_sequence()
    }

    pub fn admit_packet(&mut self, packet: &[u8]) -> Result<ReplyPackets, &'static str> {
        let completed = self
            .reassembler
            .admit(packet)
            .map_err(|_| "invalid-fragment")?;
        let Some(bytes) = completed else {
            return Ok(ReplyPackets::new());
        };
        let frame = decode_session_frame(bytes, PAYLOAD_BYTES, MAXIMUM_BLE_FRAME_BYTES as u32)
            .map_err(|_| "invalid-session-frame")?;
        let message = frame.message;
        self.machine
            .admit_inbound(frame)
            .map_err(|_| "session-admission")?;

        let mut replies = ReplyPackets::new();
        let binding = self.binding.clone();
        match message {
            SessionMessage::Hello(_) => self.push_frame(binding.hello_frame(), &mut replies)?,
            SessionMessage::Ready => {
                self.push_frame(binding.frame(SessionMessage::Ready), &mut replies)?;
            }
            SessionMessage::Offered { sequence, .. } => {
                self.push_frame(
                    binding.frame(SessionMessage::Accepted { sequence }),
                    &mut replies,
                )?;
                self.push_frame(
                    binding.frame(SessionMessage::Delivered { sequence }),
                    &mut replies,
                )?;
            }
            _ => return Err("unexpected-source-message"),
        }
        Ok(replies)
    }

    fn push_frame(
        &mut self,
        frame: conduit_wire::SessionFrame<'_>,
        packets: &mut ReplyPackets,
    ) -> Result<(), &'static str> {
        self.machine
            .admit_outbound(frame)
            .map_err(|_| "outbound-session-admission")?;
        let mut encoded = [0_u8; MAXIMUM_BLE_FRAME_BYTES];
        let length = encode_session_frame_into(
            frame,
            &mut encoded,
            PAYLOAD_BYTES,
            MAXIMUM_BLE_FRAME_BYTES as u32,
        )
        .map_err(|_| "session-encode")?;
        let count = fragment_count(length, BleGattProfile::FIRST).map_err(|_| "fragment-count")?;
        if packets.len().saturating_add(usize::from(count)) > MAXIMUM_REPLY_PACKETS {
            return Err("reply-pressure");
        }
        for index in 0..count {
            let mut packet = [0_u8; MAXIMUM_BLE_GATT_PACKET_BYTES];
            let packet_length = encode_fragment(
                &encoded[..length],
                self.send_sequence,
                index,
                BleGattProfile::FIRST,
                &mut packet,
            )
            .map_err(|_| "fragment-encode")?;
            let mut reply = ReplyPacket::new();
            reply
                .extend_from_slice(&packet[..packet_length])
                .map_err(|_| "reply-packet-pressure")?;
            packets.push(reply).map_err(|_| "reply-pressure")?;
        }
        self.send_sequence = self.send_sequence.wrapping_add(1);
        Ok(())
    }
}

fn binding(boot: &crate::receipts::BootIdentity) -> SessionBinding {
    let plan_id = PlanId::from("bluetooth/physical-capstone-plan");
    let source_host = HostId::from("bluetooth/source-host");
    let source_boot = BootId::from("bluetooth/source-boot");
    let sink_host = HostId::from(boot.host_id());
    let sink_boot = BootId::from(boot.boot_id());
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        source_active_play_id: bind_active_play(&plan_id, &source_host, &source_boot, 0)
            .active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &sink_host, &sink_boot, 0).active_play_id,
        plan_id,
        source_fragment_id: FragmentId::from("bluetooth/source-fragment"),
        sink_fragment_id: FragmentId::from("bluetooth/sink-fragment"),
        connection_id: ConnectionId::from("bluetooth/unchanged-signal-cord"),
        source: SessionEndpointIdentity {
            host_id: source_host.clone(),
            boot_id: source_boot.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink_host.clone(),
            boot_id: sink_boot.clone(),
        },
        value_kind: KindId::from("conduit.signal/level@1"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: PAYLOAD_BYTES,
            maximum_buffered_bytes: PAYLOAD_BYTES,
        },
        attachment: LineAttachment {
            line_id: "bluetooth/physical-line".into(),
            link_binding_id: LinkBindingId::from("bluetooth/physical-binding"),
            base: ConnectionBase::BluetoothLeGatt,
            base_instance_id: ConnectionBaseInstanceId::from("bluetooth/physical-session"),
            source_host_id: source_host,
            source_boot_id: source_boot,
            source_endpoint_id: LinkEndpointId::from("bluetooth/source-write"),
            sink_host_id: sink_host,
            sink_boot_id: sink_boot,
            sink_endpoint_id: LinkEndpointId::from("bluetooth/sink-indicate"),
            limits: BleGattProfile::FIRST
                .link_limits()
                .expect("the frozen BLE profile remains valid"),
        },
    }
}
