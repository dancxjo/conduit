use std::time::Duration;

use bluer::{Device, Session};

use super::BluezBleGattError;

pub(super) struct PairingRetention {
    pub(super) agent: Option<bluer::agent::AgentHandle>,
}

pub(super) async fn prepare_agent(
    session: &Session,
    allow_pairing: bool,
) -> Result<Option<bluer::agent::AgentHandle>, BluezBleGattError> {
    if allow_pairing {
        Ok(Some(
            session
                // BlueZ selects the agent registered by the same application
                // that invokes Device.Pair; global default-agent authority is
                // neither needed nor appropriate for this exact realization.
                .register_agent(bluer::agent::Agent::default())
                .await
                .map_err(|error| BluezBleGattError::PairingFailed(error.to_string()))?,
        ))
    } else {
        Ok(None)
    }
}

pub(super) async fn prepare_device(
    device: &Device,
    agent: Option<bluer::agent::AgentHandle>,
    allow_pairing: bool,
) -> Result<PairingRetention, BluezBleGattError> {
    if device
        .is_paired()
        .await
        .map_err(|_| BluezBleGattError::DeviceUnavailable)?
    {
        return Ok(PairingRetention { agent });
    }
    if !allow_pairing {
        return Err(BluezBleGattError::NotPaired);
    }

    // BlueZ may complete controller encryption and publish the paired device
    // fact several seconds after Pair is issued on the Pico W controller.
    // Drive the one pairing future in this task so every failure path drops it
    // and therefore invokes BlueZ CancelPairing instead of detaching work.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let pairing = device.pair();
    tokio::pin!(pairing);
    let mut pair_call_complete = false;
    loop {
        if !pair_call_complete {
            tokio::select! {
                result = &mut pairing => {
                    result.map_err(|error| BluezBleGattError::PairingFailed(error.to_string()))?;
                    pair_call_complete = true;
                }
                () = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
        }
        let paired = device
            .is_paired()
            .await
            .map_err(|_| BluezBleGattError::DeviceUnavailable)?;
        if pair_call_complete && paired {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(BluezBleGattError::PairingFailed(
                "BlueZ pairing timed out before Pair completed with a paired fact".into(),
            ));
        }
        if pair_call_complete {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    Ok(PairingRetention { agent })
}
