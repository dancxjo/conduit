//! One finite, identity-bound distributed-Lenia worker over the planned BLE Lines.

#![allow(clippy::needless_borrows_for_generic_args)]

use conduit_bluetooth::{
    encode_fragment, fragment_count, BleGattProfile, BleReassembler, CONDUIT_BLE_SERVICE_UUID,
    MAXIMUM_BLE_GATT_PACKET_BYTES,
};
use conduit_core::{
    DistributedLeniaWorker, LeniaLineFrameIdentity, LeniaLineFrameView, LeniaWorkerAdmission,
    LENIA_LINE_FRAME_MAX_BYTES,
};
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
    lenia_image,
    receipts::{RuntimeTranscriptIdentity, UsbCdc},
};

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 2;

#[gatt_server]
struct Server {
    conduit: ConduitService,
}

#[gatt_service(uuid = "9f105e51-7731-4524-9688-0d8a61021401")]
struct ConduitService {
    #[characteristic(
        uuid = "9f105e51-7731-4524-9688-0d8a61021402",
        write_without_response,
        permissions(encrypted)
    )]
    write: Vec<u8, MAXIMUM_BLE_GATT_PACKET_BYTES>,
    #[characteristic(
        uuid = "9f105e51-7731-4524-9688-0d8a61021403",
        read,
        notify,
        permissions(encrypted)
    )]
    notify: Vec<u8, MAXIMUM_BLE_GATT_PACKET_BYTES>,
}

#[allow(clippy::too_many_arguments)]
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
    let (bt_driver, _) = crate::radio::init_cyw43_bluetooth(
        spawner, pio0, dma_ch0, dma_ch1, pin23, pin24, pin25, pin29, fw, btfw, nvram, clm,
    )
    .await;
    let controller: ExternalController<_, 10> = ExternalController::new(bt_driver);
    let mut resources: HostResources<_, DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let mut address = [0; 6];
    address.copy_from_slice(&flash_unique_id[..6]);
    address[5] = (address[5] & 0x3f) | 0xc0;
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(Address::random(address))
        .set_io_capabilities(IoCapabilities::NoInputNoOutput)
        .build();
    let mut peripheral = stack.peripheral();
    let runner = stack.runner();
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "Conduit Lenia Pico",
        appearance: &appearance::computer::GENERIC_COMPUTER,
    }))
    .unwrap();
    let mut worker = DistributedLeniaWorker::new();
    if worker.prepare().is_err() {
        let _ = sign.write_marker("CONDUIT_LENIA_PREPARE_REFUSED").await;
        halt().await;
    }
    let _ = sign
        .write_lenia_boot(runtime, lenia_image::PLAN_ID, lenia_image::HOST_ID)
        .await;
    let _ = sign.write_marker("CONDUIT_LENIA_CONTROLLER_READY").await;
    let line = async {
        let connection = match advertise(&mut peripheral, &server).await {
            Ok(connection) => connection,
            Err(_) => {
                let _ = sign.write_marker("CONDUIT_LENIA_ADVERTISE_FAILED").await;
                halt().await
            }
        };
        let _ = sign.write_marker("CONDUIT_LENIA_LINE_CONNECTED").await;
        let result = serve(&stack, &server, &connection, sign, runtime, &mut worker).await;
        let _ = sign
            .write_marker(if result.is_ok() {
                "CONDUIT_LENIA_REGION_COMPLETE"
            } else {
                "CONDUIT_LENIA_LINE_LOST"
            })
            .await;
        reboot_after_terminal(sign).await
    };
    match select(run_host(runner), line).await {
        Either::First(_) => {
            let _ = sign.write_marker("CONDUIT_LENIA_HOST_FAILED").await;
            reboot_after_terminal(sign).await
        }
        Either::Second(value) => value,
    }
}

async fn run_host<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    let _ = runner.run().await;
}

async fn advertise<'v, 's, C: Controller>(
    peripheral: &mut Peripheral<'v, C, DefaultPacketPool>,
    server: &'s Server<'v>,
) -> Result<GattConnection<'v, 's, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut data = [0; 31];
    let uuid = u128::from_be_bytes(CONDUIT_BLE_SERVICE_UUID);
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteServiceUuids128(&[uuid.to_le_bytes()]),
        ],
        &mut data,
    )?;
    let mut scan = [0; 31];
    let scan_len = AdStructure::encode_slice(
        &[AdStructure::CompleteLocalName(b"Conduit Lenia Pico")],
        &mut scan,
    )?;
    let connection = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &data[..len],
                scan_data: &scan[..scan_len],
            },
        )
        .await?
        .accept()
        .await?;
    connection.set_bondable(true)?;
    Ok(connection.with_attribute_server(server)?)
}

async fn serve<C: Controller>(
    stack: &Stack<'_, C, DefaultPacketPool>,
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    sign: &mut UsbCdc,
    runtime: &RuntimeTranscriptIdentity,
    worker: &mut DistributedLeniaWorker,
) -> Result<(), ()> {
    let mut reassembler = BleReassembler::new(BleGattProfile::FIRST);
    let mut frame = [0; LENIA_LINE_FRAME_MAX_BYTES];
    let mut session = None;
    Timer::after(Duration::from_millis(500)).await;
    connection.raw().request_security().map_err(|_| ())?;
    loop {
        match connection.next().await {
            GattConnectionEvent::Disconnected { .. } => return Err(()),
            GattConnectionEvent::PairingComplete { bond, .. } => {
                if let Some(bond) = bond {
                    let _ = stack.add_bond_information(bond);
                }
                let _ = sign.write_marker("CONDUIT_LENIA_PEER_PAIRED").await;
            }
            GattConnectionEvent::PairingFailed(_) => return Err(()),
            GattConnectionEvent::Gatt {
                event: GattEvent::Write(event),
            } if event.handle() == server.conduit.write.handle => {
                let mut packet = [0; MAXIMUM_BLE_GATT_PACKET_BYTES];
                let length = event.with_data(|_, data| {
                    if data.len() <= packet.len() {
                        packet[..data.len()].copy_from_slice(data);
                    }
                    data.len()
                });
                if let Ok(reply) = event.accept() {
                    reply.send().await;
                }
                if length > packet.len()
                    || connection.raw().security_level() == Ok(SecurityLevel::NoEncryption)
                {
                    return Err(());
                }
                if let Some(bytes) = reassembler.admit(&packet[..length]).map_err(|_| ())? {
                    frame[..bytes.len()].copy_from_slice(bytes);
                    let incoming =
                        LeniaLineFrameView::decode(&frame[..bytes.len()]).map_err(|_| ())?;
                    validate_work(&incoming, runtime, &mut session)?;
                    match worker.admit(incoming.chunk).map_err(|_| ())? {
                        LeniaWorkerAdmission::Progress { .. } => {
                            let _ = sign.write_marker("CONDUIT_LENIA_BOUNDARY_ADMITTED").await;
                        }
                        LeniaWorkerAdmission::ResultReady => {
                            let _ = sign.write_marker("CONDUIT_LENIA_RESULT_READY").await;
                            send_result(worker, server, connection, runtime, session.unwrap())
                                .await?;
                            return Ok(());
                        }
                    }
                }
            }
            GattConnectionEvent::Gatt { event } => {
                if let Ok(reply) = event.accept() {
                    reply.send().await;
                }
            }
            _ => {}
        }
    }
}

fn validate_work(
    frame: &LeniaLineFrameView<'_>,
    runtime: &RuntimeTranscriptIdentity,
    session: &mut Option<[u8; 16]>,
) -> Result<(), ()> {
    let id = frame.identity;
    if id.plan_id != lenia_image::PLAN_ID
        || id.play_id != lenia_image::WORK_PLAY_ID
        || id.line_id != lenia_image::WORK_LINE_ID
        || id.source_host_id != lenia_image::WORK_SOURCE_HOST_ID
        || id.source_boot_id != lenia_image::WORK_SOURCE_BOOT_ID
        || id.sink_host_id != lenia_image::WORK_SINK_HOST_ID
        || id.sink_boot_id != runtime.boot_id()
    {
        return Err(());
    }
    match *session {
        Some(expected) if expected != id.session_id => Err(()),
        None => {
            *session = Some(id.session_id);
            Ok(())
        }
        _ => Ok(()),
    }
}

async fn send_result(
    worker: &DistributedLeniaWorker,
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    runtime: &RuntimeTranscriptIdentity,
    session: [u8; 16],
) -> Result<(), ()> {
    let identity = worker.result_identity().map_err(|_| ())?;
    let mut offset = 0;
    let mut chunk = [0; conduit_alife::LENIA_REGION_CHUNK_MAX_BYTES];
    let mut frame = [0; LENIA_LINE_FRAME_MAX_BYTES];
    let mut sequence = 0;
    while offset < identity.total_cells {
        let chunk_len = worker
            .encode_result_chunk(offset, &mut chunk)
            .map_err(|_| ())?;
        let frame_len = LeniaLineFrameIdentity {
            plan_id: lenia_image::PLAN_ID,
            play_id: lenia_image::RESULT_PLAY_ID,
            line_id: lenia_image::RESULT_LINE_ID,
            source_host_id: lenia_image::HOST_ID,
            source_boot_id: runtime.boot_id(),
            sink_host_id: lenia_image::RESULT_SINK_HOST_ID,
            sink_boot_id: lenia_image::RESULT_SINK_BOOT_ID,
            session_id: session,
        }
        .encode(&chunk[..chunk_len], &mut frame)
        .map_err(|_| ())?;
        send_frame(&frame[..frame_len], sequence, server, connection).await?;
        let view =
            conduit_alife::LeniaRegionChunkView::decode(&chunk[..chunk_len]).map_err(|_| ())?;
        offset += u32::from(view.header.cell_count);
        sequence = sequence.wrapping_add(1);
    }
    Ok(())
}

async fn send_frame(
    frame: &[u8],
    sequence: u8,
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
) -> Result<(), ()> {
    let profile = BleGattProfile::FIRST;
    let count = fragment_count(frame.len(), profile).map_err(|_| ())?;
    let mut packet = [0; MAXIMUM_BLE_GATT_PACKET_BYTES];
    for index in 0..count {
        let length =
            encode_fragment(frame, sequence, index, profile, &mut packet).map_err(|_| ())?;
        let value = Vec::from_slice(&packet[..length]).map_err(|_| ())?;
        server
            .conduit
            .notify
            .notify(connection, &value, false)
            .await
            .map_err(|_| ())?;
        if index + 1 < count {
            Timer::after(Duration::from_millis(5)).await;
        }
    }
    Ok(())
}

async fn reboot_after_terminal(sign: &UsbCdc) -> ! {
    for _ in 0..500 {
        if !sign.dtr() {
            break;
        }
        Timer::after(Duration::from_millis(10)).await;
    }
    rp_pac::PSM
        .wdsel()
        .write_value(rp_pac::psm::regs::Wdsel(0x0001_fffc));
    rp_pac::WATCHDOG
        .ctrl()
        .modify(|value| value.set_trigger(true));
    loop {
        cortex_m::asm::wfi();
    }
}

async fn halt() -> ! {
    core::future::pending().await
}
