//! Finite PCM spectral projections. Frequencies are exact Hz Quantities;
//! magnitudes are f64 digital-full-scale observations, not certified real values.
use crate::{spectral_fft, PcmAnalysisInterval, PcmTimeAnalysisRefusal, PreparedPcmTimeAnalysis};
use conduit_core::{Quantity, Unit};
use spectral_fft::Complex;

pub const PCM_SPECTRAL_MAXIMUM_FRAMES: usize = 1024;
pub const PCM_SPECTRAL_PROFILE: &str = "audio/one-sided-radix2-full-scale-f64@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmSpectralWindow {
    Rectangular,
    /// w[n] = (1 - cos(2*pi*n/N))/2; denominator is N, not N-1.
    PeriodicHann,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmSpectralRefusal {
    Input(PcmTimeAnalysisRefusal),
    UnsupportedWindow,
    InvalidStride,
    WrongSourceFrames,
    TooManyWindows,
    WindowIndexOutsideGeometry,
    ArithmeticOverflow,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PcmSpectralBin {
    pub index: u16,
    pub frequency: Quantity,
    /// One-sided peak amplitude, coherent-gain normalized. DC/Nyquist are not doubled.
    pub peak_amplitude: f64,
    /// |X[k]|^2/(N*sum(w^2)), doubled only for interior bins. Not watts or PSD.
    pub mean_square_contribution: f64,
}

/// Caller-owned scratch; construction and reuse require no allocation.
pub struct PcmSpectralWorkspace {
    values: [Complex; PCM_SPECTRAL_MAXIMUM_FRAMES],
}
impl Default for PcmSpectralWorkspace {
    fn default() -> Self {
        Self {
            values: [Complex::default(); PCM_SPECTRAL_MAXIMUM_FRAMES],
        }
    }
}
pub struct PreparedPcmSpectrum<'a> {
    values: &'a [Complex],
    interval: PcmAnalysisInterval,
    digest: [u8; 32],
    window: PcmSpectralWindow,
    window_sum: f64,
    window_square_sum: f64,
}
impl PcmSpectralWorkspace {
    pub fn analyze<'a>(
        &'a mut self,
        encoded: &[u8],
        clock_id: u64,
        sample_rate_hz: u32,
        offset: u16,
        frames: u16,
        window: PcmSpectralWindow,
    ) -> Result<PreparedPcmSpectrum<'a>, PcmSpectralRefusal> {
        let n = usize::from(frames);
        if !(16..=PCM_SPECTRAL_MAXIMUM_FRAMES).contains(&n) || !n.is_power_of_two() {
            return Err(PcmSpectralRefusal::UnsupportedWindow);
        }
        let input =
            PreparedPcmTimeAnalysis::prepare(encoded, clock_id, sample_rate_hz, offset, frames)
                .map_err(PcmSpectralRefusal::Input)?;
        let mut window_sum = 0.0;
        let mut window_square_sum = 0.0;
        for (i, sample) in input.waveform().enumerate() {
            let weight = match window {
                PcmSpectralWindow::Rectangular => 1.0,
                PcmSpectralWindow::PeriodicHann => {
                    0.5 - 0.5 * libm::cos(2.0 * core::f64::consts::PI * i as f64 / n as f64)
                }
            };
            self.values[i] = Complex {
                real: sample * weight,
                imaginary: 0.0,
            };
            window_sum += weight;
            window_square_sum += weight * weight;
        }
        spectral_fft::forward(&mut self.values[..n])
            .map_err(|_| PcmSpectralRefusal::ArithmeticOverflow)?;
        Ok(PreparedPcmSpectrum {
            values: &self.values[..n],
            interval: input.interval(),
            digest: input.source_digest(),
            window,
            window_sum,
            window_square_sum,
        })
    }
}
impl PreparedPcmSpectrum<'_> {
    pub fn interval(&self) -> PcmAnalysisInterval {
        self.interval
    }
    pub fn source_digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn window(&self) -> PcmSpectralWindow {
        self.window
    }
    pub fn bins(&self) -> impl ExactSizeIterator<Item = PcmSpectralBin> + '_ {
        let n = self.values.len();
        (0..n / 2 + 1).map(move |index| {
            let value = self.values[index];
            let square = value.real * value.real + value.imaginary * value.imaginary;
            let sided = if index == 0 || index == n / 2 {
                1.0
            } else {
                2.0
            };
            // N=2^p: (rate*k)/N = (rate*k*5^p)*10^-p, exactly.
            let power = n.trailing_zeros();
            let coefficient =
                i128::from(self.interval.sample_rate_hz) * index as i128 * 5_i128.pow(power);
            let frequency = Quantity::from_decimal(coefficient, -(power as i16), Unit::Hertz)
                .expect("1024-frame, 192000-Hz bin coordinate fits the exact decimal profile");
            PcmSpectralBin {
                index: index as u16,
                frequency,
                peak_amplitude: sided * libm::sqrt(square) / self.window_sum,
                mean_square_contribution: sided * square / (n as f64 * self.window_square_sum),
            }
        })
    }
}
