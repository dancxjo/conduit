//! Live terminal and browser event collection for the exact three-peer R1 Form.

use std::io::BufRead;
use std::net::SocketAddr;

use crate::r1_control::{R1ControlPeer, R1InputEvent, R1MergedInput};

pub fn run_live_three_peer_events(
    bind: &str,
    ready: impl FnOnce(SocketAddr) -> Result<(), String>,
    mut deliver: impl FnMut(R1InputEvent) -> Result<R1MergedInput, String>,
) -> Result<(), String> {
    let address = bind
        .parse()
        .map_err(|error| format!("invalid R1 input bind address: {error}"))?;
    let mut listener = crate::external_websocket::ExternalWebSocketListener::bind(address, 2, 10)
        .map_err(|error| format!("R1 browser input listener: {error:?}"))?;
    ready(listener.local_addr().map_err(debug_error)?)?;

    let browser_a = listener.accept_peer().map_err(debug_error)?;
    let browser_b = listener.accept_peer().map_err(debug_error)?;
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    for (peer_sequence, expected) in [(0, "down"), (1, "up")] {
        let line = lines
            .next()
            .ok_or_else(|| format!("terminal input ended before {expected}"))?
            .map_err(debug_error)?;
        if line.trim() != expected {
            return Err(format!("terminal input expected '{expected}'"));
        }
        deliver(R1InputEvent {
            peer: R1ControlPeer::Terminal,
            peer_sequence,
            level: expected == "down",
        })?;
    }

    for (socket, peer) in [
        (browser_a, R1ControlPeer::BrowserA),
        (browser_b, R1ControlPeer::BrowserB),
    ] {
        for peer_sequence in 0..2 {
            let mut frame = [0_u8; 10];
            let length = listener
                .receive_binary(socket, &mut frame)
                .map_err(debug_error)?;
            let input = decode_browser_event(&frame[..length], peer)?;
            if input.peer_sequence != peer_sequence {
                return Err(format!("browser peer sequence expected {peer_sequence}"));
            }
            let merged = deliver(input)?;
            let encoded = conduit_signal::encode_signal_fixed(&merged.signal);
            listener
                .send_binary(socket, &encoded)
                .map_err(debug_error)?;
        }
        listener.disconnect(socket).map_err(debug_error)?;
    }
    Ok(())
}

fn decode_browser_event(bytes: &[u8], expected: R1ControlPeer) -> Result<R1InputEvent, String> {
    let [peer, sequence @ .., level] = bytes else {
        return Err("browser input frame must be exactly ten bytes".into());
    };
    let actual = match *peer {
        1 => R1ControlPeer::BrowserA,
        2 => R1ControlPeer::BrowserB,
        _ => return Err("browser input frame has an unknown peer".into()),
    };
    if actual != expected || (*level != 0 && *level != 1) {
        return Err("browser input frame peer or level mismatch".into());
    }
    let sequence: [u8; 8] = sequence
        .try_into()
        .map_err(|_| "browser input frame sequence width")?;
    Ok(R1InputEvent {
        peer: actual,
        peer_sequence: u64::from_le_bytes(sequence),
        level: *level == 1,
    })
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_frames_bind_exact_peer_width_sequence_and_level() {
        let mut valid = [0_u8; 10];
        valid[0] = 1;
        valid[1..9].copy_from_slice(&7_u64.to_le_bytes());
        valid[9] = 1;
        assert_eq!(
            decode_browser_event(&valid, R1ControlPeer::BrowserA),
            Ok(R1InputEvent {
                peer: R1ControlPeer::BrowserA,
                peer_sequence: 7,
                level: true,
            })
        );
        assert!(decode_browser_event(&valid[..9], R1ControlPeer::BrowserA).is_err());
        assert!(decode_browser_event(&valid, R1ControlPeer::BrowserB).is_err());
        valid[0] = 9;
        assert!(decode_browser_event(&valid, R1ControlPeer::BrowserA).is_err());
        valid[0] = 1;
        valid[9] = 2;
        assert!(decode_browser_event(&valid, R1ControlPeer::BrowserA).is_err());
    }
}
