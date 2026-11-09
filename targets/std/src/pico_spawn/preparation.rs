//! Bounded carriage over existing CDC frames; no authority or execution engine.
use super::*;
use conduit_body::{
    HostCommandChallenge, HOST_PREPARATION_REQUEST, HOST_PREPARED_WORKSET_CONTRACT,
};
use sha2::{Digest, Sha256};
use std::time::Instant;

/// Only bytes supplied by the independent owner approval boundary. This
/// transport descriptor itself proves no Source, Plan or signing authority.
pub struct PreparationCommandBytes<'a> {
    pub payload: &'a [u8],
    pub signature: &'a [u8; 64],
}
/// An observed device report, not a signed preparation/admission/Play receipt.
#[derive(Debug, PartialEq, Eq)]
pub struct ReportedPreparationInput {
    pub payload_sha256: [u8; 32],
}
#[derive(Debug, PartialEq, Eq)]
pub enum PreparationExchange {
    InputReported(ReportedPreparationInput),
    /// No new challenge or approval was requested; the previous input remains.
    Occupied(ReportedPreparationInput),
}
#[derive(Debug, PartialEq, Eq)]
pub enum PreparationTransportError {
    Bounds,
    Challenge,
    ApprovalRefused,
    BeforeSubmission,
    Deadline,
    /// Submission began. A lost/malformed reply never proves rollback.
    OutcomeUnknown,
}

// Whole-document bound is distinct from the unchanged per-frame bound.
const MAX_PREPARATION_CHUNKS: usize = 64;
const PREPARATION_FRAME_BYTES: usize = 512;
const MAX_PREPARATION_DOCUMENT_BYTES: usize = MAX_PREPARATION_CHUNKS
    * (PREPARATION_FRAME_BYTES - conduit_wire::BOUNDED_DOCUMENT_HEADER_BYTES);

trait Frames {
    fn send(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), ()>;
    fn receive<'a>(&mut self, output: &'a mut [u8], timeout: Duration) -> Result<&'a [u8], ()>;
}
impl Frames for NativePathCdcLine {
    fn send(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), ()> {
        self.send_raw_stream_frame(bytes, timeout).map_err(|_| ())
    }
    fn receive<'a>(&mut self, output: &'a mut [u8], timeout: Duration) -> Result<&'a [u8], ()> {
        self.receive_raw_stream_frame(output, timeout)
            .map_err(|_| ())
    }
}
impl PicoSpawnSocket {
    /// One original deadline covers request, challenge, owner approval, every
    /// chunk, signature and final report. This never retries or invokes Start.
    /// The callback must independently check current member/issuer facts and
    /// derive the signed document from the owner's checked Body proposal.
    pub fn exchange_preparation<'a>(
        &mut self,
        timeout: Duration,
        maximum_document_bytes: usize,
        approve: impl FnOnce(&HostCommandChallenge<'_>) -> Result<PreparationCommandBytes<'a>, ()>,
    ) -> Result<PreparationExchange, PreparationTransportError> {
        exchange(
            &mut self.line,
            timeout,
            maximum_document_bytes,
            approve,
            Instant::now,
        )
    }
}
fn left(
    deadline: Instant,
    clock: &impl Fn() -> Instant,
) -> Result<Duration, PreparationTransportError> {
    deadline
        .checked_duration_since(clock())
        .filter(|d| !d.is_zero())
        .ok_or(PreparationTransportError::Deadline)
}
fn exchange<'a>(
    frames: &mut impl Frames,
    timeout: Duration,
    maximum_document_bytes: usize,
    approve: impl FnOnce(&HostCommandChallenge<'_>) -> Result<PreparationCommandBytes<'a>, ()>,
    clock: impl Fn() -> Instant,
) -> Result<PreparationExchange, PreparationTransportError> {
    use PreparationTransportError as E;
    if maximum_document_bytes == 0 || maximum_document_bytes > MAX_PREPARATION_DOCUMENT_BYTES {
        return Err(E::Bounds);
    }
    let start = clock();
    let mut deadline = start + timeout.min(Duration::from_secs(10));
    frames
        .send(HOST_PREPARATION_REQUEST, left(deadline, &clock)?)
        .map_err(|_| E::BeforeSubmission)?;
    let mut response = [0u8; MAX_PICO_ADMISSION_FRAME_BYTES];
    let bytes = frames
        .receive(&mut response, left(deadline, &clock)?)
        .map_err(|_| E::BeforeSubmission)?;
    left(deadline, &clock)?;
    if let Ok(report) = reported_input(bytes) {
        return Ok(PreparationExchange::Occupied(report));
    }
    let challenge: HostCommandChallenge<'_> =
        serde_json::from_slice(bytes).map_err(|_| E::Challenge)?;
    let lifetime = challenge
        .expires_at_millis
        .checked_sub(challenge.issued_at_millis)
        .ok_or(E::Challenge)?;
    if !(1..=10_000).contains(&lifetime)
        || challenge.maximum_payload_bytes == 0
        || challenge.maximum_payload_bytes as usize > maximum_document_bytes
        || challenge.payload_contract != HOST_PREPARED_WORKSET_CONTRACT
    {
        return Err(E::Challenge);
    }
    // Compare durations only, never the receiver's clock epoch with ours.
    deadline = deadline.min(start + Duration::from_millis(lifetime));
    left(deadline, &clock)?;
    let command = approve(&challenge).map_err(|_| E::ApprovalRefused)?;
    if command.payload.is_empty()
        || command.payload.len() > challenge.maximum_payload_bytes as usize
    {
        return Err(E::Bounds);
    }
    left(deadline, &clock)?;
    let digest: [u8; 32] = Sha256::digest(command.payload).into();
    let mut frame = [0u8; PREPARATION_FRAME_BYTES];
    let mut offset = 0;
    let mut count = 0;
    while offset < command.payload.len() {
        let (length, next) = conduit_wire::encode_bounded_document_chunk(
            &mut frame,
            command.payload,
            offset,
            &digest,
        )
        .map_err(|_| E::OutcomeUnknown)?;
        count += 1;
        if count > MAX_PREPARATION_CHUNKS {
            return Err(E::OutcomeUnknown);
        }
        frames
            .send(
                &frame[..length],
                left(deadline, &clock).map_err(|_| E::OutcomeUnknown)?,
            )
            .map_err(|_| E::OutcomeUnknown)?;
        offset = next;
    }
    frames
        .send(
            command.signature,
            left(deadline, &clock).map_err(|_| E::OutcomeUnknown)?,
        )
        .map_err(|_| E::OutcomeUnknown)?;
    let bytes = frames
        .receive(
            &mut response,
            left(deadline, &clock).map_err(|_| E::OutcomeUnknown)?,
        )
        .map_err(|_| E::OutcomeUnknown)?;
    left(deadline, &clock).map_err(|_| E::OutcomeUnknown)?;
    let report = reported_input(bytes).map_err(|_| E::OutcomeUnknown)?;
    if report.payload_sha256 != digest {
        return Err(E::OutcomeUnknown);
    }
    Ok(PreparationExchange::InputReported(report))
}
fn reported_input(bytes: &[u8]) -> Result<ReportedPreparationInput, ()> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Report<'a> {
        phase: &'a str,
        custody: &'a str,
        payload_sha256: &'a str,
        resources: &'a str,
        start: &'a str,
    }
    let report: Report<'_> = serde_json::from_slice(bytes).map_err(|_| ())?;
    if report.phase != "input-retained"
        || report.custody != "ram"
        || report.resources != "not-admitted"
        || report.start != "not-started"
        || report.payload_sha256.len() != 64
    {
        return Err(());
    }
    let mut digest = [0u8; 32];
    let digit = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(()),
    };
    for (index, pair) in report
        .payload_sha256
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .enumerate()
    {
        digest[index] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(ReportedPreparationInput {
        payload_sha256: digest,
    })
}
#[cfg(test)]
#[path = "preparation_tests.rs"]
mod tests;
