//! Preparation-only bounded mono PCM observations, independent of speech intent.
//! Amplitude is relative to digital full scale; power is its square, never watts.
use crate::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, SoundInfoError};

pub const PCM_TIME_ANALYSIS_PROFILE: &str = "audio/rectangular-full-scale-f64@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmTimeAnalysisRefusal {
    Frame(SoundInfoError),
    UnsupportedLayout,
    WrongTimebase,
    Discontinuous,
    InsufficientWindow,
    WindowOutsideFrame,
    FrameIndexOverflow,
    NonfiniteSample,
}

/// Exact half-open sample interval. Seconds are numerator/sample_rate_hz;
/// no floating timestamp, resampling, padding or hidden alignment is involved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmAnalysisInterval {
    pub clock_id: u64,
    pub start_frame: u64,
    pub frames: u16,
    pub sample_rate_hz: u32,
}

/// Descriptive floating projection, not an exact real-arithmetic certificate.
/// `square_sum` has units of full-scale amplitude squared times sample count;
/// dividing it by sample rate gives the corresponding time integral, not joules.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PcmTimeStatistics {
    pub square_sum: f64,
    pub mean_square: f64,
    pub rms: f64,
    pub absolute_peak: f64,
    pub samples_at_or_above_full_scale: u16,
}

/// Immutable borrowed waveform and exact source-content/time custody. All input
/// validation happens at preparation; neither preparation nor iteration allocates.
pub struct PreparedPcmTimeAnalysis<'a> {
    source_digest: [u8; 32],
    header: PcmFrameHeader,
    payload: &'a [u8],
    offset: u16,
    interval: PcmAnalysisInterval,
    statistics: PcmTimeStatistics,
}
impl<'a> PreparedPcmTimeAnalysis<'a> {
    pub fn prepare(
        encoded: &'a [u8],
        clock_id: u64,
        sample_rate_hz: u32,
        offset: u16,
        frames: u16,
    ) -> Result<Self, PcmTimeAnalysisRefusal> {
        use PcmTimeAnalysisRefusal as R;
        let (header, payload) = PcmFrameHeader::decode_frame(encoded).map_err(R::Frame)?;
        if header.layout() != PcmChannelLayout::Mono {
            return Err(R::UnsupportedLayout);
        }
        if clock_id != header.clock_id() || sample_rate_hz != header.sample_rate_hz() {
            return Err(R::WrongTimebase);
        }
        if header.discontinuity() {
            return Err(R::Discontinuous);
        }
        if frames < 2 {
            return Err(R::InsufficientWindow);
        }
        if u32::from(offset) + u32::from(frames) > u32::from(header.frame_count()) {
            return Err(R::WindowOutsideFrame);
        }
        header
            .start_frame()
            .checked_add(u64::from(header.frame_count()))
            .ok_or(R::FrameIndexOverflow)?;
        // Reject nonfinite input even outside the selected observation window.
        for i in 0..header.frame_count() {
            if !sample(header, payload, i).is_finite() {
                return Err(R::NonfiniteSample);
            }
        }
        let mut sum = 0.0;
        let mut correction = 0.0;
        let mut peak = 0.0_f64;
        let mut full_scale = 0;
        for i in offset..offset + frames {
            let value = sample(header, payload, i);
            let adjusted = value * value - correction;
            let next = sum + adjusted;
            correction = (next - sum) - adjusted;
            sum = next;
            peak = peak.max(value.abs());
            full_scale += u16::from(value.abs() >= 1.0);
        }
        let mean_square = sum / f64::from(frames);
        Ok(Self {
            source_digest: conduit_core::semantic_digest(crate::AUDIO_PCM_INFO_ID, encoded),
            header,
            payload,
            offset,
            interval: PcmAnalysisInterval {
                clock_id,
                sample_rate_hz,
                frames,
                start_frame: header.start_frame() + u64::from(offset),
            },
            statistics: PcmTimeStatistics {
                square_sum: sum,
                mean_square,
                rms: libm::sqrt(mean_square),
                absolute_peak: peak,
                samples_at_or_above_full_scale: full_scale,
            },
        })
    }
    pub fn source_digest(&self) -> [u8; 32] {
        self.source_digest
    }
    pub fn interval(&self) -> PcmAnalysisInterval {
        self.interval
    }
    /// Complete validated source support, including samples outside the window.
    pub fn source_interval(&self) -> PcmAnalysisInterval {
        PcmAnalysisInterval {
            clock_id: self.interval.clock_id,
            start_frame: self.header.start_frame(),
            frames: self.header.frame_count(),
            sample_rate_hz: self.interval.sample_rate_hz,
        }
    }
    pub fn statistics(&self) -> PcmTimeStatistics {
        self.statistics
    }
    pub fn waveform(&self) -> impl ExactSizeIterator<Item = f64> + '_ {
        (self.offset..self.offset + self.interval.frames)
            .map(|i| sample(self.header, self.payload, i))
    }
}
fn sample(header: PcmFrameHeader, payload: &[u8], index: u16) -> f64 {
    let offset = usize::from(index) * usize::from(header.representation().bytes_per_sample());
    match header.representation() {
        PcmSampleRepresentation::Signed16LittleEndian => {
            f64::from(i16::from_le_bytes(
                payload[offset..offset + 2].try_into().unwrap(),
            )) / 32768.0
        }
        PcmSampleRepresentation::Signed24LittleEndian => {
            let b = &payload[offset..offset + 3];
            let sign = if b[2] & 128 != 0 { 255 } else { 0 };
            f64::from(i32::from_le_bytes([b[0], b[1], b[2], sign])) / 8388608.0
        }
        PcmSampleRepresentation::Float32LittleEndian => f64::from(f32::from_le_bytes(
            payload[offset..offset + 4].try_into().unwrap(),
        )),
    }
}
