//! Owned checked Native observations are created only during preparation.
use crate::*;
use alloc::vec::Vec;
use conduit_core::{IeeeF64, StructuredInfoRefusal};
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal};

impl PreparedPcmTimeAnalysis<'_> {
    /// Builds a finite owned observation outside Play. Native transport preserves
    /// exact source digest and frame interval; floating statistics stay projections.
    pub fn native_observation(&self) -> Result<AudioTimeAnalysisObservation, NativeBindingRefusal> {
        let invalid = || NativeBindingRefusal::InvalidValue(StructuredInfoRefusal::WrongType);
        let mut pages = Vec::with_capacity(2);
        let mut samples = self.waveform();
        loop {
            let values = BoundedSequence::try_from_iter(
                samples
                    .by_ref()
                    .take(1024)
                    .map(|v| AudioTimeSampleAmplitude::new(IeeeF64::from_value(v)))
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .map_err(|_| invalid())?;
            if values.is_empty() {
                break;
            }
            pages.push(AudioTimeAnalysisWaveformPage::new(values)?);
        }
        let waveform = AudioTimeAnalysisWaveform::new(
            BoundedSequence::try_from_iter(pages).map_err(|_| invalid())?,
        )?;
        let digest = AudioTimeAnalysisDigest::new(self.source_digest())?;
        let time = self.interval();
        let stats = self.statistics();
        AudioTimeAnalysisObservation::new(
            AudioTimeAmplitudeMagnitude::new(IeeeF64::from_value(stats.absolute_peak))?,
            time.clock_id,
            time.frames,
            AudioTimeMeanSquarePower::new(IeeeF64::from_value(stats.mean_square))?,
            AudioTimeAnalysisReference::DigitalFullScale,
            AudioTimeAmplitudeMagnitude::new(IeeeF64::from_value(stats.rms))?,
            time.sample_rate_hz,
            stats.samples_at_or_above_full_scale,
            digest,
            AudioTimeSquareSum::new(IeeeF64::from_value(stats.square_sum))?,
            time.start_frame,
            waveform,
            AudioTimeAnalysisWindow::Rectangular,
        )
    }
}
