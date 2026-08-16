//! Bounded BlueZ observation of one exact expected Conduit peer.

use std::{collections::HashSet, time::Duration};

use bluer::{AdapterEvent, Address, Device, DiscoveryFilter, DiscoveryTransport, Session};
use conduit_bluetooth::CONDUIT_BLE_SERVICE_UUID;
use futures::StreamExt;

use super::{BluezBleGattCandidate, BluezBleGattError};

const MAXIMUM_DISCOVERY_EVENTS_INSPECTED: usize = 64;
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);

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
    if let Ok(device) = adapter.device(expected) {
        if device
            .is_connected()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?
        {
            return inspect_candidate(&device, service_uuid).await;
        }
    }
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
            return inspect_candidate(&device, service_uuid).await;
        }
        Err(BluezBleGattError::CandidateUnavailable)
    };
    tokio::time::timeout(DISCOVERY_TIMEOUT, scan)
        .await
        .map_err(|_| BluezBleGattError::CandidateUnavailable)?
}

async fn inspect_candidate(
    device: &Device,
    service_uuid: uuid::Uuid,
) -> Result<BluezBleGattCandidate, BluezBleGattError> {
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
    Ok(BluezBleGattCandidate {
        address: device.address().0,
        paired,
    })
}
