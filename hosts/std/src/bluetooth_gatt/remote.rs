//! Exact remote BlueZ GATT realization after candidate selection.

use bluer::{Address, Session};
use conduit_bluetooth::{
    BleGattProfile, BleReassembler, CONDUIT_BLE_NOTIFY_UUID, CONDUIT_BLE_SERVICE_UUID,
    CONDUIT_BLE_WRITE_UUID,
};

use super::{
    pairing, validate_characteristics, BleGattIndicate, BleGattWrite, BluezBleGattError,
    BluezBleGattLine, MAXIMUM_GATT_OBJECTS_INSPECTED,
};

pub(super) async fn connect(
    adapter_name: &str,
    address: [u8; 6],
    profile: BleGattProfile,
    allow_pairing: bool,
) -> Result<BluezBleGattLine, BluezBleGattError> {
    let profile = profile
        .validate()
        .map_err(|_| BluezBleGattError::InvalidProfile)?;
    let session = Session::new()
        .await
        .map_err(|_| BluezBleGattError::BluetoothUnavailable)?;
    let adapter = session
        .adapter(adapter_name)
        .map_err(|_| BluezBleGattError::ControllerUnavailable)?;
    let restore_pairable = if allow_pairing {
        let was_pairable = adapter
            .is_pairable()
            .await
            .map_err(|_| BluezBleGattError::ControllerUnavailable)?;
        if !was_pairable {
            adapter
                .set_pairable(true)
                .await
                .map_err(|_| BluezBleGattError::ControllerUnavailable)?;
        }
        !was_pairable
    } else {
        false
    };
    let address = Address(address);
    let device = adapter
        .device(address)
        .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
    let result = async {
        // Install the exact headless pairing policy before the first physical
        // connection. Service resolution may itself encounter an encrypted
        // characteristic and must never race ahead of this process's agent.
        let agent = pairing::prepare_agent(&session, allow_pairing).await?;
        if !allow_pairing
            && !device
                .is_paired()
                .await
                .map_err(|_| BluezBleGattError::DeviceUnavailable)?
        {
            return Err(BluezBleGattError::NotPaired);
        }
        if !device
            .is_connected()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?
        {
            tokio::time::timeout(std::time::Duration::from_secs(20), device.connect())
                .await
                .map_err(|_| BluezBleGattError::ConnectFailed("BlueZ connect timed out".into()))?
                .map_err(|error| BluezBleGattError::ConnectFailed(error.to_string()))?;
        }

        let service_uuid = uuid::Uuid::from_bytes(CONDUIT_BLE_SERVICE_UUID);
        let write_uuid = uuid::Uuid::from_bytes(CONDUIT_BLE_WRITE_UUID);
        let notify_uuid = uuid::Uuid::from_bytes(CONDUIT_BLE_NOTIFY_UUID);
        let pairing = pairing::prepare_device(&device, agent, allow_pairing).await?;
        if !device
            .is_connected()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?
        {
            tokio::time::timeout(std::time::Duration::from_secs(20), device.connect())
                .await
                .map_err(|_| BluezBleGattError::ConnectFailed("BlueZ reconnect timed out".into()))?
                .map_err(|error| BluezBleGattError::ConnectFailed(error.to_string()))?;
        }
        // Resolve the encrypted GATT contract only after explicit pairing has
        // completed and BlueZ publishes paired+connected. Requiring service
        // resolution first can deadlock security-sensitive characteristics.
        let services = tokio::time::timeout(std::time::Duration::from_secs(20), device.services())
            .await
            .map_err(|_| BluezBleGattError::MissingService)?
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

        let write_mtu = write_characteristic
            .mtu()
            .await
            .map_err(|_| BluezBleGattError::MtuMismatch)?;
        let indicate_mtu = indicate_characteristic
            .mtu()
            .await
            .map_err(|_| BluezBleGattError::MtuMismatch)?;
        let required_mtu = usize::from(profile.maximum_gatt_packet_bytes);
        if write_mtu < required_mtu || indicate_mtu < required_mtu {
            return Err(BluezBleGattError::MtuMismatch);
        }
        let indicate = Box::pin(
            indicate_characteristic
                .notify()
                .await
                .map_err(|_| BluezBleGattError::CharacteristicContractMismatch)?,
        );

        Ok(BluezBleGattLine {
            _agent: pairing.agent,
            address,
            profile,
            write: BleGattWrite::Remote(write_characteristic),
            indicate: BleGattIndicate::Remote(indicate),
            send_sequence: 0,
            reassembler: BleReassembler::new(profile),
        })
    }
    .await;
    if allow_pairing && result.is_err() {
        // A failed Pair call can leave BlueZ's physical connection alive even
        // after the D-Bus operation is canceled. Release only this exact peer
        // within a finite cleanup bound so the peripheral can close honestly.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), device.disconnect()).await;
    }
    if restore_pairable {
        adapter
            .set_pairable(false)
            .await
            .map_err(|_| BluezBleGattError::ControllerUnavailable)?;
    }
    result
}
