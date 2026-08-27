//! ESP radio/GATT realization below the generic bounded Conduit BLE profile.
//!
//! The Espressif controller, radio task, ATT handles, and advertising bytes are
//! Base facts. They do not define Host, Line, Cord, or semantic identities.

use embassy_futures::join::join;
use esp_hal::{
    efuse::{self, InterfaceMacAddress},
    rng::Trng,
};
use heapless::Vec;
use trouble_host::prelude::*;

#[cfg(feature = "distributed-lenia")]
use crate::lenia_session::ConduitLeniaSession as ConduitBleSession;
#[cfg(not(feature = "distributed-lenia"))]
use crate::session::ConduitBleSession;

const CONNECTIONS_MAXIMUM: usize = 1;
const L2CAP_CHANNELS_MAXIMUM: usize = 2;
const PACKET_BYTES_MAXIMUM: usize = conduit_bluetooth::MAXIMUM_BLE_GATT_PACKET_BYTES;
const CONDUIT_SERVICE_UUID_LE: [u8; 16] = 0x9f105e517731452496880d8a61021401_u128.to_le_bytes();

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
    write: Vec<u8, PACKET_BYTES_MAXIMUM>,
    #[characteristic(
        uuid = "9f105e51-7731-4524-9688-0d8a61021403",
        notify,
        permissions(encrypted)
    )]
    notify: Vec<u8, PACKET_BYTES_MAXIMUM>,
}

pub async fn run<C>(
    controller: C,
    boot: &crate::receipts::BootIdentity,
    security_rng: &mut Trng,
    kernel: &'static mut ActiveKernel,
) where
    C: Controller,
{
    let mac = efuse::interface_mac_address(InterfaceMacAddress::Bluetooth);
    let mut address = [0_u8; 6];
    address.copy_from_slice(mac.as_bytes());
    // The controller API currently accepts a random-address configuration.
    // Preserve the chip-derived 46-bit suffix while marking it static-random;
    // this transport address remains distinct from Host identity.
    address[0] |= 0xc0;
    let address = Address::random(address);

    let mut resources: HostResources<
        DefaultPacketPool,
        CONNECTIONS_MAXIMUM,
        L2CAP_CHANNELS_MAXIMUM,
    > = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address)
        .set_random_generator_seed(security_rng);
    stack.set_io_capabilities(IoCapabilities::NoInputNoOutput);
    let Host {
        mut peripheral,
        runner,
        ..
    } = stack.build();
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "Conduit ESP32",
        appearance: &appearance::computer::GENERIC_COMPUTER,
    }))
    .expect("the fixed Conduit GATT table must fit");

    esp_println::println!(
        "CONDUIT_ESP32_BLE_BASE_READY address={} profile=ble-gatt-first mtu={} packet-bytes={} frame-bytes={} connections={} l2cap-channels={} pairing=just-works bond-slots=1 bond-retention=boot-only",
        address,
        conduit_bluetooth::BleGattProfile::FIRST.negotiated_att_mtu,
        conduit_bluetooth::BleGattProfile::FIRST.maximum_gatt_packet_bytes,
        conduit_bluetooth::BleGattProfile::FIRST.maximum_frame_bytes,
        CONNECTIONS_MAXIMUM,
        L2CAP_CHANNELS_MAXIMUM,
    );
    boot.print_host_offer(address);

    let _ = join(run_controller(runner), async {
        let connection = advertise(&mut peripheral, &server)
            .await
            .expect("bounded BLE advertising must remain available");
        connection
            .raw()
            .set_bondable(true)
            .expect("the fresh connection security policy must be configurable");
        // Repeat the exact physical identity at the accepted-session boundary.
        // The UART proof reader may attach after boot; acceptance must not rely
        // on ambient serial backlog to bind this session to its Host and Boot.
        boot.print_boot();
        boot.print_host_offer(address);
        esp_println::println!("CONDUIT_ESP32_BLE_CONNECTED");
        let _ = serve_connection(&server, &connection, boot, &stack, kernel).await;
        esp_println::println!("CONDUIT_ESP32_BLE_LOST");
        core::future::pending::<()>().await;
    })
    .await;
}

#[cfg(not(feature = "distributed-lenia"))]
type ActiveKernel = crate::remote_kernel::Esp32RemoteSignalKernel;
#[cfg(feature = "distributed-lenia")]
type ActiveKernel = conduit_alife::DistributedLeniaWorker;

async fn run_controller<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        runner
            .run()
            .await
            .expect("the admitted BLE controller task must remain live");
    }
}

async fn advertise<'values, 'server, C: Controller>(
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertisement = [0_u8; 31];
    let length = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids128(&[CONDUIT_SERVICE_UUID_LE]),
            AdStructure::ShortenedLocalName(b"Conduit"),
        ],
        &mut advertisement,
    )?;
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertisement[..length],
                scan_data: &[],
            },
        )
        .await?;
    esp_println::println!("CONDUIT_ESP32_BLE_ADVERTISING");
    Ok(advertiser.accept().await?.with_attribute_server(server)?)
}

async fn serve_connection<C: Controller, P: PacketPool>(
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, P>,
    boot: &crate::receipts::BootIdentity,
    stack: &Stack<'_, C, P>,
    kernel: &mut ActiveKernel,
) -> Result<(), Error> {
    let mut session = ConduitBleSession::new(boot, kernel)
        .expect("the exact boot-scoped BLE session binding must validate");
    loop {
        match connection.next().await {
            GattConnectionEvent::Disconnected { .. } => return Ok(()),
            GattConnectionEvent::PairingComplete {
                security_level,
                bond,
            } => {
                let retained = if let Some(bond) = bond {
                    stack.add_bond_information(bond)?;
                    true
                } else {
                    false
                };
                esp_println::println!(
                    "CONDUIT_ESP32_BLE_PAIRED security={:?} retained-boot-bond={}",
                    security_level,
                    retained
                );
            }
            GattConnectionEvent::PairingFailed(error) => {
                esp_println::println!("CONDUIT_ESP32_BLE_PAIRING_FAILED error={:?}", error);
                return Err(error);
            }
            GattConnectionEvent::Gatt { event } => match event {
                GattEvent::Write(write) if write.handle() == server.conduit.write.handle => {
                    let result = session.admit_packet(write.data());
                    drop(write.into_payload());
                    let replies = match result {
                        Ok(replies) => replies,
                        Err(reason) => {
                            esp_println::println!(
                                "CONDUIT_ESP32_BLE_SESSION_REFUSED reason={}",
                                reason
                            );
                            connection.raw().disconnect();
                            continue;
                        }
                    };
                    for packet in replies.iter() {
                        let mut value = Vec::<u8, PACKET_BYTES_MAXIMUM>::new();
                        value
                            .extend_from_slice(packet)
                            .map_err(|_| Error::InsufficientSpace)?;
                        server.conduit.notify.notify(connection, &value).await?;
                        // Controller admission is not over-air delivery. Keep
                        // every result burst within the receiver's one fixed
                        // reassembly slot.
                        embassy_time::Timer::after_millis(5).await;
                    }
                    if !replies.is_empty() {
                        esp_println::println!(
                            "CONDUIT_ESP32_BLE_SESSION replies={} next-sequence={}",
                            replies.len(),
                            session.next_sequence()
                        );
                    }
                }
                other => {
                    other.accept()?.send().await;
                }
            },
            _ => {}
        }
    }
}
