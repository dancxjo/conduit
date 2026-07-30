//! Bounded hosted framing for `conduct --run --format=ndjson`.
//!
//! Channel records preserve compatibility I/O bytes only. They deliberately
//! do not claim semantic value, port, transaction, or evidence identity.

use std::fmt;
use std::io::{self, Write};

use conduit_runtime::{ExactEvidenceRecord, ExecutionSummary, OwnedExecutionEvent};
use serde::Serialize;

pub const RUN_STREAM_SCHEMA: &str = "conduit.run/v2";
pub const RUN_STREAM_SCHEMA_VERSION: u16 = 2;
pub const RUN_CHANNEL_CHUNK_MAX_BYTES: usize = 4_096;
pub const RUN_CHANNEL_CHUNK_MAX_HEX_BYTES: usize = RUN_CHANNEL_CHUNK_MAX_BYTES * 2;
pub const RUN_CHANNEL_RECORD_MAX_BYTES: usize = 8_448;
pub const RUN_SUMMARY_RECORD_MAX_BYTES: usize = 512;
pub const RUN_STRUCTURED_RECORD_MAX_BYTES: usize = 65_536;

const WITHDRAWN_RUN_STREAM_SCHEMA: &str = "conduit.run/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStreamVersionError {
    WithdrawnV1,
    Unsupported,
}

impl fmt::Display for RunStreamVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WithdrawnV1 => formatter.write_str("conduit.run/v1 was withdrawn before release"),
            Self::Unsupported => formatter.write_str("unsupported conduit run-stream version"),
        }
    }
}

impl std::error::Error for RunStreamVersionError {}

/// Validate the outer transport identity without falling back to another
/// version. Version 1 remains recognizable only so consumers can reject the
/// deliberately withdrawn contract distinctly.
pub fn validate_run_stream_version(
    schema: &str,
    schema_version: u16,
) -> Result<(), RunStreamVersionError> {
    if schema == RUN_STREAM_SCHEMA && schema_version == RUN_STREAM_SCHEMA_VERSION {
        Ok(())
    } else if schema == WITHDRAWN_RUN_STREAM_SCHEMA && schema_version == 1 {
        Err(RunStreamVersionError::WithdrawnV1)
    } else {
        Err(RunStreamVersionError::Unsupported)
    }
}

#[derive(Serialize)]
struct RunChannelChunkRecord<'a> {
    schema: &'static str,
    schema_version: u16,
    sequence: u64,
    record: &'static str,
    channel: &'static str,
    encoding: &'static str,
    payload_bytes: usize,
    payload_hex: &'a str,
}

#[derive(Serialize)]
struct RunSummaryRecord {
    schema: &'static str,
    schema_version: u16,
    sequence: u64,
    record: &'static str,
    nodes_completed: usize,
    cords_conducted: usize,
}

#[derive(Serialize)]
struct RunExecutionEventRecord<'a> {
    schema: &'static str,
    schema_version: u16,
    sequence: u64,
    record: &'static str,
    event: &'a OwnedExecutionEvent,
}

#[derive(Serialize)]
struct RunExactEvidenceRecord<'a> {
    schema: &'static str,
    schema_version: u16,
    sequence: u64,
    record: &'static str,
    evidence: &'a ExactEvidenceRecord,
}

/// One globally ordered run-stream encoder.
pub struct RunNdjsonState<W> {
    pub inner: W,
    sequence: u64,
}

impl<W: Write> RunNdjsonState<W> {
    pub const fn new(inner: W) -> Self {
        Self { inner, sequence: 0 }
    }

    pub fn write_summary(&mut self, summary: ExecutionSummary) -> io::Result<()> {
        let next_sequence = checked_next_sequence(self.sequence)?;
        let record = RunSummaryRecord {
            schema: RUN_STREAM_SCHEMA,
            schema_version: RUN_STREAM_SCHEMA_VERSION,
            sequence: self.sequence,
            record: "summary",
            nodes_completed: summary.nodes_completed,
            cords_conducted: summary.cords_conducted,
        };
        self.write_record(&record, RUN_SUMMARY_RECORD_MAX_BYTES)?;
        self.sequence = next_sequence;
        Ok(())
    }

    pub fn write_channel_chunk(&mut self, channel: &'static str, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        if bytes.len() > RUN_CHANNEL_CHUNK_MAX_BYTES {
            return Err(invalid_data("run-stream-channel-chunk-too-large"));
        }

        let next_sequence = checked_next_sequence(self.sequence)?;
        let payload_hex = encode_hex_bounded(bytes)?;
        let record = RunChannelChunkRecord {
            schema: RUN_STREAM_SCHEMA,
            schema_version: RUN_STREAM_SCHEMA_VERSION,
            sequence: self.sequence,
            record: "channel_chunk",
            channel,
            encoding: "hex",
            payload_bytes: bytes.len(),
            payload_hex: &payload_hex,
        };
        self.write_record(&record, RUN_CHANNEL_RECORD_MAX_BYTES)?;
        self.sequence = next_sequence;
        Ok(())
    }

    /// Direct structured path for immutable executor-owned evidence.
    ///
    /// The evidence is nested without being reconstructed from channel bytes.
    /// A future typed-publication record must follow the same dedicated path
    /// rather than passing through `write_channel_chunk`.
    pub fn write_execution_event(&mut self, event: &OwnedExecutionEvent) -> io::Result<()> {
        let next_sequence = checked_next_sequence(self.sequence)?;
        let record = RunExecutionEventRecord {
            schema: RUN_STREAM_SCHEMA,
            schema_version: RUN_STREAM_SCHEMA_VERSION,
            sequence: self.sequence,
            record: "execution_event",
            event,
        };
        self.write_record(&record, RUN_STRUCTURED_RECORD_MAX_BYTES)?;
        self.sequence = next_sequence;
        Ok(())
    }

    /// Direct bounded path for exact executor evidence.
    pub fn write_exact_evidence(&mut self, evidence: &ExactEvidenceRecord) -> io::Result<()> {
        let next_sequence = checked_next_sequence(self.sequence)?;
        let record = RunExactEvidenceRecord {
            schema: RUN_STREAM_SCHEMA,
            schema_version: RUN_STREAM_SCHEMA_VERSION,
            sequence: self.sequence,
            record: "exact_execution_evidence",
            evidence,
        };
        self.write_record(&record, RUN_STRUCTURED_RECORD_MAX_BYTES)?;
        self.sequence = next_sequence;
        Ok(())
    }

    fn write_record(&mut self, record: &impl Serialize, maximum: usize) -> io::Result<()> {
        let mut encoded = BoundedBuffer::new(maximum);
        serde_json::to_writer(&mut encoded, record).map_err(|error| {
            if encoded.limit_reached {
                invalid_data("run-stream-record-too-large")
            } else {
                io::Error::other(error)
            }
        })?;
        encoded.write_all(b"\n")?;
        self.inner.write_all(&encoded.bytes)
    }
}

fn checked_next_sequence(sequence: u64) -> io::Result<u64> {
    sequence
        .checked_add(1)
        .ok_or_else(|| invalid_data("run-stream-sequence-overflow"))
}

fn encode_hex_bounded(bytes: &[u8]) -> io::Result<String> {
    let encoded_len = checked_hex_len(bytes.len())?;

    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::new();
    output
        .try_reserve_exact(encoded_len)
        .map_err(|_| invalid_data("run-stream-payload-allocation-failed"))?;
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(output)
}

fn checked_hex_len(decoded_len: usize) -> io::Result<usize> {
    let encoded_len = decoded_len
        .checked_mul(2)
        .ok_or_else(|| invalid_data("run-stream-payload-size-overflow"))?;
    if decoded_len > RUN_CHANNEL_CHUNK_MAX_BYTES {
        return Err(invalid_data("run-stream-channel-chunk-too-large"));
    }
    if encoded_len > RUN_CHANNEL_CHUNK_MAX_HEX_BYTES {
        return Err(invalid_data("run-stream-payload-size-overflow"));
    }
    Ok(encoded_len)
}

fn invalid_data(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason)
}

struct BoundedBuffer {
    bytes: Vec<u8>,
    maximum: usize,
    limit_reached: bool,
}

impl BoundedBuffer {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(maximum),
            maximum,
            limit_reached: false,
        }
    }
}

impl Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(next_len) = self.bytes.len().checked_add(bytes.len()) else {
            self.limit_reached = true;
            return Err(invalid_data("run-stream-record-size-overflow"));
        };
        if next_len > self.maximum {
            self.limit_reached = true;
            return Err(invalid_data("run-stream-record-too-large"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_policy_accepts_only_v2() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/c3/conduct-run-stream-v2.json"
        ))
        .unwrap();
        for case in fixture["version_cases"].as_array().unwrap() {
            let expected = match case["expected"]["reason"].as_str() {
                None => Ok(()),
                Some("withdrawn-v1") => Err(RunStreamVersionError::WithdrawnV1),
                Some("unsupported-version") => Err(RunStreamVersionError::Unsupported),
                reason => panic!("unexpected fixture reason {reason:?}"),
            };
            assert_eq!(
                validate_run_stream_version(
                    case["schema"].as_str().unwrap(),
                    case["schema_version"].as_u64().unwrap() as u16
                ),
                expected,
                "{}",
                case["id"]
            );
        }
    }

    #[test]
    fn implementation_limits_match_the_conformance_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/c3/conduct-run-stream-v2.json"
        ))
        .unwrap();
        let limits = &fixture["limits"];
        assert_eq!(limits["decoded_chunk_bytes"], RUN_CHANNEL_CHUNK_MAX_BYTES);
        assert_eq!(
            limits["encoded_payload_bytes"],
            RUN_CHANNEL_CHUNK_MAX_HEX_BYTES
        );
        assert_eq!(
            limits["serialized_channel_record_bytes"],
            RUN_CHANNEL_RECORD_MAX_BYTES
        );
        assert_eq!(
            limits["serialized_summary_record_bytes"],
            RUN_SUMMARY_RECORD_MAX_BYTES
        );
        assert_eq!(
            limits["serialized_structured_record_bytes"],
            RUN_STRUCTURED_RECORD_MAX_BYTES
        );
    }

    #[test]
    fn exact_maximum_chunk_fits_the_declared_record_ceiling() {
        let mut stream = RunNdjsonState::new(Vec::new());
        stream
            .write_channel_chunk("stderr", &[0xff; RUN_CHANNEL_CHUNK_MAX_BYTES])
            .unwrap();
        assert!(stream.inner.len() <= RUN_CHANNEL_RECORD_MAX_BYTES);

        let record: serde_json::Value = serde_json::from_slice(&stream.inner).unwrap();
        assert_eq!(record["record"], "channel_chunk");
        assert_eq!(record["payload_bytes"], RUN_CHANNEL_CHUNK_MAX_BYTES);
        assert_eq!(
            record["payload_hex"].as_str().unwrap().len(),
            RUN_CHANNEL_CHUNK_MAX_HEX_BYTES
        );
    }

    #[test]
    fn empty_and_oversized_chunks_do_not_emit() {
        let mut stream = RunNdjsonState::new(Vec::new());
        stream.write_channel_chunk("stdout", &[]).unwrap();
        assert!(stream.inner.is_empty());

        let error = stream
            .write_channel_chunk("stdout", &[0; RUN_CHANNEL_CHUNK_MAX_BYTES + 1])
            .unwrap_err();
        assert_eq!(error.to_string(), "run-stream-channel-chunk-too-large");
        assert!(stream.inner.is_empty());
    }

    #[test]
    fn sequence_overflow_is_rejected_before_serialization() {
        let mut stream = RunNdjsonState::new(Vec::new());
        stream.sequence = u64::MAX;
        let error = stream.write_channel_chunk("stdout", b"x").unwrap_err();
        assert_eq!(error.to_string(), "run-stream-sequence-overflow");
        assert!(stream.inner.is_empty());
    }

    #[test]
    fn payload_size_arithmetic_overflow_is_rejected() {
        let error = checked_hex_len(usize::MAX).unwrap_err();
        assert_eq!(error.to_string(), "run-stream-payload-size-overflow");
    }

    #[test]
    fn bounded_serializer_rejects_before_exceeding_its_capacity() {
        #[derive(Serialize)]
        struct Oversized<'a> {
            payload: &'a str,
        }

        let mut stream = RunNdjsonState::new(Vec::new());
        let error = stream
            .write_record(
                &Oversized {
                    payload: "too large",
                },
                4,
            )
            .unwrap_err();
        assert_eq!(error.to_string(), "run-stream-record-too-large");
        assert!(stream.inner.is_empty());
    }

    #[test]
    fn execution_events_use_the_direct_bounded_outer_path() {
        let source = include_str!("../../../conformance/c2/execution-event-v1.ndjson");
        let first_line = source.lines().next().unwrap();
        let event: OwnedExecutionEvent = serde_json::from_str(first_line).unwrap();
        let expected_event: serde_json::Value = serde_json::from_str(first_line).unwrap();

        let mut stream = RunNdjsonState::new(Vec::new());
        stream.write_execution_event(&event).unwrap();
        assert!(stream.inner.len() <= RUN_STRUCTURED_RECORD_MAX_BYTES);

        let record: serde_json::Value = serde_json::from_slice(&stream.inner).unwrap();
        assert_eq!(record["schema"], RUN_STREAM_SCHEMA);
        assert_eq!(record["record"], "execution_event");
        assert_eq!(record["event"], expected_event);
        assert!(record.get("channel").is_none());
        assert!(record.get("payload_hex").is_none());
    }

    #[test]
    fn exact_executor_evidence_uses_the_direct_bounded_outer_path() {
        let evidence = ExactEvidenceRecord {
            schema: "conduit.exact-execution-evidence/v1",
            schema_version: 1,
            plan_identity:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            plan_epoch: 1,
            run_id: "fixture/run".to_owned(),
            sequence: 0,
            tick: 1,
            subject_kind: "cord",
            subject_id: "root/source.out->root/sink.in".to_owned(),
            node_id: None,
            cord_id: Some("root/source.out->root/sink.in".to_owned()),
            from_port: Some("root/source.out".to_owned()),
            to_port: Some("root/sink.in".to_owned()),
            implementation_id: None,
            implementation_identity: None,
            artifact_id: None,
            host_id: None,
            host_observation_id: None,
            pressure: Some("block"),
            event_kind: "value-accepted",
            event_detail: None,
            terminal_cause: None,
            occupancy_items: 1,
            occupancy_bytes: 8,
            scheduling_latency_ticks: 0,
            processing_latency_ticks: 1,
        };
        let mut bytes = Vec::new();
        let mut stream = RunNdjsonState::new(&mut bytes);
        stream.write_exact_evidence(&evidence).unwrap();

        let record: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(record["record"], "exact_execution_evidence");
        assert_eq!(
            record["evidence"]["schema"],
            "conduit.exact-execution-evidence/v1"
        );
        assert_eq!(record["evidence"]["pressure"], "block");
    }
}
