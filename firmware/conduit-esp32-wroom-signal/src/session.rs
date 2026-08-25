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
    SessionMessage, SessionRole, SessionTerminalDisposition, decode_session_frame,
    encode_session_frame_into,
};
use heapless::Vec;

const PAYLOAD_BYTES: u32 = 96;
const MAXIMUM_REPLY_PACKETS: usize = 8;
type ReplyPacket = Vec<u8, MAXIMUM_BLE_GATT_PACKET_BYTES>;
pub type ReplyPackets = Vec<ReplyPacket, MAXIMUM_REPLY_PACKETS>;

pub struct ConduitBleSession<'kernel> {
    binding: SessionBinding,
    machine: SessionMachine,
    reassembler: BleReassembler,
    send_sequence: u8,
    kernel: &'kernel mut crate::remote_kernel::Esp32RemoteSignalKernel,
    kernel_complete: bool,
}

impl<'kernel> ConduitBleSession<'kernel> {
    pub fn new(
        boot: &crate::receipts::BootIdentity,
        kernel: &'kernel mut crate::remote_kernel::Esp32RemoteSignalKernel,
    ) -> Result<Self, &'static str> {
        let binding = binding(boot)?;
        let machine = SessionMachine::new(binding.clone(), SessionRole::Sink)
            .map_err(|_| "invalid-binding")?;
        Ok(Self {
            binding,
            machine,
            reassembler: BleReassembler::new(BleGattProfile::FIRST),
            send_sequence: 0,
            kernel,
            kernel_complete: false,
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
            SessionMessage::Offered { sequence, payload } => {
                match self.kernel.admit(sequence, payload)? {
                    conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. } => {}
                    conduit_kernel::scheduler::RemoteIngressOutcome::Full { .. } => {
                        self.push_frame(
                            binding.frame(SessionMessage::Pressure { sequence }),
                            &mut replies,
                        )?;
                        return Ok(replies);
                    }
                }
                self.push_frame(
                    binding.frame(SessionMessage::Accepted { sequence }),
                    &mut replies,
                )?;
                self.kernel.present_accepted(sequence)?;
                self.push_frame(
                    binding.frame(SessionMessage::Delivered { sequence }),
                    &mut replies,
                )?;
            }
            SessionMessage::InputClosed { final_sequence } => {
                self.kernel.close_and_complete(final_sequence)?;
                self.kernel_complete = true;
            }
            SessionMessage::Terminal {
                disposition,
                final_sequence,
            } => {
                if disposition != SessionTerminalDisposition::Completed || !self.kernel_complete {
                    return Err("unexpected-source-terminal");
                }
                self.push_frame(
                    binding.frame(SessionMessage::Terminal {
                        disposition,
                        final_sequence,
                    }),
                    &mut replies,
                )?;
                esp_println::println!(
                    "CONDUIT_ESP32_LINE_COMPLETE final-sequence={}",
                    final_sequence
                );
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

fn binding(boot: &crate::receipts::BootIdentity) -> Result<SessionBinding, &'static str> {
    let source_host = HostId::from(crate::generated::GENERATED_REMOTE_ENDPOINT_PEER_HOSTS[0]);
    let source_boot = BootId::from(crate::generated::GENERATED_REMOTE_ENDPOINT_PEER_BOOTS[0]);
    let sink_host = HostId::from(crate::generated::HOST_ID);
    let sink_boot = BootId::from(crate::generated::BOOT_ID);
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        source_active_play_id: bind_active_play(
            &PlanId::from(crate::generated::PLAN_ID),
            &source_host,
            &source_boot,
            0,
        )
        .active_play_id,
        sink_active_play_id: bind_active_play(
            &PlanId::from(crate::generated::PLAN_ID),
            &sink_host,
            &sink_boot,
            0,
        )
        .active_play_id,
        plan_id: PlanId::from(crate::generated::PLAN_ID),
        source_fragment_id: FragmentId::from(
            crate::generated::GENERATED_REMOTE_ENDPOINT_SOURCE_FRAGMENT_IDS[0],
        ),
        sink_fragment_id: FragmentId::from(
            crate::generated::GENERATED_REMOTE_ENDPOINT_SINK_FRAGMENT_IDS[0],
        ),
        connection_id: ConnectionId::from(
            crate::generated::GENERATED_REMOTE_ENDPOINT_CONNECTION_IDS[0],
        ),
        source: SessionEndpointIdentity {
            host_id: source_host.clone(),
            boot_id: source_boot.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink_host.clone(),
            boot_id: sink_boot.clone(),
        },
        value_kind: KindId::from(crate::generated::GENERATED_REMOTE_ENDPOINT_VALUE_KINDS[0]),
        limits: SessionLimits {
            maximum_in_flight_items:
                crate::generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_IN_FLIGHT_ITEMS[0],
            maximum_payload_bytes: crate::generated::CORD_VALUE_BYTES,
            maximum_buffered_bytes: crate::generated::CORD_VALUE_BYTES,
        },
        attachment: LineAttachment {
            line_id: crate::generated::GENERATED_REMOTE_ENDPOINT_LINE_IDS[0].into(),
            link_binding_id: LinkBindingId::from(
                crate::generated::GENERATED_REMOTE_ENDPOINT_LINK_BINDING_IDS[0],
            ),
            base: ConnectionBase::BluetoothLeGatt,
            base_instance_id: ConnectionBaseInstanceId::from(
                crate::generated::GENERATED_REMOTE_ENDPOINT_BASE_INSTANCE_IDS[0],
            ),
            source_host_id: source_host,
            source_boot_id: source_boot.clone(),
            source_endpoint_id: LinkEndpointId::from(
                crate::generated::GENERATED_REMOTE_ENDPOINT_PEER_ENDPOINTS[0],
            ),
            sink_host_id: sink_host,
            sink_boot_id: sink_boot,
            sink_endpoint_id: LinkEndpointId::from(
                crate::generated::GENERATED_REMOTE_ENDPOINT_LOCAL_ENDPOINTS[0],
            ),
            limits: BleGattProfile::FIRST
                .link_limits()
                .expect("the frozen BLE profile remains valid"),
        },
    }
    .with_observed_boots(source_boot, BootId::from(boot.boot_id()))
    .map_err(|_| "runtime-binding")
}
