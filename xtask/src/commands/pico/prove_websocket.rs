//! Physical R1 proof for the attachment-dependent Pico WebSocket route.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use conduit_core::BootId;
use conduit_std_host::usb_cdc::{NativePathCdcCarrier, NativePathCdcLineReader};
use conduit_std_host::websocket::NativeWebSocketCarrier;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionMachine, SessionMessage, SessionRole,
};

use super::firmware::FirmwareIdentity;
use super::transcript::RuntimeTranscriptIdentity;
use super::PicoResult;

pub(super) fn verify(
    usb: &mut NativePathCdcCarrier,
    clue: &mut NativePathCdcLineReader,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    usb.send_raw_stream_frame(conduit_net::R1_WEBSOCKET_BASE_QUERY, Duration::from_secs(2))?;
    let mut raw = [0_u8; 2048];
    if usb.receive_raw_stream_frame(&mut raw, Duration::from_secs(3))?
        != conduit_net::R1_WEBSOCKET_BASE_READY
    {
        return Err("Pico returned an unexpected WebSocket Base readiness payload".into());
    }
    usb.send_raw_stream_frame(
        conduit_net::R1_WEBSOCKET_ENDPOINT_CLUE_READY,
        Duration::from_secs(2),
    )?;
    let endpoint_line = clue
        .read_line(Duration::from_secs(3))
        .map_err(|error| format!("timed out reading WebSocket endpoint Clue: {error}"))?;
    let address = verify_endpoint_clue(&endpoint_line, identity, runtime)?;
    let socket_address = SocketAddr::V4(SocketAddrV4::new(address, conduit_net::R1_WEBSOCKET_PORT));
    let url = format!("ws://{socket_address}/conduit");
    let mut websocket =
        NativeWebSocketCarrier::connect(socket_address, &url, conduit_net::R1_MAXIMUM_FRAME_BYTES)
            .map_err(|error| format!("failed to connect bounded WebSocket carrier: {error:?}"))?;

    let binding = conduit_net::r1_websocket_probe_binding(BootId::from(runtime.boot_id.as_str()));
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Source)
        .map_err(|error| format!("invalid WebSocket Session binding: {error:?}"))?;
    exchange(
        &mut websocket,
        &mut machine,
        binding.hello_frame(),
        |message| matches!(message, SessionMessage::Hello { .. }),
    )?;
    exchange(
        &mut websocket,
        &mut machine,
        binding.frame(SessionMessage::Ready),
        |message| matches!(message, SessionMessage::Ready),
    )?;
    if !machine.is_active() {
        return Err("WebSocket Session did not become active".into());
    }

    let link_line = clue
        .read_line(Duration::from_secs(3))
        .map_err(|error| format!("timed out reading WebSocket link Clue: {error}"))?;
    verify_link_clue(&link_line, identity, runtime, &binding)?;
    super::usb_continuity::verify(usb, identity)?;
    websocket
        .close()
        .map_err(|error| format!("failed to close WebSocket carrier: {error:?}"))?;
    println!("==> Physical WebSocket Session, exact route Clue, and simultaneous USB continuity verified");
    Ok(())
}

fn exchange(
    carrier: &mut NativeWebSocketCarrier,
    machine: &mut SessionMachine,
    outbound: conduit_wire::SessionFrame<'_>,
    expected: impl Fn(SessionMessage<'_>) -> bool,
) -> PicoResult<()> {
    let mut bytes = [0_u8; conduit_net::R1_MAXIMUM_FRAME_BYTES as usize];
    machine
        .admit_outbound(outbound)
        .map_err(|error| format!("WebSocket Session rejected outbound frame: {error:?}"))?;
    let length = encode_session_frame_into(
        outbound,
        &mut bytes,
        conduit_net::R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES,
        conduit_net::R1_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("failed encoding WebSocket Session frame: {error:?}"))?;
    carrier
        .send_binary(&bytes[..length])
        .map_err(|error| format!("failed sending WebSocket Session frame: {error:?}"))?;
    let length = carrier
        .receive_binary(&mut bytes)
        .map_err(|error| format!("failed receiving WebSocket Session frame: {error:?}"))?;
    let inbound = decode_session_frame(
        &bytes[..length],
        conduit_net::R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES,
        conduit_net::R1_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("failed decoding WebSocket Session frame: {error:?}"))?;
    if !expected(inbound.message) {
        return Err("Pico returned an unexpected WebSocket Session frame".into());
    }
    machine
        .admit_inbound(inbound)
        .map_err(|error| format!("WebSocket Session rejected inbound frame: {error:?}"))?;
    Ok(())
}

fn verify_endpoint_clue(
    line: &str,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<Ipv4Addr> {
    let record: serde_json::Value = serde_json::from_str(line)
        .map_err(|error| format!("malformed WebSocket endpoint Clue: {error}"))?;
    verify_fields(
        &record,
        &[
            ("schema", "conduit.network/websocket-endpoint-clue@1"),
            ("firmware_build_id", identity.firmware_build_id.as_str()),
            ("host_id", identity.generated_image.host_id.as_str()),
            ("runtime_boot_id", runtime.boot_id.as_str()),
            ("attachment_id", "r1/pico-network-attachment-1"),
            ("interface_pool_id", conduit_net::R1_WIFI_STATION_POOL_ID),
            (
                "base_instance_id",
                conduit_net::R1_WEBSOCKET_BASE_INSTANCE_ID,
            ),
            (
                "sink_endpoint_id",
                conduit_net::R1_PICO_WEBSOCKET_ENDPOINT_ID,
            ),
        ],
    )?;
    if record["port"].as_u64() != Some(u64::from(conduit_net::R1_WEBSOCKET_PORT))
        || record["maximum_frame_bytes"].as_u64()
            != Some(u64::from(conduit_net::R1_MAXIMUM_FRAME_BYTES))
    {
        return Err("WebSocket endpoint Clue bounds mismatched".into());
    }
    let octets = record["ipv4"]
        .as_array()
        .ok_or("WebSocket endpoint IPv4 is absent")?;
    if octets.len() != 4 {
        return Err("WebSocket endpoint IPv4 is malformed".into());
    }
    let mut address = [0_u8; 4];
    for (target, value) in address.iter_mut().zip(octets) {
        *target = u8::try_from(
            value
                .as_u64()
                .ok_or("WebSocket endpoint IPv4 is malformed")?,
        )?;
    }
    let address = Ipv4Addr::from(address);
    if address.is_unspecified() || address.is_loopback() || address.is_multicast() {
        return Err("WebSocket endpoint is not a usable LAN address".into());
    }
    Ok(address)
}

fn verify_link_clue(
    line: &str,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
    binding: &conduit_wire::SessionBinding,
) -> PicoResult<()> {
    let record: serde_json::Value = serde_json::from_str(line)
        .map_err(|error| format!("malformed WebSocket link Clue: {error}"))?;
    verify_fields(
        &record,
        &[
            ("schema", "conduit.network/websocket-link-clue@1"),
            ("firmware_build_id", identity.firmware_build_id.as_str()),
            ("host_id", identity.generated_image.host_id.as_str()),
            ("runtime_boot_id", runtime.boot_id.as_str()),
            (
                "websocket_active_play_id",
                binding.sink_active_play_id.as_str(),
            ),
            ("attachment_id", "r1/pico-network-attachment-1"),
            ("usb_link_binding_id", conduit_net::R1_USB_LINK_BINDING_ID),
            (
                "websocket_link_binding_id",
                conduit_net::R1_WEBSOCKET_LINK_BINDING_ID,
            ),
            (
                "base_instance_id",
                conduit_net::R1_WEBSOCKET_BASE_INSTANCE_ID,
            ),
            (
                "source_endpoint_id",
                conduit_net::R1_STD_WEBSOCKET_ENDPOINT_ID,
            ),
            (
                "sink_endpoint_id",
                conduit_net::R1_PICO_WEBSOCKET_ENDPOINT_ID,
            ),
            ("clue_id", conduit_net::R1_WEBSOCKET_ROUTE_CLUE_ID),
        ],
    )?;
    if record["handshake"].as_bool() != Some(true)
        || record["maximum_frame_bytes"].as_u64()
            != Some(u64::from(conduit_net::R1_MAXIMUM_FRAME_BYTES))
    {
        return Err("WebSocket link Clue handshake or bound mismatched".into());
    }
    Ok(())
}

fn verify_fields(record: &serde_json::Value, expected: &[(&str, &str)]) -> PicoResult<()> {
    for (field, value) in expected {
        if record[*field].as_str() != Some(*value) {
            return Err(format!("WebSocket Clue field `{field}` mismatched").into());
        }
    }
    Ok(())
}
