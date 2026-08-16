use std::str::FromStr;

use conduit_bluetooth::BleGattProfile;
use conduit_core::{
    bind_active_play, BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId,
    HostId, KindId, LinkBindingId, LinkEndpointId, PlanId, PROTOCOL_VERSION,
};
use conduit_std_host::bluetooth_gatt::{
    discover_ble_gatt_candidate, BluezBleGattLine, BluezBleGattListener,
};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, LineAttachment, SessionBinding,
    SessionEndpointIdentity, SessionLimits, SessionMachine, SessionMessage, SessionRole,
};

const FRAME_BYTES: usize = 2_048;
const PAYLOAD_BYTES: u32 = 96;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args().collect::<Vec<_>>();
    if !matches!(arguments.len(), 4 | 6) || !matches!(arguments[1].as_str(), "source" | "sink") {
        return Err("usage: bluetooth-line-probe <source|sink> <adapter> <peer-address> [peer-host-id peer-boot-id]".into());
    }
    let address = bluer::Address::from_str(&arguments[3])
        .map_err(|_| "peer address must be six colon-separated hexadecimal bytes")?
        .0;
    let binding = binding(
        arguments.get(4).map(String::as_str),
        arguments.get(5).map(String::as_str),
    )?;
    match arguments[1].as_str() {
        "source" => {
            let candidate = discover_ble_gatt_candidate(&arguments[2], address)
                .await
                .map_err(|error| format!("discover: {error:?}"))?;
            if !candidate.paired {
                return Err("discovered compatible candidate is not paired".into());
            }
            let mut line = BluezBleGattLine::connect(&arguments[2], address, BleGattProfile::FIRST)
                .await
                .map_err(|error| format!("connect: {error:?}"))?;
            source(&binding, &mut line).await?;
        }
        "sink" => {
            let mut listener = BluezBleGattListener::bind(&arguments[2], BleGattProfile::FIRST)
                .await
                .map_err(|error| format!("bind: {error:?}"))?;
            let mut line = listener
                .accept(address)
                .await
                .map_err(|error| format!("accept: {error:?}"))?;
            sink(&binding, &mut line).await?;
        }
        _ => unreachable!(),
    }
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "success": true,
            "role": arguments[1],
            "adapter": arguments[2],
            "peer_address": bluer::Address(address).to_string(),
            "base": "bluetooth-le-gatt",
            "plan_id": binding.plan_id.as_str(),
            "connection_id": binding.connection_id.as_str(),
            "source_host_id": binding.source.host_id.as_str(),
            "source_boot_id": binding.source.boot_id.as_str(),
            "sink_host_id": binding.sink.host_id.as_str(),
            "sink_boot_id": binding.sink.boot_id.as_str(),
            "base_instance_id": binding.attachment.base_instance_id.as_str(),
            "maximum_frame_bytes": FRAME_BYTES,
        }))?
    );
    Ok(())
}

async fn source(
    binding: &SessionBinding,
    line: &mut BluezBleGattLine,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Source)
        .map_err(|error| format!("source session: {error:?}"))?;
    send(&mut machine, line, binding.hello_frame()).await?;
    receive(&mut machine, line).await?;
    send(&mut machine, line, binding.frame(SessionMessage::Ready)).await?;
    receive(&mut machine, line).await?;
    if !machine.is_active() {
        return Err("source session did not become active".into());
    }
    send(
        &mut machine,
        line,
        binding.frame(SessionMessage::Offered {
            sequence: 0,
            payload: b"signal-0",
        }),
    )
    .await?;
    receive(&mut machine, line).await?;
    receive(&mut machine, line).await?;
    if machine.next_sequence() != 1 {
        return Err("source did not observe exact delivery".into());
    }
    Ok(())
}

async fn sink(
    binding: &SessionBinding,
    line: &mut BluezBleGattLine,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink)
        .map_err(|error| format!("sink session: {error:?}"))?;
    receive(&mut machine, line).await?;
    send(&mut machine, line, binding.hello_frame()).await?;
    receive(&mut machine, line).await?;
    send(&mut machine, line, binding.frame(SessionMessage::Ready)).await?;
    if !machine.is_active() {
        return Err("sink session did not become active".into());
    }
    receive(&mut machine, line).await?;
    send(
        &mut machine,
        line,
        binding.frame(SessionMessage::Accepted { sequence: 0 }),
    )
    .await?;
    send(
        &mut machine,
        line,
        binding.frame(SessionMessage::Delivered { sequence: 0 }),
    )
    .await?;
    Ok(())
}

async fn send(
    machine: &mut SessionMachine,
    line: &mut BluezBleGattLine,
    frame: conduit_wire::SessionFrame<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    machine
        .admit_outbound(frame)
        .map_err(|error| format!("outbound session refusal: {error:?}"))?;
    let mut bytes = [0_u8; FRAME_BYTES];
    let length = encode_session_frame_into(frame, &mut bytes, PAYLOAD_BYTES, FRAME_BYTES as u32)
        .map_err(|error| format!("encode: {error:?}"))?;
    line.send_frame(&bytes[..length])
        .await
        .map_err(|error| format!("send: {error:?}"))?;
    Ok(())
}

async fn receive(
    machine: &mut SessionMachine,
    line: &mut BluezBleGattLine,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; FRAME_BYTES];
    let length = line
        .receive_frame(&mut bytes)
        .await
        .map_err(|error| format!("receive: {error:?}"))?;
    let frame = decode_session_frame(&bytes[..length], PAYLOAD_BYTES, FRAME_BYTES as u32)
        .map_err(|error| format!("decode: {error:?}"))?;
    machine
        .admit_inbound(frame)
        .map_err(|error| format!("inbound session refusal: {error:?}"))?;
    Ok(())
}

fn binding(
    peer_host_id: Option<&str>,
    peer_boot_id: Option<&str>,
) -> Result<SessionBinding, Box<dyn std::error::Error>> {
    if peer_host_id.is_some() != peer_boot_id.is_some() {
        return Err("peer HostId and BootId must be supplied together".into());
    }
    let plan_id = PlanId::from("bluetooth/physical-capstone-plan");
    let source_host = HostId::from("bluetooth/source-host");
    let source_boot = BootId::from("bluetooth/source-boot");
    let sink_host = HostId::from(peer_host_id.unwrap_or("bluetooth/sink-host"));
    let sink_boot = BootId::from(peer_boot_id.unwrap_or("bluetooth/sink-boot"));
    Ok(SessionBinding {
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
            limits: BleGattProfile::FIRST.link_limits().unwrap(),
        },
    })
}
