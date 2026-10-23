//! Bounded immutable JSON content for the existing Resource snapshot Fore.
//! This encoder grants no storage authority, chooses no generation and makes no
//! durable-acceptance claim. Root must admit and complete publication separately.
use crate::observation::{
    ObservationContract, ObservationRefusal, PreparedObservationQuantityForms,
};
mod host_call;
mod publication;
mod reader;
mod snapshot;
use alloc::string::String;
use conduit_core::{ActivePlayId, BootId, HostId, PlanId, StructuredInfoRefusal};
pub use host_call::{RecordedRange, RecordingCommitObserver, RecordingSnapshotBack};
pub use publication::PreparedRecordingInput;
pub use reader::{PreparedRecordingReader, RecordedSample, RecordingReadRefusal};
pub use snapshot::{PreparedRecordingSnapshot, RecordingSnapshotRefusal};
#[allow(dead_code)]
mod forms {
    include!(concat!(env!("OUT_DIR"), "/recording-forms.rs"));
}
pub use forms::{RecordingDisposition, RecordingDispositionForm};

pub const MAXIMUM_CHUNK_BYTES: usize = 4096;
pub const MAXIMUM_CHUNK_SAMPLES: usize = 64;
pub const SAMPLE_BYTES: usize = 21; // sequence u64le, boot ms u64le, status Form, quantity Forms
const PREFIX_BYTES: usize = MAXIMUM_CHUNK_BYTES - MAXIMUM_CHUNK_SAMPLES * SAMPLE_BYTES * 2 - 2;

pub struct RecordingContext<'a> {
    pub host: &'a HostId,
    pub boot: &'a BootId,
    pub plan: &'a PlanId,
    pub play: &'a ActivePlayId,
}
#[derive(Debug, PartialEq, Eq)]
pub enum RecordingRefusal {
    ContextExtent,
    Empty,
    Full,
    Sealed,
    InvalidSequence,
    Conflict,
    OutOfOrder,
    ClockReversed,
    MissingObservation,
    UnexpectedObservation,
    Canonical(StructuredInfoRefusal),
    Observation(ObservationRefusal),
}
impl From<StructuredInfoRefusal> for RecordingRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Canonical(value)
    }
}
impl From<ObservationRefusal> for RecordingRefusal {
    fn from(value: ObservationRefusal) -> Self {
        Self::Observation(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkAppend {
    Duplicate,
    Buffered { gap_before: Option<(u64, u64)> },
}
/// One planned chunk. All recurring storage is reserved at preparation; sealing
/// preserves the exact bytes across failed publication attempts. No implicit
/// flush, retry, eviction, next generation or mutable store is hidden here.
pub struct PreparedRecordingChunk {
    quantities: PreparedObservationQuantityForms,
    prefix: String,
    output: String,
    samples: [[u8; SAMPLE_BYTES]; MAXIMUM_CHUNK_SAMPLES],
    count: usize,
    sealed: bool,
}
impl PreparedRecordingChunk {
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
    ) -> Result<Self, RecordingRefusal> {
        let mut prefix = String::with_capacity(MAXIMUM_CHUNK_BYTES);
        prefix.push_str("{\"schema\":\"conduit.environment/recording-chunk@1\",\"identity_encoding\":\"utf8-hex\",\"record_encoding\":\"hex\",\"record_bytes\":21,\"clock\":\"producer-boot-ms\"");
        for (name, value) in [
            ("host", context.host.as_str()),
            ("boot", context.boot.as_str()),
            ("plan", context.plan.as_str()),
            ("play", context.play.as_str()),
        ] {
            if value.is_empty() || value.len() > 128 {
                return Err(RecordingRefusal::ContextExtent);
            }
            prefix.push_str(",\"");
            prefix.push_str(name);
            prefix.push_str("\":\"");
            hex(&mut prefix, value.as_bytes());
            prefix.push('"');
        }
        prefix.push_str(",\"observation_type\":\"");
        hex(&mut prefix, &contract.observation_type().semantic_digest()?);
        prefix.push_str("\",\"temperature_form\":\"");
        prefix.push_str(PreparedObservationQuantityForms::TEMPERATURE_FORM_ID);
        prefix.push_str("\",\"humidity_form\":\"");
        prefix.push_str(PreparedObservationQuantityForms::HUMIDITY_FORM_ID);
        prefix.push_str("\",\"disposition_form\":\"");
        prefix.push_str(forms::RecordingDispositionForm::IDENTITY);
        prefix.push_str("\",\"records\":\"");
        if prefix.len() > PREFIX_BYTES {
            return Err(RecordingRefusal::ContextExtent);
        }
        Ok(Self {
            quantities: PreparedObservationQuantityForms::prepare(contract)?,
            prefix,
            output: String::with_capacity(MAXIMUM_CHUNK_BYTES),
            samples: [[0; SAMPLE_BYTES]; MAXIMUM_CHUNK_SAMPLES],
            count: 0,
            sealed: false,
        })
    }
    /// Inputs use the shared committed PublicationSnapshot sequence across all
    /// producer outcomes. Failures never copy a prior temperature; their four
    /// quantity bytes are absent (canonical zero).
    pub fn append(
        &mut self,
        sequence: u64,
        sampled_at_boot_ms: u64,
        disposition: RecordingDisposition,
        observation: Option<&[u8]>,
    ) -> Result<ChunkAppend, RecordingRefusal> {
        use RecordingRefusal as E;
        if self.sealed {
            return Err(E::Sealed);
        }
        if sequence == 0 {
            return Err(E::InvalidSequence);
        }
        let tag = forms::RecordingDispositionForm::encode(disposition)[0];
        let mut entry = [0; SAMPLE_BYTES];
        entry[..8].copy_from_slice(&sequence.to_le_bytes());
        entry[8..16].copy_from_slice(&sampled_at_boot_ms.to_le_bytes());
        entry[16] = tag;
        if tag == forms::RecordingDispositionForm::encode(RecordingDisposition::Available)[0] {
            entry[17..].copy_from_slice(
                &self
                    .quantities
                    .pack(observation.ok_or(E::MissingObservation)?)?,
            );
        } else if observation.is_some() {
            return Err(E::UnexpectedObservation);
        }
        for old in &self.samples[..self.count] {
            if old[..8] == entry[..8] {
                return if old == &entry {
                    Ok(ChunkAppend::Duplicate)
                } else {
                    Err(E::Conflict)
                };
            }
        }
        let mut gap_before = None;
        if self.count != 0 {
            let last = &self.samples[self.count - 1];
            let previous = u64::from_le_bytes(last[..8].try_into().unwrap());
            if sequence < previous {
                return Err(E::OutOfOrder);
            }
            let previous_time = u64::from_le_bytes(last[8..16].try_into().unwrap());
            if sampled_at_boot_ms < previous_time {
                return Err(E::ClockReversed);
            }
            if sequence - previous > 1 {
                gap_before = Some((previous + 1, sequence - 1));
            }
        }
        if self.count == MAXIMUM_CHUNK_SAMPLES {
            return Err(E::Full);
        }
        self.samples[self.count] = entry;
        self.count += 1;
        Ok(ChunkAppend::Buffered { gap_before })
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    /// Sealing is not publication. The caller supplies these same JSON bytes to
    /// its exact admitted snapshot call and retains them until actual completion.
    pub fn seal(&mut self) -> Result<&str, RecordingRefusal> {
        if self.count == 0 {
            return Err(RecordingRefusal::Empty);
        }
        if !self.sealed {
            self.output.push_str(&self.prefix);
            for record in &self.samples[..self.count] {
                hex(&mut self.output, record);
            }
            self.output.push_str("\"}");
            self.sealed = true;
        }
        Ok(&self.output)
    }
}
fn hex(output: &mut String, bytes: &[u8]) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 15)] as char);
    }
}
