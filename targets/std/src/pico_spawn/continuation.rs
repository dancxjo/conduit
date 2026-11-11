//! Finite carriage of owner bytes and an unverified original Root response.
use super::{preparation::Frames, PicoSpawnSocket};
use conduit_body::continuation_artifact::{ARTIFACT_BYTES, PREFIX_BYTES};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

// Root's existing selector (host-runtime::root::CONTINUATION_INSTALL_REQUEST).
// Deliberately no MCU runtime dependency in the Linux controller.
const INSTALL_REQUEST: &[u8] = b"conduit.host/install-continuation@1";
// Match the existing Root stream ceiling, including Root response chunks.
const FRAME_BYTES: usize = conduit_body::MAX_PICO_ADMISSION_FRAME_BYTES;
const MAX_CHUNKS: usize = 32;

/// Transport-checked bytes only. The controller has NOT verified Root's
/// acceptance signature, installed pins, authority, admission or execution.
/// Actual Root boot must independently verify the selected physical prefix.
#[derive(Debug, PartialEq, Eq)]
pub struct UnverifiedContinuationPrefix {
    pub prefix: [u8; PREFIX_BYTES],
}

#[derive(Debug, PartialEq, Eq)]
pub enum ContinuationInstallTransportError {
    /// The original deadline elapsed before sending any request.
    Deadline,
    /// A request send was attempted. Partial carriage or selection may have
    /// happened: a missing, malformed or late reply never proves rollback.
    OutcomeUnknown,
}

impl PicoSpawnSocket {
    /// Submit exactly one owner-exported candidate and return an UNVERIFIED
    /// prefix received through the existing length-prefixed CDC stream. One
    /// original deadline, capped at ten seconds, covers all frames. No retries,
    /// flash writes or Start occur here. Success checks transport digest and
    /// unchanged owner artifact bytes only; it confers no approval or authority.
    pub fn exchange_continuation_install(
        &mut self,
        timeout: Duration,
        candidate: &[u8; PREFIX_BYTES],
    ) -> Result<UnverifiedContinuationPrefix, ContinuationInstallTransportError> {
        exchange(&mut self.line, timeout, candidate, Instant::now)
    }
}

fn exchange(
    frames: &mut impl Frames,
    timeout: Duration,
    candidate: &[u8; PREFIX_BYTES],
    clock: impl Fn() -> Instant,
) -> Result<UnverifiedContinuationPrefix, ContinuationInstallTransportError> {
    use ContinuationInstallTransportError as E;
    let deadline = clock() + timeout.min(Duration::from_secs(10));
    let left = || {
        deadline
            .checked_duration_since(clock())
            .filter(|d| !d.is_zero())
            .ok_or(E::OutcomeUnknown)
    };
    let initial = left().map_err(|_| E::Deadline)?;
    // Even an unsuccessful write can have partially reached Root.
    frames
        .send(INSTALL_REQUEST, initial)
        .map_err(|_| E::OutcomeUnknown)?;
    let digest: [u8; 32] = Sha256::digest(candidate).into();
    let mut frame = [0; FRAME_BYTES];
    let mut offset = 0;
    let mut count = 0;
    while offset < PREFIX_BYTES {
        let (length, next) =
            conduit_wire::encode_bounded_document_chunk(&mut frame, candidate, offset, &digest)
                .map_err(|_| E::OutcomeUnknown)?;
        count += 1;
        if count > MAX_CHUNKS {
            return Err(E::OutcomeUnknown);
        }
        frames
            .send(&frame[..length], left()?)
            .map_err(|_| E::OutcomeUnknown)?;
        offset = next;
    }
    let mut prefix = [0; PREFIX_BYTES];
    let mut assembly = conduit_wire::BorrowedBoundedDocumentAssembly::new(&mut prefix, MAX_CHUNKS);
    for _ in 0..MAX_CHUNKS {
        let bytes = frames
            .receive(&mut frame, left()?)
            .map_err(|_| E::OutcomeUnknown)?;
        left()?;
        if assembly.push(bytes).map_err(|_| E::OutcomeUnknown)? {
            let (bytes, digest) = assembly.completed().ok_or(E::OutcomeUnknown)?;
            if bytes.len() != PREFIX_BYTES
                || Sha256::digest(bytes).as_slice() != digest
                || bytes[..ARTIFACT_BYTES] != candidate[..ARTIFACT_BYTES]
            {
                return Err(E::OutcomeUnknown);
            }
            left()?;
            return Ok(UnverifiedContinuationPrefix { prefix });
        }
    }
    Err(E::OutcomeUnknown)
}

#[cfg(test)]
#[path = "continuation_tests.rs"]
mod tests;
