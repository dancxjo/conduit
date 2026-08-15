//! Linux BlueZ adapter for the first bounded Conduit BLE GATT Line profile.
//!
//! This module owns only platform I/O. Discovery and pairing remain current
//! platform observations, the shared session machine owns exact Plan/endpoint
//! admission, and the production kernel remains the scheduler.

use bluer::gatt::{remote::Characteristic, CharacteristicReader, CharacteristicWriter};
use bluer::{
    adv::{Advertisement, AdvertisementHandle, Type as AdvertisementType},
    gatt::local::{
        characteristic_control, Application, ApplicationHandle,
        Characteristic as LocalCharacteristic, CharacteristicControl, CharacteristicControlEvent,
        CharacteristicNotify, CharacteristicNotifyMethod, CharacteristicWrite,
        CharacteristicWriteMethod, Service,
    },
    Adapter, AdapterEvent, Address, DiscoveryFilter, DiscoveryTransport, Session,
};
use conduit_bluetooth::{
    encode_fragment, fragment_count, BleGattProfile, BleReassembler, CONDUIT_BLE_NOTIFY_UUID,
    CONDUIT_BLE_SERVICE_UUID, CONDUIT_BLE_WRITE_UUID, MAXIMUM_BLE_GATT_PACKET_BYTES,
};
use futures::StreamExt;
use std::{
    collections::{BTreeSet, HashSet},
    pin::Pin,
    time::Duration,
};
use tokio::io::AsyncReadExt;

const MAXIMUM_GATT_OBJECTS_INSPECTED: usize = 16;
const MAXIMUM_DISCOVERY_EVENTS_INSPECTED: usize = 64;
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BluezBleGattError {
    InvalidProfile,
    BluetoothUnavailable,
    ControllerUnavailable,
    ControllerPoweredOff,
    ApplicationUnavailable(String),
    AdvertisementUnavailable(String),
    DiscoveryUnavailable(String),
    CandidateUnavailable,
    IncompatibleProfile,
    DeviceUnavailable,
    NotPaired,
    ConnectFailed,
    MissingService,
    MissingWriteCharacteristic,
    MissingIndicateCharacteristic,
    CharacteristicContractMismatch,
    MtuMismatch,
    OversizedFrame,
    OutputTooSmall,
    Disconnected,
    Transport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BluezBleGattCandidate {
    pub address: [u8; 6],
    pub paired: bool,
}

/// Discover one currently present compatible peer. The scan is bounded by
/// both elapsed time and inspected events and retains no nearby-device list.
pub async fn discover_ble_gatt_candidate(
    adapter_name: &str,
    expected_address: [u8; 6],
) -> Result<BluezBleGattCandidate, BluezBleGattError> {
    let session = Session::new()
        .await
        .map_err(|_| BluezBleGattError::BluetoothUnavailable)?;
    let adapter = session
        .adapter(adapter_name)
        .map_err(|_| BluezBleGattError::ControllerUnavailable)?;
    if !adapter
        .is_powered()
        .await
        .map_err(|_| BluezBleGattError::ControllerUnavailable)?
    {
        return Err(BluezBleGattError::ControllerPoweredOff);
    }
    let service_uuid = uuid::Uuid::from_bytes(CONDUIT_BLE_SERVICE_UUID);
    adapter
        .set_discovery_filter(DiscoveryFilter {
            uuids: HashSet::from([service_uuid]),
            transport: DiscoveryTransport::Le,
            ..Default::default()
        })
        .await
        .map_err(|error| BluezBleGattError::DiscoveryUnavailable(error.to_string()))?;
    let expected = Address(expected_address);
    let scan = async {
        let events = adapter
            .discover_devices_with_changes()
            .await
            .map_err(|error| BluezBleGattError::DiscoveryUnavailable(error.to_string()))?;
        futures::pin_mut!(events);
        for _ in 0..MAXIMUM_DISCOVERY_EVENTS_INSPECTED {
            let address = match events.next().await {
                Some(AdapterEvent::DeviceAdded(address)) => address,
                Some(_) => continue,
                None => return Err(BluezBleGattError::CandidateUnavailable),
            };
            if address != expected {
                continue;
            }
            let device = adapter
                .device(address)
                .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
            if device
                .rssi()
                .await
                .map_err(|_| BluezBleGattError::DeviceUnavailable)?
                .is_none()
            {
                continue;
            }
            let uuids = device
                .uuids()
                .await
                .map_err(|_| BluezBleGattError::DeviceUnavailable)?
                .unwrap_or_default();
            if !uuids.contains(&service_uuid) {
                return Err(BluezBleGattError::IncompatibleProfile);
            }
            let paired = device
                .is_paired()
                .await
                .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
            return Ok(BluezBleGattCandidate {
                address: address.0,
                paired,
            });
        }
        Err(BluezBleGattError::CandidateUnavailable)
    };
    tokio::time::timeout(DISCOVERY_TIMEOUT, scan)
        .await
        .map_err(|_| BluezBleGattError::CandidateUnavailable)?
}

/// One exact connected GATT mechanism. Construct this during Host preparation;
/// the object retains no discovery list, reconnect policy, or semantic state.
pub struct BluezBleGattLine {
    address: Address,
    profile: BleGattProfile,
    write: CharacteristicWriter,
    indicate: CharacteristicReader,
    send_sequence: u8,
    reassembler: BleReassembler,
}

pub struct BluezBleGattListener {
    _session: Session,
    adapter: Adapter,
    profile: BleGattProfile,
    _advertisement: AdvertisementHandle,
    _application: ApplicationHandle,
    write_control: Pin<Box<CharacteristicControl>>,
    indicate_control: Pin<Box<CharacteristicControl>>,
}

impl BluezBleGattListener {
    pub async fn bind(
        adapter_name: &str,
        profile: BleGattProfile,
    ) -> Result<Self, BluezBleGattError> {
        let profile = profile
            .validate()
            .map_err(|_| BluezBleGattError::InvalidProfile)?;
        let session = Session::new()
            .await
            .map_err(|_| BluezBleGattError::BluetoothUnavailable)?;
        let adapter = session
            .adapter(adapter_name)
            .map_err(|_| BluezBleGattError::ControllerUnavailable)?;
        if !adapter
            .is_powered()
            .await
            .map_err(|_| BluezBleGattError::ControllerUnavailable)?
        {
            return Err(BluezBleGattError::ControllerPoweredOff);
        }

        let service_uuid = uuid::Uuid::from_bytes(CONDUIT_BLE_SERVICE_UUID);
        let (write_control, write_handle) = characteristic_control();
        let (indicate_control, indicate_handle) = characteristic_control();
        let application = adapter
            .serve_gatt_application(Application {
                services: vec![Service {
                    uuid: service_uuid,
                    primary: true,
                    characteristics: vec![
                        LocalCharacteristic {
                            uuid: uuid::Uuid::from_bytes(CONDUIT_BLE_WRITE_UUID),
                            write: Some(CharacteristicWrite {
                                write_without_response: true,
                                method: CharacteristicWriteMethod::Io,
                                ..Default::default()
                            }),
                            control_handle: write_handle,
                            ..Default::default()
                        },
                        LocalCharacteristic {
                            uuid: uuid::Uuid::from_bytes(CONDUIT_BLE_NOTIFY_UUID),
                            notify: Some(CharacteristicNotify {
                                notify: true,
                                method: CharacteristicNotifyMethod::Io,
                                ..Default::default()
                            }),
                            control_handle: indicate_handle,
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            })
            .await
            .map_err(|error| BluezBleGattError::ApplicationUnavailable(error.to_string()))?;
        let advertisement = adapter
            .advertise(Advertisement {
                advertisement_type: AdvertisementType::Peripheral,
                service_uuids: BTreeSet::from([service_uuid]),
                ..Default::default()
            })
            .await
            .map_err(|error| BluezBleGattError::AdvertisementUnavailable(error.to_string()))?;
        Ok(Self {
            _session: session,
            adapter,
            profile,
            _advertisement: advertisement,
            _application: application,
            write_control: Box::pin(write_control),
            indicate_control: Box::pin(indicate_control),
        })
    }

    pub async fn accept(
        &mut self,
        expected_address: [u8; 6],
    ) -> Result<BluezBleGattLine, BluezBleGattError> {
        let expected_address = Address(expected_address);
        let mut reader = None;
        let mut writer = None;
        while reader.is_none() || writer.is_none() {
            tokio::select! {
                event = self.write_control.next(), if reader.is_none() => {
                    let request = match event {
                        Some(CharacteristicControlEvent::Write(request)) => request,
                        Some(CharacteristicControlEvent::Notify(_)) => {
                            return Err(BluezBleGattError::CharacteristicContractMismatch);
                        }
                        None => return Err(BluezBleGattError::Disconnected),
                    };
                    if request.device_address() != expected_address
                        || request.mtu() < usize::from(self.profile.maximum_gatt_packet_bytes)
                        || !self.peer_is_paired(expected_address).await?
                    {
                        return Err(BluezBleGattError::NotPaired);
                    }
                    reader = Some(
                        request
                            .accept()
                            .map_err(|_| BluezBleGattError::Transport)?,
                    );
                }
                event = self.indicate_control.next(), if writer.is_none() => {
                    let candidate = match event {
                        Some(CharacteristicControlEvent::Notify(candidate)) => candidate,
                        Some(CharacteristicControlEvent::Write(_)) => {
                            return Err(BluezBleGattError::CharacteristicContractMismatch);
                        }
                        None => return Err(BluezBleGattError::Disconnected),
                    };
                    if candidate.device_address() != expected_address
                        || candidate.mtu() < usize::from(self.profile.maximum_gatt_packet_bytes)
                        || !self.peer_is_paired(expected_address).await?
                    {
                        return Err(BluezBleGattError::NotPaired);
                    }
                    writer = Some(candidate);
                }
            }
        }
        Ok(BluezBleGattLine {
            address: expected_address,
            profile: self.profile,
            write: writer.expect("checked finite writer slot"),
            indicate: reader.expect("checked finite reader slot"),
            send_sequence: 0,
            reassembler: BleReassembler::new(self.profile),
        })
    }

    async fn peer_is_paired(&self, address: Address) -> Result<bool, BluezBleGattError> {
        self.adapter
            .device(address)
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?
            .is_paired()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)
    }
}

impl BluezBleGattLine {
    pub async fn connect(
        adapter_name: &str,
        address: [u8; 6],
        profile: BleGattProfile,
    ) -> Result<Self, BluezBleGattError> {
        let profile = profile
            .validate()
            .map_err(|_| BluezBleGattError::InvalidProfile)?;
        let session = Session::new()
            .await
            .map_err(|_| BluezBleGattError::BluetoothUnavailable)?;
        let adapter = session
            .adapter(adapter_name)
            .map_err(|_| BluezBleGattError::ControllerUnavailable)?;
        let address = Address(address);
        let device = adapter
            .device(address)
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
        if !device
            .is_paired()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?
        {
            return Err(BluezBleGattError::NotPaired);
        }
        device
            .connect()
            .await
            .map_err(|_| BluezBleGattError::ConnectFailed)?;

        let service_uuid = uuid::Uuid::from_bytes(CONDUIT_BLE_SERVICE_UUID);
        let write_uuid = uuid::Uuid::from_bytes(CONDUIT_BLE_WRITE_UUID);
        let notify_uuid = uuid::Uuid::from_bytes(CONDUIT_BLE_NOTIFY_UUID);
        let services = device
            .services()
            .await
            .map_err(|_| BluezBleGattError::MissingService)?;
        let mut service = None;
        for candidate in services.into_iter().take(MAXIMUM_GATT_OBJECTS_INSPECTED) {
            if candidate
                .uuid()
                .await
                .map_err(|_| BluezBleGattError::MissingService)?
                == service_uuid
            {
                service = Some(candidate);
                break;
            }
        }
        let service = service.ok_or(BluezBleGattError::MissingService)?;
        let characteristics = service
            .characteristics()
            .await
            .map_err(|_| BluezBleGattError::MissingService)?;
        let mut write_characteristic = None;
        let mut indicate_characteristic = None;
        for characteristic in characteristics
            .into_iter()
            .take(MAXIMUM_GATT_OBJECTS_INSPECTED)
        {
            let uuid = characteristic
                .uuid()
                .await
                .map_err(|_| BluezBleGattError::CharacteristicContractMismatch)?;
            if uuid == write_uuid {
                write_characteristic = Some(characteristic);
            } else if uuid == notify_uuid {
                indicate_characteristic = Some(characteristic);
            }
        }

        let write_characteristic =
            write_characteristic.ok_or(BluezBleGattError::MissingWriteCharacteristic)?;
        let indicate_characteristic =
            indicate_characteristic.ok_or(BluezBleGattError::MissingIndicateCharacteristic)?;
        validate_characteristics(&write_characteristic, &indicate_characteristic).await?;

        let write = write_characteristic
            .write_io()
            .await
            .map_err(|_| BluezBleGattError::CharacteristicContractMismatch)?;
        let indicate = indicate_characteristic
            .notify_io()
            .await
            .map_err(|_| BluezBleGattError::CharacteristicContractMismatch)?;
        let required_mtu = usize::from(profile.maximum_gatt_packet_bytes);
        if write.mtu() < required_mtu || indicate.mtu() < required_mtu {
            return Err(BluezBleGattError::MtuMismatch);
        }

        Ok(Self {
            address,
            profile,
            write,
            indicate,
            send_sequence: 0,
            reassembler: BleReassembler::new(profile),
        })
    }

    pub fn address(&self) -> [u8; 6] {
        self.address.0
    }

    pub fn profile(&self) -> BleGattProfile {
        self.profile
    }

    pub async fn send_frame(&mut self, frame: &[u8]) -> Result<(), BluezBleGattError> {
        let count = fragment_count(frame.len(), self.profile)
            .map_err(|_| BluezBleGattError::OversizedFrame)?;
        let mut packet = [0_u8; MAXIMUM_BLE_GATT_PACKET_BYTES];
        for index in 0..count {
            let length =
                encode_fragment(frame, self.send_sequence, index, self.profile, &mut packet)
                    .map_err(|_| BluezBleGattError::OversizedFrame)?;
            self.write
                .send(&packet[..length])
                .await
                .map_err(map_io_error)?;
        }
        self.send_sequence = self.send_sequence.wrapping_add(1);
        Ok(())
    }

    pub async fn receive_frame(&mut self, output: &mut [u8]) -> Result<usize, BluezBleGattError> {
        if output.len() < usize::try_from(self.profile.maximum_frame_bytes).unwrap_or(usize::MAX) {
            return Err(BluezBleGattError::OutputTooSmall);
        }
        let mut packet = [0_u8; 517];
        loop {
            let count = self
                .indicate
                .read(&mut packet)
                .await
                .map_err(map_io_error)?;
            if count == 0 {
                return Err(BluezBleGattError::Disconnected);
            }
            let completed = self
                .reassembler
                .admit(&packet[..count])
                .map_err(|_| BluezBleGattError::Transport)?;
            if let Some(frame) = completed {
                output[..frame.len()].copy_from_slice(frame);
                return Ok(frame.len());
            }
        }
    }
}

async fn validate_characteristics(
    write: &Characteristic,
    indicate: &Characteristic,
) -> Result<(), BluezBleGattError> {
    let write_flags = write
        .flags()
        .await
        .map_err(|_| BluezBleGattError::CharacteristicContractMismatch)?;
    let indicate_flags = indicate
        .flags()
        .await
        .map_err(|_| BluezBleGattError::CharacteristicContractMismatch)?;
    if !write_flags.write_without_response || !indicate_flags.notify {
        return Err(BluezBleGattError::CharacteristicContractMismatch);
    }
    Ok(())
}

fn map_io_error(error: std::io::Error) -> BluezBleGattError {
    match error.kind() {
        std::io::ErrorKind::BrokenPipe
        | std::io::ErrorKind::ConnectionAborted
        | std::io::ErrorKind::ConnectionReset
        | std::io::ErrorKind::NotConnected
        | std::io::ErrorKind::UnexpectedEof => BluezBleGattError::Disconnected,
        _ => BluezBleGattError::Transport,
    }
}
