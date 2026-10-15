//! Four admitted immutable canonical shapes; Step changes only numeric cells.
use crate::*;
use alloc::{boxed::Box, vec, vec::Vec};
use conduit_core::{validate_canonical_structured_value, StructuredInfoRefusal};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum PcmPitchEncodingRefusal {
    Analysis(PcmPitchRefusal),
    Native(NativeBindingRefusal),
    Structured(StructuredInfoRefusal),
}
impl From<NativeBindingRefusal> for PcmPitchEncodingRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::Native(value)
    }
}
struct PitchSlot {
    output: Box<[u8]>,
    first: usize,
    other: usize,
    confidence: usize,
    start: usize,
    digest: [usize; 32],
}
impl PitchSlot {
    fn prepare(value: AudioPitchObservation) -> Result<Self, PcmPitchEncodingRefusal> {
        use PcmPitchEncodingRefusal as R;
        let output = value.encode()?.into_boxed_slice();
        let view = validate_canonical_structured_value(&output).map_err(R::Structured)?;
        let invalid = || R::Structured(StructuredInfoRefusal::WrongType);
        let offset = |bytes: &[u8]| -> Result<usize, R> {
            let offset = (bytes.as_ptr() as usize)
                .checked_sub(output.as_ptr() as usize)
                .ok_or_else(invalid)?;
            if offset + bytes.len() > output.len() {
                return Err(invalid());
            }
            Ok(offset)
        };
        let field = |name: &str, kind: &str| -> Result<usize, R> {
            offset(
                view.record_field(name)
                    .map_err(R::Structured)?
                    .ok_or_else(invalid)?
                    .primitive_representation_bytes(kind)
                    .map_err(R::Structured)?,
            )
        };
        let first = field("candidate_lag", "value/u16")?;
        let other = field("competing_lag", "value/u16")?;
        let confidence = field("correlation_ppm", "value/u32")?;
        let start = field("window_start_frame", "value/u64")?;
        let digest_view = view
            .record_field("source_digest")
            .map_err(R::Structured)?
            .ok_or_else(invalid)?;
        let mut digest = [0; 32];
        for (i, slot) in digest.iter_mut().enumerate() {
            *slot = offset(
                digest_view
                    .collection_index(i as u16)
                    .map_err(R::Structured)?
                    .ok_or_else(invalid)?
                    .primitive_representation_bytes("value/u8")
                    .map_err(R::Structured)?,
            )?;
        }
        Ok(Self {
            output,
            first,
            other,
            confidence,
            start,
            digest,
        })
    }
    fn update(&mut self, observation: PcmPitchObservation) {
        let (first, other, confidence) = match observation.estimate {
            PcmPitchEstimate::Periodic {
                lag_frames,
                correlation,
            } => (lag_frames, 0, (correlation * 1_000_000.0) as u32),
            PcmPitchEstimate::Uncertain {
                first_lag,
                competing_lag,
            } => (first_lag, competing_lag, 0),
            _ => (0, 0, 0),
        };
        self.output[self.first..self.first + 2].copy_from_slice(&first.to_le_bytes());
        self.output[self.other..self.other + 2].copy_from_slice(&other.to_le_bytes());
        self.output[self.confidence..self.confidence + 4]
            .copy_from_slice(&confidence.to_le_bytes());
        self.output[self.start..self.start + 8]
            .copy_from_slice(&observation.interval.start_frame.to_le_bytes());
        for (offset, byte) in self.digest.iter().zip(observation.source_digest) {
            self.output[*offset] = byte;
        }
    }
}
pub struct PreparedPcmPitchObservationEncoder {
    profile: PcmPitchProfile,
    scratch: Box<PcmPitchWorkspace>,
    slots: Box<[PitchSlot]>,
    active: usize,
}
impl PreparedPcmPitchObservationEncoder {
    pub fn prepare(profile: PcmPitchProfile) -> Result<Self, PcmPitchEncodingRefusal> {
        use PcmPitchEncodingRefusal as R;
        let input_error = |e| R::Analysis(PcmPitchRefusal::Input(PcmTimeAnalysisRefusal::Frame(e)));
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            profile.sample_rate_hz(),
            PcmChannelLayout::Mono,
            profile.source_frames(),
            profile.clock_id(),
            0,
            false,
        )
        .map_err(input_error)?;
        let encoded = header
            .encode_frame(&vec![0; usize::from(profile.source_frames()) * 2])
            .map_err(input_error)?;
        let mut scratch: Box<PcmPitchWorkspace> = Box::default();
        let template = profile
            .observe(&mut scratch, &encoded)
            .map_err(R::Analysis)?;
        let mut slots = Vec::with_capacity(4);
        for estimate in [
            PcmPitchEstimate::InsufficientEnergy,
            PcmPitchEstimate::Unknown,
            // An equal-endpoint profile cannot produce competing lags. Its
            // unreachable slot retains a valid unknown shape instead.
            if template.minimum_lag < template.maximum_lag {
                PcmPitchEstimate::Uncertain {
                    first_lag: template.minimum_lag,
                    competing_lag: template.minimum_lag + 1,
                }
            } else {
                PcmPitchEstimate::Unknown
            },
            PcmPitchEstimate::Periodic {
                lag_frames: template.minimum_lag,
                correlation: 1.0,
            },
        ] {
            let mut value = template;
            value.estimate = estimate;
            slots.push(PitchSlot::prepare(value.prepare_native()?)?);
        }
        Ok(Self {
            profile,
            scratch,
            slots: slots.into_boxed_slice(),
            active: 0,
        })
    }
    pub fn encoded(&self) -> &[u8] {
        &self.slots[self.active].output
    }
    pub fn maximum_encoded_len(&self) -> usize {
        self.slots.iter().map(|s| s.output.len()).max().unwrap_or(0)
    }
    pub fn encode(&mut self, input: &[u8]) -> Result<&[u8], PcmPitchEncodingRefusal> {
        let observation = self
            .profile
            .observe(&mut self.scratch, input)
            .map_err(PcmPitchEncodingRefusal::Analysis)?;
        self.active = match observation.estimate {
            PcmPitchEstimate::InsufficientEnergy => 0,
            PcmPitchEstimate::Unknown => 1,
            PcmPitchEstimate::Uncertain { .. } => 2,
            PcmPitchEstimate::Periodic { .. } => 3,
        };
        self.slots[self.active].update(observation);
        Ok(self.encoded())
    }
}
