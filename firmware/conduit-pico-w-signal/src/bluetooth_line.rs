//! Finite Pico W peripheral for the first Conduit BLE GATT Line profile.

#![allow(
    clippy::needless_borrows_for_generic_args,
    reason = "Trouble's GATT derive emits borrowed characteristic values"
)]

use conduit_bluetooth::{
    encode_fragment, fragment_count, BleGattProfile, BleReassembler, CONDUIT_BLE_SERVICE_UUID,
    MAXIMUM_BLE_GATT_PACKET_BYTES,
};
use conduit_core::{
    bind_active_play, BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId,
    HostId, KindId, LineId, LinkBindingId, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, LineAttachment, SessionBinding,
    SessionEndpointIdentity, SessionMachine, SessionMessage, SessionRole,
    SessionTerminalDisposition,
};
use cyw43::Control;
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_rp::{
    peripherals::{DMA_CH0, DMA_CH1, PIN_23, PIN_24, PIN_25, PIN_29, PIO0},
    Peri,
};
use embassy_time::{Duration, Timer};
use heapless09::Vec;
use trouble_host::prelude::*;

use crate::{
    receipts::{RuntimeTranscriptIdentity, UsbCdc},
    remote_error::RemoteError as UsbLinkError,
    remote_kernel::RemoteSignalKernel,
    signal_execution_identity::SignalExecutionIdentity,
    signal_image::generated_remote_endpoint,
};

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 2;
const FRAME_BYTES: usize = 2_048;
const SESSION_PAYLOAD_BYTES: u32 = 96;

#[gatt_server]
struct Server {
    conduit: ConduitService,
}

#[gatt_service(uuid = "9f105e51-7731-4524-9688-0d8a61021401")]
struct ConduitService {
    #[characteristic(uuid = "9f105e51-7731-4524-9688-0d8a61021402", write_without_response, permissions(encrypted))]
    write: Vec<u8, MAXIMUM_BLE_GATT_PACKET_BYTES>,
    #[characteristic(uuid = "9f105e51-7731-4524-9688-0d8a61021403", read, notify, permissions(encrypted))]
    notify: Vec<u8, MAXIMUM_BLE_GATT_PACKET_BYTES>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "the constrained Host boundary names every fixed peripheral and admitted asset"
)]
pub async fn run(
    spawner: &Spawner,
    sign: &mut UsbCdc,
    pio0: Peri<'static, PIO0>,
    dma_ch0: Peri<'static, DMA_CH0>,
    dma_ch1: Peri<'static, DMA_CH1>,
    pin23: Peri<'static, PIN_23>,
    pin24: Peri<'static, PIN_24>,
    pin25: Peri<'static, PIN_25>,
    pin29: Peri<'static, PIN_29>,
    fw: &'static aligned::Aligned<aligned::A4, [u8]>,
    btfw: &'static aligned::Aligned<aligned::A4, [u8]>,
    nvram: &'static aligned::Aligned<aligned::A4, [u8]>,
    clm: &'static [u8],
    runtime: &RuntimeTranscriptIdentity,
    flash_unique_id: [u8; 8],
) -> ! {
    let (bt_driver, mut control) = crate::radio::init_cyw43_bluetooth(
        spawner, pio0, dma_ch0, dma_ch1, pin23, pin24, pin25, pin29, fw, btfw, nvram, clm,
    )
    .await;
    let controller: ExternalController<_, 10> = ExternalController::new(bt_driver);
    let mut resources: HostResources<_, DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let mut address_bytes = [0_u8; 6];
    address_bytes.copy_from_slice(&flash_unique_id[..6]);
    address_bytes[5] = (address_bytes[5] & 0x3f) | 0xc0;
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(Address::random(address_bytes))
        .set_io_capabilities(IoCapabilities::NoInputNoOutput)
        .build();
    let mut peripheral = stack.peripheral();
    let runner = stack.runner();
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "Conduit Pico W",
        appearance: &appearance::computer::GENERIC_COMPUTER,
    }))
    .unwrap();

    let identity = SignalExecutionIdentity::plan_a();
    let _ = sign.write_boot_identity(identity.boot(), runtime).await;
    let _ = sign.write_marker("CONDUIT_BLE_CONTROLLER_READY").await;
    let line = async {
        let mut consecutive_advertise_failures = 0_u8;
        let connection = loop {
            match advertise(&mut peripheral, &server).await {
                Ok(connection) => break connection,
                Err(_) if consecutive_advertise_failures < 7 => {
                    consecutive_advertise_failures += 1;
                    Timer::after(Duration::from_millis(250)).await;
                }
                Err(_) => {
                    let _ = sign.write_marker("CONDUIT_BLE_ADVERTISE_FAILED").await;
                    return core::future::pending::<()>().await;
                }
            }
        };
        let _ = sign.write_marker("CONDUIT_BLE_LINE_CONNECTED").await;
        let result = serve_connection(
            &stack,
            &server,
            &connection,
            &mut control,
            sign,
            runtime,
        )
        .await;
        let marker = if result.is_ok() {
            "CONDUIT_BLE_LINE_COMPLETE"
        } else {
            "CONDUIT_BLE_LINE_LOST"
        };
        let _ = sign.write_marker(marker).await;
        core::future::pending::<()>().await
    };
    match select(run_host_until_stopped(runner), line).await {
        Either::First(host_failed) => {
            let marker = if host_failed {
                "CONDUIT_BLE_HOST_FAILED"
            } else {
                "CONDUIT_BLE_HOST_RETURNED"
            };
            let _ = sign.write_marker(marker).await;
            core::future::pending().await
        }
        Either::Second(()) => {
            let _ = sign.write_marker("CONDUIT_BLE_LINE_STOPPED").await;
            core::future::pending().await
        }
    }
}

async fn run_host_until_stopped<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) -> bool {
    runner.run().await.is_err()
}

async fn advertise<'values, 'server, C: Controller>(
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut data = [0_u8; 31];
    let uuid = u128::from_be_bytes(CONDUIT_BLE_SERVICE_UUID);
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteServiceUuids128(&[uuid.to_le_bytes()]),
        ],
        &mut data,
    )?;
    let mut scan_data = [0_u8; 31];
    let scan_len = AdStructure::encode_slice(
        &[AdStructure::CompleteLocalName(b"Conduit Pico W")],
        &mut scan_data,
    )?;
    let connection = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &data[..len],
                scan_data: &scan_data[..scan_len],
            },
        )
        .await?
        .accept()
        .await?;
    // Establish bondability before the central starts SMP. The selected BlueZ
    // application owns the one bounded Device.Pair operation after this
    // connection has entered its GATT event loop.
    connection.set_bondable(true)?;
    Ok(connection.with_attribute_server(server)?)
}

async fn serve_connection<C: Controller>(
    stack: &Stack<'_, C, DefaultPacketPool>,
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    control: &mut Control<'_>,
    sign: &mut UsbCdc,
    runtime: &RuntimeTranscriptIdentity,
) -> Result<(), UsbLinkError> {
    let binding = binding(runtime)?;
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink)?;
    let identity = SignalExecutionIdentity::plan_a();
    let mut kernel = RemoteSignalKernel::new(identity)?;
    let mut reassembler = BleReassembler::new(BleGattProfile::FIRST);
    let mut frame_bytes = [0_u8; FRAME_BYTES];
    let mut send_sequence = 0_u8;

    // BlueZ completes its controller feature exchange before its bounded
    // Device.Pair operation can consume a peripheral Security Request. An
    // immediate request at accept races that exchange on the CYW43439; a
    // finite stabilization delay keeps the request inside this connection
    // realization while allowing the controller handshake to finish.
    Timer::after(Duration::from_millis(500)).await;
    connection
        .raw()
        .request_security()
        .map_err(|_| UsbLinkError::InvalidGeneratedEndpoint)?;

    loop {
        match connection.next().await {
            GattConnectionEvent::Disconnected { .. } => return Err(UsbLinkError::UsbDisconnected),
            GattConnectionEvent::PairingComplete { bond, .. } => {
                if let Some(bond) = bond {
                    let _ = stack.add_bond_information(bond);
                }
                let _ = sign.write_marker("CONDUIT_BLE_PEER_PAIRED").await;
            }
            GattConnectionEvent::PairingFailed(_) => return Err(UsbLinkError::InvalidGeneratedEndpoint),
            GattConnectionEvent::Gatt {
                event: GattEvent::Write(event),
            } if event.handle() == server.conduit.write.handle => {
                let mut packet = [0_u8; MAXIMUM_BLE_GATT_PACKET_BYTES];
                let packet_len = event.with_data(|_, data| {
                    if data.len() <= packet.len() {
                        packet[..data.len()].copy_from_slice(data);
                    }
                    data.len()
                });
                if packet_len > packet.len() {
                    return Err(UsbLinkError::BufferOverflow);
                }
                if let Ok(reply) = event.accept() {
                    reply.send().await;
                }
                if connection.raw().security_level() == Ok(SecurityLevel::NoEncryption) {
                    return Err(UsbLinkError::InvalidGeneratedEndpoint);
                }
                if let Some(frame) = reassembler
                    .admit(&packet[..packet_len])
                    .map_err(|_| UsbLinkError::BufferOverflow)?
                {
                    frame_bytes[..frame.len()].copy_from_slice(frame);
                    let decoded = decode_session_frame(
                        &frame_bytes[..frame.len()],
                        SESSION_PAYLOAD_BYTES,
                        FRAME_BYTES as u32,
                    )?;
                    handle_session_frame(
                        decoded,
                        &binding,
                        &mut machine,
                        &mut kernel,
                        server,
                        connection,
                        control,
                        sign,
                        runtime,
                        &mut send_sequence,
                    )
                    .await?;
                }
            }
            GattConnectionEvent::Gatt { event } => {
                if let Ok(reply) = event.accept() {
                    reply.send().await;
                }
            }
            _ => {}
        }
        if machine.is_terminal() {
            return Ok(());
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_session_frame(
    frame: conduit_wire::SessionFrame<'_>,
    binding: &SessionBinding,
    machine: &mut SessionMachine,
    kernel: &mut RemoteSignalKernel,
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    control: &mut Control<'_>,
    sign: &mut UsbCdc,
    runtime: &RuntimeTranscriptIdentity,
    send_sequence: &mut u8,
) -> Result<(), UsbLinkError> {
    machine.admit_inbound(frame)?;
    match frame.message {
        SessionMessage::Hello(_) => {
            send_session(binding.hello_frame(), machine, server, connection, send_sequence).await?;
        }
        SessionMessage::Ready => {
            send_session(
                binding.frame(SessionMessage::Ready),
                machine,
                server,
                connection,
                send_sequence,
            )
            .await?;
        }
        SessionMessage::Offered { sequence, payload } => {
            match kernel.admit(sequence, payload)? {
                RemoteIngressOutcome::Accepted { .. } => {}
                RemoteIngressOutcome::Full { .. } => {
                    send_session(
                        binding.frame(SessionMessage::Pressure { sequence }),
                        machine,
                        server,
                        connection,
                        send_sequence,
                    )
                    .await?;
                    return Ok(());
                }
            }
            send_session(
                binding.frame(SessionMessage::Accepted { sequence }),
                machine,
                server,
                connection,
                send_sequence,
            )
            .await?;
            kernel
                .present_accepted(sequence, control, sign, runtime)
                .await?;
            send_session(
                binding.frame(SessionMessage::Delivered { sequence }),
                machine,
                server,
                connection,
                send_sequence,
            )
            .await?;
        }
        SessionMessage::InputClosed { final_sequence } => {
            kernel.close_and_complete(final_sequence)?;
        }
        SessionMessage::Terminal {
            disposition,
            final_sequence,
        } => {
            send_session(
                binding.frame(SessionMessage::Terminal {
                    disposition,
                    final_sequence,
                }),
                machine,
                server,
                connection,
                send_sequence,
            )
            .await?;
            if disposition == SessionTerminalDisposition::Completed {
                sign.write_terminal(true, SignalExecutionIdentity::plan_a().terminal(), runtime)
                    .await?;
            }
        }
        _ => {}
    }
    Ok(())
}

async fn send_session(
    frame: conduit_wire::SessionFrame<'_>,
    machine: &mut SessionMachine,
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    send_sequence: &mut u8,
) -> Result<(), UsbLinkError> {
    machine.admit_outbound(frame)?;
    let mut bytes = [0_u8; FRAME_BYTES];
    let length = encode_session_frame_into(
        frame,
        &mut bytes,
        SESSION_PAYLOAD_BYTES,
        FRAME_BYTES as u32,
    )?;
    let profile = BleGattProfile::FIRST;
    let count = fragment_count(length, profile).map_err(|_| UsbLinkError::BufferOverflow)?;
    let mut packet = [0_u8; MAXIMUM_BLE_GATT_PACKET_BYTES];
    for index in 0..count {
        let packet_len = encode_fragment(
            &bytes[..length],
            *send_sequence,
            index,
            profile,
            &mut packet,
        )
            .map_err(|_| UsbLinkError::BufferOverflow)?;
        let value = Vec::from_slice(&packet[..packet_len]).map_err(|_| UsbLinkError::BufferOverflow)?;
        server
            .conduit
            .notify
            .notify(connection, &value, false)
            .await
            .map_err(|_| UsbLinkError::UsbDisconnected)?;
    }
    *send_sequence = send_sequence.wrapping_add(1);
    Ok(())
}

fn binding(runtime: &RuntimeTranscriptIdentity) -> Result<SessionBinding, UsbLinkError> {
    let planned = generated_remote_endpoint().ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
    let plan_id = PlanId::from(crate::signal_image::PLAN_ID);
    let source_host = HostId::from(planned.peer_host);
    let source_boot = BootId::from(planned.peer_boot);
    let sink_host = HostId::from(planned.local_host);
    let sink_boot = BootId::from(planned.local_boot);
    SessionBinding {
        protocol_version: 1,
        source_active_play_id: bind_active_play(&plan_id, &source_host, &source_boot, 0).active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &sink_host, &sink_boot, 0).active_play_id,
        plan_id,
        source_fragment_id: FragmentId::from(planned.source_fragment_id),
        sink_fragment_id: FragmentId::from(planned.sink_fragment_id),
        connection_id: ConnectionId::from(planned.connection_id),
        source: SessionEndpointIdentity {
            host_id: source_host.clone(),
            boot_id: source_boot.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink_host.clone(),
            boot_id: sink_boot.clone(),
        },
        value_kind: KindId::from(planned.value_kind),
        limits: conduit_wire::SessionLimits {
            maximum_in_flight_items: planned.session_item_capacity,
            maximum_payload_bytes: planned.session_byte_capacity,
            maximum_buffered_bytes: planned.session_byte_capacity,
        },
        attachment: LineAttachment {
            line_id: LineId::from(planned.line_id),
            link_binding_id: LinkBindingId::from(planned.link_binding_id),
            base: ConnectionBase::BluetoothLeGatt,
            base_instance_id: ConnectionBaseInstanceId::from(planned.base_instance_id),
            source_host_id: source_host,
            source_boot_id: source_boot,
            source_endpoint_id: LinkEndpointId::from(planned.peer_endpoint),
            sink_host_id: sink_host,
            sink_boot_id: sink_boot,
            sink_endpoint_id: LinkEndpointId::from(planned.local_endpoint),
            limits: LinkLimits {
                maximum_in_flight_items: planned.maximum_in_flight_items,
                maximum_payload_bytes: planned.maximum_payload_bytes,
                maximum_buffered_bytes: planned.maximum_buffered_bytes,
                maximum_frame_bytes: planned.maximum_frame_bytes,
            },
        },
    }
    .with_observed_boots(BootId::from(planned.peer_boot), BootId::from(runtime.boot_id()))
    .map_err(UsbLinkError::Codec)
}
