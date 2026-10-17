//! Exact finite window/stride geometry. Every fitting window is emitted or the
//! complete request refuses; no hidden truncation, padding, resampling or history.
use crate::{
    PcmFrameHeader, PcmSpectralRefusal, PcmSpectralWindow, PcmSpectralWorkspace,
    PcmTimeAnalysisRefusal, PreparedPcmSpectrum,
};

pub const PCM_SPECTRAL_MAXIMUM_WINDOWS: u16 = 16;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmSpectralGeometry {
    source_frames: u16,
    window_frames: u16,
    stride_frames: u16,
    windows: u16,
}
impl PcmSpectralGeometry {
    pub fn new(
        source_frames: u16,
        window_frames: u16,
        stride_frames: u16,
    ) -> Result<Self, PcmSpectralRefusal> {
        if !(16..=1024).contains(&window_frames) || !window_frames.is_power_of_two() {
            return Err(PcmSpectralRefusal::UnsupportedWindow);
        }
        if !(1..=2048).contains(&stride_frames) {
            return Err(PcmSpectralRefusal::InvalidStride);
        }
        if source_frames < window_frames || source_frames > 2048 {
            return Err(PcmSpectralRefusal::WrongSourceFrames);
        }
        let windows = (source_frames - window_frames) / stride_frames + 1;
        if windows > PCM_SPECTRAL_MAXIMUM_WINDOWS {
            return Err(PcmSpectralRefusal::TooManyWindows);
        }
        Ok(Self {
            source_frames,
            window_frames,
            stride_frames,
            windows,
        })
    }
    pub fn source_frames(self) -> u16 {
        self.source_frames
    }
    pub fn window_frames(self) -> u16 {
        self.window_frames
    }
    pub fn stride_frames(self) -> u16 {
        self.stride_frames
    }
    pub fn windows(self) -> u16 {
        self.windows
    }
    pub fn offset(self, index: u16) -> Result<u16, PcmSpectralRefusal> {
        if index >= self.windows {
            return Err(PcmSpectralRefusal::WindowIndexOutsideGeometry);
        }
        Ok(index * self.stride_frames)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmSpectralProfile {
    geometry: PcmSpectralGeometry,
    clock_id: u64,
    sample_rate_hz: u32,
    window: PcmSpectralWindow,
}
impl PcmSpectralProfile {
    pub fn new(
        geometry: PcmSpectralGeometry,
        clock_id: u64,
        sample_rate_hz: u32,
        window: PcmSpectralWindow,
    ) -> Result<Self, PcmSpectralRefusal> {
        if clock_id == 0 || !(8000..=192000).contains(&sample_rate_hz) {
            return Err(PcmSpectralRefusal::Input(
                PcmTimeAnalysisRefusal::WrongTimebase,
            ));
        }
        Ok(Self {
            geometry,
            clock_id,
            sample_rate_hz,
            window,
        })
    }
    pub fn geometry(self) -> PcmSpectralGeometry {
        self.geometry
    }
    pub fn clock_id(self) -> u64 {
        self.clock_id
    }
    pub fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }
    pub fn window(self) -> PcmSpectralWindow {
        self.window
    }
    pub fn observe_window<'a>(
        self,
        workspace: &'a mut PcmSpectralWorkspace,
        encoded: &[u8],
        index: u16,
    ) -> Result<PreparedPcmSpectrum<'a>, PcmSpectralRefusal> {
        let offset = self.geometry.offset(index)?;
        let (header, _) = PcmFrameHeader::decode_frame(encoded)
            .map_err(|e| PcmSpectralRefusal::Input(PcmTimeAnalysisRefusal::Frame(e)))?;
        if header.frame_count() != self.geometry.source_frames {
            return Err(PcmSpectralRefusal::WrongSourceFrames);
        }
        workspace.analyze(
            encoded,
            self.clock_id,
            self.sample_rate_hz,
            offset,
            self.geometry.window_frames,
            self.window,
        )
    }
}
