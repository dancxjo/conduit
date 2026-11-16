//! One unsigned terminal diagnostic over the caller's original Root transport.
//! This is neither a Fore session message nor evidence of admission or Start.
use crate::transport::MessageTransport;

pub const MAXIMUM_BYTES: usize = 512;
const FALLBACK: &[u8] = b"{\"phase\":\"refused-before-play\",\"reason\":\"diagnostic-capacity\"}";

pub struct Details<'a> {
    pub reason: &'a str,
    pub detail: &'a str,
    pub heap_used_bytes: usize,
    pub heap_peak_bytes: Option<usize>,
    pub heap_limit_bytes: usize,
}
#[derive(serde::Serialize)]
struct Status<'a> {
    phase: &'static str,
    reason: &'a str,
    detail: &'a str,
    heap_used_bytes: usize,
    heap_peak_bytes: Option<usize>,
    heap_limit_bytes: usize,
}

/// Caller has proved an initial preparation refusal and ended that failed
/// future's carrier loan. Reuse existing response storage, capped at 512 bytes.
/// Exactly one send is attempted, including on encoding failure. No receive,
/// reset, pump, allocation, retry, release or new execution authority occurs.
pub async fn send_once<T: MessageTransport>(
    transport: &mut T,
    response: &mut [u8],
    details: &Details<'_>,
) -> Result<(), T::Error> {
    let limit = response.len().min(MAXIMUM_BYTES);
    let encoded = serde_json_core::to_slice(
        &Status {
            phase: "refused-before-play",
            reason: details.reason,
            detail: details.detail,
            heap_used_bytes: details.heap_used_bytes,
            heap_peak_bytes: details.heap_peak_bytes,
            heap_limit_bytes: details.heap_limit_bytes,
        },
        &mut response[..limit],
    );
    match encoded {
        Ok(length) => transport.send(&response[..length]).await,
        Err(_) => transport.send(FALLBACK).await,
    }
}

#[cfg(test)]
#[path = "preparation_refusal/tests.rs"]
mod tests;
