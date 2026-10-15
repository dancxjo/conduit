//! Preparation-only checked observation materialization. Play uses admitted
//! encoding storage; this allocating Native constructor is not a Step path.
use crate::{
    AudioPitchCorrelation, AudioPitchDigest, AudioPitchEstimator, AudioPitchObservation,
    AudioPitchState, PcmPitchEstimate, PcmPitchObservation,
};
use conduit_core::{Quantity, Unit};
use conduit_plot::rust_binding::NativeBindingRefusal;

impl PcmPitchObservation {
    pub fn prepare_native(self) -> Result<AudioPitchObservation, NativeBindingRefusal> {
        let (state, first, other, correlation) = match self.estimate {
            PcmPitchEstimate::UnknownInsufficientEnergy => {
                (AudioPitchState::UnknownInsufficientEnergy, 0, 0, 0)
            }
            PcmPitchEstimate::Unknown => (AudioPitchState::Unknown, 0, 0, 0),
            PcmPitchEstimate::Uncertain {
                first_lag,
                competing_lag,
            } => (AudioPitchState::Uncertain, first_lag, competing_lag, 0),
            PcmPitchEstimate::Periodic {
                lag_frames,
                correlation,
            } => (
                AudioPitchState::Periodic,
                lag_frames,
                0,
                (correlation * 1_000_000.0) as u32,
            ),
        };
        AudioPitchObservation::new(
            first,
            self.interval.clock_id,
            other,
            AudioPitchCorrelation::new(correlation)?,
            AudioPitchEstimator::CenteredNormalizedCorrelationF64PpmV1,
            self.maximum_lag,
            self.minimum_lag,
            Quantity::new(i64::from(self.interval.sample_rate_hz), Unit::Hertz),
            AudioPitchDigest::new(self.source_digest)?,
            state,
            self.interval.frames,
            self.interval.start_frame,
        )
    }
}

impl AudioPitchObservation {
    /// Descriptive agreement on this exact estimator/scale. This score creates
    /// no Claim, calibration, linguistic truth, authority or execution evidence.
    pub fn periodicity_score(&self) -> Option<conduit_core::claims::ClaimScore<'static>> {
        use conduit_core::claims::{ClaimScore, ClaimText};
        if *self.state() != AudioPitchState::Periodic {
            return None;
        }
        let text = |value| ClaimText::new(value).expect("bounded static pitch score identity");
        Some(
            ClaimScore::new(
                text("centered-normalized-correlation-ppm@1"),
                None,
                text(crate::PCM_PITCH_PROFILE),
                0,
                1_000_000,
                i64::from(*self.correlation_ppm().get()),
            )
            .expect("checked pitch correlation fits its declared score scale"),
        )
    }
}
