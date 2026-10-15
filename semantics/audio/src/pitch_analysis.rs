//! Finite, descriptive periodicity observations. These are signal evidence,
//! never the commanded pitch or an inferred linguistic fact.
use crate::{PcmAnalysisInterval, PcmTimeAnalysisRefusal, PreparedPcmTimeAnalysis};

pub const PCM_PITCH_PROFILE: &str = "audio/centered-normalized-correlation-f64-ppm@1";
pub const PCM_PITCH_MAXIMUM_FRAMES: usize = 2048;
pub const PCM_PITCH_MAXIMUM_LAG: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmPitchRefusal {
    Input(PcmTimeAnalysisRefusal),
    UnsupportedWindow,
    InvalidLagRange,
    WrongSourceFrames,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PcmPitchEstimate {
    /// Includes silence and constant/DC input after centering.
    UnknownInsufficientEnergy,
    /// No sufficiently periodic local peak in the admitted lag interval.
    Unknown,
    /// Competing periodicities, including possible octave errors.
    Uncertain { first_lag: u16, competing_lag: u16 },
    /// Frequency is exactly `sample_rate_hz / lag_frames` for this lag-grid
    /// estimate. Correlation is descriptive confidence, not probability.
    Periodic { lag_frames: u16, correlation: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PcmPitchObservation {
    pub interval: PcmAnalysisInterval,
    pub source_start_frame: u64,
    pub source_frames: u16,
    pub source_available_frame: u64,
    pub source_digest: [u8; 32],
    pub minimum_lag: u16,
    pub maximum_lag: u16,
    pub estimate: PcmPitchEstimate,
}

/// Complete-block validation makes source end the earliest media availability
/// coordinate. Window support remains separate; there is no hidden history.
/// Arrays are caller-owned, fixed capacity, and reused without allocation.
pub struct PcmPitchWorkspace {
    samples: [f64; PCM_PITCH_MAXIMUM_FRAMES],
    correlations: [f64; PCM_PITCH_MAXIMUM_LAG + 2],
}
impl Default for PcmPitchWorkspace {
    fn default() -> Self {
        Self {
            samples: [0.0; PCM_PITCH_MAXIMUM_FRAMES],
            correlations: [0.0; PCM_PITCH_MAXIMUM_LAG + 2],
        }
    }
}
impl PcmPitchWorkspace {
    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        &mut self,
        encoded: &[u8],
        clock_id: u64,
        sample_rate_hz: u32,
        offset: u16,
        frames: u16,
        minimum_lag: u16,
        maximum_lag: u16,
    ) -> Result<PcmPitchObservation, PcmPitchRefusal> {
        let n = usize::from(frames);
        let low = usize::from(minimum_lag);
        let high = usize::from(maximum_lag);
        if !(64..=PCM_PITCH_MAXIMUM_FRAMES).contains(&n) {
            return Err(PcmPitchRefusal::UnsupportedWindow);
        }
        // At least three complete candidate periods; reserve one neighbouring
        // lag for local-maximum admission. Work is at most 513 * 2048 pairs.
        if low < 2 || low > high || high > PCM_PITCH_MAXIMUM_LAG || high * 3 > n {
            return Err(PcmPitchRefusal::InvalidLagRange);
        }
        let input =
            PreparedPcmTimeAnalysis::prepare(encoded, clock_id, sample_rate_hz, offset, frames)
                .map_err(PcmPitchRefusal::Input)?;
        let source = input.source_interval();
        let source_available_frame = source
            .start_frame
            .checked_add(u64::from(source.frames))
            .ok_or(PcmPitchRefusal::Input(
                PcmTimeAnalysisRefusal::FrameIndexOverflow,
            ))?;
        let mean = input.waveform().sum::<f64>() / n as f64;
        let mut energy = 0.0;
        for (slot, sample) in self.samples[..n].iter_mut().zip(input.waveform()) {
            *slot = sample - mean;
            energy += *slot * *slot;
        }
        let estimate = if energy / n as f64 <= 1.0e-10 {
            PcmPitchEstimate::UnknownInsufficientEnergy
        } else {
            // Search below the admitted minimum too: a clear shorter period
            // must not be silently relabelled as its in-range subharmonic.
            for lag in 1..=high + 1 {
                let (mut cross, mut left, mut right) = (0.0, 0.0, 0.0);
                for i in lag..n {
                    let a = self.samples[i];
                    let b = self.samples[i - lag];
                    cross += a * b;
                    left += a * a;
                    right += b * b;
                }
                self.correlations[lag] = if left > 0.0 && right > 0.0 {
                    (cross / libm::sqrt(left * right)).clamp(-1.0, 1.0)
                } else {
                    0.0
                };
            }
            self.classify(low, high)
        };
        Ok(PcmPitchObservation {
            interval: input.interval(),
            source_start_frame: source.start_frame,
            source_frames: source.frames,
            source_available_frame,
            source_digest: input.source_digest(),
            minimum_lag,
            maximum_lag,
            estimate,
        })
    }

    fn peak(&self, lag: usize) -> bool {
        self.correlations[lag] > self.correlations[lag - 1]
            && self.correlations[lag] >= self.correlations[lag + 1]
    }

    fn classify(&self, low: usize, high: usize) -> PcmPitchEstimate {
        let Some(first) = (2..=high).find(|&lag| self.peak(lag) && self.correlations[lag] >= 0.9)
        else {
            return PcmPitchEstimate::Unknown;
        };
        if first < low {
            return PcmPitchEstimate::Unknown;
        }
        for other in first + 1..=high {
            if !self.peak(other) || self.correlations[other] < 0.9 {
                continue;
            }
            // Exact repeats of a period are not independent pitch evidence.
            // A materially stronger repeat is an octave/subharmonic warning;
            // a distinct similarly strong period is competing evidence.
            let remainder = other % first;
            let repeat = remainder <= 1 || first - remainder <= 1;
            let stronger = self.correlations[other] > self.correlations[first] + 0.0001;
            let comparable = self.correlations[other] >= self.correlations[first] - 0.03;
            if stronger || (!repeat && comparable) {
                return PcmPitchEstimate::Uncertain {
                    first_lag: first as u16,
                    competing_lag: other as u16,
                };
            }
        }
        PcmPitchEstimate::Periodic {
            lag_frames: first as u16,
            correlation: self.correlations[first],
        }
    }
}
