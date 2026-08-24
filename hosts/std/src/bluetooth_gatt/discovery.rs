//! Bounded BlueZ discovery, pairing, and explicit transport-loss operations.

use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

use bluer::{AdapterEvent, Address, DiscoveryFilter, DiscoveryTransport, Session};
use conduit_bluetooth::CONDUIT_BLE_SERVICE_UUID;
use futures::StreamExt;

use super::{BluezBleGattCandidate, BluezBleGattError};

const MAXIMUM_DISCOVERY_EVENTS_INSPECTED: usize = 256;
const MAXIMUM_CACHED_DEVICES_INSPECTED: usize = 32;
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);

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
    require_powered(&adapter).await?;
    let service_uuid = configure_discovery(&adapter).await?;
    let expected = Address(expected_address);
    let cached = adapter
        .device_addresses()
        .await
        .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
    if cached.contains(&expected) {
        let expected_device = adapter
            .device(expected)
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
        if expected_device
            .is_connected()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?
        {
            let uuids = expected_device
                .uuids()
                .await
                .map_err(|_| BluezBleGattError::DeviceUnavailable)?
                .unwrap_or_default();
            if !uuids.contains(&service_uuid) {
                return Err(BluezBleGattError::IncompatibleProfile);
            }
            return Ok(BluezBleGattCandidate {
                address: expected_address,
                paired: expected_device
                    .is_paired()
                    .await
                    .map_err(|_| BluezBleGattError::DeviceUnavailable)?,
            });
        }
        if let Some(candidate) = inspect_candidate(&adapter, expected, service_uuid).await? {
            return Ok(candidate);
        }
    }
    let scan = async {
        let events = adapter
            .discover_devices_with_changes()
            .await
            .map_err(|error| BluezBleGattError::DiscoveryUnavailable(error.to_string()))?;
        futures::pin_mut!(events);
        let mut inspection = tokio::time::interval(Duration::from_millis(100));
        let mut inspected_events = 0;
        while inspected_events < MAXIMUM_DISCOVERY_EVENTS_INSPECTED {
            tokio::select! {
                event = events.next() => match event {
                    Some(_) => inspected_events += 1,
                    None => return Err(BluezBleGattError::CandidateUnavailable),
                },
                _ = inspection.tick() => {}
            }
            // BlueZ can publish DeviceAdded before RSSI and UUID properties are
            // populated, or auto-connect a cached device without an adapter
            // add/remove event. Reinspect the one selected address after each
            // bounded event or timer tick instead of treating either as final.
            if let Some(candidate) = inspect_candidate(&adapter, expected, service_uuid).await? {
                return Ok(candidate);
            }
        }
        Err(BluezBleGattError::CandidateUnavailable)
    };
    tokio::time::timeout(DISCOVERY_TIMEOUT, scan)
        .await
        .map_err(|_| BluezBleGattError::CandidateUnavailable)?
}

/// Discover the one currently present compatible peer without promoting its
/// transient address into Plan identity. More than one candidate fails closed.
pub async fn discover_one_ble_gatt_candidate(
    adapter_name: &str,
) -> Result<BluezBleGattCandidate, BluezBleGattError> {
    let session = Session::new()
        .await
        .map_err(|_| BluezBleGattError::BluetoothUnavailable)?;
    let adapter = session
        .adapter(adapter_name)
        .map_err(|_| BluezBleGattError::ControllerUnavailable)?;
    require_powered(&adapter).await?;
    let service_uuid = configure_discovery(&adapter).await?;
    let mut candidate = None;
    for address in adapter
        .device_addresses()
        .await
        .map_err(|_| BluezBleGattError::DeviceUnavailable)?
        .into_iter()
        .take(MAXIMUM_CACHED_DEVICES_INSPECTED)
    {
        let Some(observed) = inspect_candidate(&adapter, address, service_uuid).await? else {
            continue;
        };
        if candidate.replace(observed).is_some() {
            return Err(BluezBleGattError::MultipleCandidates);
        }
    }
    if let Some(candidate) = candidate {
        return Ok(candidate);
    }
    let events = adapter
        .discover_devices_with_changes()
        .await
        .map_err(|error| BluezBleGattError::DiscoveryUnavailable(error.to_string()))?;
    futures::pin_mut!(events);
    let deadline = tokio::time::Instant::now() + DISCOVERY_TIMEOUT;
    let mut candidate = None;
    let mut recent = VecDeque::with_capacity(MAXIMUM_CACHED_DEVICES_INSPECTED);
    for _ in 0..MAXIMUM_DISCOVERY_EVENTS_INSPECTED {
        let event = match tokio::time::timeout_at(deadline, events.next()).await {
            Ok(Some(event)) => event,
            Ok(None) | Err(_) => break,
        };
        if let AdapterEvent::DeviceAdded(address) = event {
            if recent.len() == MAXIMUM_CACHED_DEVICES_INSPECTED {
                recent.pop_front();
            }
            recent.push_back(address);
        }
        candidate = None;
        let mut addresses = recent.iter().copied().collect::<HashSet<_>>();
        for address in adapter
            .device_addresses()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?
        {
            if addresses.len() == MAXIMUM_CACHED_DEVICES_INSPECTED {
                break;
            }
            addresses.insert(address);
        }
        for address in addresses {
            let Some(observed) = inspect_candidate(&adapter, address, service_uuid).await? else {
                continue;
            };
            if candidate.replace(observed).is_some() {
                return Err(BluezBleGattError::MultipleCandidates);
            }
        }
        if candidate.is_some() {
            break;
        }
    }
    candidate.ok_or(BluezBleGattError::CandidateUnavailable)
}

async fn inspect_candidate(
    adapter: &bluer::Adapter,
    address: Address,
    service_uuid: uuid::Uuid,
) -> Result<Option<BluezBleGattCandidate>, BluezBleGattError> {
    let device = adapter
        .device(address)
        .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
    let Ok(connected) = device.is_connected().await else {
        return Ok(None);
    };
    if !connected {
        let Ok(rssi) = device.rssi().await else {
            return Ok(None);
        };
        if rssi.is_none() {
            return Ok(None);
        }
    }
    let Ok(uuids) = device.uuids().await else {
        return Ok(None);
    };
    let uuids = uuids.unwrap_or_default();
    if !uuids.contains(&service_uuid) {
        return Ok(None);
    }
    Ok(Some(BluezBleGattCandidate {
        address: address.0,
        paired: device
            .is_paired()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?,
    }))
}

/// Ask BlueZ to pair one explicitly selected compatible observation. Pairing
/// remains a repository-development operation; it does not create a Plan.
pub async fn pair_ble_gatt_candidate(
    adapter_name: &str,
    address: [u8; 6],
) -> Result<BluezBleGattCandidate, BluezBleGattError> {
    let candidate = discover_ble_gatt_candidate(adapter_name, address).await?;
    let session = Session::new()
        .await
        .map_err(|_| BluezBleGattError::BluetoothUnavailable)?;
    let _agent = session
        .register_agent(bluer::agent::Agent::default())
        .await
        .map_err(|error| BluezBleGattError::PairingFailed(error.to_string()))?;
    let adapter = session
        .adapter(adapter_name)
        .map_err(|_| BluezBleGattError::ControllerUnavailable)?;
    let device = adapter
        .device(Address(address))
        .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
    if candidate.paired {
        return Ok(candidate);
    }
    if let Err(error) = device.pair().await {
        let _ = device.disconnect().await;
        return Err(BluezBleGattError::PairingFailed(error.to_string()));
    }
    if !device
        .is_paired()
        .await
        .map_err(|_| BluezBleGattError::DeviceUnavailable)?
    {
        let _ = device.disconnect().await;
        return Err(BluezBleGattError::PairingFailed(
            "BlueZ returned without a paired device fact".into(),
        ));
    }
    Ok(BluezBleGattCandidate {
        address,
        paired: true,
    })
}

/// Explicitly end the selected physical transport. This changes current Line
/// availability only; callers retain the immutable Plan as historical truth.
pub async fn disconnect_ble_gatt_candidate(
    adapter_name: &str,
    address: [u8; 6],
) -> Result<(), BluezBleGattError> {
    let session = Session::new()
        .await
        .map_err(|_| BluezBleGattError::BluetoothUnavailable)?;
    let adapter = session
        .adapter(adapter_name)
        .map_err(|_| BluezBleGattError::ControllerUnavailable)?;
    let device = adapter
        .device(Address(address))
        .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
    device
        .disconnect()
        .await
        .map_err(|_| BluezBleGattError::Disconnected)
}

async fn require_powered(adapter: &bluer::Adapter) -> Result<(), BluezBleGattError> {
    if adapter
        .is_powered()
        .await
        .map_err(|_| BluezBleGattError::ControllerUnavailable)?
    {
        Ok(())
    } else {
        Err(BluezBleGattError::ControllerPoweredOff)
    }
}

async fn configure_discovery(adapter: &bluer::Adapter) -> Result<uuid::Uuid, BluezBleGattError> {
    let service_uuid = uuid::Uuid::from_bytes(CONDUIT_BLE_SERVICE_UUID);
    adapter
        .set_discovery_filter(DiscoveryFilter {
            uuids: HashSet::from([service_uuid]),
            transport: DiscoveryTransport::Le,
            ..Default::default()
        })
        .await
        .map_err(|error| BluezBleGattError::DiscoveryUnavailable(error.to_string()))?;
    Ok(service_uuid)
}
