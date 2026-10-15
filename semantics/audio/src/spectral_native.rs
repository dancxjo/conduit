//! Owned checked spectrum values are constructed only during preparation.
use crate::*;
use alloc::vec::Vec;
use conduit_core::{IeeeF64, Quantity, StructuredInfoRefusal, Unit};
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal};

#[derive(Debug)]
pub enum PcmSpectralNativeRefusal {
    Analysis(PcmSpectralRefusal),
    Native(NativeBindingRefusal),
}
impl From<NativeBindingRefusal> for PcmSpectralNativeRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::Native(value)
    }
}
impl PcmSpectralProfile {
    /// Exact source/clock/window custody plus finite owned magnitudes. Allocation
    /// here is preparation work; the Play encoder uses admitted slots.
    pub fn prepare_observation(
        self,
        workspace: &mut PcmSpectralWorkspace,
        encoded: &[u8],
        index: u16,
    ) -> Result<AudioSpectrumObservation, PcmSpectralNativeRefusal> {
        let spectrum = self
            .observe_window(workspace, encoded, index)
            .map_err(PcmSpectralNativeRefusal::Analysis)?;
        let geometry = self.geometry();
        let interval = spectrum.interval();
        let spacing = spectrum
            .bins()
            .nth(1)
            .expect("admitted spectrum has at least nine bins")
            .frequency;
        let mut amplitudes = Vec::with_capacity(spectrum.bins().len());
        let mut powers = Vec::with_capacity(spectrum.bins().len());
        for bin in spectrum.bins() {
            amplitudes.push(AudioSpectrumAmplitude::new(IeeeF64::from_value(
                bin.peak_amplitude,
            ))?);
            powers.push(AudioSpectrumMeanSquare::new(IeeeF64::from_value(
                bin.mean_square_contribution,
            ))?);
        }
        let invalid = || NativeBindingRefusal::InvalidValue(StructuredInfoRefusal::WrongType);
        let amplitudes = AudioSpectrumAmplitudes::new(
            BoundedSequence::try_from_iter(amplitudes).map_err(|_| invalid())?,
        )?;
        let powers = AudioSpectrumMeanSquares::new(
            BoundedSequence::try_from_iter(powers).map_err(|_| invalid())?,
        )?;
        Ok(AudioSpectrumObservation::new(
            spacing,
            interval.clock_id,
            AudioSpectrumEstimator::Radix2F64,
            Quantity::new(0, Unit::Hertz),
            powers,
            amplitudes,
            AudioSpectrumReference::DigitalFullScale,
            Quantity::new(i64::from(interval.sample_rate_hz), Unit::Hertz),
            AudioSpectrumDigest::new(spectrum.source_digest())?,
            geometry.source_frames(),
            interval.start_frame
                - u64::from(
                    geometry
                        .offset(index)
                        .map_err(PcmSpectralNativeRefusal::Analysis)?,
                ),
            geometry.stride_frames(),
            match self.window() {
                PcmSpectralWindow::Rectangular => AudioSpectrumWindow::Rectangular,
                PcmSpectralWindow::PeriodicHann => AudioSpectrumWindow::PeriodicHann,
            },
            geometry.window_frames(),
            index,
            interval.start_frame,
        )?)
    }
}
