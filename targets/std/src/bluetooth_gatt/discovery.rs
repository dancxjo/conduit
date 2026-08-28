//! Bounded BlueZ discovery, pairing, and explicit transport-loss operations.

use std::{collections::HashSet, time::Duration};

use bluer::{AdapterEvent, Address, DiscoveryFilter, DiscoveryTransport, Session};
use conduit_bluetooth::CONDUIT_BLE_SERVICE_UUID;
use futures::StreamExt;

use super::{BluezBleGattCandidate, BluezBleGattError};

const MAXIMUM_DISCOVERY_EVENTS_INSPECTED: usize = 256;
const MAXIMUM_CACHED_DEVICES_INSPECTED: usize = 32;
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);
const AMBIGUITY_WINDOW: Duration = Duration::from_secs(2);

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
    }
    let mut initially_known = cached
        .into_iter()
        .filter(|address| *address == expected)
        .collect::<HashSet<_>>();
    let scan = async {
        let events = adapter
            .discover_devices_with_changes()
            .await
            .map_err(|error| BluezBleGattError::DiscoveryUnavailable(error.to_string()))?;
        futures::pin_mut!(events);
        let mut inspected_events = 0;
        while inspected_events < MAXIMUM_DISCOVERY_EVENTS_INSPECTED {
            match events.next().await {
                Some(AdapterEvent::DeviceAdded(address)) => {
                    inspected_events += 1;
                    if address != expected || !is_fresh_observation(&mut initially_known, address) {
                        continue;
                    }
                    if let Some(candidate) =
                        inspect_candidate(&adapter, expected, service_uuid).await?
                    {
                        return Ok(candidate);
                    }
                }
                Some(_) => inspected_events += 1,
                None => return Err(BluezBleGattError::CandidateUnavailable),
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
    let initially_known = adapter
        .device_addresses()
        .await
        .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
    if initially_known.len() > MAXIMUM_CACHED_DEVICES_INSPECTED {
        return Err(BluezBleGattError::DiscoveryCapacityExceeded);
    }
    let mut candidate = None;
    for address in initially_known
        .iter()
        .copied()
        .take(MAXIMUM_CACHED_DEVICES_INSPECTED)
    {
        let device = adapter
            .device(address)
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
        if !device
            .is_connected()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?
        {
            continue;
        }
        let Some(observed) = inspect_candidate(&adapter, address, service_uuid).await? else {
            continue;
        };
        admit_candidate(&mut candidate, observed)?;
    }
    let mut initially_known = initially_known
        .into_iter()
        .take(MAXIMUM_CACHED_DEVICES_INSPECTED)
        .collect::<HashSet<_>>();
    let scan = async {
        let events = adapter
            .discover_devices_with_changes()
            .await
            .map_err(|error| BluezBleGattError::DiscoveryUnavailable(error.to_string()))?;
        futures::pin_mut!(events);
        let mut ambiguity_deadline = candidate
            .as_ref()
            .map(|_| tokio::time::Instant::now() + AMBIGUITY_WINDOW);
        for _ in 0..MAXIMUM_DISCOVERY_EVENTS_INSPECTED {
            let event = if let Some(deadline) = ambiguity_deadline {
                match tokio::time::timeout_at(deadline, events.next()).await {
                    Ok(Some(event)) => event,
                    Ok(None) | Err(_) => {
                        return candidate.ok_or(BluezBleGattError::CandidateUnavailable);
                    }
                }
            } else {
                match events.next().await {
                    Some(event) => event,
                    None => return Err(BluezBleGattError::CandidateUnavailable),
                }
            };
            if let AdapterEvent::DeviceAdded(address) = event {
                if !is_fresh_observation(&mut initially_known, address) {
                    continue;
                }
                let Some(observed) = inspect_candidate(&adapter, address, service_uuid).await?
                else {
                    continue;
                };
                let first = candidate.is_none();
                admit_candidate(&mut candidate, observed)?;
                if first {
                    ambiguity_deadline = Some(tokio::time::Instant::now() + AMBIGUITY_WINDOW);
                }
            }
        }
        candidate.ok_or(BluezBleGattError::CandidateUnavailable)
    };
    tokio::time::timeout(DISCOVERY_TIMEOUT, scan)
        .await
        .map_err(|_| BluezBleGattError::CandidateUnavailable)?
}

/// `discover_devices_with_changes` first replays every known BlueZ device,
/// including peers that are no longer in range. A second event for a known
/// address (or the first event for a new address) is the finite evidence that
/// this discovery session observed it.
fn is_fresh_observation(initially_known: &mut HashSet<Address>, address: Address) -> bool {
    !initially_known.remove(&address)
}

fn admit_candidate(
    slot: &mut Option<BluezBleGattCandidate>,
    observed: BluezBleGattCandidate,
) -> Result<(), BluezBleGattError> {
    match slot {
        Some(current) if current.address != observed.address => {
            Err(BluezBleGattError::MultipleCandidates)
        }
        Some(_) => Ok(()),
        None => {
            *slot = Some(observed);
            Ok(())
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_replay_is_not_current_reachability() {
        let address = Address([1, 2, 3, 4, 5, 6]);
        let mut initially_known = HashSet::from([address]);

        assert!(!is_fresh_observation(&mut initially_known, address));
        assert!(is_fresh_observation(&mut initially_known, address));
    }

    #[test]
    fn first_observation_of_new_address_is_current() {
        let mut initially_known = HashSet::new();

        assert!(is_fresh_observation(
            &mut initially_known,
            Address([6, 5, 4, 3, 2, 1])
        ));
    }

    #[test]
    fn two_distinct_fresh_candidates_fail_closed() {
        let mut candidate = None;
        admit_candidate(
            &mut candidate,
            BluezBleGattCandidate {
                address: [1, 2, 3, 4, 5, 6],
                paired: false,
            },
        )
        .unwrap();

        assert_eq!(
            admit_candidate(
                &mut candidate,
                BluezBleGattCandidate {
                    address: [6, 5, 4, 3, 2, 1],
                    paired: true,
                },
            ),
            Err(BluezBleGattError::MultipleCandidates)
        );
    }

    #[test]
    fn repeated_observation_of_same_candidate_is_not_ambiguous() {
        let observed = BluezBleGattCandidate {
            address: [1, 2, 3, 4, 5, 6],
            paired: false,
        };
        let mut candidate = Some(observed);

        assert_eq!(admit_candidate(&mut candidate, observed), Ok(()));
    }
}
