//! Bounded content recovery. Decoded scope is descriptive, NEVER authenticated
//! by incoming bytes. Root must independently authorize the physical residence,
//! producer and history before retaining, displaying or acknowledging records.
use super::{log_encoder::*, *};
#[derive(Debug, PartialEq, Eq)]
pub enum LogReadRefusal {
    Frame,
    Context,
    ContextDigest,
    ExpectedContext,
    MissingContext,
    Discontinuity,
    Sequence,
    ClockReversed,
    Observation(ObservationRefusal),
}
pub enum LogFrame<'a> {
    Context {
        complete: bool,
    },
    /// `sampled_at_boot_ms` is local acceptance time, identified by LOG_CLOCK.
    Sample {
        sample: RecordedSample<'a>,
        media_gap_before: bool,
    },
}
/// Descriptive retained producer provenance, not an admission/identity proof.
pub struct RecoveredLogContext<'a> {
    pub body: &'a str,
    pub host: &'a str,
    pub boot: &'a str,
    pub plan: &'a str,
    pub play: &'a str,
    pub stream: [u8; 32],
    pub observation_type: [u8; 32],
    pub first_sequence: u64,
}
fn context_view(
    bytes: &[u8],
    observation: [u8; 32],
) -> Result<RecoveredLogContext<'_>, LogReadRefusal> {
    use LogReadRefusal as E;
    if bytes.first() != Some(&1) {
        return Err(E::Context);
    }
    let mut offset = 1;
    let mut strings = [""; 9];
    for text in &mut strings {
        let length = usize::from(*bytes.get(offset).ok_or(E::Context)?);
        offset += 1;
        if length == 0 || length > 128 {
            return Err(E::Context);
        }
        *text = core::str::from_utf8(bytes.get(offset..offset + length).ok_or(E::Context)?)
            .map_err(|_| E::Context)?;
        offset += length;
    }
    if strings[5] != PreparedObservationQuantityPacker::TEMPERATURE_FORM_ID
        || strings[6] != PreparedObservationQuantityPacker::HUMIDITY_FORM_ID
        || strings[7] != RecordingDispositionForm::IDENTITY
        || strings[8] != LOG_CLOCK
        || bytes.len() != offset + 72
    {
        return Err(E::Context);
    }
    let stream: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
    offset += 32;
    let observation_type: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
    offset += 32;
    let first_sequence = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    if stream == [0; 32] || observation_type != observation || first_sequence == 0 {
        return Err(E::Context);
    }
    Ok(RecoveredLogContext {
        body: strings[0],
        host: strings[1],
        boot: strings[2],
        plan: strings[3],
        play: strings[4],
        stream,
        observation_type,
        first_sequence,
    })
}
pub struct PreparedLogReader {
    bytes: [u8; LOG_CONTEXT_BYTES],
    len: usize,
    digest: [u8; 32],
    ordinal: u16,
    total: u16,
    complete: bool,
    expected: Option<[u8; 32]>,
    observation: [u8; 32],
    first: u64,
    previous_context: Option<[u8; 32]>,
    previous: Option<(u64, u64)>,
    media_gap: bool,
    quantities: PreparedObservationQuantityForms,
}
impl PreparedLogReader {
    /// Optional independently selected exact producer context. This is separate
    /// from content decoding and cannot be selected by any on-media frame.
    pub fn prepare(
        context: LogContext<'_>,
        contract: &ObservationContract,
        first_sequence: u64,
    ) -> Result<Self, LogRefusal> {
        let expected = PreparedLogContext::prepare(context, contract, first_sequence)?.digest;
        let mut reader = Self::prepare_content(contract)?;
        reader.expected = Some(expected);
        Ok(reader)
    }
    /// Recover previously unknown Boot contexts as bounded descriptive content.
    /// No recovered field confers identity, admission or acknowledgement rights.
    pub fn prepare_content(contract: &ObservationContract) -> Result<Self, LogRefusal> {
        Ok(Self {
            bytes: [0; LOG_CONTEXT_BYTES],
            len: 0,
            digest: [0; 32],
            ordinal: 0,
            total: 0,
            complete: false,
            expected: None,
            observation: contract.observation_type().semantic_digest()?,
            first: 0,
            previous_context: None,
            previous: None,
            media_gap: false,
            quantities: PreparedObservationQuantityForms::prepare(contract)?,
        })
    }
    /// Only owned heap capacities. Inline Self includes all1536 context bytes.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.quantities.retained_backing_bytes()
    }
    /// End-of-residence must not hide an interrupted context tail.
    pub fn finish(&self) -> Result<(), LogReadRefusal> {
        if self.ordinal != 0 && !self.complete {
            Err(LogReadRefusal::MissingContext)
        } else {
            Ok(())
        }
    }
    pub fn context(&self) -> Option<RecoveredLogContext<'_>> {
        self.complete
            .then(|| context_view(&self.bytes[..self.len], self.observation).ok())
            .flatten()
    }
    /// Every damaged/uncertain historical slot remains explicit. A previously
    /// complete context survives a missing sample; no missing context is inferred.
    pub fn note_media_gap(&mut self) {
        self.media_gap = true;
    }
    pub fn read(&mut self, bytes: &[u8]) -> Result<LogFrame<'_>, LogReadRefusal> {
        use LogReadRefusal as E;
        if bytes.len() < 34 || bytes[1] != 1 {
            return Err(E::Frame);
        }
        let digest: [u8; 32] = bytes[2..34].try_into().unwrap();
        if self.expected.is_some_and(|expected| expected != digest) {
            return Err(E::ExpectedContext);
        }
        match bytes[0] {
            1 => {
                if bytes.len() < 40 || bytes.len() > LOG_RECORD_BYTES {
                    return Err(E::Frame);
                }
                let ordinal = u16::from_le_bytes(bytes[34..36].try_into().unwrap());
                let total = u16::from_le_bytes(bytes[36..38].try_into().unwrap());
                let len = usize::from(u16::from_le_bytes(bytes[38..40].try_into().unwrap()));
                if len == 0
                    || len > LOG_CONTEXT_BYTES
                    || usize::from(total) != len.div_ceil(LOG_CONTEXT_PART_BYTES)
                    || ordinal >= total
                {
                    return Err(E::Context);
                }
                if ordinal == 0 {
                    self.ordinal = 0;
                    self.complete = false;
                    self.len = len;
                    self.total = total;
                    self.digest = digest;
                }
                if ordinal != self.ordinal
                    || len != self.len
                    || total != self.total
                    || digest != self.digest
                {
                    return Err(E::Discontinuity);
                }
                let start = usize::from(ordinal) * LOG_CONTEXT_PART_BYTES;
                let end = (start + LOG_CONTEXT_PART_BYTES).min(len);
                if bytes.len() != 40 + end - start {
                    return Err(E::Frame);
                }
                self.bytes[start..end].copy_from_slice(&bytes[40..]);
                self.ordinal += 1;
                if self.ordinal == total {
                    if conduit_core::semantic_digest(
                        "conduit.environment/recording-context@1",
                        &self.bytes[..len],
                    ) != digest
                    {
                        return Err(E::ContextDigest);
                    }
                    let context = context_view(&self.bytes[..len], self.observation)?;
                    self.first = context.first_sequence;
                    if self.previous_context != Some(digest) {
                        self.previous = None;
                        self.previous_context = Some(digest);
                    }
                    self.complete = true;
                }
                Ok(LogFrame::Context {
                    complete: self.complete,
                })
            }
            2 => {
                if !self.complete || digest != self.digest {
                    return Err(E::MissingContext);
                }
                if bytes.len() != 55 {
                    return Err(E::Frame);
                }
                let sample = &bytes[34..];
                let sequence = u64::from_le_bytes(sample[..8].try_into().unwrap());
                let time = u64::from_le_bytes(sample[8..16].try_into().unwrap());
                let disposition =
                    RecordingDispositionForm::decode(&sample[16..17]).map_err(|_| E::Frame)?;
                if disposition != RecordingDisposition::Available {
                    return Err(E::Frame);
                }
                let expected = self
                    .previous
                    .map_or(self.first, |(sequence, _)| sequence.saturating_add(1));
                if sequence < expected
                    || self
                        .previous
                        .is_some_and(|(previous, _)| sequence <= previous)
                {
                    return Err(E::Sequence);
                }
                if self.previous.is_some_and(|(_, previous)| time < previous) {
                    return Err(E::ClockReversed);
                }
                let observation = self
                    .quantities
                    .unpack(&sample[17..])
                    .map_err(E::Observation)?;
                self.previous = Some((sequence, time));
                let media_gap_before = core::mem::take(&mut self.media_gap);
                Ok(LogFrame::Sample {
                    sample: RecordedSample {
                        sequence,
                        sampled_at_boot_ms: time,
                        disposition,
                        gap_before: (sequence > expected).then_some((expected, sequence - 1)),
                        observation: Some(observation),
                    },
                    media_gap_before,
                })
            }
            _ => Err(E::Frame),
        }
    }
}
