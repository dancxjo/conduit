use std::time::Duration;

use bluer::{Adapter, Device, Session};

use super::BluezBleGattError;

pub(super) struct PairingRetention {
    pub(super) agent: Option<bluer::agent::AgentHandle>,
    pub(super) task: Option<tokio::task::JoinHandle<Result<(), bluer::Error>>>,
}

pub(super) async fn prepare_device(
    session: &Session,
    adapter: &Adapter,
    device: &Device,
    allow_pairing: bool,
) -> Result<PairingRetention, BluezBleGattError> {
    let agent = if allow_pairing {
        adapter
            .set_pairable(true)
            .await
            .map_err(|_| BluezBleGattError::ControllerUnavailable)?;
        Some(
            session
                .register_agent(bluer::agent::Agent {
                    request_default: true,
                    ..Default::default()
                })
                .await
                .map_err(|error| BluezBleGattError::PairingFailed(error.to_string()))?,
        )
    } else {
        None
    };
    if device
        .is_paired()
        .await
        .map_err(|_| BluezBleGattError::DeviceUnavailable)?
    {
        return Ok(PairingRetention { agent, task: None });
    }
    if !allow_pairing {
        return Err(BluezBleGattError::NotPaired);
    }

    // Own the selected physical connection before pairing. BlueZ may otherwise
    // create an ephemeral pairing connection and close it before the admitted
    // GATT Line can adopt the same link.
    device
        .connect()
        .await
        .map_err(|error| BluezBleGattError::ConnectFailed(error.to_string()))?;
    let pairing_device = device.clone();
    let mut task = Some(tokio::spawn(async move { pairing_device.pair().await }));
    // BlueZ may complete controller encryption and publish the paired device
    // fact several seconds after Pair is issued on the Pico W controller.
    // Keep the one retained operation slot alive for a finite physical bound.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        if task.as_ref().is_some_and(|task| task.is_finished()) {
            task.take()
                .expect("the finished pairing task remains present")
                .await
                .map_err(|error| BluezBleGattError::PairingFailed(error.to_string()))?
                .map_err(|error| BluezBleGattError::PairingFailed(error.to_string()))?;
            break;
        }
        let paired = device
            .is_paired()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
        let connected = device
            .is_connected()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
        if paired && connected {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            if let Some(task) = task.take() {
                task.abort();
            }
            return Err(BluezBleGattError::PairingFailed(
                "BlueZ pairing timed out without paired and connected facts".into(),
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(PairingRetention { agent, task })
}
