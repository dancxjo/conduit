use crate::external_websocket::{
    ExternalPeerId, ExternalWebSocketError, ExternalWebSocketListener,
};
use conduit_core::{ConfigurationValue, PlanFragment};
use std::net::SocketAddr;

pub(super) enum ExternalHostCompletion {
    Output,
    ReturnedInput,
    NoOutput,
    Disconnected,
}

pub(super) fn prepare(
    fragment: &PlanFragment,
) -> Result<Option<ExternalWebSocketListener>, String> {
    let Some(placement) = fragment.placements.iter().find(|placement| {
        placement.implementation_id.as_str() == "std/native-external-websocket-listener@1"
    }) else {
        return Ok(None);
    };
    let address = placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            ("bind", ConfigurationValue::Text(value)) => value.parse::<SocketAddr>().ok(),
            _ => None,
        })
        .ok_or_else(|| {
            "external WebSocket bind address is not an exact socket address".to_string()
        })?;
    ExternalWebSocketListener::bind(
        address,
        conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_PEERS,
        conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
    )
    .map(Some)
    .map_err(|error| format!("bind external WebSocket listener: {error:?}"))
}

pub(super) fn execute(
    contract: &str,
    input: &[u8],
    listener: &mut Option<ExternalWebSocketListener>,
    output: &mut Vec<u8>,
) -> Result<ExternalHostCompletion, String> {
    let listener = listener
        .as_mut()
        .ok_or_else(|| "external WebSocket request has no prepared listener".to_string())?;
    output.clear();
    match contract {
        conduit_net::EXTERNAL_WEBSOCKET_LISTENER_ACCEPT_HOST_OPERATION => {
            listener
                .accept_peer()
                .map_err(|error| format!("accept external WebSocket peer: {error:?}"))?;
            Ok(ExternalHostCompletion::NoOutput)
        }
        conduit_net::EXTERNAL_WEBSOCKET_LISTENER_RECEIVE_HOST_OPERATION => {
            let Some(peer) = input.first() else {
                return Err("external WebSocket receive command is malformed".to_string());
            };
            let peer_id = ExternalPeerId::from_index(u16::from(*peer));
            output.extend_from_slice(&[0, *peer]);
            output.resize(
                conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES as usize + 2,
                0,
            );
            match listener.receive_binary(peer_id, &mut output[2..]) {
                Ok(count) => {
                    let next = listener
                        .next_connected_after(peer_id)
                        .ok_or_else(|| "received from a missing listener peer".to_string())?;
                    output[0] = next.index() as u8;
                    output.truncate(count + 2);
                    Ok(ExternalHostCompletion::Output)
                }
                Err(ExternalWebSocketError::Disconnected) => {
                    output.clear();
                    if let Some(next) = listener.next_connected_after(peer_id) {
                        output.push(next.index() as u8);
                    }
                    Ok(ExternalHostCompletion::Disconnected)
                }
                Err(error) => Err(format!("receive external WebSocket message: {error:?}")),
            }
        }
        conduit_net::EXTERNAL_WEBSOCKET_LISTENER_SEND_HOST_OPERATION => {
            let Some(message) = input.get(2..) else {
                return Err("external WebSocket send command is malformed".to_string());
            };
            for peer in 0..conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_PEERS {
                match listener.send_binary(ExternalPeerId::from_index(peer), message) {
                    Ok(()) | Err(ExternalWebSocketError::UnknownPeer) => {}
                    Err(error) => {
                        return Err(format!("send external WebSocket message: {error:?}"));
                    }
                }
            }
            Ok(ExternalHostCompletion::ReturnedInput)
        }
        _ => Err("unsupported external WebSocket host-operation contract".to_string()),
    }
}
