//! Exact-context recovery of canonical immutable chunk content. A successful
//! parse establishes content validity, never storage authority or authenticity.
use super::*;

#[derive(Debug, PartialEq, Eq)]
pub enum RecordingReadRefusal {
    Extent,
    ContextOrRepresentation,
    Hex,
    Disposition,
    NoncanonicalFailure,
    Sequence,
    ClockReversed,
    Observation(ObservationRefusal),
}

pub struct RecordedSample<'a> {
    pub sequence: u64,
    pub timestamp_basis: RecordingTimestampBasis,
    /// Producer-Boot milliseconds with the exact event meaning above; not UTC.
    pub at_boot_ms: u64,
    pub disposition: RecordingDisposition,
    pub gap_before: Option<(u64, u64)>,
    /// Exact canonical observation containing Temperature and relative humidity.
    pub observation: Option<&'a [u8]>,
}

/// One chunk's finite recovery workspace. The caller supplies the expected
/// authenticated producer context. This reader does not infer one from bytes.
pub struct PreparedRecordingReader {
    timestamp_basis: RecordingTimestampBasis,
    prefix: String,
    quantities: PreparedObservationQuantityForms,
    samples: [[u8; SAMPLE_BYTES]; MAXIMUM_CHUNK_SAMPLES],
    count: usize,
    preceding_sequence: Option<u64>,
}
impl PreparedRecordingReader {
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
    ) -> Result<Self, RecordingRefusal> {
        let prepared = PreparedRecordingChunk::prepare(context, contract)?;
        Ok(Self {
            timestamp_basis: prepared.timestamp_basis,
            prefix: prepared.prefix,
            quantities: prepared.quantities,
            samples: [[0; SAMPLE_BYTES]; MAXIMUM_CHUNK_SAMPLES],
            count: 0,
            preceding_sequence: None,
        })
    }
    /// Accept precisely the encoder's canonical JSON, including all producer,
    /// semantic Type and Form identities. Invalid input preserves the previous
    /// loaded chunk; no prefix of a refused snapshot becomes visible.
    pub fn read(&mut self, json: &str) -> Result<(), RecordingReadRefusal> {
        self.read_after(json, None)
    }
    /// The history owner supplies the last accepted sample, never context taken
    /// from incoming content. Validate the boundary before replacing any state.
    pub(super) fn read_after(
        &mut self,
        json: &str,
        previous: Option<(u64, u64)>,
    ) -> Result<(), RecordingReadRefusal> {
        use RecordingReadRefusal as E;
        if json.len() > MAXIMUM_CHUNK_BYTES {
            return Err(E::Extent);
        }
        let records = json
            .strip_prefix(self.prefix.as_str())
            .and_then(|s| s.strip_suffix("\"}"))
            .ok_or(E::ContextOrRepresentation)?;
        if records.is_empty()
            || records.len() % (SAMPLE_BYTES * 2) != 0
            || records.len() > MAXIMUM_CHUNK_SAMPLES * SAMPLE_BYTES * 2
        {
            return Err(E::Extent);
        }
        let mut samples = [[0; SAMPLE_BYTES]; MAXIMUM_CHUNK_SAMPLES];
        let mut count = 0;
        let mut last = previous;
        for encoded in records.as_bytes().as_chunks::<{ SAMPLE_BYTES * 2 }>().0 {
            let sample = &mut samples[count];
            for (octet, pair) in sample.iter_mut().zip(encoded.as_chunks::<2>().0) {
                *octet = (digit(pair[0])? << 4) | digit(pair[1])?;
            }
            let disposition =
                RecordingDispositionForm::decode(&sample[16..17]).map_err(|_| E::Disposition)?;
            if disposition == RecordingDisposition::Available {
                self.quantities
                    .unpack(&sample[17..])
                    .map_err(E::Observation)?;
            } else if sample[17..] != [0; 4] {
                return Err(E::NoncanonicalFailure);
            }
            let sequence = number(&sample[..8]);
            let time = number(&sample[8..16]);
            if sequence == 0 || last.is_some_and(|(seq, _)| sequence <= seq) {
                return Err(E::Sequence);
            }
            if last.is_some_and(|(_, previous)| time < previous) {
                return Err(E::ClockReversed);
            }
            last = Some((sequence, time));
            count += 1;
        }
        self.samples = samples;
        self.count = count;
        self.preceding_sequence = previous.map(|(sequence, _)| sequence);
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    /// Reconstitute first-class quantities with the prepared semantic codec.
    /// Standalone reads prove no earlier history; ordered history reads also
    /// expose a gap between the previous chunk and this chunk's first sample.
    pub fn sample(&mut self, index: usize) -> Option<RecordedSample<'_>> {
        if index >= self.count {
            return None;
        }
        let sample = self.samples[index];
        let sequence = number(&sample[..8]);
        let disposition = RecordingDispositionForm::decode(&sample[16..17]).ok()?;
        let previous = index
            .checked_sub(1)
            .map(|i| number(&self.samples[i][..8]))
            .or(self.preceding_sequence);
        let gap_before = previous
            .filter(|previous| sequence - previous > 1)
            .map(|previous| (previous + 1, sequence - 1));
        Some(RecordedSample {
            sequence,
            timestamp_basis: self.timestamp_basis,
            at_boot_ms: number(&sample[8..16]),
            disposition,
            gap_before,
            observation: if disposition == RecordingDisposition::Available {
                Some(self.quantities.unpack(&sample[17..]).ok()?)
            } else {
                None
            },
        })
    }
}
fn digit(byte: u8) -> Result<u8, RecordingReadRefusal> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(RecordingReadRefusal::Hex),
    }
}
fn number(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().unwrap())
}
