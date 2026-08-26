use std::str::FromStr;
use std::time::Duration;

use conduit_bluetooth::BleGattProfile;
use conduit_core::{BootId, HostId};
use conduit_signal::{exact_std_esp32_bluetooth_plan_for_host, exact_std_pico_bluetooth_plan};
use conduit_std_host::bluetooth_gatt::{
    disconnect_ble_gatt_candidate, discover_ble_gatt_candidate, discover_one_ble_gatt_candidate,
    pair_ble_gatt_candidate, BluezBleGattLine, BluezBleGattListener,
};
use conduit_std_host::pico_usb_source::PicoUsbSource;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition,
};

const FRAME_BYTES: usize = 2_048;
const PAYLOAD_BYTES: u32 = 96;
const SESSION_IO_TIMEOUT: Duration = Duration::from_secs(15);
const LISTENER_ACCEPT_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.len() == 3 && arguments[1] == "prepare" {
        let observed = discover_one_ble_gatt_candidate(&arguments[2])
            .await
            .map_err(|error| format!("discover: {error:?}"))?;
        let paired = pair_ble_gatt_candidate(&arguments[2], observed.address)
            .await
            .map_err(|error| format!("pair: {error:?}"))?;
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "success": true,
                "operation": "prepare",
                "adapter": arguments[2],
                "peer_address": bluer::Address(paired.address).to_string(),
                "paired": paired.paired,
            }))?
        );
        return Ok(());
    }
    let role_accepts_binding = matches!(arguments[1].as_str(), "source" | "sink" | "loss");
    if !matches!(arguments.len(), 4 | 6)
        || !matches!(arguments[1].as_str(), "source" | "sink" | "loss")
        || (arguments.len() == 6 && !role_accepts_binding)
    {
        return Err(
            "usage: bluetooth-line-probe prepare <adapter> | <source|sink> <adapter> <peer-address> [peer-host-id peer-boot-id] | loss <adapter> <peer-address>"
                .into(),
        );
    }
    let address = if arguments[3] == "auto" {
        discover_one_ble_gatt_candidate(&arguments[2])
            .await
            .map_err(|error| format!("discover: {error:?}"))?
            .address
    } else {
        bluer::Address::from_str(&arguments[3])
            .map_err(|_| "peer address must be six colon-separated hexadecimal bytes")?
            .0
    };
    let binding = binding(
        arguments.get(4).map(String::as_str),
        arguments.get(5).map(String::as_str),
    )?;
    if arguments[1] == "loss" {
        let candidate = discover_ble_gatt_candidate(&arguments[2], address)
            .await
            .map_err(|error| format!("discover: {error:?}"))?;
        let line = BluezBleGattLine::pair_and_connect(
            &arguments[2],
            candidate.address,
            BleGattProfile::FIRST,
        )
        .await
        .map_err(|error| format!("connect: {error:?}"))?;
        disconnect_ble_gatt_candidate(&arguments[2], address)
            .await
            .map_err(|error| format!("disconnect: {error:?}"))?;
        drop(line);
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "success": true,
                "role": "loss",
                "adapter": arguments[2],
                "peer_address": bluer::Address(address).to_string(),
                "base": "bluetooth-le-gatt",
                "plan_id": binding.plan_id.as_str(),
                "connection_id": binding.connection_id.as_str(),
                "base_instance_id": binding.attachment.base_instance_id.as_str(),
                "disposition": "transport-lost",
                "paired": true,
            }))?
        );
        return Ok(());
    }
    match arguments[1].as_str() {
        "source" => {
            let candidate = discover_ble_gatt_candidate(&arguments[2], address)
                .await
                .map_err(|error| format!("discover: {error:?}"))?;
            let mut line = BluezBleGattLine::pair_and_connect(
                &arguments[2],
                candidate.address,
                BleGattProfile::FIRST,
            )
            .await
            .map_err(|error| format!("connect: {error:?}"))?;
            source(&binding, &mut line).await?;
        }
        "sink" => {
            let mut listener = BluezBleGattListener::bind(&arguments[2], BleGattProfile::FIRST)
                .await
                .map_err(|error| format!("bind: {error:?}"))?;
            let mut line = tokio::time::timeout(LISTENER_ACCEPT_TIMEOUT, listener.accept(address))
                .await
                .map_err(|_| "accept: timed out waiting for the exact planned peer")?
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
            "paired": true,
        }))?
    );
    Ok(())
}

async fn source(
    binding: &SessionBinding,
    line: &mut BluezBleGattLine,
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = if binding.sink.host_id.as_str().starts_with("esp32/") {
        exact_std_esp32_bluetooth_plan_for_host(line.address(), binding.sink.host_id.as_str())?.plan
    } else {
        exact_std_pico_bluetooth_plan(line.address())?.plan
    };
    let source_host = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == binding.source.host_id)
        .ok_or("Bluetooth Plan lacks source fragment")?
        .host_id
        .clone();
    let mut source = PicoUsbSource::prepare_plan_with_observed_boots(
        plan,
        &HostId::from(source_host.as_str()),
        Some((binding.source.boot_id.clone(), binding.sink.boot_id.clone())),
    )?;
    if source.binding() != binding {
        return Err("kernel source binding disagrees with canonical Bluetooth binding".into());
    }
    send_source(&mut source, line, binding.hello_frame()).await?;
    receive_source(&mut source, line).await?;
    send_source(&mut source, line, binding.frame(SessionMessage::Ready)).await?;
    receive_source(&mut source, line).await?;
    if !source.is_active() {
        return Err("source session did not become active".into());
    }
    while let Some((sequence, payload)) = source.next_offer()? {
        loop {
            send_source(
                &mut source,
                line,
                binding.frame(SessionMessage::Offered {
                    sequence,
                    payload: &payload,
                }),
            )
            .await?;
            match receive_source(&mut source, line).await? {
                InboundFact::Pressure(observed) if observed == sequence => {
                    source.pressure(sequence)?;
                }
                InboundFact::Accepted(observed) if observed == sequence => {
                    source.accepted(sequence)?;
                    break;
                }
                fact => return Err(format!("unexpected source acknowledgement: {fact:?}").into()),
            }
        }
        match receive_source(&mut source, line).await? {
            InboundFact::Delivered(observed) if observed == sequence => {
                source.delivered(sequence)?;
            }
            fact => return Err(format!("unexpected delivery fact: {fact:?}").into()),
        }
    }
    let final_sequence = source.finish_kernel()?;
    send_source(
        &mut source,
        line,
        binding.frame(SessionMessage::InputClosed { final_sequence }),
    )
    .await?;
    send_source(
        &mut source,
        line,
        binding.frame(SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence,
        }),
    )
    .await?;
    receive_source(&mut source, line).await?;
    if !source.is_terminal() {
        return Err("source session did not reach exact terminal agreement".into());
    }
    Ok(())
}

#[derive(Debug)]
enum InboundFact {
    Hello,
    Ready,
    Pressure(u64),
    Accepted(u64),
    Delivered(u64),
    Terminal,
    Unexpected,
}

async fn send_source(
    source: &mut PicoUsbSource,
    line: &mut BluezBleGattLine,
    frame: conduit_wire::SessionFrame<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    source.admit_outbound(frame)?;
    send_encoded(line, frame).await
}

async fn receive_source(
    source: &mut PicoUsbSource,
    line: &mut BluezBleGattLine,
) -> Result<InboundFact, Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; FRAME_BYTES];
    let length = tokio::time::timeout(SESSION_IO_TIMEOUT, line.receive_frame(&mut bytes))
        .await
        .map_err(|_| "receive: timed out waiting for the exact session frame")?
        .map_err(|error| format!("receive: {error:?}"))?;
    let frame = decode_session_frame(&bytes[..length], PAYLOAD_BYTES, FRAME_BYTES as u32)
        .map_err(|error| format!("decode: {error:?}"))?;
    source.admit_inbound(frame)?;
    Ok(match frame.message {
        SessionMessage::Hello(_) => InboundFact::Hello,
        SessionMessage::Ready => InboundFact::Ready,
        SessionMessage::Pressure { sequence } => InboundFact::Pressure(sequence),
        SessionMessage::Accepted { sequence } => InboundFact::Accepted(sequence),
        SessionMessage::Delivered { sequence } => InboundFact::Delivered(sequence),
        SessionMessage::Terminal { .. } => InboundFact::Terminal,
        _ => InboundFact::Unexpected,
    })
}

async fn send_encoded(
    line: &mut BluezBleGattLine,
    frame: conduit_wire::SessionFrame<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; FRAME_BYTES];
    let length = encode_session_frame_into(frame, &mut bytes, PAYLOAD_BYTES, FRAME_BYTES as u32)
        .map_err(|error| format!("encode: {error:?}"))?;
    tokio::time::timeout(SESSION_IO_TIMEOUT, line.send_frame(&bytes[..length]))
        .await
        .map_err(|_| "send: timed out writing the exact session frame")?
        .map_err(|error| format!("send: {error:?}"))?;
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
    let final_sequence = loop {
        match receive_sink(&mut machine, line).await? {
            SinkInboundFact::Offered(sequence) => {
                send(
                    &mut machine,
                    line,
                    binding.frame(SessionMessage::Accepted { sequence }),
                )
                .await?;
                send(
                    &mut machine,
                    line,
                    binding.frame(SessionMessage::Delivered { sequence }),
                )
                .await?;
            }
            SinkInboundFact::InputClosed(final_sequence) => break final_sequence,
            fact => return Err(format!("unexpected sink fact: {fact:?}").into()),
        }
    };
    let terminal = match receive_sink(&mut machine, line).await? {
        SinkInboundFact::Terminal(disposition, observed) if observed == final_sequence => binding
            .frame(SessionMessage::Terminal {
                disposition,
                final_sequence,
            }),
        fact => return Err(format!("unexpected sink terminal: {fact:?}").into()),
    };
    send(&mut machine, line, terminal).await?;
    if !machine.is_terminal() {
        return Err("sink session did not reach exact terminal agreement".into());
    }
    Ok(())
}

#[derive(Debug)]
enum SinkInboundFact {
    Offered(u64),
    InputClosed(u64),
    Terminal(SessionTerminalDisposition, u64),
    Unexpected,
}

async fn receive_sink(
    machine: &mut SessionMachine,
    line: &mut BluezBleGattLine,
) -> Result<SinkInboundFact, Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; FRAME_BYTES];
    let length = tokio::time::timeout(SESSION_IO_TIMEOUT, line.receive_frame(&mut bytes))
        .await
        .map_err(|_| "receive: timed out waiting for the exact session frame")?
        .map_err(|error| format!("receive: {error:?}"))?;
    let frame = decode_session_frame(&bytes[..length], PAYLOAD_BYTES, FRAME_BYTES as u32)
        .map_err(|error| format!("decode: {error:?}"))?;
    machine
        .admit_inbound(frame)
        .map_err(|error| format!("inbound session refusal: {error:?}"))?;
    Ok(match frame.message {
        SessionMessage::Offered { sequence, .. } => SinkInboundFact::Offered(sequence),
        SessionMessage::InputClosed { final_sequence } => {
            SinkInboundFact::InputClosed(final_sequence)
        }
        SessionMessage::Terminal {
            disposition,
            final_sequence,
        } => SinkInboundFact::Terminal(disposition, final_sequence),
        _ => SinkInboundFact::Unexpected,
    })
}

async fn send(
    machine: &mut SessionMachine,
    line: &mut BluezBleGattLine,
    frame: conduit_wire::SessionFrame<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    machine
        .admit_outbound(frame)
        .map_err(|error| format!("outbound session refusal: {error:?}"))?;
    send_encoded(line, frame).await
}

async fn receive(
    machine: &mut SessionMachine,
    line: &mut BluezBleGattLine,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; FRAME_BYTES];
    let length = tokio::time::timeout(SESSION_IO_TIMEOUT, line.receive_frame(&mut bytes))
        .await
        .map_err(|_| "receive: timed out waiting for the exact session frame")?
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
    let (Some(peer_host_id), Some(peer_boot_id)) = (peer_host_id, peer_boot_id) else {
        return conduit_signal::std_pico_bluetooth_session_binding()
            .map_err(|error| format!("canonical Bluetooth Session binding: {error:?}").into());
    };
    if peer_host_id.starts_with("esp32/") {
        return conduit_signal::std_esp32_bluetooth_session_binding_for_host(
            peer_host_id,
            BootId::from(peer_boot_id),
        )
        .map_err(|error| format!("ESP32 Bluetooth Session binding: {error:?}").into());
    }
    if peer_host_id == conduit_signal::STD_PICO_USB_SINK_HOST_ID {
        let canonical = conduit_signal::std_pico_bluetooth_session_binding()
            .map_err(|error| format!("canonical Bluetooth Session binding: {error:?}"))?;
        let source_boot = canonical.source.boot_id.clone();
        return canonical
            .with_observed_boots(source_boot, BootId::from(peer_boot_id))
            .map_err(|error| format!("Pico Bluetooth Session binding: {error:?}").into());
    }
    Err(format!("unsupported Bluetooth sink Host identity: {peer_host_id}").into())
}
