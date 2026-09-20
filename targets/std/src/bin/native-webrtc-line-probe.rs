//! One-shot browser-to-native proof entrance for the generic WebRTC Base.

use std::io::{BufRead, Write};
use std::time::Duration;

use conduit_std_host::native_webrtc::NativeWebRtcEndpoint;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Offer {
    sdp: String,
}

#[derive(Serialize)]
struct Answer<'a> {
    kind: &'static str,
    sdp: &'a str,
}

#[derive(Serialize)]
struct Receipt {
    kind: &'static str,
    implementation_id: &'static str,
    received_bytes: usize,
    disposition: &'static str,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), String> {
    let line = std::io::stdin()
        .lock()
        .lines()
        .next()
        .ok_or_else(|| "native WebRTC probe received no offer".to_string())?
        .map_err(|error| format!("read browser offer: {error}"))?;
    if line.len() > 4_192 {
        return Err("browser offer exceeds signaling bound".into());
    }
    let offer: Offer =
        serde_json::from_str(&line).map_err(|error| format!("decode browser offer: {error}"))?;
    let timeout = Duration::from_secs(10);
    let mut answer = NativeWebRtcEndpoint::answer(None, 0, timeout, offer.sdp)
        .await
        .map_err(|error| format!("answer browser offer: {error:?}"))?;
    println!(
        "{}",
        serde_json::to_string(&Answer {
            kind: "answer",
            sdp: &answer.sdp,
        })
        .map_err(|error| format!("encode native answer: {error}"))?
    );
    std::io::stdout()
        .flush()
        .map_err(|error| format!("flush native answer: {error}"))?;
    answer
        .endpoint
        .await_open()
        .await
        .map_err(|error| format!("open native DataChannel: {error:?}"))?;
    let mut frame = [0_u8; 256];
    let received = answer
        .endpoint
        .receive(&mut frame)
        .await
        .map_err(|error| format!("receive browser frame: {error:?}"))?;
    answer
        .endpoint
        .send(&frame[..received])
        .await
        .map_err(|error| format!("echo browser frame: {error:?}"))?;
    let acknowledgement = answer
        .endpoint
        .receive(&mut frame)
        .await
        .map_err(|error| format!("receive browser delivery acknowledgement: {error:?}"))?;
    if &frame[..acknowledgement] != b"delivered" {
        return Err("browser delivery acknowledgement was not exact".into());
    }
    println!(
        "{}",
        serde_json::to_string(&Receipt {
            kind: "receipt",
            implementation_id: conduit_std_host::native_webrtc::NATIVE_WEBRTC_IMPLEMENTATION_ID,
            received_bytes: received,
            disposition: "line-ready-value-echoed",
        })
        .map_err(|error| format!("encode native receipt: {error}"))?
    );
    std::io::stdout()
        .flush()
        .map_err(|error| format!("flush native receipt: {error}"))?;
    // This one-shot proof fixture has no next session to retain. Dropping the
    // endpoint fences the exact negotiation; process termination is its
    // externally observable Host-loss boundary.
    drop(answer.endpoint);
    std::process::exit(0)
}
